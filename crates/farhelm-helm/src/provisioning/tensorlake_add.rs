//! Tensorlake's outer add plan: authorize creation once, record ownership
//! before billing, then boot a prepared clone or recover an ordinary install.
//! The recipe copy freezes choices; named credential values resolve at delivery.

use super::super::{
    http::StepView,
    plan::{PlanStep, PlanSteps},
};
use super::*;
use crate::ssh::SshAccessKind;
use crate::store::{NewHostEntry, NewTensorlakeHost};
use crate::tensorlake::{
    Tensorlake,
    api::{CreateSandbox, Resources, SandboxSource, SandboxStatus, TensorlakeError},
    config::helm_tag,
    prepared::PreparedInstallFacts,
    recipes::StoredRecipe,
};
use farhelm_proto::tensorlake::recipes::RecipeSize;

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
        let (host, name, recipe_name, sandbox, size, recorded_sandbox, new_install, saved_prepared) =
            match request.target {
                ProbeDestination::Tensorlake { name, recipe } => {
                    let tag = helm_tag(&self.store.registry_namespace().await?);
                    (
                        None,
                        name.clone(),
                        recipe,
                        sandbox_name(&tag, &name),
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

    /// Select prepared-clone creation or ordinary recovery from durable
    /// ownership and admission facts. The free probe cannot establish which
    /// sandbox an outstanding request will eventually reveal.
    async fn tensorlake_steps(
        &self,
        host: HostId,
        plan: &TensorlakeAddPlan,
        index: &mut usize,
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
                .prepared_tensorlake_steps(host, plan, index, &key, needs_snapshot)
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
        self.install_tensorlake_host(host, plan, index).await
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
    async fn prepared_tensorlake_steps(
        &self,
        host: HostId,
        plan: &TensorlakeAddPlan,
        index: &mut usize,
        key: &crate::tensorlake::api::ApiKey,
        needs_snapshot: bool,
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
        let target = ProvisioningTarget::Ssh {
            destination: plan.destination.clone(),
            access: SshAccessKind::Tensorlake,
        };
        // The API's Running state can precede the gateway. Wait without running
        // the ordinary preparation script: this clone must boot on its own.
        self.backend.wait_tensorlake_ssh(&target).await?;
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
            return self.install_tensorlake_host(host, plan, index).await;
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
        let mut memory = self.memory.lock().await;
        if let Some(run) = memory.runs.get_mut(&host) {
            run.status = RunStatus::Completed;
        }
        memory.busy.remove(&host);
        drop(memory);
        self.manager.events().bump();
        Ok(())
    }

    /// Recovery of an ordinary sandbox keeps the inspection and copied-state
    /// admission rules, even when the same run first considered preparation.
    async fn install_tensorlake_host(
        &self,
        host: HostId,
        plan: &TensorlakeAddPlan,
        index: &mut usize,
    ) -> anyhow::Result<()> {
        let target = ProvisioningTarget::Ssh {
            destination: plan.destination.clone(),
            access: SshAccessKind::Tensorlake,
        };
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
        self.execute_actions(host, inner, *index).await;
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
        key: &crate::tensorlake::api::ApiKey,
        source: Option<SandboxSource>,
        prepared: Option<&PreparedInstallFacts>,
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
            Ok(info) => {
                anyhow::ensure!(
                    binding.sandbox_id.is_some() || binding.pending_create_at.is_some(),
                    "a sandbox named {} already exists, but this host has no record of asking for it. Delete this host entry and add a new host; deleting this entry leaves the existing sandbox untouched",
                    plan.sandbox
                );
                (info.id, false)
            }
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
                    source: source.ok_or_else(|| {
                        refused("the sandbox disappeared before setup; run setup again")
                    })?,
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
                let recorded = match prepared {
                    Some(facts) => {
                        self.store
                            .set_tensorlake_prepared_create(host, now_secs(), facts)
                            .await?
                    }
                    None => {
                        self.store
                            .set_tensorlake_pending_create(host, Some(now_secs()))
                            .await?
                    }
                };
                anyhow::ensure!(recorded, "the host disappeared before create");
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
}
