//! Process-local orchestration authority for probes and one run per host.

use super::backend::{
    ActionOutcome, BackendFailure, PreparedPayload, ProbeObservation, ProbeTarget,
    ProvisioningBackend, ReachOutcome, SystemBackend, path_text, stage_payload,
};
use super::e2e::{E2E_BACKEND_ENV, E2ePayloads, E2eProvisioningBackend};
use super::http::{
    ProbeDestination, ProbeRequest, ProbeResponse, ProvisionRequest, ProvisioningRequestError,
    ProvisioningView, RunAccepted, RunStatus, StepStatus, UpdatePlanResponse,
};
use super::payloads::{PayloadSelection, PayloadSource, production_payloads};
use super::plan::{
    PlanLayout, ProvisioningAction, ProvisioningOperation, ProvisioningPlan, ProvisioningTarget,
};
use crate::manager::{ConnectionManager, HostState};
use crate::store::{
    DialedAs, FirstContactOutcome, HelmStore, HostId, HostKind, HostRow, HostStoreError,
};
use anyhow::{Context as _, bail};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// What the hosts panel says about the helm's own machine when nothing
/// there is already somebody's supervisor.
///
/// The local row stopped installing supervisors in the distribution plan's
/// D1: a helm machine's units are `farhelm helm setup`'s to write, and the
/// panel's job is to say so rather than to build a second, differently
/// shaped installation beside it.
const LOCAL_SETUP_HANDOFF: &str = "this is the helm's own machine; run farhelm helm setup here instead of provisioning from \
     the panel";

const ATTACH_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PENDING_PLANS: usize = 64;
const MAX_CONCURRENT_RUNS: usize = 4;
const MAX_CONCURRENT_PLANS: usize = 4;
const MAX_CONCURRENT_PROGRESS_READS: usize = 8;

/// Bounded process-local state behind plan confirmation and progress reads.
#[derive(Default)]
pub(super) struct ProvisioningMemory {
    pub(super) plans: HashMap<String, PendingPlan>,
    plan_order: VecDeque<String>,
    pub(super) runs: HashMap<HostId, ProvisioningView>,
    pub(super) busy: std::collections::HashSet<HostId>,
    tasks: HashMap<HostId, tokio::task::JoinHandle<()>>,
}

/// A confirmed plan retains the registration inputs used before execution.
#[derive(Clone)]
pub(super) struct PendingPlan {
    plan: ProvisioningPlan,
    confirmation: PendingConfirmation,
}

/// The facts a confirmation must revalidate before its first mutation.
#[derive(Clone)]
enum PendingConfirmation {
    Add {
        registration: ProbeRegistration,
        original_target: ProbeTarget,
    },
    Update {
        host: HostId,
        target: ProbeTarget,
        expected_identity: Option<String>,
        registration: ProbeRegistration,
    },
}

/// Confirmation either preserves the frozen executor plan or discovers that
/// ADD must stop and adopt an answering supervisor instead.
enum Revalidation {
    Execute,
    UseAsIs(String),
}

/// Registry input paired with a probe, separate from the executor's target
/// because registration owns SSH metadata while execution owns transport.
#[derive(Clone, PartialEq, Eq)]
enum ProbeRegistration {
    Local,
    Ssh {
        destination: String,
        remote_farhelm: Option<String>,
        remote_state_dir: Option<String>,
    },
}

/// The exact connection coordinates and identity proved by one completed
/// hello. Keeping this owned lets registration commit the same facts after
/// the probe process has been reaped.
struct DiscoveredDial {
    farhelm: PathBuf,
    state_dir: Option<PathBuf>,
    identity: Option<String>,
}

/// Process-local orchestration authority for probes and one run per host.
pub(crate) struct ProvisioningService {
    backend: Arc<dyn ProvisioningBackend>,
    payloads: Arc<dyn PayloadSource>,
    store: HelmStore,
    manager: Arc<ConnectionManager>,
    layout: PlanLayout,
    local_farhelm: PathBuf,
    pub(super) memory: tokio::sync::Mutex<ProvisioningMemory>,
    run_slots: Arc<tokio::sync::Semaphore>,
    /// Bound fleet-wide transport inspection without serializing unrelated
    /// rows behind the browser's page lock.
    plan_slots: tokio::sync::Semaphore,
    /// Feed bumps can make every mounted row read at once. Queue those cheap
    /// reads modestly so fleet size cannot become handler fan-out.
    progress_read_slots: tokio::sync::Semaphore,
    /// Fail the next durable-to-live registry handoff so tests can prove the
    /// database and actor set do not diverge after registration commits.
    #[cfg(test)]
    pub(super) fail_registry_sync: std::sync::atomic::AtomicBool,
}

impl ProvisioningService {
    /// Production composition uses real process/file operations and
    /// resolves `selection`/`release_build` (D13, D18) into a real payload
    /// source via [`production_payloads`] — see that function for what each
    /// [`PayloadSelection`] currently does and what Step 3b adds to it.
    pub(crate) fn production(
        store: HelmStore,
        manager: Arc<ConnectionManager>,
        helm_state_dir: PathBuf,
        selection: PayloadSelection,
        release_build: bool,
    ) -> anyhow::Result<Arc<Self>> {
        let local_farhelm =
            std::env::current_exe().context("locating the running farhelm binary")?;
        if let Some(root) = std::env::var_os(E2E_BACKEND_ENV) {
            let root = PathBuf::from(root);
            let backend = E2eProvisioningBackend::new(root.clone(), &helm_state_dir)?;
            eprintln!(
                "WARNING: E2E-only injected provisioning backend enabled from {}; host setup actions are simulated",
                root.display()
            );
            return Ok(Arc::new(Self {
                backend: Arc::new(backend),
                payloads: Arc::new(E2ePayloads(root.join("ENABLED"))),
                store,
                manager,
                layout: PlanLayout::production(helm_state_dir),
                local_farhelm,
                memory: tokio::sync::Mutex::new(ProvisioningMemory::default()),
                run_slots: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_RUNS)),
                plan_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PLANS),
                progress_read_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PROGRESS_READS),
                #[cfg(test)]
                fail_registry_sync: std::sync::atomic::AtomicBool::new(false),
            }));
        }
        // The directory a RELATIVE `--payload-dir` is spelled against
        // (F1, review round 3) — `production_payloads`'s legacy-cache alias
        // guard needs it to resolve such a selection the same way the shell
        // that launched this process would have.
        let cwd = std::env::current_dir().context("reading the current working directory")?;
        Ok(Arc::new(Self {
            backend: Arc::new(SystemBackend::new(helm_state_dir.clone())),
            payloads: production_payloads(selection, &helm_state_dir, release_build, &cwd)?,
            store,
            manager,
            layout: PlanLayout::production(helm_state_dir),
            local_farhelm,
            memory: tokio::sync::Mutex::new(ProvisioningMemory::default()),
            run_slots: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_RUNS)),
            plan_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PLANS),
            progress_read_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PROGRESS_READS),
            #[cfg(test)]
            fail_registry_sync: std::sync::atomic::AtomicBool::new(false),
        }))
    }

    #[cfg(test)]
    pub(super) fn injected(
        store: HelmStore,
        manager: Arc<ConnectionManager>,
        backend: Arc<dyn ProvisioningBackend>,
        payloads: Arc<dyn PayloadSource>,
        layout: PlanLayout,
        local_farhelm: PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self {
            backend,
            payloads,
            store,
            manager,
            layout,
            local_farhelm,
            memory: tokio::sync::Mutex::new(ProvisioningMemory::default()),
            run_slots: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_RUNS)),
            plan_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PLANS),
            progress_read_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PROGRESS_READS),
            fail_registry_sync: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Split one request into probe transport and eventual registry input.
    fn target(&self, request: &ProbeRequest) -> anyhow::Result<(ProbeTarget, ProbeRegistration)> {
        match &request.target {
            ProbeDestination::Local => {
                if request.remote_farhelm.is_some() || request.remote_state_dir.is_some() {
                    return Err(anyhow::Error::new(ProvisioningRequestError::InvalidProbe(
                        "the local probe does not accept remote_farhelm or remote_state_dir"
                            .to_string(),
                    )));
                }
                Ok((
                    ProbeTarget {
                        transport: ProvisioningTarget::Local,
                        probe_farhelm: self.local_farhelm.clone(),
                        probe_state_dir: Some(self.layout.local_state_dir.clone()),
                    },
                    ProbeRegistration::Local,
                ))
            }
            ProbeDestination::Ssh { destination } => {
                if !crate::store::destination_is_usable(destination) {
                    return Err(anyhow::Error::new(ProvisioningRequestError::InvalidProbe(
                        format!("{destination:?} is not a usable ssh destination"),
                    )));
                }
                let registration = ProbeRegistration::Ssh {
                    destination: destination.clone(),
                    remote_farhelm: request.remote_farhelm.clone(),
                    remote_state_dir: request.remote_state_dir.clone(),
                };
                Ok((
                    ProbeTarget {
                        transport: ProvisioningTarget::Ssh {
                            destination: destination.clone(),
                        },
                        probe_farhelm: PathBuf::from(
                            request.remote_farhelm.as_deref().unwrap_or("farhelm"),
                        ),
                        probe_state_dir: request.remote_state_dir.as_deref().map(PathBuf::from),
                    },
                    registration,
                ))
            }
        }
    }

    /// What to tell the operator instead of installing or updating a
    /// supervisor on the helm's own machine.
    ///
    /// The panel does neither of those here any more (D1/D9): the
    /// helm-machine layout belongs to `farhelm helm setup`, which is the
    /// only thing that knows how to write units for the binary this helm
    /// is running. Every caller therefore gets a reason, never a
    /// permission — the only question this answers is WHICH reason:
    ///
    /// - A `farhelm-supervisor.service` whose `ExecStart=` resolves to
    ///   this helm's own binary means somebody already owns the
    ///   supervisor here. `farhelm helm setup` wrote it (marked) or a
    ///   person did, and the two get different advice: the first can be
    ///   driven with `systemctl --user restart`, while the second is its
    ///   author's to manage and is only reported as off limits.
    /// - Anything else — no unit, or one running some OTHER farhelm — is
    ///   the ordinary first-run answer: run setup here.
    ///
    /// Resolution is by canonical path on both sides, so a symlinked
    /// `~/.local/bin/farhelm` and the binary it points at are recognized
    /// as the same program. A unit that exists but names no classifiable
    /// program FAILS CLOSED into the hand-written wording: something is
    /// there, this code cannot tell what it runs, and the one answer that
    /// must never come out of that is "there is nothing here".
    async fn local_handoff_reason(&self) -> anyhow::Result<String> {
        let unit = crate::units::SUPERVISOR_UNIT_NAME;
        let handwritten = format!(
            "{unit} on this machine already runs this farhelm and was written by hand; it is not \
             touched from the hosts panel"
        );
        let text = self
            .backend
            .read_user_unit(unit)
            .await
            .map_err(anyhow::Error::new)?;
        let Some(text) = text else {
            return Ok(LOCAL_SETUP_HANDOFF.to_string());
        };
        let Some(program) = crate::units::exec_start_program(&text) else {
            return Ok(handwritten);
        };
        let resolved = (
            std::fs::canonicalize(&program),
            std::fs::canonicalize(&self.local_farhelm),
        );
        if !matches!(resolved, (Ok(unit_program), Ok(ours)) if unit_program == ours) {
            return Ok(LOCAL_SETUP_HANDOFF.to_string());
        }
        Ok(if crate::units::is_managed(&text) {
            format!(
                "{unit} on this machine is managed by farhelm helm setup; it is not touched from \
                 the hosts panel. Start or restart it with: systemctl --user restart {unit}"
            )
        } else {
            handwritten
        })
    }

    /// Complete discovery before either registering an answer or retaining a
    /// non-mutating plan for later confirmation.
    pub(super) async fn probe(&self, mut request: ProbeRequest) -> anyhow::Result<ProbeResponse> {
        let _slot = self
            .plan_slots
            .acquire()
            .await
            .expect("the provisioning planning semaphore is never closed");
        // A rerun against a row this helm already knows must probe the
        // installed path recorded at registration, not fall back to PATH.
        // A brand-new helm has no such row; the remote probe itself also
        // checks the standard flat install path for that recovery case.
        if let ProbeDestination::Ssh { destination } = &request.target
            && (request.remote_farhelm.is_none() || request.remote_state_dir.is_none())
            && let Some(row) = self
                .store
                .list_hosts()
                .await?
                .into_iter()
                .find(|row| row.destination.as_deref() == Some(destination.as_str()))
        {
            request.remote_farhelm = request.remote_farhelm.or(row.remote_farhelm);
            request.remote_state_dir = request.remote_state_dir.or(row.remote_state_dir);
        }
        let (target, registration) = self.target(&request)?;
        // Discovery comes FIRST, on every transport including the local
        // one. Reading a unit file is not what decides whether a
        // supervisor is there — a running one answers the protocol hello
        // and gets registered and used as-is, with its unit untouched.
        // The local handoff below is about INSTALLING, and only an absent
        // supervisor raises that question.
        match self
            .backend
            .probe(&target)
            .await
            .map_err(anyhow::Error::new)?
        {
            ProbeObservation::Supervisor {
                build_version,
                host_identity,
                dial_farhelm,
                dial_state_dir,
            } => {
                let host_id = self
                    .register(
                        &registration,
                        None,
                        Some(DiscoveredDial {
                            farhelm: dial_farhelm,
                            state_dir: dial_state_dir,
                            identity: host_identity.clone(),
                        }),
                    )
                    .await?;
                self.resolve_failed_add_discovery(host_id, &build_version)
                    .await;
                Ok(ProbeResponse::Discovered {
                    host_id,
                    build_version,
                    host_identity,
                })
            }
            ProbeObservation::Absent => {
                // Nothing answered on the helm's OWN machine, so the next
                // step would have been to install one — which is exactly
                // what the panel no longer does here. Hand the operator to
                // `farhelm helm setup`, naming whoever already owns the
                // supervisor unit if anybody does.
                if matches!(target.transport, ProvisioningTarget::Local) {
                    return Ok(ProbeResponse::Manual {
                        reason: self.local_handoff_reason().await?,
                    });
                }
                let reach = match self
                    .backend
                    .inspect(&target)
                    .await
                    .map_err(anyhow::Error::new)?
                {
                    ReachOutcome::Supported(reach) => reach,
                    ReachOutcome::Manual(reason) => return Ok(ProbeResponse::Manual { reason }),
                };
                let probe_id = uuid::Uuid::new_v4().to_string();
                let plan = self.layout.plan(
                    ProvisioningOperation::Add,
                    target.transport.clone(),
                    &reach,
                    &probe_id,
                )?;
                let confirmation = plan.confirmation();
                let mut memory = self.memory.lock().await;
                while memory.plan_order.len() >= MAX_PENDING_PLANS {
                    if let Some(expired) = memory.plan_order.pop_front() {
                        memory.plans.remove(&expired);
                    }
                }
                memory.plan_order.push_back(probe_id.clone());
                memory.plans.insert(
                    probe_id.clone(),
                    PendingPlan {
                        plan: plan.clone(),
                        confirmation: PendingConfirmation::Add {
                            registration,
                            original_target: target,
                        },
                    },
                );
                Ok(ProbeResponse::Provisionable {
                    probe_id,
                    plan,
                    confirmation,
                })
            }
        }
    }

    /// Establish the host id and the exact dial configuration proved by the
    /// probe or chosen by a confirmed plan before a run starts.
    async fn register(
        &self,
        registration: &ProbeRegistration,
        plan: Option<&ProvisioningPlan>,
        discovered: Option<DiscoveredDial>,
    ) -> anyhow::Result<HostId> {
        let (host, inserted) = match registration {
            ProbeRegistration::Local => {
                let row = self
                    .store
                    .list_hosts()
                    .await?
                    .into_iter()
                    .find(|row| row.kind == HostKind::Local)
                    .context("the guaranteed local host row is missing")?;
                if let Some(identity) = discovered.and_then(|dial| dial.identity) {
                    match self
                        .store
                        .record_first_contact(row.id, &DialedAs::of(&row), &identity)
                        .await?
                    {
                        FirstContactOutcome::Recorded => {}
                        outcome => bail!(
                            "the local supervisor identity could not be registered: {outcome:?}"
                        ),
                    }
                }
                (row.id, false)
            }
            ProbeRegistration::Ssh {
                destination,
                remote_farhelm,
                remote_state_dir,
            } => {
                let discovered_identity = discovered
                    .as_ref()
                    .and_then(|dial| dial.identity.as_deref());
                let installed_farhelm = plan
                    .map(|plan| plan.farhelm_path.as_path())
                    .or_else(|| discovered.as_ref().map(|dial| dial.farhelm.as_path()));
                let installed_state = plan.map(|plan| plan.state_dir.as_path()).or_else(|| {
                    discovered
                        .as_ref()
                        .and_then(|dial| dial.state_dir.as_deref())
                });
                let farhelm = installed_farhelm
                    .map(path_text)
                    .transpose()?
                    .or_else(|| remote_farhelm.clone());
                let state_dir = installed_state
                    .map(path_text)
                    .transpose()?
                    .or_else(|| remote_state_dir.clone());
                self.store
                    .register_probed_ssh_host(
                        destination,
                        farhelm.as_deref(),
                        state_dir.as_deref(),
                        discovered_identity,
                    )
                    .await?
            }
        };
        let reconciled = async {
            self.sync_registry().await?;
            if !self.manager.retry_now(host).await? {
                bail!("the registered host actor was not available for retry");
            }
            anyhow::Ok(())
        }
        .await;
        if let Err(error) = reconciled {
            if inserted {
                let rollback = self.store.remove_ssh_host(host).await;
                self.manager.stop_actor(host).await;
                if let Err(rollback) = rollback {
                    return Err(error.context(format!(
                        "the new host row could not be reconciled, and rolling it back also failed ({rollback:#})"
                    )));
                }
            }
            return Err(error);
        }
        Ok(host)
    }

    /// Reconcile durable registry changes with the actor set. The test seam
    /// exists to prove that a newly inserted row is rolled back when this
    /// post-commit step fails.
    async fn sync_registry(&self) -> anyhow::Result<()> {
        #[cfg(test)]
        if self
            .fail_registry_sync
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            bail!("planted registry synchronization failure");
        }
        self.manager.sync_registry().await
    }

    /// Consume a confirmed plan exactly once, register first, then start ADD.
    pub(super) async fn start_add(
        self: &Arc<Self>,
        request: ProvisionRequest,
    ) -> anyhow::Result<RunAccepted> {
        let mut pending = self.consume_plan(&request.probe_id).await?;
        let PendingConfirmation::Add { registration, .. } = &pending.confirmation else {
            return Err(anyhow::Error::new(ProvisioningRequestError::UnknownPlan));
        };
        let host = self
            .register(registration, Some(&pending.plan), None)
            .await?;
        let registered = registration_for_row(&self.host_row(host).await?)?;
        if let PendingConfirmation::Add { registration, .. } = &mut pending.confirmation {
            *registration = registered;
        }
        self.start_run(host, pending).await
    }

    /// Inspect an existing row and retain one exact UPDATE plan without
    /// changing either the registry or the host.
    ///
    /// A LOCAL row never gets that far. UPDATE is the path that writes the
    /// unit file and the binary beside it, and on the helm's own machine
    /// that is `farhelm helm setup`'s job whether or not a supervisor unit
    /// happens to be there right now (D1). Refusing every local row —
    /// rather than only the ones already carrying a recognizable unit —
    /// is what closes the alternate route to the install the ADD path
    /// stopped offering.
    ///
    /// Refusing before the probe also removes a time-of-check problem the
    /// narrower rule had: a plan retained here is confirmed later, under
    /// the host write claim, and nothing re-read the unit file in between.
    /// With no local plan reachable at all, there is no stale local plan
    /// for a newly written unit to lose a race against — see the note in
    /// [`Self::start_update`].
    pub(super) async fn plan_update(&self, host: HostId) -> anyhow::Result<UpdatePlanResponse> {
        let row = self.host_row(host).await?;
        if row.kind == HostKind::Local {
            bail!(self.local_handoff_reason().await?);
        }
        self.plan_update_unguarded(host).await
    }

    /// [`Self::plan_update`] without the local-row refusal, for the tests
    /// that must still drive the direct-local executor.
    ///
    /// The executor's local branch — file operations and process spawns
    /// with no ssh anywhere — is still production code reached by SSH
    /// planning's shared action vocabulary, and the real-systemd
    /// integration test is the only thing that exercises it end to end
    /// against a live user manager. That test used to enter through the
    /// panel's own ADD, then through its UPDATE; both are now closed for
    /// local rows on purpose, so it enters here instead. This seam
    /// deliberately exposes no HTTP surface and no production caller: the
    /// panel cannot reach it, which is the whole point of the rule above.
    #[cfg(test)]
    pub(super) async fn plan_update_for_local_executor_tests(
        &self,
        host: HostId,
    ) -> anyhow::Result<UpdatePlanResponse> {
        self.plan_update_unguarded(host).await
    }

    /// The UPDATE planner itself. See [`Self::plan_update`] for the local
    /// row's refusal, which is deliberately NOT part of this.
    async fn plan_update_unguarded(&self, host: HostId) -> anyhow::Result<UpdatePlanResponse> {
        let _slot = self
            .plan_slots
            .acquire()
            .await
            .expect("the provisioning planning semaphore is never closed");
        let row = self.host_row(host).await?;
        self.require_update_trusted(host)
            .map_err(anyhow::Error::new)?;
        let original_target = self.probe_target_for_row(&row);
        let observation = self
            .backend
            .probe(&original_target)
            .await
            .map_err(anyhow::Error::new)?;
        let mut effective_row = row.clone();
        let mut expected_identity = row.host_identity.clone();
        let target = match observation {
            ProbeObservation::Supervisor {
                host_identity,
                dial_farhelm,
                dial_state_dir,
                ..
            } => {
                if let (Some(recorded), Some(reported)) = (&row.host_identity, &host_identity)
                    && recorded != reported
                {
                    return Err(anyhow::Error::new(HostStoreError::IdentityMismatch {
                        host,
                        expected: recorded.clone(),
                        actual: Some(reported.clone()),
                    }));
                }
                expected_identity = expected_identity.or(host_identity);
                if row.kind == HostKind::Ssh {
                    effective_row.remote_farhelm = Some(path_text(&dial_farhelm)?);
                    effective_row.remote_state_dir =
                        dial_state_dir.as_deref().map(path_text).transpose()?;
                }
                ProbeTarget {
                    transport: original_target.transport.clone(),
                    probe_farhelm: dial_farhelm,
                    probe_state_dir: dial_state_dir,
                }
            }
            ProbeObservation::Absent => original_target,
        };
        let reach = match self
            .backend
            .inspect(&target)
            .await
            .map_err(anyhow::Error::new)?
        {
            ReachOutcome::Supported(reach) => reach,
            ReachOutcome::Manual(reason) => bail!(reason),
        };
        let probe_id = uuid::Uuid::new_v4().to_string();
        let plan = self.layout.plan_for_row(
            &effective_row,
            target.transport.clone(),
            &reach,
            &probe_id,
        )?;
        let confirmation = plan.confirmation();
        // Confirmation compares the row the user actually planned from.
        // Any resolved dial coordinates are committed only after that check.
        let registration = registration_for_row(&row)?;
        self.retain_plan(
            probe_id.clone(),
            PendingPlan {
                plan: plan.clone(),
                confirmation: PendingConfirmation::Update {
                    host,
                    target,
                    expected_identity,
                    registration,
                },
            },
        )
        .await;
        Ok(UpdatePlanResponse {
            probe_id,
            plan,
            confirmation,
        })
    }

    /// Consume one host-bound UPDATE plan and only then claim the run.
    ///
    /// Confirmation revalidates the registry row, update trust, and the
    /// supervisor identity, but deliberately does NOT re-read this
    /// machine's supervisor unit. It has nothing to re-read: a plan can
    /// only exist for a row [`Self::plan_update`] accepted, and it accepts
    /// no local row at all. A unit appearing between planning and
    /// confirmation therefore cannot be overwritten by a stale local plan,
    /// because no such plan can be minted. If a local plan ever becomes
    /// reachable again, the ownership check has to be repeated HERE, under
    /// the host write claim — planning-time evidence is stale by then.
    pub(super) async fn start_update(
        self: &Arc<Self>,
        host: HostId,
        request: ProvisionRequest,
    ) -> anyhow::Result<RunAccepted> {
        let pending = self.consume_plan(&request.probe_id).await?;
        if !matches!(
            pending.confirmation,
            PendingConfirmation::Update { host: planned, .. } if planned == host
        ) {
            return Err(anyhow::Error::new(ProvisioningRequestError::UnknownPlan));
        }
        self.start_run(host, pending).await
    }

    pub(super) async fn consume_plan(&self, probe_id: &str) -> anyhow::Result<PendingPlan> {
        let mut memory = self.memory.lock().await;
        let pending = memory
            .plans
            .remove(probe_id)
            .ok_or_else(|| anyhow::Error::new(ProvisioningRequestError::UnknownPlan))?;
        memory.plan_order.retain(|id| id != probe_id);
        Ok(pending)
    }

    async fn retain_plan(&self, probe_id: String, pending: PendingPlan) {
        let mut memory = self.memory.lock().await;
        while memory.plan_order.len() >= MAX_PENDING_PLANS {
            if let Some(expired) = memory.plan_order.pop_front() {
                memory.plans.remove(&expired);
            }
        }
        memory.plan_order.push_back(probe_id.clone());
        memory.plans.insert(probe_id, pending);
    }

    async fn start_run(
        self: &Arc<Self>,
        host: HostId,
        pending: PendingPlan,
    ) -> anyhow::Result<RunAccepted> {
        let run_id = uuid::Uuid::new_v4().to_string();
        {
            let mut memory = self.memory.lock().await;
            if !memory.busy.insert(host) {
                return Err(anyhow::Error::new(ProvisioningRequestError::Busy(host)));
            }
            memory.runs.insert(
                host,
                ProvisioningView::for_plan(host, run_id.clone(), &pending.plan),
            );
        }
        let host_write = self.manager.host_write_lock(host).await;
        if let Err(error) = self.host_row(host).await {
            let mut memory = self.memory.lock().await;
            memory.busy.remove(&host);
            memory.runs.remove(&host);
            return Err(error);
        }
        self.manager.events().bump();
        let service = Arc::clone(self);
        let task = tokio::spawn(async move {
            let _host_write = host_write;
            let _slot = Arc::clone(&service.run_slots)
                .acquire_owned()
                .await
                .expect("the provisioning semaphore is never closed");
            service.revalidate_and_execute(host, pending).await;
        });
        self.memory.lock().await.tasks.insert(host, task);
        Ok(RunAccepted {
            host_id: host,
            run_id,
        })
    }

    /// Forget process-local progress alongside the durable row. Pending
    /// confirmation tokens for that host are removed as well.
    pub(crate) async fn forget_host(&self, host: HostId) {
        let mut memory = self.memory.lock().await;
        let task = memory.tasks.remove(&host);
        if let Some(task) = &task {
            task.abort();
        }
        memory.runs.remove(&host);
        let removed: std::collections::HashSet<String> = memory
            .plans
            .iter()
            .filter(|(_, pending)| {
                matches!(
                    &pending.confirmation,
                    PendingConfirmation::Update { host: planned, .. } if *planned == host
                )
            })
            .map(|(id, _)| id.clone())
            .collect();
        memory.plans.retain(|id, _| !removed.contains(id));
        memory.plan_order.retain(|id| !removed.contains(id));
        drop(memory);
        if let Some(task) = task {
            let _ = task.await;
        }
    }

    #[cfg(test)]
    pub(super) async fn abort_run(&self, host: HostId) {
        let task = self.memory.lock().await.tasks.remove(&host);
        if let Some(task) = task {
            task.abort();
            let _ = task.await;
        }
        self.memory.lock().await.busy.remove(&host);
    }

    /// Re-probe after confirmation but before the first mutation. ADD uses
    /// an answering supervisor as-is; UPDATE refuses a changed identity.
    async fn revalidate_and_execute(&self, host: HostId, pending: PendingPlan) {
        let preflight = self.revalidate(host, &pending).await;
        match preflight {
            Ok(Revalidation::Execute) => self.execute(host, pending.plan).await,
            Ok(Revalidation::UseAsIs(message)) => self.finish_use_as_is(host, message).await,
            Err(error) => self.finish_preflight_failure(host, error.rendered()).await,
        }
    }

    async fn revalidate(
        &self,
        host: HostId,
        pending: &PendingPlan,
    ) -> Result<Revalidation, BackendFailure> {
        match &pending.confirmation {
            PendingConfirmation::Add {
                registration,
                original_target,
            } => {
                self.require_registration_unchanged(host, registration)
                    .await?;
                let mut observation = self.backend.probe(original_target).await?;
                let installed_target = ProbeTarget {
                    transport: pending.plan.target.clone(),
                    probe_farhelm: pending.plan.farhelm_path.clone(),
                    probe_state_dir: Some(pending.plan.state_dir.clone()),
                };
                if matches!(observation, ProbeObservation::Absent)
                    && matches!(original_target.transport, ProvisioningTarget::Ssh { .. })
                    && (original_target.probe_farhelm != installed_target.probe_farhelm
                        || original_target.probe_state_dir != installed_target.probe_state_dir)
                {
                    observation = self.backend.probe(&installed_target).await?;
                }
                if let ProbeObservation::Supervisor {
                    build_version,
                    host_identity,
                    dial_farhelm,
                    dial_state_dir,
                } = observation
                {
                    self.register(
                        registration,
                        None,
                        Some(DiscoveredDial {
                            farhelm: dial_farhelm,
                            state_dir: dial_state_dir,
                            identity: host_identity,
                        }),
                    )
                    .await
                    .map_err(|error| {
                        BackendFailure::new(
                            "registering the supervisor found at confirmation",
                            format!("{error:#}"),
                        )
                    })?;
                    return Ok(Revalidation::UseAsIs(format!(
                        "a supervisor answered during confirmation (build {build_version}); ADD used it as-is"
                    )));
                }
                Ok(Revalidation::Execute)
            }
            PendingConfirmation::Update {
                target,
                expected_identity,
                registration,
                ..
            } => {
                self.require_registration_unchanged(host, registration)
                    .await?;
                self.require_update_trusted(host)?;
                let observation = self.backend.probe(target).await?;
                let (plan, discovered) = match observation {
                    ProbeObservation::Supervisor {
                        host_identity,
                        dial_farhelm,
                        dial_state_dir,
                        ..
                    } => {
                        if expected_identity.is_some() && &host_identity != expected_identity {
                            return Err(BackendFailure::new(
                                "the supervisor identity changed after UPDATE planning",
                                format!(
                                    "expected {expected_identity:?}, reported {host_identity:?}; plan again"
                                ),
                            ));
                        }
                        (
                            None,
                            Some(DiscoveredDial {
                                farhelm: dial_farhelm,
                                state_dir: dial_state_dir,
                                identity: host_identity,
                            }),
                        )
                    }
                    ProbeObservation::Absent => (Some(&pending.plan), None),
                };
                self.register(registration, plan, discovered)
                    .await
                    .map_err(|error| {
                        BackendFailure::new(
                            "recording the confirmed UPDATE target",
                            format!("{error:#}"),
                        )
                    })?;
                Ok(Revalidation::Execute)
            }
        }
    }

    async fn require_registration_unchanged(
        &self,
        host: HostId,
        expected: &ProbeRegistration,
    ) -> Result<(), BackendFailure> {
        let row = self.host_row(host).await.map_err(|error| {
            BackendFailure::new("re-reading the confirmed host row", format!("{error:#}"))
        })?;
        let current = registration_for_row(&row).map_err(|error| {
            BackendFailure::new(
                "reading the confirmed host configuration",
                format!("{error:#}"),
            )
        })?;
        if &current != expected {
            return Err(BackendFailure::new(
                "the host configuration changed after the plan was shown",
                "discard this plan and plan again",
            ));
        }
        Ok(())
    }

    /// UPDATE cannot resolve an identity decision that the connection
    /// manager has deliberately frozen for the user. Refusing both while
    /// planning and immediately before mutation closes the confirmation
    /// window without treating a software update as implicit adoption.
    fn require_update_trusted(&self, host: HostId) -> Result<(), BackendFailure> {
        match self.manager.state(host) {
            Some(HostState::IdentityMismatch { recorded, reported }) => Err(BackendFailure::new(
                "UPDATE is refused while the host identity is frozen",
                format!(
                    "recorded {recorded:?}, reported {reported:?}; adopt the identity or fix the destination first"
                ),
            )),
            Some(HostState::Duplicate { twin, identity }) => Err(BackendFailure::new(
                "UPDATE is refused while the host duplicates another registry row",
                format!(
                    "identity {identity:?} belongs to host {twin}; resolve the duplicate first"
                ),
            )),
            _ => Ok(()),
        }
    }

    async fn finish_use_as_is(&self, host: HostId, message: String) {
        let mut memory = self.memory.lock().await;
        if let Some(run) = memory.runs.get_mut(&host) {
            run.status = RunStatus::Completed;
            run.message = Some(message.clone());
            for step in &mut run.steps {
                step.status = StepStatus::Skipped;
                step.message = Some(message.clone());
            }
        }
        memory.busy.remove(&host);
        drop(memory);
        self.manager.events().bump();
    }

    /// Resolve a retained failed ADD when fresh discovery proves the install
    /// is now usable as-is.
    ///
    /// Failure after starting the supervisor but before attachment is the
    /// important case: the next ADD probe correctly discovers a live peer,
    /// yet without this reconciliation the old failed run remains the latest
    /// progress forever and the UI keeps offering a rerun that has already
    /// succeeded. Completed steps stay completed; every unfinished step is
    /// marked skipped because discovery, rather than another executor pass,
    /// established the final state.
    async fn resolve_failed_add_discovery(&self, host: HostId, build_version: &str) {
        let message = format!(
            "a supervisor answered during recovery (build {build_version}); ADD used it as-is"
        );
        let mut memory = self.memory.lock().await;
        let Some(run) = memory.runs.get_mut(&host) else {
            return;
        };
        if run.status != RunStatus::Failed || run.operation != Some(ProvisioningOperation::Add) {
            return;
        }
        run.status = RunStatus::Completed;
        run.message = Some(message.clone());
        for step in &mut run.steps {
            if step.status != StepStatus::Completed {
                step.status = StepStatus::Skipped;
                step.message = Some(message.clone());
            }
        }
        memory.busy.remove(&host);
        drop(memory);
        self.manager.events().bump();
    }

    async fn finish_preflight_failure(&self, host: HostId, message: String) {
        let mut memory = self.memory.lock().await;
        if let Some(run) = memory.runs.get_mut(&host) {
            run.status = RunStatus::Failed;
            run.message = Some(message.clone());
            if let Some(step) = run.steps.first_mut() {
                step.status = StepStatus::Failed;
                step.message = Some(message);
            }
        }
        memory.busy.remove(&host);
        drop(memory);
        self.manager.events().bump();
    }

    /// Consume the plan in order, retaining every completed outcome and
    /// stopping at the first failure without rollback.
    async fn execute(&self, host: HostId, plan: ProvisioningPlan) {
        let prepared = match self.prepare_payloads(&plan).await {
            Ok(prepared) => prepared,
            Err((index, error)) => {
                self.fail_action(host, index, &plan.actions[index], error)
                    .await;
                return;
            }
        };
        for (index, action) in plan.actions.iter().enumerate() {
            self.set_step(host, index, StepStatus::Running, None).await;
            let outcome = self
                .execute_action(host, &plan, action, prepared.get(&index))
                .await;
            let (status, message) = match outcome {
                Ok(ActionOutcome::Completed) => (StepStatus::Completed, None),
                Ok(ActionOutcome::Skipped(message)) => (StepStatus::Skipped, Some(message)),
                Ok(ActionOutcome::Degraded(message)) => (StepStatus::Degraded, Some(message)),
                Err(error) => {
                    self.fail_action(host, index, action, error).await;
                    return;
                }
            };
            self.set_step(host, index, status, message).await;
        }
        let mut memory = self.memory.lock().await;
        if let Some(run) = memory.runs.get_mut(&host) {
            run.status = RunStatus::Completed;
            let degraded = run
                .steps
                .iter()
                .any(|step| step.status == StepStatus::Degraded);
            run.message = degraded.then(|| "starts at login, not at boot".to_string());
        }
        memory.busy.remove(&host);
        drop(memory);
        self.manager.events().bump();
    }

    /// Resolve and stage every payload before the first plan action runs.
    /// A missing release artifact is a planning/execution boundary failure,
    /// not permission to leave newly created directories on the host.
    async fn prepare_payloads(
        &self,
        plan: &ProvisioningPlan,
    ) -> Result<HashMap<usize, PreparedPayload>, (usize, BackendFailure)> {
        let mut prepared = HashMap::new();
        for (index, action) in plan.actions.iter().enumerate() {
            let ProvisioningAction::InstallPayload { payload, arch, .. } = action else {
                continue;
            };
            let source = self
                .payloads
                .path(*payload, *arch)
                .await
                .map_err(|error| (index, BackendFailure::new(format!("{error:#}"), "")))?;
            let staged = stage_payload(&source)
                .await
                .map_err(|error| (index, error))?;
            prepared.insert(index, staged);
        }
        Ok(prepared)
    }

    /// Retain one failed action and release the host claim without undoing
    /// any earlier completed steps.
    async fn fail_action(
        &self,
        host: HostId,
        index: usize,
        action: &ProvisioningAction,
        error: BackendFailure,
    ) {
        let message = format!(
            "step {} ({}) failed: {}; rerun provisioning to continue",
            index + 1,
            action.label(),
            error.rendered()
        );
        self.set_step(host, index, StepStatus::Failed, Some(message.clone()))
            .await;
        let mut memory = self.memory.lock().await;
        if let Some(run) = memory.runs.get_mut(&host) {
            run.status = RunStatus::Failed;
            run.message = Some(message);
        }
        memory.busy.remove(&host);
        drop(memory);
        self.manager.events().bump();
    }

    /// Dispatch one typed action. Attach is owned here because it joins the
    /// host registry/manager; every host-side action remains in the backend.
    async fn execute_action(
        &self,
        host: HostId,
        plan: &ProvisioningPlan,
        action: &ProvisioningAction,
        prepared: Option<&PreparedPayload>,
    ) -> Result<ActionOutcome, BackendFailure> {
        match action {
            ProvisioningAction::EnsureDirectories { directories } => {
                self.backend
                    .ensure_directories(&plan.target, directories)
                    .await
            }
            ProvisioningAction::InstallPayload {
                payload,
                destination,
                temporary,
                ..
            } => {
                let source = prepared.ok_or_else(|| {
                    BackendFailure::new("the prepared payload disappeared before installation", "")
                })?;
                self.backend
                    .install_path(
                        &plan.target,
                        *payload,
                        source,
                        destination,
                        temporary,
                        0o755,
                    )
                    .await
            }
            ProvisioningAction::WriteUnit {
                content,
                destination,
                temporary,
                ..
            } => {
                self.backend
                    .install_bytes(
                        &plan.target,
                        content.as_bytes(),
                        destination,
                        temporary,
                        0o644,
                    )
                    .await
            }
            ProvisioningAction::DaemonReload => self.backend.daemon_reload(&plan.target).await,
            ProvisioningAction::EnableSupervisor {
                unit, unit_path, ..
            } => self.backend.enable_now(&plan.target, unit, unit_path).await,
            ProvisioningAction::EnableLinger { .. } => {
                self.backend.enable_linger(&plan.target).await
            }
            ProvisioningAction::RestartSupervisor { unit } => {
                self.backend.restart(&plan.target, unit).await
            }
            ProvisioningAction::AttachSupervisor => {
                if let Some(outcome) = self.backend.injected_attach(&plan.target).await? {
                    return Ok(outcome);
                }
                let previous_incarnation =
                    self.manager.status(host).map(|status| status.incarnation);
                self.manager.retry_now(host).await.map_err(|error| {
                    BackendFailure::new("requesting supervisor attach", error.to_string())
                })?;
                let attached = tokio::time::timeout(ATTACH_TIMEOUT, async {
                    loop {
                        let status = self.manager.status(host)?;
                        if status.state.is_connected()
                            && status.client.is_some()
                            && Some(status.incarnation) != previous_incarnation
                        {
                            return Some(status.state);
                        }
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                })
                .await
                .map_err(|_| {
                    BackendFailure::new("waiting for the provisioned supervisor", "timed out")
                })?;
                attached.ok_or_else(|| {
                    BackendFailure::new(
                        "waiting for the provisioned supervisor",
                        "the registered host actor disappeared",
                    )
                })?;
                Ok(ActionOutcome::Completed)
            }
        }
    }

    /// Publish one progress transition through the fleet's no-data feed.
    async fn set_step(
        &self,
        host: HostId,
        index: usize,
        status: StepStatus,
        message: Option<String>,
    ) {
        let mut memory = self.memory.lock().await;
        if let Some(step) = memory
            .runs
            .get_mut(&host)
            .and_then(|run| run.steps.get_mut(index))
        {
            step.status = status;
            step.message = message;
        }
        drop(memory);
        self.manager.events().bump();
    }

    /// Read the retained run, returning an explicit idle view for a valid
    /// host that has never been provisioned by this helm process.
    pub(super) async fn view(&self, host: HostId) -> anyhow::Result<ProvisioningView> {
        let _slot = self
            .progress_read_slots
            .acquire()
            .await
            .expect("the provisioning progress semaphore is never closed");
        self.host_row(host).await?;
        Ok(self
            .memory
            .lock()
            .await
            .runs
            .get(&host)
            .cloned()
            .unwrap_or_else(|| ProvisioningView::idle(host)))
    }

    pub(super) async fn host_row(&self, host: HostId) -> anyhow::Result<HostRow> {
        self.store
            .list_hosts()
            .await?
            .into_iter()
            .find(|row| row.id == host)
            .ok_or_else(|| anyhow::Error::new(HostStoreError::HostNotFound(host)))
    }

    fn probe_target_for_row(&self, row: &HostRow) -> ProbeTarget {
        match row.kind {
            HostKind::Local => ProbeTarget {
                transport: ProvisioningTarget::Local,
                probe_farhelm: self.local_farhelm.clone(),
                probe_state_dir: Some(self.layout.local_state_dir.clone()),
            },
            HostKind::Ssh => ProbeTarget {
                transport: ProvisioningTarget::Ssh {
                    destination: row.destination.clone().expect("ssh row destination"),
                },
                probe_farhelm: PathBuf::from(row.remote_farhelm.as_deref().unwrap_or("farhelm")),
                probe_state_dir: row.remote_state_dir.as_deref().map(PathBuf::from),
            },
        }
    }
}

fn registration_for_row(row: &HostRow) -> anyhow::Result<ProbeRegistration> {
    match row.kind {
        HostKind::Local => Ok(ProbeRegistration::Local),
        HostKind::Ssh => Ok(ProbeRegistration::Ssh {
            destination: row
                .destination
                .clone()
                .context("an ssh host row has no destination")?,
            remote_farhelm: row.remote_farhelm.clone(),
            remote_state_dir: row.remote_state_dir.clone(),
        }),
    }
}
