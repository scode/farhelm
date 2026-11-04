//! Tensorlake's outer add plan: authorize creation once, record ownership
//! before billing, then discover and splice the ordinary Farhelm setup plan.
//! The recipe copy carries secrets and is never formatted or serialized.

use super::super::{
    agent_endpoints,
    http::StepView,
    plan::{DirectorySpec, PlanStep, PlanSteps},
};
use super::*;
use crate::ssh::SshAccessKind;
use crate::store::{NewHostEntry, NewTensorlakeHost};
use crate::tensorlake::{
    Tensorlake,
    api::{CreateSandbox, Resources, SandboxSource, SandboxStatus, TensorlakeError},
    config::helm_tag,
    recipes::StoredRecipe,
};
use farhelm_proto::tensorlake::recipes::{RecipeSize, RecipeSource};

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

#[cfg(test)]
#[path = "tensorlake_add_tests.rs"]
mod tests;

/// Name the sandbox with a per-helm ownership tag, a readable slug, and a
/// random suffix. Always valid Tensorlake grammar, at most 63 ASCII bytes.
fn sandbox_name(tag: &str, display: &str) -> String {
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
        "farhelm-host-{tag}-{slug}-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..4]
    )
}

/// Reuse the same description in confirmation and progress. A managed
/// recipe always has a snapshot step: a ready snapshot reports a useful skip.
fn steps(sandbox: &str, name: &str, recipe: &StoredRecipe, ready: bool) -> PlanSteps {
    let mut descriptions = vec![];
    if matches!(recipe.source, RecipeSource::Packages { .. }) {
        descriptions.push(("build-snapshot", if ready { format!("Use the snapshot for recipe {name}") } else { format!("Build the snapshot for recipe {name} (packages, Claude Code and Codex; takes a few minutes)") }));
    }
    descriptions.push((
        "create-sandbox",
        format!(
            "Create sandbox {sandbox} with {} CPUs, {} GB memory and {} GB disk",
            recipe.size.cpus, recipe.size.memory_gb, recipe.size.disk_gb
        ),
    ));
    descriptions.push((
        "prepare-sandbox",
        "Start the sandbox's login manager and keep the user manager running".into(),
    ));
    if recipe.claude.is_some() || recipe.codex.is_some() {
        descriptions.push((
            "configure-agents",
            "Configure the agents' API endpoints, preserving other settings".into(),
        ));
    }
    descriptions.push((
        "set-up-farhelm",
        "Install Farhelm's supervisor, as on any host".into(),
    ));
    PlanSteps {
        lead: "Create a Tensorlake sandbox and set it up as a host".into(),
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
    }
}

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
        let (host, name, recipe_name, sandbox, size) = match request.target {
            ProbeDestination::Tensorlake { name, recipe } => {
                let tag = helm_tag(&self.store.registry_namespace().await?);
                (None, name.clone(), recipe, sandbox_name(&tag, &name), None)
            }
            ProbeDestination::TensorlakeHost { host } => {
                if self.memory.lock().await.busy.contains(&host) {
                    return Err(anyhow::Error::new(ProvisioningRequestError::Busy(host)));
                }
                let row = self.host_row(host).await?;
                let binding = row
                    .tensorlake
                    .ok_or_else(|| refused("this host is not a Tensorlake sandbox"))?;
                (
                    Some(host),
                    row.alias.unwrap_or_else(|| binding.sandbox.clone()),
                    binding.recipe,
                    binding.sandbox,
                    binding.resources,
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
        if let Some(size) = size {
            recipe.size = size;
        }
        tensorlake.ensure_host_key().await.map_err(refused)?;
        let steps = steps(&sandbox, &recipe_name, &recipe, state.snapshot.is_some());
        let confirmation = format!(
            "{}\n{}\nThis sandbox bills while running. See https://www.tensorlake.ai/pricing. On Usage Credits, Tensorlake suspends it after 24 hours without a connection. Farhelm never resumes it on its own.",
            steps.lead,
            steps
                .steps
                .iter()
                .map(|s| s.name.as_str())
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
                let host = self
                    .store
                    .register_tensorlake_host(NewTensorlakeHost {
                        destination: plan.destination.clone(),
                        entry: plan.entry.clone(),
                        sandbox: plan.sandbox.clone(),
                        recipe: plan.recipe_name.clone(),
                        resources: plan.recipe.size,
                    })
                    .await?;
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
                    let proof = crate::tensorlake::hosts::SandboxDeleted::never_created(host);
                    self.store.remove_host_row(host, Some(&proof)).await?;
                    self.manager.stop_actor(host).await;
                    self.manager.forget_cache_lock(host);
                    return Err(error);
                }
                host
            }
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
        let host_write = self.manager.host_provision_lock(host).await;
        let valid = self.host_row(host).await.is_ok_and(|row| {
            row.tensorlake
                .as_ref()
                .is_some_and(|binding| binding.sandbox == plan.sandbox)
        });
        if !valid {
            let mut memory = self.memory.lock().await;
            memory.busy.remove(&host);
            memory.runs.remove(&host);
            return Err(refused(
                "this Tensorlake host was removed or changed; probe again",
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
            let _write = host_write;
            let _slot = Arc::clone(&service.run_slots)
                .acquire_owned()
                .await
                .expect("run slots stay open");
            service.execute_tensorlake_add(host, plan).await;
        });
        self.memory.lock().await.tasks.insert(host, task);
        Ok(RunAccepted {
            host_id: host,
            run_id,
        })
    }

    /// Hold dialing until installation coordinates are durable. On failure
    /// keep the row and all completed work; rerun adopts by the same name.
    async fn execute_tensorlake_add(&self, host: HostId, plan: TensorlakeAddPlan) {
        let mut index = 0;
        let result = self.tensorlake_steps(host, &plan, &mut index).await;
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
            if let Some(run) = memory.runs.get_mut(&host) {
                run.status = RunStatus::Failed;
                run.message = Some(message);
            }
            memory.busy.remove(&host);
            drop(memory);
            self.manager.events().bump();
        }
    }

    /// Walk the outer steps, replacing the final placeholder with the
    /// inspected host's concrete setup actions only when it exists.
    async fn tensorlake_steps(
        &self,
        host: HostId,
        plan: &TensorlakeAddPlan,
        index: &mut usize,
    ) -> anyhow::Result<()> {
        let key = plan.tensorlake.api_key().await.map_err(refused)?;
        let source = match &plan.recipe.source {
            RecipeSource::Packages { .. } => {
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
                SandboxSource::Snapshot(snapshot)
            }
            RecipeSource::Image { image } => SandboxSource::Image(image.clone()),
            RecipeSource::Snapshot { snapshot_id } => SandboxSource::Snapshot(snapshot_id.clone()),
        };
        self.set_step(host, *index, StepStatus::Running, None).await;
        let created = self
            .create_host_sandbox(host, plan, &key, source, *index)
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
        let target = ProvisioningTarget::Ssh {
            destination: plan.destination.clone(),
            access: SshAccessKind::Tensorlake,
        };
        self.set_step(host, *index, StepStatus::Running, None).await;
        let home = self.backend.prepare_tensorlake_sandbox(&target).await?;
        self.set_step(host, *index, StepStatus::Completed, None)
            .await;
        *index += 1;
        if plan.recipe.claude.is_some() || plan.recipe.codex.is_some() {
            self.set_step(host, *index, StepStatus::Running, None).await;
            self.configure_agents(&target, &home, &plan.recipe).await?;
            self.set_step(host, *index, StepStatus::Completed, None)
                .await;
            *index += 1;
        }
        self.set_step(host, *index, StepStatus::Running, None).await;
        let probe = ProbeTarget {
            transport: target.clone(),
            probe_farhelm: "farhelm".into(),
            probe_state_dir: None,
        };
        let reach = match self.backend.inspect(&probe).await? {
            ReachOutcome::Supported(reach) => reach,
            ReachOutcome::Manual(reason) => return Err(refused(reason)),
            ReachOutcome::SetupManaged => {
                return Err(refused(
                    "this sandbox has an installation managed by farhelm helm setup; use an image without a Farhelm installation",
                ));
            }
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
        let described = ProvisioningView::for_plan(host, nonce, &inner);
        if let Some(run) = self.memory.lock().await.runs.get_mut(&host) {
            run.steps.splice(*index.., described.steps);
        }
        self.manager.events().bump();
        self.manager.release_hold(host, true);
        self.execute_actions(host, inner, *index).await;
        Ok(())
    }

    /// Look up the pre-chosen name before create and adopt after an ambiguous
    /// answer. The row remains the ownership record even if no reply arrives.
    async fn create_host_sandbox(
        &self,
        host: HostId,
        plan: &TensorlakeAddPlan,
        key: &crate::tensorlake::api::ApiKey,
        source: SandboxSource,
        step_index: usize,
    ) -> anyhow::Result<bool> {
        let api = plan.tensorlake.api();
        let row = self.host_row(host).await?;
        let binding = row
            .tensorlake
            .as_ref()
            .ok_or_else(|| refused("the host is no longer a Tensorlake sandbox"))?;
        let existing = api
            .get_sandbox(key, binding.sandbox_id.as_deref().unwrap_or(&plan.sandbox))
            .await;
        let (id, created) = match existing {
            Ok(info) => (info.id, false),
            Err(TensorlakeError::NotFound { .. }) if binding.sandbox_id.is_none() => {
                use crate::tensorlake::{
                    build::{CREATE_SETTLE_SECS, create_was_refused},
                    recipes::now_secs,
                };
                anyhow::ensure!(
                    binding
                        .pending_create_at
                        .is_none_or(|sent| now_secs() >= sent + CREATE_SETTLE_SECS),
                    "a sandbox Farhelm asked Tensorlake for may still be starting; try setup or Delete sandbox again in a few minutes"
                );
                let RecipeSize {
                    cpus,
                    memory_gb,
                    disk_gb,
                } = plan.recipe.size;
                let request = CreateSandbox {
                    name: plan.sandbox.clone(),
                    source,
                    resources: Resources {
                        cpus: cpus.into(),
                        memory_mb: u64::from(memory_gb) * 1024,
                        disk_mb: u64::from(disk_gb) * 1024,
                    },
                    timeout_secs: 0,
                    auto_resume: false,
                    max_pending_secs: 900,
                };
                // Persist before the request: a crash or lost response must
                // not let Delete mistake a transient 404 for never created.
                anyhow::ensure!(
                    self.store
                        .set_tensorlake_pending_create(host, Some(now_secs()))
                        .await?,
                    "the host disappeared before create"
                );
                match api.create_sandbox(key, &request).await {
                    Ok(created) => (created.sandbox_id, true),
                    Err(error) if create_was_refused(&error) => {
                        self.store.set_tensorlake_pending_create(host, None).await?;
                        return Err(error.into());
                    }
                    Err(error) => match api.get_sandbox(key, &plan.sandbox).await {
                        Ok(info) => (info.id, false),
                        _ => return Err(error.into()),
                    },
                }
            }
            Err(error) => return Err(error.into()),
        };
        anyhow::ensure!(
            self.store.set_tensorlake_sandbox_id(host, &id).await?,
            "the host disappeared before its sandbox ID was recorded"
        );
        tokio::time::timeout(Duration::from_secs(900), async {
            loop {
                let info = api.get_sandbox(key, &id).await?;
                match info.status {
                    SandboxStatus::Running => return Ok(created),
                    SandboxStatus::Terminated => anyhow::bail!("Tensorlake terminated this sandbox: {}. Delete this host and add a new one", info.termination_reason.or(info.error_details).unwrap_or_else(|| "no reason given".into())),
                    SandboxStatus::Suspended | SandboxStatus::Suspending | SandboxStatus::ResumeFailed => anyhow::bail!("the sandbox is suspended; resume it and run setup again"),
                    _ => {}
                }
                if let Some(reason) = info.pending_reason { self.set_step(host, step_index, StepStatus::Running, Some(peer_text(&reason))).await; }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }).await.map_err(|_| refused("Tensorlake did not start the sandbox within 15 minutes; run setup again"))?
    }

    /// Merge files locally, then stream private writes through the normal
    /// backend. Unset endpoints never read or touch their agent's file.
    async fn configure_agents(
        &self,
        target: &ProvisioningTarget,
        home: &Path,
        recipe: &StoredRecipe,
    ) -> anyhow::Result<()> {
        type Merge = fn(
            Option<&[u8]>,
            &crate::tensorlake::recipes::StoredEndpoint,
        ) -> Result<Vec<u8>, String>;
        for (folder, file, endpoint, merge) in [
            (
                ".claude",
                "settings.json",
                recipe.claude.as_ref(),
                agent_endpoints::claude as Merge,
            ),
            (
                ".codex",
                "config.toml",
                recipe.codex.as_ref(),
                agent_endpoints::codex as Merge,
            ),
        ] {
            let Some(endpoint) = endpoint else {
                continue;
            };
            let dir = home.join(folder);
            let path = dir.join(file);
            let previous = self.backend.read_small_file(target, &path).await?;
            let bytes = merge(previous.as_deref(), endpoint).map_err(refused)?;
            self.backend
                .ensure_directories(
                    target,
                    &[DirectorySpec {
                        path: dir,
                        mode: 0o700,
                        shared: true,
                    }],
                )
                .await?;
            let temporary =
                path.with_file_name(format!(".{file}.farhelm-tmp-{}", uuid::Uuid::new_v4()));
            self.backend
                .install_bytes(target, &bytes, &path, &temporary, 0o600)
                .await?;
        }
        Ok(())
    }
}
