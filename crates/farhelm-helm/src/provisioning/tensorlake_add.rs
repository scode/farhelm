//! One setup task owns environment creation and the first session result.
//!
//! Shared session launches dispatch through the provider registry. Tensorlake's
//! ordinary Add path also lives here because it retains confirmation and legacy
//! install recovery. A Replace source never enters fresh-environment cleanup.
//! Frozen recipe choices retain named credentials, whose current values resolve
//! only at delivery.

use super::super::{
    http::{ProgressViews as _, StepView},
    plan::{PlanStep, PlanSteps},
};
use super::*;
use crate::store::{NewHostEntry, NewTensorlakeHost};
#[cfg(test)]
use crate::tensorlake::api::{CreateSandbox, Resources};
use crate::tensorlake::{
    Tensorlake,
    api::{SandboxSource, TensorlakeError},
    config::helm_tag,
    prepared::PreparedInstallFacts,
    recipes::StoredRecipe,
};

/// A confirmed recipe and the name chosen before any request can create a
/// sandbox. `host` distinguishes a rerun from a new registry entry.
pub(super) struct TensorlakeAddPlan {
    pub(super) host: Option<HostId>,
    sandbox: String,
    destination: String,
    entry: NewHostEntry,
    recipe_name: String,
    recipe: StoredRecipe,
    steps: PlanSteps,
    tensorlake: Tensorlake,
}

/// Adapt the existing operation view to provider creation without giving the
/// provider ownership of a session task or a second progress registry.
struct CreationProgress<'a> {
    service: &'a ProvisioningService,
    host: HostId,
    step_index: usize,
}

/// Preparation can join a job without doing any work. Reveal its extra step
/// only after the provider reports that renewal or a software build is needed.
struct SessionPreparation<'a> {
    service: &'a ProvisioningService,
    host: HostId,
    visible: std::sync::atomic::AtomicBool,
    /// The provider's name for the step ([`crate::environment::LaunchStepNames`]),
    /// fixed when the launch was admitted like the run's other step names.
    name: String,
}

#[async_trait::async_trait]
impl crate::environment::PreparationProgress for SessionPreparation<'_> {
    async fn needed(&self) {
        if !self.visible.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let step = preparation_step();
            if let Some(run) = self.service.memory.lock().await.runs.get_mut(&self.host) {
                run.steps.insert(
                    1,
                    StepView {
                        step: step.id,
                        name: self.name.clone(),
                        paths: step.paths,
                        explanation: step.explanation,
                        status: StepStatus::Running,
                        message: None,
                    },
                );
            }
            self.service.manager.events().bump();
        }
    }

    async fn step(&self, text: String) {
        let index = usize::from(self.visible.load(std::sync::atomic::Ordering::SeqCst));
        self.service
            .set_step(self.host, index, StepStatus::Running, Some(text))
            .await;
    }
}

#[async_trait::async_trait]
impl crate::environment::PreparationProgress for CreationProgress<'_> {
    async fn needed(&self) {}
    async fn step(&self, text: String) {
        self.service
            .set_step(self.host, self.step_index, StepStatus::Running, Some(text))
            .await;
    }
}

/// The initial session belongs to the existing provisioning task. The launch's
/// end is published on its run either way; `result` is for an in-process caller
/// that waits for it (an agent's request), and the HTTP route drops it at once.
/// Dropping that waiter never cancels resource cleanup.
struct SessionLaunch {
    state: Arc<crate::AppState>,
    /// The launch's run, which its end is published on.
    run_id: String,
    request: crate::sessions::CreateReq,
    name_kind: Option<farhelm_proto::launcher::TemplateNameKind>,
    result: tokio::sync::oneshot::Sender<anyhow::Result<farhelm_proto::SessionInfo>>,
}

#[cfg(test)]
#[path = "tensorlake_add_tests.rs"]
mod tests;

/// Name the sandbox with a per-helm ownership tag, a readable slug, and a
/// random suffix. Always valid Tensorlake grammar, at most 63 ASCII bytes.
fn sandbox_name(prefix: &str, tag: &str, display: &str) -> String {
    let mut slug = String::new();
    for c in display.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "host" } else { slug };
    let slug = slug
        .get(..slug.len().min(40))
        .unwrap()
        .trim_end_matches('-');
    format!(
        "{prefix}{tag}-{slug}-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..4]
    )
}

/// Describe only what the local ownership record establishes. Probing does no
/// network work, so a rerun without a recorded ID must make any billed build
/// conditional on later settling the sandbox lookup and pending create.
fn steps(
    sandbox: &str,
    name: &str,
    recipe: &StoredRecipe,
    ready: bool,
    rerun: bool,
    recorded_sandbox: bool,
) -> PlanSteps {
    let mut descriptions = vec![];
    let snapshot = if recorded_sandbox {
        "Use the host's recorded sandbox (no snapshot build)".into()
    } else if rerun {
        if ready {
            format!(
                "Find the host's sandbox; use the snapshot for recipe {name} only if a new sandbox is needed"
            )
        } else {
            format!(
                "Find the host's sandbox; build the snapshot for recipe {name} only if the sandbox is absent and any pending create has settled"
            )
        }
    } else if ready {
        format!("Use the snapshot for recipe {name}")
    } else {
        format!(
            "Build the snapshot for recipe {name} (packages, Claude Code and Codex; takes a few minutes)"
        )
    };
    descriptions.push(("build-snapshot", snapshot));
    descriptions.push((
        "create-sandbox",
        format!(
            "{} sandbox {sandbox} with {} CPUs, {} GB memory and {} GB disk",
            if recorded_sandbox {
                "Use the recorded"
            } else if rerun {
                "Find or create"
            } else {
                "Create"
            },
            recipe.size.cpus,
            recipe.size.memory_gb,
            recipe.size.disk_gb
        ),
    ));
    descriptions.push((
        "prepare-sandbox",
        "Start the sandbox's login manager and keep the user manager running".into(),
    ));
    // None is a real Codex selection: removing old Farhelm provider settings
    // can do work or fail even when no credential is selected in this recipe.
    descriptions.push((
        "configure-agents",
        "Configure host credentials and agent endpoints, preserving other settings".into(),
    ));
    descriptions.push((
        "set-up-farhelm",
        if recipe.tunnels.is_empty() {
            "Install Farhelm's supervisor, as on any host".into()
        } else {
            // Forwarding belongs to the connection, rather than a one-shot
            // install action. The confirmation still names that grant.
            "Install Farhelm's supervisor and open the recipe's tunnels when the host connects"
                .into()
        },
    ));
    PlanSteps {
        lead: if rerun {
            "Continue setting up the Tensorlake sandbox as a host"
        } else {
            "Create a Tensorlake sandbox and set it up as a host"
        }
        .into(),
        kept: None,
        steps: descriptions
            .into_iter()
            .map(|(id, name)| PlanStep {
                id: id.into(),
                name,
                paths: vec![],
                explanation: None,
            })
            .collect(),
    }
}

/// The policy is durable step text: phase progress must not replace the
/// explanation while the user is waiting for the first preparation.
const PREPARATION_EXPLANATION: &str =
    "Only the first time, and then at most once every 24 hours or after Farhelm updates.";

/// Describe the cold-clone path without promising an installation per host.
/// Existing sandboxes retain the ordinary setup plan instead; recovery can
/// select that plan once its ownership lookup establishes what already exists.
fn prepared_host_steps(
    sandbox: &str,
    name: &str,
    recipe: &StoredRecipe,
    built: bool,
    rerun: bool,
    preparation_needed: bool,
) -> PlanSteps {
    let mut plan = steps(sandbox, name, recipe, built, rerun, false);
    plan.steps.retain(|step| {
        matches!(
            step.id.as_str(),
            "build-snapshot" | "create-sandbox" | "configure-agents"
        )
    });
    if preparation_needed {
        plan.steps.insert(1, preparation_step());
    }
    if let Some(create) = plan
        .steps
        .iter_mut()
        .find(|step| step.id == "create-sandbox")
    {
        create.name.push_str(" from the prepared snapshot");
    }
    plan.steps.push(PlanStep {
        id: "attach-supervisor".into(),
        name: if recipe.tunnels.is_empty() {
            "Connect to the supervisor that starts when the sandbox boots".into()
        } else {
            "Connect to the supervisor that starts at boot and open the recipe's tunnels".into()
        },
        paths: vec![],
        explanation: None,
    });
    plan
}

/// A stable step identity lets execution insert the explanation if the cache
/// expired after the free probe, without losing the completed build step.
fn preparation_step() -> PlanStep {
    PlanStep {
        id: "prepare-snapshot".into(),
        name: "Prepare snapshot".into(),
        paths: vec![],
        explanation: Some(PREPARATION_EXPLANATION.into()),
    }
}

/// The outer plan's descriptions as a live setup run.
fn progress(host: HostId, run_id: String, steps: &PlanSteps) -> ProvisioningView {
    ProvisioningView {
        host_id: host,
        run_id: Some(run_id),
        operation: Some(ProvisioningOperation::Add),
        status: RunStatus::Running,
        message: None,
        steps: steps
            .steps
            .iter()
            .map(|s| StepView {
                step: s.id.clone(),
                name: s.name.clone(),
                paths: s.paths.clone(),
                explanation: s.explanation.clone(),
                status: StepStatus::Pending,
                message: None,
            })
            .collect(),
        outcome_unknown: false,
    }
}

/// What a launch run says when its task panicked and left no result
/// (`ProvisioningService::end_panicked_launch`): the words the streamed reply
/// of an environment create used for that case before the route took the
/// accepted form.
pub(super) const LAUNCH_ENDED_WITHOUT_A_RESULT: &str = "The launch task ended without a result. Check Hosts for the named environment before launching again.";

/// Typed form refusal; messages here contain no endpoint settings or tokens.
fn refused(message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(ProvisioningRequestError::Refused(message.into()))
}

impl ProvisioningService {
    /// Validate and retain a free, local-only outer plan. A rerun preserves
    /// the row's sandbox and size, while reading its recipe's current settings.
    pub(in super::super) async fn probe_tensorlake(
        self: &Arc<Self>,
        request: ProbeRequest,
        tensorlake: Tensorlake,
    ) -> anyhow::Result<ProbeResponse> {
        let service = Arc::clone(self);
        crate::run_owned(async move { service.probe_tensorlake_owned(request, tensorlake).await })
            .await
    }

    /// One owned provisioning task creates a fresh environment and its first
    /// session. The row and reserved name precede provider I/O; client loss
    /// does not cancel setup or confirmed cleanup of this new resource. The
    /// task ends its run even if it panics ([`Self::end_panicked_launch`]).
    pub(crate) async fn start_session_environment(
        self: Arc<Self>,
        state: Arc<crate::AppState>,
        recipe: String,
        place: farhelm_proto::Place,
        request: crate::sessions::CreateReq,
        name_kind: Option<farhelm_proto::launcher::TemplateNameKind>,
    ) -> anyhow::Result<(
        RunAccepted,
        tokio::sync::oneshot::Receiver<anyhow::Result<farhelm_proto::SessionInfo>>,
    )> {
        request.validate_sandbox_launch(&state).await?;
        let request = request.with_environment_target(farhelm_proto::LaunchTarget::Environment {
            recipe: recipe.clone(),
            place,
        });
        let (definition, saved) = self
            .store
            .tensorlake_recipe_row(&recipe)
            .await?
            .ok_or_else(|| refused("the recipe no longer exists"))?;
        anyhow::ensure!(
            definition.base.supports(place.provider()),
            "this recipe's base cannot run at the selected place"
        );
        saved.require_launch_source(place)?;
        let provider = self
            .manager
            .providers()
            .get(place.provider())
            .map_err(|error| refused(error.0))?;
        // Cached place metadata is free. The provider's fresh launch check
        // remains in ensure_prepared, after this task owns its progress row.
        let available = provider
            .places()
            .await?
            .into_iter()
            .find(|candidate| candidate.place == place)
            .ok_or_else(|| refused("the selected environment place is no longer available"))?;
        if let Some(reason) = available.reason {
            return Err(refused(reason));
        }
        // A container's progress names its place host as Hosts shows it.
        // Read before the row is registered, so a failed read needs no cleanup.
        let place_name = match place.host() {
            Some(id) => {
                let row = self.host_row(id).await?;
                Some(crate::aggregate::host_display_name(
                    row.kind,
                    row.destination.as_deref(),
                    row.alias.as_deref(),
                ))
            }
            None => None,
        };
        let _confirming = Arc::clone(&self.add_confirmations).lock_owned().await;
        let tag = helm_tag(&self.store.registry_namespace().await?);
        let name = sandbox_name("farhelm-sess-", &tag, &recipe);
        let host = crate::environment::ProviderHandle::register_session(
            &self.store,
            name.clone(),
            recipe.clone(),
            place,
            definition.size,
        )
        .await?;
        let row = self.host_row(host).await?;
        let binding = row
            .environment_binding()
            .context("the registered environment has no binding")?;
        let reconcile = async {
            anyhow::ensure!(
                self.store.tensorlake_recipe_row(&recipe).await?.is_some(),
                "the recipe was deleted while launching; choose it again"
            );
            self.sync_registry().await
        }
        .await;
        if let Err(error) = reconcile {
            self.store
                .remove_host_row(host, Some(&binding.handle.never_created(host)))
                .await?;
            self.manager.stop_actor(host).await;
            self.manager.forget_cache_lock(host);
            return Err(error);
        }
        let crate::environment::LaunchStepNames {
            preparing,
            creating,
            starting,
        } = crate::environment::LaunchStepNames::at(place, &recipe, place_name.as_deref());
        let steps = PlanSteps {
            lead: format!("Launch a session on {recipe}"),
            kept: None,
            steps: [
                ("build-snapshot", "Select the recipe source".to_owned()),
                ("create-sandbox", creating),
                (
                    "configure-agents",
                    "Configure credentials and agent endpoints".to_owned(),
                ),
                ("attach-supervisor", starting),
                ("create-session", "Launch the session".to_owned()),
            ]
            .into_iter()
            .map(|(id, name)| PlanStep {
                id: id.into(),
                name,
                paths: vec![],
                explanation: None,
            })
            .collect(),
        };
        let run_id = uuid::Uuid::new_v4().to_string();
        {
            let mut memory = self.memory.lock().await;
            anyhow::ensure!(
                memory.busy.insert(host),
                "this environment already has setup in flight"
            );
            memory
                .runs
                .insert(host, progress(host, run_id.clone(), &steps));
        }
        let host_write = self.manager.host_provision_lock(host).await;
        self.manager
            .hold_connection(
                host,
                HostState::Unreachable {
                    cause: crate::manager::UnreachableCause::TransportFailure,
                    last_error: "Farhelm is setting up this environment".into(),
                },
            )
            .await;
        let setup_state = self.manager.state(host);
        self.manager.events().bump();
        let (result, receiver) = tokio::sync::oneshot::channel();
        let launch = SessionLaunch {
            state,
            run_id: run_id.clone(),
            request,
            name_kind,
            result,
        };
        let service = self.clone();
        let task_run = run_id.clone();
        let task = tokio::spawn(async move {
            use futures_util::FutureExt as _;
            let _slot = service
                .run_slots
                .clone()
                .acquire_owned()
                .await
                .expect("run slots stay open");
            let launched = std::panic::AssertUnwindSafe(async {
                #[cfg(test)]
                if service
                    .panic_session_launch
                    .swap(false, std::sync::atomic::Ordering::SeqCst)
                {
                    panic!("injected session launch panic");
                }
                let mut index = 0;
                let result = service
                    .environment_session_steps(
                        &recipe,
                        &definition,
                        place,
                        binding,
                        preparing,
                        &mut index,
                    )
                    .await;
                service
                    .finish_session_launch(
                        host,
                        &name,
                        launch,
                        result,
                        index,
                        setup_state,
                        host_write,
                    )
                    .await;
            })
            .catch_unwind()
            .await;
            if launched.is_err() {
                service.end_panicked_launch(host, &task_run).await;
            }
        });
        self.memory.lock().await.tasks.insert(host, task);
        Ok((
            RunAccepted {
                host_id: host,
                run_id,
            },
            receiver,
        ))
    }

    /// End the run of a launch whose task panicked, which would otherwise stay
    /// running for good: nothing else ends it, and a client waiting on the run
    /// (the accepted environment create's only outcome) would wait forever.
    /// The words are the ones the streamed reply used for a task that ended
    /// without a result ([`LAUNCH_ENDED_WITHOUT_A_RESULT`]); the run and each
    /// step it was still running are marked failed with them, and the run is
    /// marked `outcome_unknown`, since the environment, or even its session,
    /// may be left behind. The run may be the host's own or, when the panic
    /// came after cleanup removed the host, its kept copy. The unwind has
    /// already let go of the provisioning lock; the busy marker goes here, and
    /// so does the "setting up" hold on a host that is still registered, which
    /// would otherwise show the environment as being set up for good.
    ///
    /// Only a run still running is touched. A panic after the launch published
    /// its end must not overwrite that outcome, nor clear a busy marker that
    /// by then belongs to a later run of the host.
    async fn end_panicked_launch(&self, host: HostId, run_id: &str) {
        let mut memory = self.memory.lock().await;
        let Some(run) = memory
            .launch_run_to_end(host, run_id)
            .filter(|run| run.status == RunStatus::Running)
        else {
            return;
        };
        for step in &mut run.steps {
            if step.status == StepStatus::Running {
                step.status = StepStatus::Failed;
                step.message = Some(LAUNCH_ENDED_WITHOUT_A_RESULT.into());
            }
        }
        run.status = RunStatus::Failed;
        run.message = Some(LAUNCH_ENDED_WITHOUT_A_RESULT.into());
        run.outcome_unknown = true;
        memory.busy.remove(&host);
        drop(memory);
        // A no-op once cleanup has removed the host: its actor is gone with it.
        self.manager.release_hold(host, true);
        self.manager.events().bump();
    }

    /// A prepared create's durable facts certify the common installer's
    /// whole-state removal before capture. Checking a newly booted clone for
    /// absent state would reject its own supervisor database. Admission uses
    /// only that create's saved facts, never a newly renewed recipe image.
    async fn environment_session_steps(
        &self,
        name: &str,
        recipe: &StoredRecipe,
        place: farhelm_proto::Place,
        binding: crate::environment::EnvironmentBinding,
        preparing: String,
        index: &mut usize,
    ) -> anyhow::Result<()> {
        // The new environment's own host row, which its binding was read from.
        let host = binding.host;
        let provider = self
            .manager
            .providers()
            .get(place.provider())
            .map_err(|error| refused(error.0))?;
        self.set_step(host, 0, StepStatus::Running, None).await;
        let preparation = SessionPreparation {
            service: self,
            host,
            visible: false.into(),
            name: preparing,
        };
        let selected = match provider
            .ensure_prepared(
                name,
                recipe,
                place,
                self.manager.events().clone(),
                &preparation,
            )
            .await
        {
            Ok(selected) => selected,
            Err(error) => {
                *index = usize::from(
                    preparation
                        .visible
                        .load(std::sync::atomic::Ordering::SeqCst),
                );
                return Err(error);
            }
        };
        let visible = preparation
            .visible
            .load(std::sync::atomic::Ordering::SeqCst);
        self.set_step(host, 0, StepStatus::Completed, None).await;
        if visible {
            self.set_step(host, 1, StepStatus::Completed, None).await;
        }
        *index = 1 + usize::from(visible);
        self.set_step(host, *index, StepStatus::Running, None).await;
        let progress = CreationProgress {
            service: self,
            host,
            step_index: *index,
        };
        let created = provider.create(&binding, Some(selected), &progress).await?;
        self.set_step(
            host,
            *index,
            if created.created {
                StepStatus::Completed
            } else {
                StepStatus::Skipped
            },
            None,
        )
        .await;
        let row = self.host_row(host).await?;
        let current = row
            .environment_binding()
            .context("the environment binding disappeared before admission")?;
        let facts = current
            .handle
            .prepared_install(&self.store, host)
            .await?
            .context("this environment has no saved prepared installation; launch it again")?;
        anyhow::ensure!(
            facts.home.is_absolute()
                && facts.farhelm_path.is_absolute()
                && facts.state_dir.is_absolute(),
            "the prepared installation has invalid paths"
        );
        anyhow::ensure!(
            current
                .handle
                .admit_install(
                    &self.store,
                    &row,
                    &path_text(&facts.farhelm_path)?,
                    &path_text(&facts.state_dir)?
                )
                .await?,
            "the environment changed before installation admission"
        );
        self.sync_registry().await?;
        *index += 1;
        self.set_step(host, *index, StepStatus::Running, None).await;
        let row = self.host_row(host).await?;
        let home_notice = self
            .delivery
            .remember_home(&row, facts.home.clone())
            .await
            .err();
        let notice = self
            .delivery
            .setup(&row, recipe, &facts.home, self.manager.events())
            .await
            .err();
        self.set_step(host, *index, StepStatus::Completed, notice.or(home_notice))
            .await;
        *index += 1;
        self.set_step(host, *index, StepStatus::Running, None).await;
        let target = self.probe_target_for_row(&row).await?.transport;
        self.manager.release_hold(host, true);
        let (status, notice) = match self.attach_supervisor(host, &target).await? {
            ActionOutcome::Completed => (StepStatus::Completed, None),
            ActionOutcome::Skipped(text) => (StepStatus::Skipped, Some(text)),
            ActionOutcome::Degraded(text) => (StepStatus::Degraded, Some(text)),
        };
        self.set_step(host, *index, status, notice).await;
        Ok(())
    }

    async fn probe_tensorlake_owned(
        &self,
        request: ProbeRequest,
        tensorlake: Tensorlake,
    ) -> anyhow::Result<ProbeResponse> {
        let _slot = self
            .plan_slots
            .acquire()
            .await
            .expect("planning slots stay open");
        if request.remote_farhelm.is_some()
            || request.remote_state_dir.is_some()
            || request.new_entry.is_some()
        {
            return Err(anyhow::Error::new(ProvisioningRequestError::InvalidProbe(
                "Tensorlake probes do not accept remote_farhelm, remote_state_dir or new_entry"
                    .into(),
            )));
        }
        tensorlake
            .api_key()
            .await
            .map_err(|_| refused("Save a Tensorlake API key in Settings → Tensorlake first"))?;
        let (host, name, recipe_name, sandbox, size, recorded_sandbox, new_install, saved_prepared) =
            match request.target {
                ProbeDestination::Tensorlake { name, recipe } => {
                    let tag = helm_tag(&self.store.registry_namespace().await?);
                    (
                        None,
                        name.clone(),
                        recipe,
                        sandbox_name(crate::tensorlake::orphans::HOST_PREFIX, &tag, &name),
                        None,
                        false,
                        true,
                        false,
                    )
                }
                ProbeDestination::TensorlakeHost { host } => {
                    if self.memory.lock().await.busy.contains(&host) {
                        return Err(anyhow::Error::new(ProvisioningRequestError::Busy(host)));
                    }
                    let row = self.host_row(host).await?;
                    let binding = row
                        .tensorlake
                        .ok_or_else(|| refused("this host is not a Tensorlake sandbox"))?;
                    let saved_prepared = self
                        .store
                        .tensorlake_prepared_install(host)
                        .await?
                        .is_some();
                    // Existing ordinary installations retain their install plan.
                    // A saved prepared create already chose its source and must
                    // not promise another billed preparation on recovery.
                    let new_install = saved_prepared
                        || (row.remote_farhelm.is_none()
                            && row.remote_state_dir.is_none()
                            && binding.sandbox_id.is_none()
                            && binding.pending_create_at.is_none());
                    (
                        Some(host),
                        row.alias.unwrap_or_else(|| binding.sandbox.clone()),
                        binding.recipe,
                        binding.sandbox,
                        binding.resources,
                        binding.sandbox_id.is_some(),
                        new_install,
                        saved_prepared,
                    )
                }
                _ => return Err(refused("this is not a Tensorlake probe")),
            };
        if name.trim().is_empty() {
            return Err(anyhow::Error::new(ProvisioningRequestError::InvalidProbe(
                "a Tensorlake host needs a name".into(),
            )));
        }
        let entry = NewHostEntry::new(Some(&name))?;
        let destination = crate::tensorlake::ssh::destination(&sandbox);
        if host.is_none() {
            self.store.check_new_entry(&destination, &entry).await?;
        }
        let (mut recipe, state) = self
            .store
            .tensorlake_recipe_row(&recipe_name)
            .await?
            .ok_or_else(|| refused(format!("no Tensorlake recipe is named {recipe_name}")))?;
        // A cached checkpoint can outlive a switch to a Podman-only base.
        // New work must honor that choice, while recovery of a recorded create
        // continues from its saved installation facts after recipe edits.
        if new_install
            && !saved_prepared
            && !recipe
                .base
                .supports(farhelm_proto::ProviderKind::Tensorlake)
        {
            return Err(refused("this recipe's base cannot run on Tensorlake"));
        }
        if let Some(size) = size {
            recipe.size = size;
        }
        tensorlake.ensure_host_key().await.map_err(refused)?;
        let steps = if new_install {
            prepared_host_steps(
                &sandbox,
                &recipe_name,
                &recipe,
                state.snapshot.is_some(),
                host.is_some(),
                !saved_prepared
                    && tensorlake
                        .builds()
                        .fresh_prepared_snapshot(&recipe_name)
                        .await?
                        .is_none(),
            )
        } else {
            steps(
                &sandbox,
                &recipe_name,
                &recipe,
                state.snapshot.is_some(),
                host.is_some(),
                recorded_sandbox,
            )
        };
        let confirmation = format!(
            "{}\n{}\nThis sandbox bills while running. See https://www.tensorlake.ai/pricing. On Usage Credits, Tensorlake suspends it after 24 hours without a connection. Farhelm never resumes it on its own.",
            steps.lead,
            steps
                .steps
                .iter()
                .map(|s| match &s.explanation {
                    Some(explanation) => format!("{}\n  {explanation}", s.name),
                    None => s.name.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n")
        );
        let probe_id = uuid::Uuid::new_v4().to_string();
        let response = ProbeResponse::TensorlakeProvisionable {
            probe_id: probe_id.clone(),
            sandbox: sandbox.clone(),
            destination: destination.clone(),
            recipe: recipe_name.clone(),
            resources: recipe.size,
            confirmation,
            steps: Box::new(steps.clone()),
        };
        let plan = TensorlakeAddPlan {
            host,
            sandbox,
            destination,
            entry,
            recipe_name,
            recipe,
            steps,
            tensorlake,
        };
        let mut memory = self.memory.lock().await;
        while memory.plan_order.len() >= MAX_PENDING_PLANS {
            if let Some(id) = memory.plan_order.pop_front() {
                memory.plans.remove(&id);
                memory.tensorlake_plans.remove(&id);
            }
        }
        memory.plan_order.push_back(probe_id.clone());
        memory.tensorlake_plans.insert(probe_id, plan);
        Ok(response)
    }

    /// Register before any billed work, claim progress, and run on an owned
    /// task under the normal host lock and fleet-wide run limit.
    ///
    /// A setup run again on an existing sandbox (`plan.host`) takes that
    /// host's provisioning lock without waiting, before it claims the host's
    /// progress, and is refused as busy while the lock is taken by another
    /// operation: a Suspend, Resume or removal can hold it for minutes, and
    /// this holds `add_confirmations`, which every other host's confirmation
    /// waits for too, so waiting would stall every Add host's confirmation
    /// behind one host's lifecycle and keep the GUI's channel waiting on this
    /// one (SPEC.md, "Waiting between operations on one host"). The refusal
    /// is plain words with no busy marker: the plan is already spent, so
    /// sending the same confirmation again after the wait could only be
    /// refused as an unknown plan, and the remedy is running setup again.
    /// Taken before the claim, the refusal leaves a failed earlier run's
    /// progress (its Re-run) as it was. A new sandbox's lock is expected to be
    /// free, as its row was registered just above; it is waited for as
    /// before.
    pub(super) async fn start_tensorlake_add(
        self: Arc<Self>,
        plan: TensorlakeAddPlan,
    ) -> anyhow::Result<RunAccepted> {
        let _confirming = Arc::clone(&self.add_confirmations).lock_owned().await;
        let host = match plan.host {
            Some(host) => {
                self.host_row(host).await?;
                host
            }
            None => {
                let new_host = NewTensorlakeHost {
                    destination: plan.destination.clone(),
                    entry: plan.entry.clone(),
                    sandbox: plan.sandbox.clone(),
                    recipe: plan.recipe_name.clone(),
                    resources: plan.recipe.size,
                };
                let host = self.store.register_tensorlake_host(new_host).await?;
                let reconciled = async {
                    // Deletion may have won while confirmation was open.
                    // Check after insertion: from this point the recipe's
                    // transactional host-use check prevents its deletion.
                    if self.store.tensorlake_recipe_row(&plan.recipe_name).await?.is_none() {
                        return Err(refused("the recipe was deleted while confirmation was open; choose a recipe again"));
                    }
                    self.sync_registry().await
                }.await;
                if let Err(error) = reconciled {
                    let proof = crate::environment::Destroyed::Tensorlake(
                        crate::tensorlake::hosts::SandboxDeleted::never_created(host),
                    );
                    self.store.remove_host_row(host, Some(&proof)).await?;
                    self.manager.stop_actor(host).await;
                    self.manager.forget_cache_lock(host);
                    return Err(error);
                }
                host
            }
        };
        // Taken without waiting, ahead of the claim: holding a host's
        // provisioning lock while taking `memory` is the order every run
        // already uses, and nothing waits for this lock while holding
        // `memory`. The holder may be any other operation on the host (a
        // Suspend, Resume or removal, another setup, an update, an edit), so
        // the refusal names none.
        let rerun_write = match plan.host {
            Some(_) => Some(self.manager.try_host_provision_lock(host).ok_or_else(|| {
                refused(
                    "this sandbox is busy with another operation; once it finishes, run setup again if it \
                     is still needed",
                )
            })?),
            None => None,
        };
        let run_id = uuid::Uuid::new_v4().to_string();
        {
            let mut memory = self.memory.lock().await;
            if !memory.busy.insert(host) {
                return Err(anyhow::Error::new(ProvisioningRequestError::Busy(host)));
            }
            memory
                .runs
                .insert(host, progress(host, run_id.clone(), &plan.steps));
        }
        let host_write = match rerun_write {
            Some(host_write) => host_write,
            None => self.manager.host_provision_lock(host).await,
        };
        let valid = self.host_row(host).await.is_ok_and(|row| {
            row.tensorlake
                .as_ref()
                .is_some_and(|binding| binding.sandbox == plan.sandbox)
        });
        if !valid {
            self.withdraw_claim(host).await;
            return Err(refused(
                "this Tensorlake host was removed or changed; probe again",
            ));
        }
        // A sandbox whose Farhelm sealed it is being deleted, and setting it
        // up again (reinstalling Farhelm into a machine about to be
        // destroyed) can only get in the way of that. Checked under the
        // provisioning lock the deletion takes too.
        if self.manager.is_sealed(host) {
            self.withdraw_claim(host).await;
            return Err(refused(
                "this sandbox is being deleted, so it cannot be set up again; use Retry to finish deleting it",
            ));
        }
        // A fleet-wide run slot can stay queued for minutes. Stop dialing
        // now, before that wait, so a copied supervisor cannot be claimed
        // while this add is still waiting to check its state.
        self.manager
            .hold_connection(
                host,
                HostState::Unreachable {
                    cause: crate::manager::UnreachableCause::TransportFailure,
                    last_error: "Farhelm is setting up this Tensorlake sandbox".into(),
                },
            )
            .await;
        self.manager.events().bump();
        let service = Arc::clone(&self);
        let task = tokio::spawn(async move {
            let _slot = Arc::clone(&service.run_slots)
                .acquire_owned()
                .await
                .expect("run slots stay open");
            service.execute_tensorlake_add(host, plan, host_write).await;
        });
        self.memory.lock().await.tasks.insert(host, task);
        Ok(RunAccepted {
            host_id: host,
            run_id,
        })
    }

    /// Withdraw the busy claim and progress [`Self::start_tensorlake_add`] took
    /// for `host` when it refuses after taking them (the host was removed or
    /// changed, or is sealed). A connected GUI may already show that run (any
    /// feed bump since the claim re-read it), so the withdrawal bumps the
    /// feed, as every progress change does.
    async fn withdraw_claim(&self, host: HostId) {
        {
            let mut memory = self.memory.lock().await;
            memory.busy.remove(&host);
            memory.runs.remove(&host);
        }
        self.manager.events().bump();
    }

    /// Set up a Tensorlake host, retaining failed progress for a setup retry.
    ///
    /// `host_write`, the host's provisioning lock, goes to whichever step
    /// publishes the run's end, which gives it up in the same critical
    /// section (see `end_run`). For an add (Add host, or setup run
    /// again) that is the setup's own last step: the prepared clone's
    /// attachment, or the inner install plan's end.
    async fn execute_tensorlake_add(
        &self,
        host: HostId,
        plan: TensorlakeAddPlan,
        host_write: crate::manager::ProvisionLockGuard,
    ) {
        let mut index = 0;
        let mut setup_lock = Some(host_write);
        let result = self
            .tensorlake_steps(host, &plan, &mut index, &mut setup_lock)
            .await;
        if let Err(error) = result {
            // Initial failures keep the admission hold. Missing install
            // coordinates also gate dialing after a manager restart; only
            // the copied-state check may open that durable gate.
            if self
                .host_row(host)
                .await
                .is_ok_and(|row| row.remote_state_dir.is_some() && row.remote_farhelm.is_some())
            {
                self.manager.release_hold(host, true);
            }
            let message = format!("{}; run setup again to continue", error);
            self.set_step(host, index, StepStatus::Failed, Some(message.clone()))
                .await;
            let mut memory = self.memory.lock().await;
            // Let go of the host before the failure is readable, so a removal
            // right after it is not refused as busy (see `end_run`). A step
            // that failed before publishing anything never took the lock.
            drop(setup_lock);
            if let Some(run) = memory.runs.get_mut(&host) {
                run.status = RunStatus::Failed;
                run.message = Some(message);
            }
            memory.busy.remove(&host);
            drop(memory);
            self.manager.events().bump();
        }
    }

    /// Finish under the same resource admission used by setup. Failure may
    /// remove only this launch's fresh environment, never a Replace source.
    /// A lost reply does not cancel the owned task or weaken deletion proof.
    ///
    /// The end is published on the launch's run, after the provisioning lock
    /// is let go: completed once the session is cached under the host, or
    /// failed with the "Not launched." reason. A failure whose cleanup removed
    /// the host publishes on the run's kept copy
    /// ([`super::ProvisioningMemory::ended_launches`]), since the
    /// host's own entry went with it.
    #[allow(
        clippy::too_many_arguments,
        reason = "the owned launch keeps its result, progress position and provisioning hold through confirmed cleanup"
    )]
    async fn finish_session_launch(
        &self,
        host: HostId,
        resource: &str,
        launch: SessionLaunch,
        result: anyhow::Result<()>,
        mut index: usize,
        setup_state: Option<HostState>,
        host_write: crate::manager::ProvisionLockGuard,
    ) {
        let result = match result {
            Ok(()) => {
                index += 1;
                self.set_step(host, index, StepStatus::Running, None).await;
                // Normal attachment already waits; injected setup may finish
                // first, or that connection may drop before create. Admission
                // normally replaces the held placeholder with Connecting;
                // exclude both. Ordinary create owns the coherent state/client
                // check and refusal, including actor loss.
                let _ = self
                    .manager
                    .wait_for_state(host, |state| {
                        Some(state) != setup_state.as_ref()
                            && !matches!(state, HostState::Connecting { .. })
                    })
                    .await;
                crate::sessions::create_in_session_sandbox(
                    &launch.state,
                    host,
                    launch.request,
                    launch.name_kind,
                )
                .await
            }
            Err(error) => Err(error),
        };
        let result = match result {
            Ok(session) => {
                self.set_step(host, index, StepStatus::Completed, None)
                    .await;
                let mut memory = self.memory.lock().await;
                // Let go of the host before the launch's end and its session
                // are readable, so deleting the environment right after is not
                // refused as busy (see `end_run`).
                drop(host_write);
                if let Some(run) = memory.runs.get_mut(&host) {
                    run.status = RunStatus::Completed;
                }
                memory.busy.remove(&host);
                Ok(session)
            }
            Err(error) => {
                // A cleanup that removes the host forgets its run with it, and
                // the run is where this launch's failure is published. Keep a
                // copy first, so the listing goes on showing the run while the
                // cleanup runs and once the host is gone.
                {
                    let mut memory = self.memory.lock().await;
                    if let Some(run) = memory.runs.get(&host).cloned() {
                        memory.keep_ended_launch(run);
                    }
                }
                // This is the fresh environment created for this launch, never
                // a Replace source. A lost create reply may have started an
                // agent; the accepted cleanup rule still discards it.
                let cleanup = crate::hosts::forget_host_locked(
                    &launch.state,
                    host,
                    host_write,
                    RunTask::Detach,
                )
                .await;
                let message = match cleanup {
                    Ok(()) => format!("Not launched. {error:#}"),
                    Err(cleanup) => format!(
                        "Not launched. {error:#}. Environment {} could not be removed: {cleanup:#}. Open Hosts to retry Delete environment or recover its files manually.",
                        resource
                    ),
                };
                let mut memory = self.memory.lock().await;
                // A failed cleanup keeps the host and its own run, which then
                // carries the failure, and the copy goes.
                if let Some(run) = memory.launch_run_to_end(host, &launch.run_id) {
                    if let Some(step) = run.steps.get_mut(index) {
                        step.status = StepStatus::Failed;
                        step.message = Some(message.clone());
                    }
                    run.status = RunStatus::Failed;
                    run.message = Some(message.clone());
                }
                memory.busy.remove(&host);
                Err(refused(message))
            }
        };
        let _ = launch.result.send(result);
        self.manager.events().bump();
        #[cfg(test)]
        self.hold_at_run_end().await;
    }

    /// Select prepared-clone creation or ordinary recovery from durable
    /// ownership and admission facts. The free probe cannot establish which
    /// sandbox an outstanding request will eventually reveal.
    ///
    /// `host_write` is the host's provisioning lock, which
    /// the step that publishes the run's end takes and gives up as it
    /// publishes (see [`Self::execute_tensorlake_add`]). An error returns it
    /// untaken.
    async fn tensorlake_steps(
        &self,
        host: HostId,
        plan: &TensorlakeAddPlan,
        index: &mut usize,
        host_write: &mut Option<crate::manager::ProvisionLockGuard>,
    ) -> anyhow::Result<()> {
        let key = plan.tensorlake.api_key().await.map_err(refused)?;
        // Old reuse recipes now need a build for new hosts, but rerunning setup
        // must continue against the sandbox already created. Check the durable
        // ID or pre-chosen name before consulting recipe snapshot state. A
        // lookup failure is not permission to start billed work.
        let row = self.host_row(host).await?;
        let binding = row
            .tensorlake
            .as_ref()
            .ok_or_else(|| refused("the host is no longer a Tensorlake sandbox"))?;
        let sandbox_exists = match plan
            .tensorlake
            .api()
            .get_sandbox(&key, binding.sandbox_id.as_deref().unwrap_or(&plan.sandbox))
            .await
        {
            Ok(_) => true,
            Err(TensorlakeError::NotFound { .. }) => false,
            Err(error) => return Err(error.into()),
        };
        // A pending create's 404 does not yet prove absence. Keep the same
        // settle window as creation/deletion, before starting any billed build.
        let pending_create = binding.pending_create_at.is_some_and(|sent| {
            crate::tensorlake::recipes::now_secs()
                < sent + crate::tensorlake::build::CREATE_SETTLE_SECS
        });
        let needs_snapshot = !sandbox_exists && binding.sandbox_id.is_none() && !pending_create;
        let saved_facts = self.store.tensorlake_prepared_install(host).await?;
        if (needs_snapshot && row.remote_farhelm.is_none() && row.remote_state_dir.is_none())
            || (!needs_snapshot && saved_facts.is_some())
        {
            return self
                .prepared_tensorlake_steps(host, plan, index, &key, needs_snapshot, host_write)
                .await;
        }
        // The free probe cannot know whether a pending create has appeared.
        // Once ownership establishes an existing ordinary sandbox, preserve
        // its original recovery/install path and its corresponding step list.
        let built = self
            .store
            .tensorlake_recipe_row(&plan.recipe_name)
            .await?
            .is_some_and(|(_, state)| state.snapshot.is_some());
        self.replace_tensorlake_steps(
            host,
            &steps(
                &plan.sandbox,
                &plan.recipe_name,
                &plan.recipe,
                built,
                plan.host.is_some(),
                binding.sandbox_id.is_some(),
            ),
        )
        .await;
        let source = if needs_snapshot {
            self.set_step(host, *index, StepStatus::Running, None).await;
            let builds = plan.tensorlake.builds();
            let ready = self
                .store
                .tensorlake_recipe_row(&plan.recipe_name)
                .await?
                .is_some_and(|(_, state)| state.snapshot.is_some())
                && !builds.is_building(&plan.recipe_name);
            let snapshot = builds
                .ensure_snapshot(
                    &plan.recipe_name,
                    &plan.recipe,
                    Arc::clone(self.manager.events()),
                    |line| self.set_step(host, *index, StepStatus::Running, Some(line)),
                )
                .await?;
            self.set_step(
                host,
                *index,
                if ready {
                    StepStatus::Skipped
                } else {
                    StepStatus::Completed
                },
                ready.then(|| "the snapshot was already built".into()),
            )
            .await;
            *index += 1;
            Some(SandboxSource::Snapshot(snapshot))
        } else {
            self.set_step(
                host,
                *index,
                StepStatus::Skipped,
                Some(
                    if sandbox_exists {
                        "the host's sandbox already exists; no snapshot build is needed"
                    } else if binding.sandbox_id.is_some() {
                        "the host has a recorded sandbox; no snapshot build is needed"
                    } else {
                        "the host's sandbox may still be starting; no snapshot build is needed"
                    }
                    .into(),
                ),
            )
            .await;
            *index += 1;
            None
        };
        self.set_step(host, *index, StepStatus::Running, None).await;
        let created = self
            .create_host_sandbox(host, plan, &key, source, None, *index)
            .await?;
        self.set_step(
            host,
            *index,
            if created {
                StepStatus::Completed
            } else {
                StepStatus::Skipped
            },
            (!created).then(|| "the sandbox was already created".into()),
        )
        .await;
        *index += 1;
        self.install_tensorlake_host(host, plan, index, host_write)
            .await
    }

    /// Change the outer plan when execution learns whether this is a new
    /// clone or a recovery. Completed common steps keep their progress, while
    /// descriptions and explanations always come from the selected path.
    async fn replace_tensorlake_steps(&self, host: HostId, steps: &PlanSteps) {
        let mut memory = self.memory.lock().await;
        if let Some(run) = memory.runs.get_mut(&host) {
            let previous = std::mem::take(&mut run.steps);
            run.steps = steps
                .steps
                .iter()
                .map(|step| {
                    let old = previous.iter().find(|old| old.step == step.id);
                    StepView {
                        step: step.id.clone(),
                        name: step.name.clone(),
                        paths: step.paths.clone(),
                        explanation: step.explanation.clone(),
                        status: old.map_or(StepStatus::Pending, |old| old.status.clone()),
                        message: old.and_then(|old| old.message.clone()),
                    }
                })
                .collect();
        }
        drop(memory);
        self.manager.events().bump();
    }

    /// Create a state-free cold clone and admit its own booted supervisor.
    /// The installer never runs on this path. A retained create uses its own
    /// saved facts; the recipe's newer snapshot cannot describe that sandbox.
    ///
    /// `host_write` as for [`Self::tensorlake_steps`]: a plain add's
    /// attachment step ends the run and gives the lock up as it publishes.
    async fn prepared_tensorlake_steps(
        &self,
        host: HostId,
        plan: &TensorlakeAddPlan,
        index: &mut usize,
        key: &crate::tensorlake::api::ApiKey,
        needs_snapshot: bool,
        host_write: &mut Option<crate::manager::ProvisionLockGuard>,
    ) -> anyhow::Result<()> {
        let builds = plan.tensorlake.builds();
        let built = self
            .store
            .tensorlake_recipe_row(&plan.recipe_name)
            .await?
            .is_some_and(|(_, state)| state.snapshot.is_some());
        let built_ready = built && !builds.is_building(&plan.recipe_name);
        let mut prepare_visible = needs_snapshot
            && builds
                .fresh_prepared_snapshot(&plan.recipe_name)
                .await?
                .is_none();
        self.replace_tensorlake_steps(
            host,
            &prepared_host_steps(
                &plan.sandbox,
                &plan.recipe_name,
                &plan.recipe,
                built,
                plan.host.is_some(),
                prepare_visible,
            ),
        )
        .await;
        *index = 0;
        if needs_snapshot {
            self.set_step(host, *index, StepStatus::Running, None).await;
            builds
                .ensure_snapshot(
                    &plan.recipe_name,
                    &plan.recipe,
                    Arc::clone(self.manager.events()),
                    |line| self.set_step(host, 0, StepStatus::Running, Some(line)),
                )
                .await?;
            self.set_step(
                host,
                0,
                if built_ready {
                    StepStatus::Skipped
                } else {
                    StepStatus::Completed
                },
                built_ready.then(|| "the snapshot was already built".into()),
            )
            .await;
        } else {
            self.set_step(
                host,
                0,
                StepStatus::Skipped,
                Some("recover the host's recorded prepared-snapshot create".into()),
            )
            .await;
        }

        let mut source_guard = None;
        let mut selected_facts = None;
        let source = if needs_snapshot {
            loop {
                // Ensuring a snapshot can wait for publication. The guard is
                // acquired only afterwards, then freshness is read again so
                // a renewal cannot retire the chosen source before create.
                if builds
                    .fresh_prepared_snapshot(&plan.recipe_name)
                    .await?
                    .is_none()
                {
                    if !prepare_visible {
                        prepare_visible = true;
                        self.replace_tensorlake_steps(
                            host,
                            &prepared_host_steps(
                                &plan.sandbox,
                                &plan.recipe_name,
                                &plan.recipe,
                                true,
                                plan.host.is_some(),
                                true,
                            ),
                        )
                        .await;
                    }
                    *index = 1;
                    self.set_step(host, *index, StepStatus::Running, None).await;
                    builds
                        .ensure_prepared_snapshot(
                            &plan.recipe_name,
                            &plan.recipe,
                            Arc::clone(self.manager.events()),
                            |line| self.set_step(host, 1, StepStatus::Running, Some(line)),
                        )
                        .await?;
                }
                let guard = builds.lock_snapshot_use(&plan.recipe_name).await?;
                if let Some(prepared) = builds.fresh_prepared_snapshot(&plan.recipe_name).await? {
                    anyhow::ensure!(
                        plan.recipe.size.disk_gb >= prepared.disk_gb,
                        "the prepared snapshot needs at least {} GB disk; choose a larger host",
                        prepared.disk_gb
                    );
                    selected_facts = Some(prepared.facts);
                    source_guard = Some(guard);
                    break Some(SandboxSource::Snapshot(prepared.id));
                }
                drop(guard);
            }
        } else {
            None
        };
        if prepare_visible {
            self.set_step(host, 1, StepStatus::Completed, None).await;
        }
        *index = if prepare_visible { 2 } else { 1 };
        self.set_step(host, *index, StepStatus::Running, None).await;
        // Start after snapshot preparation so these debug-only measurements
        // distinguish clone startup from the first host's cache construction.
        // One monotonic clock covers every stage; no endpoint, token or remote
        // path is logged. The real-clone acceptance check reads these stages.
        let spawn_started = tokio::time::Instant::now();
        let created = self
            .create_host_sandbox(host, plan, key, source, selected_facts.as_ref(), *index)
            .await?;
        let running_after = spawn_started.elapsed();
        // Running is the live-process exclusion boundary. An interrupted or
        // ambiguous create retains ordinary vendor failure/retry semantics;
        // the in-memory guard is not a crash-safe source dependency record.
        drop(source_guard);
        let target =
            ProvisioningTarget::Remote(crate::tensorlake::ssh::route(plan.destination.clone()));
        // The provider already waited for its gateway after Running. A cold
        // clone must boot on its own, without ordinary machine preparation.
        self.set_step(
            host,
            *index,
            if created {
                StepStatus::Completed
            } else {
                StepStatus::Skipped
            },
            (!created).then(|| "the sandbox was already created".into()),
        )
        .await;
        *index += 1;

        let Some(facts) = self.store.tensorlake_prepared_install(host).await? else {
            // A formerly pending ordinary create can appear between the two
            // lookups. Its durable ownership permits recovery, but only its
            // own saved facts permit skipping ordinary installation.
            self.replace_tensorlake_steps(
                host,
                &steps(
                    &plan.sandbox,
                    &plan.recipe_name,
                    &plan.recipe,
                    true,
                    true,
                    true,
                ),
            )
            .await;
            *index = 2;
            return self
                .install_tensorlake_host(host, plan, index, host_write)
                .await;
        };
        anyhow::ensure!(
            facts.home.is_absolute()
                && facts.farhelm_path.is_absolute()
                && facts.state_dir.is_absolute(),
            "the prepared installation has invalid paths"
        );
        anyhow::ensure!(
            self.store
                .admit_tensorlake_install(
                    host,
                    &path_text(&facts.farhelm_path)?,
                    &path_text(&facts.state_dir)?,
                )
                .await?,
            "the Tensorlake host disappeared before installation admission"
        );
        self.sync_registry().await?;
        self.set_step(host, *index, StepStatus::Running, None).await;
        let admitted_after = spawn_started.elapsed();
        let row = self.host_row(host).await?;
        let home_notice = self
            .delivery
            .remember_home(&row, facts.home.clone())
            .await
            .err();
        let delivery_notice = self
            .delivery
            .setup(&row, &plan.recipe, &facts.home, self.manager.events())
            .await
            .err();
        self.set_step(
            host,
            *index,
            StepStatus::Completed,
            delivery_notice.or(home_notice),
        )
        .await;
        let credentials_after = spawn_started.elapsed();
        *index += 1;
        self.set_step(host, *index, StepStatus::Running, None).await;
        self.manager.release_hold(host, true);
        let (status, message) = match self.attach_supervisor(host, &target).await? {
            ActionOutcome::Completed => (StepStatus::Completed, None),
            ActionOutcome::Skipped(message) => (StepStatus::Skipped, Some(message)),
            ActionOutcome::Degraded(message) => (StepStatus::Degraded, Some(message)),
        };
        let connected_after = spawn_started.elapsed();
        tracing::debug!(
            target: "farhelm_helm::tensorlake_spawn_timing",
            host,
            create_to_running_ms = running_after.as_millis() as u64,
            admission_ms = (admitted_after - running_after).as_millis() as u64,
            credentials_ms = (credentials_after - admitted_after).as_millis() as u64,
            attach_ms = (connected_after - credentials_after).as_millis() as u64,
            total_ms = connected_after.as_millis() as u64,
            "prepared sandbox connected"
        );
        self.set_step(host, *index, status, message).await;
        // A plain add ends here: publish that and let go of the host in
        // one step, so a removal the moment the host reads as set up is
        // not refused as busy.
        self.end_run(host, host_write.take(), |memory| {
            if let Some(run) = memory.runs.get_mut(&host) {
                run.status = RunStatus::Completed;
            }
        })
        .await;
        Ok(())
    }

    /// Recovery of an ordinary sandbox keeps the inspection and copied-state
    /// admission rules, even when the same run first considered preparation.
    ///
    /// `host_write` as for [`Self::tensorlake_steps`]: the inner install
    /// plan's end is a plain add's, and gives the lock up as it publishes.
    async fn install_tensorlake_host(
        &self,
        host: HostId,
        plan: &TensorlakeAddPlan,
        index: &mut usize,
        host_write: &mut Option<crate::manager::ProvisionLockGuard>,
    ) -> anyhow::Result<()> {
        let target =
            ProvisioningTarget::Remote(crate::tensorlake::ssh::route(plan.destination.clone()));
        self.set_step(host, *index, StepStatus::Running, None).await;
        let home = self.backend.prepare_tensorlake_sandbox(&target).await?;
        let home_notice = self
            .delivery
            .remember_home(&self.host_row(host).await?, home.clone())
            .await
            .err();
        self.set_step(host, *index, StepStatus::Completed, home_notice)
            .await;
        *index += 1;
        let configure_index = *index;
        *index += 1;
        self.set_step(host, *index, StepStatus::Running, None).await;
        let probe = ProbeTarget {
            transport: target.clone(),
            probe_farhelm: "farhelm".into(),
            probe_state_dir: None,
        };
        let reach = match self.backend.inspect(&probe).await? {
            ReachOutcome::Supported(reach) => reach,
            ReachOutcome::Manual(reason) => return Err(refused(reason)),
        };
        let nonce = uuid::Uuid::new_v4().to_string();
        let inner = self
            .layout
            .plan(ProvisioningOperation::Add, target, &reach, &nonce)?;
        let row = self.host_row(host).await?;
        if row.remote_farhelm.is_none() || row.remote_state_dir.is_none() {
            self.backend
                .refuse_copied_supervisor(&inner.target, &inner.state_dir)
                .await?;
        }
        anyhow::ensure!(
            self.store
                .admit_tensorlake_install(
                    host,
                    path_text(&inner.farhelm_path)?.as_str(),
                    path_text(&inner.state_dir)?.as_str(),
                )
                .await?,
            "the Tensorlake host disappeared before installation admission"
        );
        self.sync_registry().await?;
        // Reconcile only after the copied-state check admits this installation.
        // Keep the actor held until delivery finishes; a warning must still let
        // the supervisor installation continue to make this host usable.
        self.set_step(host, configure_index, StepStatus::Running, None)
            .await;
        let delivery_notice = self
            .delivery
            .setup(
                &self.host_row(host).await?,
                &plan.recipe,
                &home,
                self.manager.events(),
            )
            .await
            .err();
        self.set_step(
            host,
            configure_index,
            StepStatus::Completed,
            delivery_notice,
        )
        .await;
        let described = ProvisioningView::for_plan(host, nonce, &inner);
        if let Some(run) = self.memory.lock().await.runs.get_mut(&host) {
            run.steps.splice(*index.., described.steps);
        }
        self.manager.events().bump();
        self.manager.release_hold(host, true);
        // The plan's end (completed, or failed at one of its steps) is a plain
        // add's end, so the lock goes with it (see `end_run`).
        self.execute_actions(host, inner, *index, host_write.take())
            .await;
        Ok(())
    }

    /// Recover by the pre-chosen name only while this row records an
    /// outstanding create. A name collision alone grants no authority to
    /// configure the sandbox or to remember its ID as this host's own.
    /// `source` is absent only when setup already observed the sandbox and
    /// skipped its build; if that sandbox disappears before this second lookup,
    /// refuse rather than create a replacement from an unbuilt base.
    async fn create_host_sandbox(
        &self,
        host: HostId,
        plan: &TensorlakeAddPlan,
        _key: &crate::tensorlake::api::ApiKey,
        source: Option<SandboxSource>,
        prepared: Option<&PreparedInstallFacts>,
        step_index: usize,
    ) -> anyhow::Result<bool> {
        let row = self.host_row(host).await?;
        let binding = row
            .environment_binding()
            .ok_or_else(|| refused("the host is no longer an environment"))?;
        plan.tensorlake
            .create_recorded(
                &binding,
                source,
                prepared,
                None,
                &CreationProgress {
                    service: self,
                    host,
                    step_index,
                },
                Some(Arc::clone(&self.backend)),
            )
            .await
    }
}
