//! Process-local orchestration authority for probes and one run per host.

use super::backend::{
    ActionOutcome, BackendFailure, PreparedPayload, ProbeObservation, ProbeTarget,
    ProvisioningBackend, ReachOutcome, SystemBackend, path_text, stage_payload,
};
use super::e2e::{E2E_BACKEND_ENV, E2ePayloads, E2eProvisioningBackend};
use super::http::{
    HostPlanResponse, ProbeDestination, ProbeRequest, ProbeResponse, ProvisionRequest,
    ProvisioningRequestError, ProvisioningView, RunAccepted, RunStatus, StepStatus,
};
use super::payloads::{PayloadSelection, PayloadSource, production_payloads};
use super::plan::{
    PlanLayout, ProvisioningAction, ProvisioningOperation, ProvisioningPlan, ProvisioningTarget,
    ResolvedPath, UninstallFacts,
};
use crate::manager::{ConnectionManager, HostState, peer_text};
use crate::store::{
    DialedAs, FirstContactOutcome, HelmStore, HostId, HostKind, HostRow, HostStoreError,
};
use anyhow::{Context as _, bail};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
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

/// What the hosts panel says when uninstall is chosen for the helm's own
/// machine. The panel never removes Farhelm there: `farhelm uninstall` does,
/// with the ownership checks that machine's installation needs.
const LOCAL_UNINSTALL_HANDOFF: &str =
    "this is the helm's own machine; remove Farhelm here with farhelm uninstall";

/// How many running sessions an uninstall refusal names before summarizing
/// the rest as a count, so a busy host cannot make the message unbounded.
const UNINSTALL_REFUSAL_NAMED_SESSIONS: usize = 10;

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
    /// UNINSTALL revalidates by planning again and requiring the same plan
    /// (see [`ProvisioningService::revalidate`]), so beyond the host it needs
    /// only the registration the plan was made against.
    Uninstall {
        host: HostId,
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

/// What [`ProvisioningService::delete_registered_host`] does with the host's
/// run task, if it still has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunTask {
    /// Abort it and wait for it to end: Remove, which holds the host's
    /// provisioning lock itself, so any task left is a finished run's.
    Abort,
    /// Only drop its handle: UNINSTALL's last step, which IS that task.
    Detach,
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
    /// Serializes ADD confirmations from looking up the destination's row
    /// to the run claiming its host, so two confirmations for the same
    /// destination cannot both see it unclaimed and the later one rewrite
    /// the row under the earlier one's run (see [`Self::start_add`]).
    add_confirmations: Arc<tokio::sync::Mutex<()>>,
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
    /// Hold the next durable-to-live registry handoff. Lets a test drop the
    /// request that is registering a host after its row is saved and before
    /// its actor is reconciled.
    #[cfg(test)]
    pub(super) registry_sync_gate: std::sync::Mutex<Option<Arc<RegistrySyncGate>>>,
    /// Stands in for this helm's build in [`Self::helm_build`]: every test
    /// binary is a development build (`0.0.0-unreleased`), for which
    /// nothing is ever newer, so the downgrade refusal needs a release
    /// build to compare against.
    #[cfg(test)]
    pub(super) helm_build_override: std::sync::Mutex<Option<String>>,
}

/// A one-shot hold on [`ProvisioningService::sync_registry`]; see
/// `registry_sync_gate`.
#[cfg(test)]
pub(super) struct RegistrySyncGate {
    /// Notified when registration reaches the reconcile.
    pub(super) reached: tokio::sync::Notify,
    /// Notified by the test to let the reconcile run.
    pub(super) release: tokio::sync::Notify,
}

/// The attach step's failure for a state in which the helm answered the
/// supervisor this run installed and refused it, or `None` for any other
/// state.
///
/// The four refusals are the connection manager's answers to a supervisor
/// that does respond: another protocol version, an identity other than the
/// one on record, no identity at all, or an identity another registry entry
/// holds. Each message names the refusal and what to do about it, reusing
/// the wording the hosts panel and the update step's trust check already
/// use, so the run fails with the reason the panel shows instead of a bare
/// "timed out". The whole sentence is the failure's context with an empty
/// second half, because [`BackendFailure::rendered`] labels that half as
/// host stderr, which this is not. Builds and identities came from the
/// peer, so they go through [`peer_text`] (escaped and bounded).
///
/// `Unreachable` and `Retired` deliberately get no early stop: the attach
/// step's fresh window outlasts an unreachable verdict, and anything else is
/// left to its ordinary 30-second budget.
fn attach_refusal(state: &HostState) -> Option<BackendFailure> {
    const REFUSED: &str = "the helm refused the supervisor this run installed";
    let sentence = match state {
        HostState::VersionSkew {
            peer_protocol,
            peer_build,
            our_protocol,
            our_build,
            remediation,
        } => {
            let skew = farhelm_proto::io::VersionSkew {
                peer_protocol: *peer_protocol,
                peer_build: peer_text(peer_build),
                our_protocol: *our_protocol,
                our_build: our_build.clone(),
            };
            format!("{REFUSED}: {skew}; {remediation}")
        }
        HostState::IdentityMismatch { recorded, reported } => format!(
            "{REFUSED}: it reports identity {}, but this host's recorded identity is {}; adopt the \
             identity or fix the destination",
            peer_text(reported),
            peer_text(recorded),
        ),
        HostState::IdentityUnverified { recorded } => format!(
            "{REFUSED}: it answered without an identity, but this host has identity {} on \
             record; wait until it reports its identity again, or change this entry's destination \
             or remove the entry",
            peer_text(recorded),
        ),
        HostState::Duplicate { twin, identity } => format!(
            "{REFUSED}: identity {} belongs to host {twin}; remove that entry or change this \
             one's destination, then press Retry",
            peer_text(identity),
        ),
        _ => return None,
    };
    Some(BackendFailure::new(sentence, ""))
}

/// Why Update must not run against a host whose supervisor reports
/// `host_build`, or `None` when it may.
///
/// Update installs this helm's own build, so on a host already running a
/// newer one it is a downgrade (SPEC.md: Update never downgrades a host).
/// That is not merely a step back: an older supervisor refuses a database
/// written with a newer schema, its unit restarts on the failure, and the
/// host stays unreachable until the newer build is reinstalled by hand.
/// A build that does not parse as a version on either side leaves the order
/// unknown, and Update stays allowed, as the host list's version advisories
/// treat an unknown order.
/// `helm_build` is this helm's own build ([`ProvisioningService::helm_build`]).
fn downgrade_refusal(host_build: &str, helm_build: &str) -> Option<String> {
    crate::hosts::build_is_newer(host_build, helm_build).then(|| {
        format!(
            "the host runs farhelm {host_build}, which is newer than this helm's {helm_build}; \
             Update never installs an older version over a newer one, so update this helm \
             instead"
        )
    })
}

impl ProvisioningService {
    /// Production composition uses real process/file operations and
    /// resolves `selection`/`release_build` (D13, D18) into a real payload
    /// source via [`production_payloads`] — see that function for what each
    /// [`PayloadSelection`] resolves to.
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
                add_confirmations: Arc::default(),
                plan_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PLANS),
                progress_read_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PROGRESS_READS),
                #[cfg(test)]
                fail_registry_sync: std::sync::atomic::AtomicBool::new(false),
                #[cfg(test)]
                registry_sync_gate: std::sync::Mutex::new(None),
                #[cfg(test)]
                helm_build_override: std::sync::Mutex::new(None),
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
            add_confirmations: Arc::default(),
            plan_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PLANS),
            progress_read_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PROGRESS_READS),
            #[cfg(test)]
            fail_registry_sync: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            registry_sync_gate: std::sync::Mutex::new(None),
            #[cfg(test)]
            helm_build_override: std::sync::Mutex::new(None),
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
            add_confirmations: Arc::default(),
            plan_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PLANS),
            progress_read_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_PROGRESS_READS),
            fail_registry_sync: std::sync::atomic::AtomicBool::new(false),
            registry_sync_gate: std::sync::Mutex::new(None),
            helm_build_override: std::sync::Mutex::new(None),
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
    ///
    /// On a helm-owned task (see `crate::run_owned`): the backend's probe
    /// starts a child in its own process group and kills that group only on
    /// its own exit paths, so a request dropped mid-probe (a page closed or
    /// reloaded during a probe of up to `PROBE_TIMEOUT`) killed only the
    /// direct child and could leave helpers it started, such as a
    /// user-configured ssh `ProxyCommand`, running. A dropped request now
    /// loses only the reply. The probe then runs to its end, so one that
    /// finds a running supervisor still registers the host, as the
    /// registration step already did on its own.
    pub(super) async fn probe(
        self: &Arc<Self>,
        request: ProbeRequest,
    ) -> anyhow::Result<ProbeResponse> {
        let service = Arc::clone(self);
        crate::run_owned(async move { service.probe_owned(request).await }).await
    }

    /// The body of [`Self::probe`], run on a helm-owned task.
    async fn probe_owned(
        self: Arc<Self>,
        mut request: ProbeRequest,
    ) -> anyhow::Result<ProbeResponse> {
        let _slot = self
            .plan_slots
            .acquire()
            .await
            .expect("the provisioning planning semaphore is never closed");
        // The stored row for this destination, if this helm knows one. Its
        // paths fill in any the request leaves out (below), and a requested
        // path EQUAL to the stored one is not new input either: the host
        // row's own set up action sends its stored paths back.
        let stored = match &request.target {
            ProbeDestination::Ssh { destination } => self
                .store
                .list_hosts()
                .await?
                .into_iter()
                .find(|row| row.destination.as_deref() == Some(destination.as_str())),
            ProbeDestination::Local => None,
        };
        // What the person entered is held to the full rule, in the registry's
        // own words, before anything is probed: probing `~/...` reported
        // "not installed" for a host where Farhelm is installed and offered
        // to set it up. A path the helm already stores for this destination
        // keeps the looser rule, so a host stored before the full rule
        // existed can be set up again, which is what repairs it (the install
        // uses absolute paths and the host is re-registered with them).
        if matches!(request.target, ProbeDestination::Ssh { .. }) {
            fn entered<'a>(requested: Option<&'a str>, kept: Option<&str>) -> Option<&'a str> {
                requested.filter(|value| Some(*value) != kept)
            }
            crate::store::require_usable_remote_paths(
                entered(
                    request.remote_farhelm.as_deref(),
                    stored
                        .as_ref()
                        .and_then(|row| row.remote_farhelm.as_deref()),
                ),
                entered(
                    request.remote_state_dir.as_deref(),
                    stored
                        .as_ref()
                        .and_then(|row| row.remote_state_dir.as_deref()),
                ),
            )
            .map_err(|refusal| {
                anyhow::Error::new(ProvisioningRequestError::InvalidProbe(refusal.to_string()))
            })?;
        }
        // A rerun against a row this helm already knows must probe the
        // installed path recorded at registration, not fall back to PATH.
        // A brand-new helm has no such row; the remote probe itself also
        // checks the standard flat install path for that recovery case.
        if let Some(row) = stored {
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
                    .register_discovered(
                        registration,
                        DiscoveredDial {
                            farhelm: dial_farhelm,
                            state_dir: dial_state_dir,
                            identity: host_identity.clone(),
                        },
                        build_version.clone(),
                    )
                    .await?;
                Ok(ProbeResponse::Discovered {
                    host_id,
                    build_version,
                    host_identity,
                })
            }
            // A supervisor that answered but speaks another protocol is
            // still DISCOVERED: register it like any answering supervisor
            // (identity unknown — the skew refusal happens before identity
            // could be exchanged) so the host shows up with the manager's
            // version-skew state and its row carries the update action that
            // fixes it. Installing over it, or erroring out, would both be
            // wrong: something live owns that state directory.
            ProbeObservation::SkewedSupervisor {
                peer_build,
                dial_farhelm,
                dial_state_dir,
            } => {
                let host_id = self
                    .register_discovered(
                        registration,
                        DiscoveredDial {
                            farhelm: dial_farhelm,
                            state_dir: dial_state_dir,
                            identity: None,
                        },
                        peer_build.clone(),
                    )
                    .await?;
                Ok(ProbeResponse::Discovered {
                    host_id,
                    build_version: peer_build,
                    host_identity: None,
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
                    ReachOutcome::SetupManaged => {
                        return Ok(ProbeResponse::Manual {
                            reason: ProvisioningOperation::Add.setup_managed_refusal(),
                        });
                    }
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

    /// Register a host a probe found already running a supervisor, and
    /// settle a failed ADD the discovery makes obsolete, on a task this
    /// service owns (SPEC_impl.md "Who owns an accepted action").
    ///
    /// Registration saves the row and then reconciles the actor set and dials
    /// the host. Run on the request's task, a request dropped between those
    /// steps (a reload, a closed tab) left a saved host with no actor: absent
    /// from the host list and never dialed, or for a re-probed host, still
    /// dialing its old paths, until something unrelated re-read the
    /// registry. A dropped request now loses only the reply. The probe that
    /// calls this now runs on a helm-owned task itself, so this inner one is
    /// belt and braces: it keeps registration owned if another caller
    /// appears.
    async fn register_discovered(
        self: &Arc<Self>,
        registration: ProbeRegistration,
        discovered: DiscoveredDial,
        build_version: String,
    ) -> anyhow::Result<HostId> {
        let service = Arc::clone(self);
        crate::run_owned(async move {
            let host_id = service
                .register(&registration, None, Some(discovered))
                .await?;
            service
                .resolve_failed_add_discovery(host_id, &build_version)
                .await;
            anyhow::Ok(host_id)
        })
        .await
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
                    .find(|row| row.kind.is_reserved_local())
                    .context("the guaranteed local host row is missing")?;
                if let Some(identity) = discovered.and_then(|dial| dial.identity) {
                    match self
                        .store
                        .record_first_contact(row.id, &DialedAs::of(&row), &identity)
                        .await?
                    {
                        FirstContactOutcome::Recorded => {}
                        // Variant roles: `actual` = helm.db's value,
                        // `expected` = the caller's; see HostStoreError::IdentityMismatch.
                        FirstContactOutcome::Mismatch { recorded, reported } => {
                            return Err(anyhow::Error::new(HostStoreError::IdentityMismatch {
                                host: row.id,
                                expected: reported,
                                actual: Some(recorded),
                            }));
                        }
                        FirstContactOutcome::Collision { owner } => {
                            return Err(anyhow::Error::new(HostStoreError::IdentityClaimed {
                                host: row.id,
                                identity,
                                owner,
                            }));
                        }
                        FirstContactOutcome::StaleAttempt { .. } => {
                            return Err(anyhow::Error::new(HostStoreError::StaleAttempt {
                                host: row.id,
                            }));
                        }
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
                if rollback.is_ok() {
                    self.manager.forget_cache_lock(host);
                }
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

    /// This helm's own build, the version Update would install.
    fn helm_build(&self) -> String {
        #[cfg(test)]
        if let Some(build) = self
            .helm_build_override
            .lock()
            .expect("helm build override mutex poisoned")
            .clone()
        {
            return build;
        }
        farhelm_proto::BUILD_VERSION.to_string()
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
        #[cfg(test)]
        {
            let gate = self
                .registry_sync_gate
                .lock()
                .expect("registry sync gate mutex poisoned")
                .take();
            if let Some(gate) = gate {
                gate.reached.notify_one();
                gate.release.notified().await;
            }
        }
        self.manager.sync_registry().await
    }

    /// Consume a confirmed plan exactly once, register first, then start ADD.
    ///
    /// Registering rewrites an existing row's dial paths and drops its
    /// connection, so for a destination that already has a row the run is
    /// claimed first: a host with a run in flight (an UPDATE restarting its
    /// supervisor is exactly when a probe mints an ADD plan for it) refuses
    /// with Busy before anything is rewritten, and the plan is kept for a
    /// later confirmation. Otherwise the refused request would still have
    /// repointed the host at paths the running UPDATE never installed, under
    /// the run that `host_provision_lock` promises a stable row.
    ///
    /// Everything from the claim on runs on a task this service owns
    /// (SPEC_impl.md "Who owns an accepted action"): a claimed host left
    /// behind by a dropped request would refuse every later run as busy.
    pub(super) async fn start_add(
        self: &Arc<Self>,
        request: ProvisionRequest,
    ) -> anyhow::Result<RunAccepted> {
        let pending = self.consume_plan(&request.probe_id).await?;
        let service = Arc::clone(self);
        tokio::spawn(async move {
            service
                .claim_register_and_start(request.probe_id, pending)
                .await
        })
        .await
        .context("the add task ended unexpectedly")?
    }

    /// The body of [`Self::start_add`] after the plan is consumed.
    async fn claim_register_and_start(
        self: Arc<Self>,
        probe_id: String,
        mut pending: PendingPlan,
    ) -> anyhow::Result<RunAccepted> {
        let PendingConfirmation::Add { registration, .. } = &pending.confirmation else {
            return Err(anyhow::Error::new(ProvisioningRequestError::UnknownPlan));
        };
        // Held until the run has claimed its host: another confirmation for
        // the same destination then finds the host busy instead of racing
        // this one past the lookup.
        let _confirming = Arc::clone(&self.add_confirmations).lock_owned().await;
        let existing = match registration {
            ProbeRegistration::Ssh { destination, .. } => self
                .store
                .list_hosts()
                .await?
                .into_iter()
                // Matched on the destination alone: only a row reached over
                // SSH has one, and naming a kind here would be the direct
                // kind comparison `HostKind`'s named questions replace.
                .find(|row| row.destination.as_deref() == Some(destination))
                .map(|row| row.id),
            ProbeRegistration::Local => None,
        };
        if let Some(existing) = existing
            && !self.memory.lock().await.busy.insert(existing)
        {
            self.retain_plan(probe_id, pending).await;
            return Err(anyhow::Error::new(ProvisioningRequestError::Busy(existing)));
        }
        let registered = async {
            let host = self
                .register(registration, Some(&pending.plan), None)
                .await?;
            anyhow::Ok((host, registration_for_row(&self.host_row(host).await?)?))
        }
        .await;
        let (host, registered) = match registered {
            Ok(registered) => registered,
            Err(error) => {
                if let Some(existing) = existing {
                    self.memory.lock().await.busy.remove(&existing);
                }
                return Err(error);
            }
        };
        if let PendingConfirmation::Add { registration, .. } = &mut pending.confirmation {
            *registration = registered;
        }
        let claimed = existing == Some(host);
        if let Some(existing) = existing.filter(|existing| *existing != host) {
            // The row found by destination was not the one registration
            // wrote (a concurrent change of the registry); release that
            // claim and let the run claim its own host.
            self.memory.lock().await.busy.remove(&existing);
        }
        self.start_run(host, pending, claimed).await
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
    pub(super) async fn plan_update(&self, host: HostId) -> anyhow::Result<HostPlanResponse> {
        let row = self.host_row(host).await?;
        if !row.kind.panel_updates() {
            return Err(anyhow::Error::new(ProvisioningRequestError::Refused(
                self.local_handoff_reason().await?,
            )));
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
    ) -> anyhow::Result<HostPlanResponse> {
        self.plan_update_unguarded(host).await
    }

    /// The UPDATE planner itself. See [`Self::plan_update`] for the local
    /// row's refusal, which is deliberately NOT part of this.
    async fn plan_update_unguarded(&self, host: HostId) -> anyhow::Result<HostPlanResponse> {
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
                build_version,
                host_identity,
                dial_farhelm,
                dial_state_dir,
            } => {
                if let Some(refusal) = downgrade_refusal(&build_version, &self.helm_build()) {
                    return Err(anyhow::Error::new(ProvisioningRequestError::Refused(
                        refusal,
                    )));
                }
                if let (Some(recorded), Some(reported)) = (&row.host_identity, &host_identity)
                    && recorded != reported
                {
                    // Variant roles: `actual` = helm.db's value, `expected` =
                    // the caller's; see HostStoreError::IdentityMismatch.
                    return Err(anyhow::Error::new(HostStoreError::IdentityMismatch {
                        host,
                        expected: reported.clone(),
                        actual: Some(recorded.clone()),
                    }));
                }
                expected_identity = expected_identity.or(host_identity);
                if row.kind.has_remote_install() {
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
            // The skewed supervisor is UPDATE's home case: a host left
            // behind by a protocol bump. Its identity cannot be verified —
            // the refusal happens before identity exchange — so the
            // recorded identity is carried forward unverified rather than
            // treated as a mismatch: refusing here would make the one host
            // update exists for permanently un-updatable, and the plan
            // still targets the same registered ssh destination the user
            // clicked. The dial coordinates resolve like the completed
            // hello's.
            ProbeObservation::SkewedSupervisor {
                peer_build,
                dial_farhelm,
                dial_state_dir,
            } => {
                if let Some(refusal) = downgrade_refusal(&peer_build, &self.helm_build()) {
                    return Err(anyhow::Error::new(ProvisioningRequestError::Refused(
                        refusal,
                    )));
                }
                if row.kind.has_remote_install() {
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
            ReachOutcome::Manual(reason) => {
                return Err(anyhow::Error::new(ProvisioningRequestError::Refused(
                    reason,
                )));
            }
            ReachOutcome::SetupManaged => {
                return Err(anyhow::Error::new(ProvisioningRequestError::Refused(
                    ProvisioningOperation::Update.setup_managed_refusal(),
                )));
            }
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
        Ok(HostPlanResponse {
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
        // On a helm-owned task (see `crate::run_owned`): `start_run` marks the
        // host busy and then waits for its provisioning lock before spawning
        // the run that clears it, so a request dropped in between (a reload
        // while an update starts) left the host busy, and refusing every
        // later update, until the helm restarted.
        let service = Arc::clone(self);
        crate::run_owned(async move { service.start_update_owned(host, request).await }).await
    }

    /// The body of [`Self::start_update`], run on a helm-owned task.
    async fn start_update_owned(
        self: Arc<Self>,
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
        self.start_run(host, pending, false).await
    }

    /// Plan removing Farhelm from a remote host and retain the plan behind a
    /// one-use id, changing neither the registry nor the host.
    ///
    /// Unlike UPDATE, the user sees this plan and confirms it: the response
    /// carries the rendered confirmation, and only posting its id back to
    /// [`Self::start_uninstall`] removes anything.
    pub(super) async fn plan_uninstall(&self, host: HostId) -> anyhow::Result<HostPlanResponse> {
        let row = self.host_row(host).await?;
        if !row.kind.panel_uninstalls() {
            return Err(anyhow::Error::new(ProvisioningRequestError::Refused(
                LOCAL_UNINSTALL_HANDOFF.to_string(),
            )));
        }
        let _slot = self
            .plan_slots
            .acquire()
            .await
            .expect("the provisioning planning semaphore is never closed");
        let plan = self.uninstall_plan(host, &row).await?;
        let probe_id = uuid::Uuid::new_v4().to_string();
        let confirmation = plan.confirmation();
        self.retain_plan(
            probe_id.clone(),
            PendingPlan {
                plan: plan.clone(),
                confirmation: PendingConfirmation::Uninstall {
                    host,
                    registration: registration_for_row(&row)?,
                },
            },
        )
        .await;
        Ok(HostPlanResponse {
            probe_id,
            plan,
            confirmation,
        })
    }

    /// Decide whether Farhelm may be removed from `row`'s host right now,
    /// and freeze the plan that would do it.
    ///
    /// Shared by planning and by confirmation, which plans again and
    /// requires the same result: every fact the plan rests on (the
    /// connection, the session list, the running binary, which files
    /// remain and where they lead) is then checked twice by one piece of
    /// code instead of being copied into a second list of facts to compare.
    ///
    /// The ordinary path needs the host connected, because only a connected
    /// host can show that none of its sessions are running, and needs the
    /// supervisor it is connected to to be the one the unit runs. There is
    /// one exception, for continuing a run that failed after it removed the
    /// unit file: with no unit file left nothing can start the supervisor
    /// again, so the remaining steps may go ahead without a connection, but
    /// only on positive evidence that no supervisor answers. A host whose
    /// supervisor answers but cannot be used (another protocol, an identity
    /// problem) is refused outright, because something is running there
    /// and, for an identity problem, the destination may now reach a
    /// different machine.
    async fn uninstall_plan(
        &self,
        host: HostId,
        row: &HostRow,
    ) -> anyhow::Result<ProvisioningPlan> {
        let refused =
            |message: String| anyhow::Error::new(ProvisioningRequestError::Refused(message));
        let status = self.manager.status(host);
        let not_connected = || {
            let phase = status
                .as_ref()
                .map_or("not connected", |status| status.state.phase());
            refused(format!(
                "uninstall needs this host connected, so Farhelm can check that none of its \
                 sessions are running; it is {phase} right now. Get it connected first, then choose \
                 uninstall again"
            ))
        };
        let client = status
            .as_ref()
            .filter(|status| status.state.is_connected())
            .and_then(|status| status.client.clone());
        let target = self.probe_target_for_row(row);
        let (dialed, state_dir) = match &client {
            Some(client) => {
                require_no_running_sessions(client).await?;
                match self
                    .backend
                    .probe(&target)
                    .await
                    .map_err(anyhow::Error::new)?
                {
                    ProbeObservation::Supervisor {
                        host_identity,
                        dial_farhelm,
                        dial_state_dir,
                        ..
                    } => {
                        // The fresh probe dials the destination again, so it
                        // must reach the same install the checked session list
                        // came from: the reported identity has to equal the
                        // recorded one exactly, a missing one included
                        // (SPEC_impl.md). Unlike UPDATE planning, a peer that
                        // reports none against a recorded identity is refused,
                        // and so is a peer reporting one for a host with none
                        // on record (the manager serves identityless
                        // supervisors, so such a row can be connected): either
                        // way the machine answering now may not be the one
                        // whose sessions were checked, and uninstall removes
                        // files.
                        if host_identity != row.host_identity {
                            let reported = host_identity
                                .as_deref()
                                .map_or_else(|| "no identity".to_string(), peer_text);
                            return Err(refused(match &row.host_identity {
                                Some(recorded) => format!(
                                    "the supervisor answering at this host's destination reports \
                                     {reported}, not the identity {} this host has on record, so \
                                     uninstall cannot tell it is the same machine; resolve that \
                                     first",
                                    peer_text(recorded),
                                ),
                                None => format!(
                                    "the supervisor answering at this host's destination reports \
                                     {reported}, but this host has no identity on record, so \
                                     uninstall cannot tell it is the same machine; resolve that \
                                     first"
                                ),
                            }));
                        }
                        (Some(dial_farhelm), dial_state_dir)
                    }
                    ProbeObservation::SkewedSupervisor { .. } | ProbeObservation::Absent => {
                        return Err(refused(
                            "the supervisor stopped answering while uninstall was checking it; \
                             choose uninstall again"
                                .to_string(),
                        ));
                    }
                }
            }
            None => {
                if let Some(status) = &status
                    && matches!(status.state, HostState::Retired { .. })
                {
                    return Err(refused(
                        "the helm's connection to this host has stopped, so uninstall cannot check \
                         its sessions; restart the helm, then choose uninstall again"
                            .to_string(),
                    ));
                }
                if let Some(status) = &status
                    && matches!(
                        status.state,
                        HostState::VersionSkew { .. }
                            | HostState::IdentityMismatch { .. }
                            | HostState::IdentityUnverified { .. }
                            | HostState::Duplicate { .. }
                    )
                {
                    return Err(refused(format!(
                        "uninstall is refused while this host is {}: a supervisor answers there \
                         that Farhelm cannot use, so it cannot check that none of its sessions are \
                         running. Resolve that first, then choose uninstall again",
                        status.state.phase()
                    )));
                }
                match self.backend.probe(&target).await {
                    Ok(ProbeObservation::Absent) => {}
                    Ok(_) | Err(_) => return Err(not_connected()),
                }
                // No running supervisor to ask, so the row's registered
                // paths stand in. Only an absolute binary path says where
                // the install is; a bare name was resolved through the
                // host's PATH and proves nothing about the lib directory.
                (
                    row.remote_farhelm
                        .as_deref()
                        .map(PathBuf::from)
                        .filter(|path| path.is_absolute()),
                    row.remote_state_dir.as_deref().map(PathBuf::from),
                )
            }
        };
        let reach = match self.backend.inspect(&target).await {
            Ok(ReachOutcome::Supported(reach)) => reach,
            Ok(ReachOutcome::Manual(reason)) => return Err(refused(reason)),
            Ok(ReachOutcome::SetupManaged) => {
                return Err(refused(
                    ProvisioningOperation::Uninstall.setup_managed_refusal(),
                ));
            }
            Err(_) if client.is_none() => return Err(not_connected()),
            Err(failure) => return Err(anyhow::Error::new(failure)),
        };
        let paths = self.layout.uninstall_paths(&reach);
        let state_dir = state_dir.or_else(|| paths.state_dir_override.clone());
        let mut asked = vec![paths.unit_path.as_path(), paths.lib_dir.as_path()];
        asked.extend(state_dir.as_deref());
        asked.extend(dialed.as_deref());
        let inspection = self
            .backend
            .inspect_uninstall(&target.transport, &self.layout.unit_name, &asked)
            .await
            .map_err(anyhow::Error::new)?;
        let [unit_file, lib_dir, rest @ ..] = inspection.paths.as_slice() else {
            return Err(anyhow::Error::new(BackendFailure::new(
                "inspecting the installation to remove answered for fewer paths than asked",
                "",
            )));
        };
        // A lib directory that is itself a symlink passes every canonical
        // check below (the binary resolves inside the link's target), yet
        // `rm -rf` of it removes only the link and leaves the installation
        // behind, so it is refused ahead of every other check of the
        // inspection. That includes the resolution check: a dangling link
        // would otherwise get "cannot tell where it leads", without the
        // pointer to Remove, which is what a user with this layout needs.
        // The session and identity checks above run before the inspection
        // exists, so such a user may first be asked to stop sessions before
        // hearing that uninstall cannot run on this layout at all.
        if lib_dir.symlink {
            let link_target = lib_dir.canonical.as_ref().map_or_else(
                || "a target the host cannot resolve".to_string(),
                |target| target.display().to_string(),
            );
            return Err(refused(format!(
                "Farhelm's lib directory {} on this host is a symbolic link to {link_target}; removing \
                 it would remove only the link and leave the installation in place, so uninstall \
                 does not run on this layout. To stop using this host without uninstalling, \
                 remove it from the list instead, which leaves the host as it is",
                lib_dir.path.display()
            )));
        }
        let unit = &self.layout.unit_name;
        if client.is_none() && unit_file.exists {
            return Err(not_connected());
        }
        if client.is_some() && !inspection.unit_running() {
            return Err(refused(format!(
                "the supervisor this host answers with is not running under {unit}; uninstall \
                 only removes a supervisor set up from the hosts panel"
            )));
        }
        // The "no unit file left" rule and the removal both look at the
        // planned unit path; a unit systemd loads from anywhere else is not
        // provisioning's and could start the supervisor again.
        if let Some(fragment) = &inspection.unit_fragment
            && fragment.canonical.is_some()
            && fragment.canonical != unit_file.canonical
        {
            return Err(refused(format!(
                "{unit} on this host is loaded from {}, not from {}; uninstall only removes a unit \
                 set up from the hosts panel",
                fragment.path.display(),
                unit_file.path.display()
            )));
        }
        let unresolved = |path: &Path| {
            refused(format!(
                "uninstall cannot tell where {} leads on this host, so it cannot check that the \
                 removal leaves the host's data alone",
                path.display()
            ))
        };
        let resolved = |host_path: &super::backend::HostPath| {
            host_path
                .canonical
                .clone()
                .map(|canonical| ResolvedPath {
                    named: host_path.path.clone(),
                    canonical,
                })
                .ok_or_else(|| unresolved(&host_path.path))
        };
        let mut rest = rest.iter();
        let state_dir = match state_dir {
            Some(_) => rest.next(),
            None => Some(&inspection.default_state_dir),
        }
        .map(resolved)
        .transpose()?
        .expect("the state directory was asked for or reported by default");
        let farhelm = rest.next();
        // A run that failed after removing the lib directory leaves an
        // unconnected host whose registered binary lies in a directory that
        // is gone: there is nothing left to protect, and insisting on
        // resolving it would refuse the retry forever. A connected host
        // keeps the check, since a supervisor is running from somewhere.
        let farhelm = if client.is_none() && !lib_dir.exists {
            None
        } else {
            farhelm.map(resolved).transpose()?
        };
        let lib_dir = lib_dir
            .exists
            .then(|| resolved(lib_dir).map(|lib| lib.canonical))
            .transpose()?;
        self.layout.plan_uninstall(
            target.transport,
            &reach,
            &paths,
            &UninstallFacts {
                unit_file: unit_file.exists,
                lib_dir,
                farhelm,
                state_dir,
            },
        )
    }

    /// Consume one host-bound UNINSTALL plan and only then claim the run.
    /// On a helm-owned task for [`Self::start_update`]'s reason: a dropped
    /// request must not leave the host marked busy.
    pub(super) async fn start_uninstall(
        self: &Arc<Self>,
        host: HostId,
        request: ProvisionRequest,
    ) -> anyhow::Result<RunAccepted> {
        let service = Arc::clone(self);
        crate::run_owned(async move {
            let pending = service.consume_plan(&request.probe_id).await?;
            if !matches!(
                pending.confirmation,
                PendingConfirmation::Uninstall { host: planned, .. } if planned == host
            ) {
                return Err(anyhow::Error::new(ProvisioningRequestError::UnknownPlan));
            }
            service.start_run(host, pending, false).await
        })
        .await
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

    /// Start a run for `host`. `claimed` says the caller already put `host`
    /// in the busy set (ADD claims an existing row before rewriting it; see
    /// [`Self::start_add`]); otherwise the claim is made here.
    async fn start_run(
        self: &Arc<Self>,
        host: HostId,
        pending: PendingPlan,
        claimed: bool,
    ) -> anyhow::Result<RunAccepted> {
        let run_id = uuid::Uuid::new_v4().to_string();
        {
            let mut memory = self.memory.lock().await;
            if !claimed && !memory.busy.insert(host) {
                return Err(anyhow::Error::new(ProvisioningRequestError::Busy(host)));
            }
            memory.runs.insert(
                host,
                ProvisioningView::for_plan(host, run_id.clone(), &pending.plan),
            );
        }
        // The provisioning lock, not the cache-write lock: the run holds it
        // for minutes, and the host's session list must keep updating
        // meanwhile. See `ActorHandle::provision_lock`.
        let host_write = self.manager.host_provision_lock(host).await;
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

    /// Delete `host`'s registry row and everything this helm keeps for it:
    /// its cached sessions (by cascade), its retained progress, its pending
    /// confirmation ids, its busy marker, its connection actor and its
    /// cache-write lock. Returns whether an actor was running.
    ///
    /// The caller must already hold the host's provisioning lock. Two
    /// callers share this: the hosts panel's Remove, which takes that lock
    /// itself and aborts any run task (there can be none while it holds the
    /// lock, but a finished run's handle may remain), and UNINSTALL's last
    /// step, which runs INSIDE the host's run and so already holds the lock
    /// and must not abort its own task. That is why Remove's own entry
    /// point cannot be reused from the run: it would wait for a lock its
    /// caller holds, or abort the run that called it.
    ///
    /// The order is Remove's: the cache-write lock so no cache write
    /// interleaves with the deletion, the row, the helm's memory of the
    /// host, then the actor, and its lock only after the row is gone.
    pub(crate) async fn delete_registered_host(
        &self,
        host: HostId,
        run_task: RunTask,
    ) -> anyhow::Result<bool> {
        let serialized = self.manager.host_write_lock(host).await;
        self.store.remove_ssh_host(host).await?;
        let task = self.forget_host_memory(host).await;
        if run_task == RunTask::Abort
            && let Some(task) = task
        {
            task.abort();
            let _ = task.await;
        }
        let stopped = self.manager.stop_actor(host).await;
        self.manager.forget_cache_lock(host);
        drop(serialized);
        Ok(stopped)
    }

    /// Drop process-local progress, pending confirmation ids and the busy
    /// marker for a host whose row is gone, handing back its run task's
    /// handle for the caller to abort or merely detach.
    async fn forget_host_memory(&self, host: HostId) -> Option<tokio::task::JoinHandle<()>> {
        let mut memory = self.memory.lock().await;
        let task = memory.tasks.remove(&host);
        memory.runs.remove(&host);
        memory.busy.remove(&host);
        let removed: std::collections::HashSet<String> = memory
            .plans
            .iter()
            .filter(|(_, pending)| match &pending.confirmation {
                PendingConfirmation::Update { host: planned, .. }
                | PendingConfirmation::Uninstall { host: planned, .. } => *planned == host,
                PendingConfirmation::Add { .. } => false,
            })
            .map(|(id, _)| id.clone())
            .collect();
        memory.plans.retain(|id, _| !removed.contains(id));
        memory.plan_order.retain(|id| !removed.contains(id));
        task
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
                match observation {
                    ProbeObservation::Supervisor {
                        build_version,
                        host_identity,
                        dial_farhelm,
                        dial_state_dir,
                    } => {
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
                        Ok(Revalidation::UseAsIs(format!(
                            "a supervisor answered during confirmation (build {build_version}); ADD used it as-is"
                        )))
                    }
                    // Same rule for a skewed one: something live owns that
                    // state directory, so executing the ADD install over it
                    // is off the table. Register it (identity unknown) and
                    // point at the action that actually fixes skew.
                    ProbeObservation::SkewedSupervisor {
                        peer_build,
                        dial_farhelm,
                        dial_state_dir,
                    } => {
                        self.register(
                            registration,
                            None,
                            Some(DiscoveredDial {
                                farhelm: dial_farhelm,
                                state_dir: dial_state_dir,
                                identity: None,
                            }),
                        )
                        .await
                        .map_err(|error| {
                            BackendFailure::new(
                                "registering the supervisor found at confirmation",
                                format!("{error:#}"),
                            )
                        })?;
                        Ok(Revalidation::UseAsIs(format!(
                            "a supervisor answered during confirmation but speaks another protocol \
                             (build {peer_build}); ADD registered it — use the host's update action \
                             to bring it to this helm's version"
                        )))
                    }
                    ProbeObservation::Absent => Ok(Revalidation::Execute),
                }
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
                        build_version,
                        host_identity,
                        dial_farhelm,
                        dial_state_dir,
                    } => {
                        // Rechecked here: the host may have been updated
                        // by other means between planning and this
                        // confirmation.
                        if let Some(refusal) = downgrade_refusal(&build_version, &self.helm_build())
                        {
                            return Err(BackendFailure::new(
                                "the host now runs a newer farhelm than this helm",
                                refusal,
                            ));
                        }
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
                    // Still skewed at confirmation — the expected state for
                    // the update that has not run yet. Identity stays
                    // unverifiable for the same reason it was at planning
                    // (the refusal precedes identity exchange), so the
                    // recorded identity is neither confirmed nor cleared
                    // (`register` only writes an identity when one is
                    // reported), and the update proceeds: it is the only
                    // path that ever makes this host verifiable again.
                    ProbeObservation::SkewedSupervisor {
                        peer_build,
                        dial_farhelm,
                        dial_state_dir,
                    } => {
                        if let Some(refusal) = downgrade_refusal(&peer_build, &self.helm_build()) {
                            return Err(BackendFailure::new(
                                "the host now runs a newer farhelm than this helm",
                                refusal,
                            ));
                        }
                        (
                            None,
                            Some(DiscoveredDial {
                                farhelm: dial_farhelm,
                                state_dir: dial_state_dir,
                                identity: None,
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
            // Plan again under the run's lock and require the same plan: the
            // session check, the connection, the running binary and the
            // files that remain are all rechecked by the code that decided
            // them at planning. A refusal now is reported in its own words;
            // a different plan means the host changed under the
            // confirmation, and the user should see the new one first.
            PendingConfirmation::Uninstall { registration, .. } => {
                self.require_registration_unchanged(host, registration)
                    .await?;
                let row = self.host_row(host).await.map_err(|error| {
                    BackendFailure::new(format!("re-reading the confirmed host row: {error:#}"), "")
                })?;
                let replanned = self
                    .uninstall_plan(host, &row)
                    .await
                    .map_err(|error| BackendFailure::new(format!("{error:#}"), ""))?;
                if replanned != pending.plan {
                    return Err(BackendFailure::new(
                        "the host changed after the uninstall plan was made; choose uninstall \
                         again to see the new plan",
                        "",
                    ));
                }
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
                "the host configuration changed after the plan was made",
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
                    "identity {identity:?} belongs to host {twin}; remove that entry or change this \
                     one's destination, then press Retry"
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
    ///
    /// Leaves the host's busy marker alone. The failed run released its own
    /// claim when it failed, so a marker present now belongs to another
    /// operation, such as a rerun confirmed moments ago that has not yet
    /// installed its progress view; clearing it let a second install or
    /// update be accepted beside that one.
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
                self.fail_action(host, plan.operation, index, &plan.actions[index], error)
                    .await;
                return;
            }
        };
        for (index, action) in plan.actions.iter().enumerate() {
            self.set_step(host, index, StepStatus::Running, None).await;
            let outcome = self.execute_action(host, &plan, action, &prepared).await;
            let (status, message) = match outcome {
                Ok(ActionOutcome::Completed) => (StepStatus::Completed, None),
                Ok(ActionOutcome::Skipped(message)) => (StepStatus::Skipped, Some(message)),
                Ok(ActionOutcome::Degraded(message)) => (StepStatus::Degraded, Some(message)),
                Err(error) => {
                    self.fail_action(host, plan.operation, index, action, error)
                        .await;
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
    ///
    /// Each payload is prepared once, even though remote plans report upload
    /// and installation as separate actions. A cold host needs both
    /// a `farhelm` and a `tmux`, the download source locks per asset so
    /// those two never contend, and a serial walk would make cold-provision
    /// latency the sum of two multi-megabyte downloads while holding one of
    /// only `MAX_CONCURRENT_RUNS` fleet-wide slots. The fan-out is bounded
    /// by the plan itself — a plan carries at most one payload of each
    /// [`PayloadKind`] — so no explicit limit is needed here; if that ever
    /// stops being true this needs a semaphore rather than a bigger fan-out.
    ///
    /// Each future carries its own action index so a failure still names the
    /// plan step the user is looking at. Nothing on the host is touched until
    /// every payload is prepared.
    ///
    /// `join_all`, NOT `try_join_all`, and the difference matters (F6, review
    /// round 2). `try_join_all` drops the remaining futures the moment one
    /// fails — but a dropped future does not stop the `spawn_blocking` task a
    /// sibling may have running, and tokio keeps that closure alive to
    /// completion. Dropping the future DOES release the per-asset mutex and
    /// abandon the `OnceCell` initialisation that were the only guarantee of
    /// a single writer, so an immediate retry could start deleting and
    /// rewriting the same fixed staging, marker, and binary paths while the
    /// detached publication was still writing them. Awaiting every future and
    /// only then returning the first error costs one extra download on a
    /// failing run and keeps cache-write ownership single at all times.
    ///
    /// Not covered by a test of its own: reproducing it needs a failure
    /// injected between a sibling's entry into `spawn_blocking` and its exit,
    /// which is not reachable through any seam the service exposes today.
    /// This comment is the record of why the cheaper `try_join_all` is wrong
    /// here.
    async fn prepare_payloads(
        &self,
        plan: &ProvisioningPlan,
    ) -> Result<HashMap<super::plan::PayloadKind, PreparedPayload>, (usize, BackendFailure)> {
        let mut seen = std::collections::HashSet::new();
        let preparations =
            plan.actions
                .iter()
                .enumerate()
                .filter_map(|(index, action)| match action {
                    ProvisioningAction::UploadPayload { payload, arch, .. }
                    | ProvisioningAction::InstallPayload { payload, arch, .. }
                        if seen.insert(*payload) =>
                    {
                        Some((index, *payload, *arch))
                    }
                    _ => None,
                })
                .map(|(index, payload, arch)| async move {
                    let source =
                        self.payloads.path(payload, arch).await.map_err(|error| {
                            (index, BackendFailure::new(format!("{error:#}"), ""))
                        })?;
                    let staged = stage_payload(&source)
                        .await
                        .map_err(|error| (index, error))?;
                    Ok::<_, (usize, BackendFailure)>((index, payload, staged))
                });
        let mut prepared = HashMap::new();
        // Reported in plan order rather than completion order, so two
        // failures in one run always name the same step.
        let mut failure: Option<(usize, BackendFailure)> = None;
        for outcome in futures_util::future::join_all(preparations).await {
            match outcome {
                Ok((_, payload, staged)) => {
                    prepared.insert(payload, staged);
                }
                Err((index, error)) => {
                    if failure.as_ref().is_none_or(|(first, _)| index < *first) {
                        failure = Some((index, error));
                    }
                }
            }
        }
        match failure {
            Some(failure) => Err(failure),
            None => Ok(prepared),
        }
    }

    /// Retain one failed action and release the host claim without undoing
    /// any earlier completed steps.
    async fn fail_action(
        &self,
        host: HostId,
        operation: ProvisioningOperation,
        index: usize,
        action: &ProvisioningAction,
        error: BackendFailure,
    ) {
        // The way to continue depends on the operation: a failed setup or
        // update reruns provisioning, a failed uninstall is chosen again.
        let remedy = match operation {
            ProvisioningOperation::Add | ProvisioningOperation::Update => {
                "rerun provisioning to continue"
            }
            ProvisioningOperation::Uninstall => "choose uninstall again to continue",
        };
        let message = format!(
            "step {} ({}) failed: {}; {remedy}",
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
        prepared: &HashMap<super::plan::PayloadKind, PreparedPayload>,
    ) -> Result<ActionOutcome, BackendFailure> {
        match action {
            ProvisioningAction::EnsureDirectories { directories } => {
                self.backend
                    .ensure_directories(&plan.target, directories)
                    .await
            }
            ProvisioningAction::UploadPayload {
                payload,
                destination,
                temporary,
                ..
            } => {
                let source = prepared.get(payload).ok_or_else(|| {
                    BackendFailure::new("the prepared payload disappeared before upload", "")
                })?;
                self.backend
                    .upload_path(&plan.target, *payload, source, destination, temporary)
                    .await
            }
            ProvisioningAction::InstallPayload {
                payload,
                destination,
                temporary,
                ..
            } => {
                let source = prepared.get(payload).ok_or_else(|| {
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
            ProvisioningAction::DisableSupervisor { unit } => {
                self.backend.disable(&plan.target, unit).await
            }
            ProvisioningAction::RemoveUnit { destination, .. } => {
                self.backend.remove_unit(&plan.target, destination).await
            }
            ProvisioningAction::StopSupervisor {
                unit,
                tmux_program,
                tmux_socket,
            } => {
                self.backend
                    .stop(&plan.target, unit, tmux_program, tmux_socket)
                    .await
            }
            ProvisioningAction::RemoveDirectory { path } => {
                self.backend.remove_directory(&plan.target, path).await
            }
            // The run holds the host's provisioning lock, so the row
            // removal must not take it again or abort this task; see
            // `delete_registered_host`. Its progress view goes with the
            // row, which is why the confirming client builds its success
            // notice from the plan it already holds.
            ProvisioningAction::ForgetHost => {
                self.delete_registered_host(host, RunTask::Detach)
                    .await
                    .map_err(|error| {
                        BackendFailure::new(
                            format!("removing the host from the host list: {error:#}"),
                            "",
                        )
                    })?;
                Ok(ActionOutcome::Completed)
            }
            ProvisioningAction::AttachSupervisor => {
                if let Some(outcome) = self.backend.injected_attach(&plan.target).await? {
                    return Ok(outcome);
                }
                let previous_incarnation =
                    self.manager.status(host).map(|status| status.incarnation);
                // A fresh window, not a plain retry: this step runs seconds
                // after starting or restarting the supervisor's own unit,
                // so the host is coming back by our own action — and a
                // single probe would race that start latency, then sit out
                // a full re-probe past this step's own deadline.
                //
                // The ticket separates the helm refusing the supervisor this
                // run just installed from a refusal the host already held
                // before this step, often the very protocol skew an update
                // is fixing: only a refusal from a dial numbered after the
                // ticket stops the wait (see
                // `ConnectionManager::retry_now_with_fresh_window`). `None`
                // (no such host) stops nothing early; the loop below then
                // reports the host as gone, as before.
                let ticket = self
                    .manager
                    .retry_now_with_fresh_window(host)
                    .await
                    .map_err(|error| {
                        BackendFailure::new("requesting supervisor attach", error.to_string())
                    })?;
                let attached = tokio::time::timeout(ATTACH_TIMEOUT, async {
                    loop {
                        let status = self.manager.status(host)?;
                        if status.state.is_connected()
                            && status.client.is_some()
                            && Some(status.incarnation) != previous_incarnation
                        {
                            return Some(Ok(()));
                        }
                        if ticket.is_some_and(|ticket| status.refusal_attempt > ticket)
                            && let Some(failure) = attach_refusal(&status.state)
                        {
                            return Some(Err(failure));
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
                })??;
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

/// Refuse UNINSTALL while the host has any session that has not ended or
/// any live terminal tab, naming them.
///
/// Reads a fresh list through the host's connection, never the helm's
/// cached snapshot: the question is what is running now. A session whose
/// status is unknown counts as running (`has_ended`, not `is_live`), since
/// "could not tell" is not evidence it stopped, and a list the supervisor
/// cut at its cap refuses too, because the sessions it left out are
/// unchecked. Nothing is stopped or killed here or later: the user stops
/// them, as SPEC.md asks of `farhelm uninstall` too.
async fn require_no_running_sessions(
    client: &crate::client::SupervisorClient,
) -> anyhow::Result<()> {
    let refused = |message: String| anyhow::Error::new(ProvisioningRequestError::Refused(message));
    let listing = client.list_sessions().await.map_err(|error| {
        refused(format!(
            "uninstall could not read this host's session list ({error:#}), so it cannot check \
             that none are running; get the host connected first"
        ))
    })?;
    if listing.truncated {
        return Err(refused(
            "uninstall is refused: this host holds more sessions than one list can carry, so \
             Farhelm cannot check that none are running; delete sessions you no longer need first"
                .to_string(),
        ));
    }
    let running: Vec<&farhelm_proto::SessionInfo> = listing
        .sessions
        .iter()
        .filter(|session| !session.status.has_ended() || !session.tabs.is_empty())
        .collect();
    if running.is_empty() {
        return Ok(());
    }
    let mut named = running
        .iter()
        .take(UNINSTALL_REFUSAL_NAMED_SESSIONS)
        .map(|session| format!("\"{}\"", peer_text(&session.title)))
        .collect::<Vec<_>>()
        .join(", ");
    if running.len() > UNINSTALL_REFUSAL_NAMED_SESSIONS {
        named.push_str(&format!(
            " and {} more",
            running.len() - UNINSTALL_REFUSAL_NAMED_SESSIONS
        ));
    }
    Err(refused(format!(
        "uninstall is refused while sessions on this host have not ended or still have terminal \
         tabs open: {named}. Stop those sessions and close their tabs, then choose uninstall again"
    )))
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
