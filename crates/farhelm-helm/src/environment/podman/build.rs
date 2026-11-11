//! Two owned helpers publish software and installation checkpoints.
//!
//! The existing registry owns admission and source exclusion. Durable names
//! precede run, replacements precede retirement, and no user base is removed.

use super::*;
use crate::environment::recipes::PodmanRecipeState;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

/// Bootstrap preserves Ubuntu's existing UID1000 identity. An upstream
/// account-layout change fails before granting sudo or boot linger.
const BOOTSTRAP: &str = r#"
log=/var/tmp/farhelm-base.log
: >"$log" || exit 1
fail() { tail -c 8192 "$log" >&2; exit 1; }
env DEBIAN_FRONTEND=noninteractive apt-get -q -y update >>"$log" 2>&1 || fail
env DEBIAN_FRONTEND=noninteractive apt-get -q -y install systemd systemd-sysv sudo curl git bash ca-certificates >>"$log" 2>&1 || fail
if getent passwd ubuntu >/dev/null; then
    [ "$(id -u ubuntu)" = 1000 ] || fail
    home=$(getent passwd ubuntu | cut -d: -f6) || fail
    [ "$home" = /home/ubuntu ] || fail
else
    if getent passwd 1000 >/dev/null; then echo 'Ubuntu UID1000 belongs to an unexpected account' >&2; exit 1; fi
    useradd --create-home --uid 1000 --shell /bin/bash ubuntu >>"$log" 2>&1 || fail
fi
mkdir -p /etc/sudoers.d /var/lib/systemd/linger /home/ubuntu || fail
printf '%s\n' 'ubuntu ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/farhelm-ubuntu || fail
chmod 0440 /etc/sudoers.d/farhelm-ubuntu || fail
touch /var/lib/systemd/linger/ubuntu || fail
chown ubuntu:ubuntu /home/ubuntu || fail
truncate -s 0 /etc/machine-id || fail
rm -f "$log" || fail
"#;

impl Podman {
    /// The registry base is user-owned; the bootstrap's committed layer is
    /// helm-owned and recorded for retirement before another helper uses it.
    async fn managed_base(&self, name: &str, expected: &HostRow) -> anyhow::Result<String> {
        let place = expected.id;
        self.pull_base(place, UBUNTU).await?;
        let id = self.helper(name, expected, "build", UBUNTU, true).await?;
        self.script(place, &id, root_user(), BOOTSTRAP, IMAGE_WORK)
            .await?;
        let tag = format!(
            "localhost/farhelm-{}-base:{}",
            self.tag().await?,
            uuid::Uuid::new_v4().simple()
        );
        let image = self.commit(name, place, &id, &tag, "managed-base").await?;
        self.clear_helper(name, place, "build", &id).await?;
        Ok(image)
    }

    /// Resolve the literal recipe account through the image's passwd data.
    /// Exec needs its actual name, home and UID rather than a guessed default.
    async fn login(
        &self,
        place: HostId,
        id: &str,
        requested: &str,
    ) -> anyhow::Result<ContainerUser> {
        let text = self
            .script(
                place,
                id,
                root_user(),
                &format!("getent passwd -- {}", crate::ssh::shell_quote(requested)),
                COMMAND,
            )
            .await?;
        let lines: Vec<_> = text.lines().collect();
        anyhow::ensure!(
            lines.len() == 1,
            "the image did not resolve exactly one login account"
        );
        let fields: Vec<_> = lines[0].split(':').collect();
        anyhow::ensure!(
            fields.len() == 7 && !fields[0].is_empty() && fields[5].starts_with('/'),
            "the image returned invalid login coordinates"
        );
        Ok(ContainerUser {
            name: fields[0].into(),
            home: fields[5].into(),
            uid: fields[2]
                .parse()
                .context("the image returned an invalid login UID")?,
        })
    }

    /// The common backend establishes logind/linger through the same route;
    /// Podman does not carry another installer or user-manager setup script.
    async fn prepare_user_manager(
        &self,
        place: HostId,
        id: &str,
        user: &ContainerUser,
    ) -> anyhow::Result<()> {
        let backend = self
            .0
            .delivery
            .backend()
            .context("this helm has no provisioning transport")?;
        let route = self.route(place, id, user.clone()).await?;
        let home = backend
            .prepare_tensorlake_sandbox(&ProvisioningTarget::Remote(route))
            .await?;
        anyhow::ensure!(
            home == std::path::Path::new(&user.home),
            "the image's login home changed during preparation"
        );
        Ok(())
    }

    /// Every base gets its own committed software layer. Catalog edits take
    /// effect on explicit Rebuild; ordinary reuse retains the recorded layer.
    async fn software(
        &self,
        name: &str,
        recipe: &Recipe,
        place: HostId,
        progress: &tokio::sync::watch::Sender<PodmanProgress>,
        events: &FleetEvents,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> anyhow::Result<BuiltImage> {
        let state = self.state(name, place).await?;
        if let Some(image) = state.image
            && self.exists(place, "image", &image.id).await?
        {
            return Ok(image);
        }
        let (expected, _, ready) = self.place(place).await?;
        check(cancel)?;
        enter(progress, events, "Preparing the recipe's base image");
        let (base, requested) = match &recipe.base {
            RecipeBase::ManagedUbuntu => (
                self.managed_base(name, &expected).await?,
                "ubuntu".to_owned(),
            ),
            RecipeBase::ContainerImage { reference, user } => {
                self.pull_base(place, reference).await?;
                (reference.clone(), user.clone())
            }
            RecipeBase::TensorlakeImage { .. } | RecipeBase::TensorlakeSnapshot { .. } => {
                anyhow::bail!("this recipe's base cannot run in a container")
            }
        };
        let id = self.helper(name, &expected, "build", &base, false).await?;
        let user = self.login(place, &id, &requested).await?;
        self.script(
            place,
            &id,
            user.clone(),
            &crate::tensorlake::build_script::ready(),
            Duration::from_secs(240),
        )
        .await?;
        self.prepare_user_manager(place, &id, &user).await?;
        self.proxies(place, &id, &user, &ready.proxy).await?;
        check(cancel)?;
        enter(progress, events, "Installing recipe packages");
        self.script(
            place,
            &id,
            user.clone(),
            &crate::tensorlake::build_script::packages(&recipe.packages),
            IMAGE_WORK,
        )
        .await?;
        check(cancel)?;
        enter(progress, events, "Installing Claude Code");
        let claude = self
            .script(
                place,
                &id,
                user.clone(),
                &crate::tensorlake::build_script::claude_code(),
                IMAGE_WORK,
            )
            .await?;
        check(cancel)?;
        enter(progress, events, "Installing Codex");
        let codex = self
            .script(
                place,
                &id,
                user.clone(),
                &crate::tensorlake::build_script::codex(),
                IMAGE_WORK,
            )
            .await?;
        check(cancel)?;
        self.script(
            place,
            &id,
            user,
            &crate::tensorlake::build_script::prepare(),
            COMMAND,
        )
        .await?;
        let tag = format!(
            "localhost/farhelm-{}-software:{}",
            self.tag().await?,
            uuid::Uuid::new_v4().simple()
        );
        let image = BuiltImage {
            id: self.commit(name, place, &id, &tag, "software").await?,
            built_at: now(),
            base: recipe.base.clone(),
            packages: recipe.packages.clone(),
            claude_version: last_line(&claude),
            codex_version: last_line(&codex),
        };
        let saved = image.clone();
        let publication = self.0.builds.lock_podman_use(name, place).await?;
        self.edit(name, place, move |state| {
            state.retired_images.retain(|id| id != &saved.id);
            if let Some(old) = state.image.replace(saved) {
                state.retired_images.push(old.id);
            }
        })
        .await?;
        drop(publication);
        self.clear_helper(name, place, "build", &id).await?;
        Ok(image)
    }

    /// Installation runs outside source exclusion. Only the initial Running
    /// boundary and final publication serialize with source retirement.
    #[allow(
        clippy::too_many_arguments,
        reason = "frozen recipe/place and job progress, cancellation and rebuild admission have distinct owners"
    )]
    pub(super) async fn prepare_images(
        &self,
        name: &str,
        recipe: &Recipe,
        place: HostId,
        progress: tokio::sync::watch::Sender<PodmanProgress>,
        events: Arc<FleetEvents>,
        cancel: Arc<std::sync::atomic::AtomicBool>,
        rebuild: bool,
    ) -> anyhow::Result<PreparedImage> {
        let mut result = async {
            check(&cancel)?;
            self.edit(name, place, |state| state.last_failure = None)
                .await?;
            if rebuild {
                let publication = self.0.builds.lock_podman_use(name, place).await?;
                self.edit(name, place, |state| {
                    if let Some(old) = state.image.take() {
                        state.retired_images.push(old.id);
                    }
                    if let Some(old) = state.prepared.take() {
                        state.retired_images.push(old.id);
                    }
                })
                .await?;
                drop(publication);
            }
            self.prepare_images_owned(name, recipe, place, progress, events.clone(), cancel)
                .await
        }
        .await;
        if result.is_err() {
            // Ordinary failures attempt cleanup, while ambiguous sends and
            // refused cleanup retain their durable names/IDs for later retry.
            if let Err(cleanup) = self.reconcile_helpers(name, place).await {
                result = result.map_err(|error| {
                    error.context(format!("image helper cleanup did not finish: {cleanup:#}"))
                });
            }
            if let Err(cleanup) = self.retire_images(name, place).await {
                result = result.map_err(|error| {
                    error.context(format!("image cleanup did not finish: {cleanup:#}"))
                });
            }
            let reason = result
                .as_ref()
                .err()
                .map(|error| crate::tensorlake::shell::shown(&error.to_string()));
            let _ = self
                .edit(name, place, move |state| state.last_failure = reason)
                .await;
        }
        events.bump();
        result
    }

    /// Installation may run without source exclusion after the helper first
    /// runs. Publication rechecks the source and retires an overtaken result.
    async fn prepare_images_owned(
        &self,
        name: &str,
        recipe: &Recipe,
        place: HostId,
        progress: tokio::sync::watch::Sender<PodmanProgress>,
        events: Arc<FleetEvents>,
        cancel: Arc<std::sync::atomic::AtomicBool>,
    ) -> anyhow::Result<PreparedImage> {
        self.reconcile_helpers(name, place).await?;
        loop {
            check(&cancel)?;
            if let Some(prepared) = self.fresh_image(name, place).await? {
                return Ok(prepared);
            }
            let source = self
                .software(name, recipe, place, &progress, &events, &cancel)
                .await?;
            let source_guard = self.0.builds.lock_podman_use(name, place).await?;
            if self
                .state(name, place)
                .await?
                .image
                .as_ref()
                .map(|image| &image.id)
                != Some(&source.id)
            {
                continue;
            }
            enter(&progress, &events, "Preparing Farhelm's installation");
            let (expected, _, _) = self.place(place).await?;
            let id = self
                .helper(name, &expected, "prepare", &source.id, false)
                .await?;
            let info = self
                .inspect(place, &id)
                .await?
                .context("the preparer disappeared before Running")?;
            anyhow::ensure!(
                info.status == "running",
                "the preparer did not become Running"
            );
            drop(source_guard);
            check(&cancel)?;
            let requested = match &source.base {
                RecipeBase::ManagedUbuntu => "ubuntu",
                RecipeBase::ContainerImage { user, .. } => user,
                RecipeBase::TensorlakeImage { .. } | RecipeBase::TensorlakeSnapshot { .. } => {
                    anyhow::bail!("the recorded software image has an incompatible base")
                }
            };
            let user = self.login(place, &id, requested).await?;
            self.prepare_user_manager(place, &id, &user).await?;
            let (_, _, ready) = self.place(place).await?;
            self.proxies(place, &id, &user, &ready.proxy).await?;
            let route = self.route(place, &id, user.clone()).await?;
            let facts = self
                .0
                .preparer
                .prepare(&route, user.home.clone().into())
                .await?;
            check(&cancel)?;
            anyhow::ensure!(
                facts.farhelm_version == farhelm_proto::BUILD_VERSION
                    && facts.home.is_absolute()
                    && facts.farhelm_path.is_absolute()
                    && facts.state_dir.is_absolute(),
                "the installer returned incompatible prepared facts"
            );
            let tag = format!(
                "localhost/farhelm-{}-prepared:{}",
                self.tag().await?,
                uuid::Uuid::new_v4().simple()
            );
            let prepared = PreparedImage {
                id: self.commit(name, place, &id, &tag, "prepared").await?,
                prepared_at: now(),
                farhelm_version: farhelm_proto::BUILD_VERSION.into(),
                source_image: source.id.clone(),
                facts,
                login: user,
            };
            let publication = self.0.builds.lock_podman_use(name, place).await?;
            let same_source = self
                .state(name, place)
                .await?
                .image
                .as_ref()
                .map(|image| &image.id)
                == Some(&source.id);
            let saved = prepared.clone();
            self.edit(name, place, move |state| {
                if same_source {
                    state.retired_images.retain(|id| id != &saved.id);
                    if let Some(old) = state.prepared.replace(saved) {
                        state.retired_images.push(old.id);
                    }
                }
            })
            .await?;
            drop(publication);
            self.clear_helper(name, place, "prepare", &id).await?;
            if let Err(error) = self.retire_images(name, place).await {
                let reason = crate::tensorlake::shell::shown(&format!(
                    "superseded image cleanup did not finish: {error:#}"
                ));
                self.edit(name, place, move |state| state.last_failure = Some(reason))
                    .await?;
            }
            if same_source {
                enter(&progress, &events, "Ready");
                return Ok(prepared);
            }
        }
    }
}

/// Bootstrap and passwd observation precede selection of the login account.
fn root_user() -> ContainerUser {
    ContainerUser {
        name: "0".into(),
        home: "/root".into(),
        uid: 0,
    }
}

/// Empty values and absent variables stay distinct during bootstrap.
pub(super) fn proxy_exports(values: &BTreeMap<String, String>) -> String {
    let mut script = format!("unset {}\n", PROXY_NAMES.join(" "));
    for (name, value) in values {
        script.push_str(&format!(
            "export {}\n",
            crate::ssh::shell_quote(&format!("{name}={value}"))
        ));
    }
    script
}

/// Retain one bounded named step; package and installer captures stay private.
fn enter(progress: &tokio::sync::watch::Sender<PodmanProgress>, events: &FleetEvents, line: &str) {
    progress.send_modify(|view| view.line = crate::manager::diagnostic_excerpt(line, 2000));
    events.bump();
}

/// Official installers append a version after any installation chatter.
fn last_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(str::to_owned)
}

/// Never cancel in the middle of a mutator; its ownership write must settle.
fn check(cancel: &std::sync::atomic::AtomicBool) -> anyhow::Result<()> {
    anyhow::ensure!(
        !cancel.load(std::sync::atomic::Ordering::SeqCst),
        "container image work was cancelled for recipe deletion"
    );
    Ok(())
}

impl Podman {
    /// A missing place entry has never owned resources; a missing recipe is
    /// a refusal, because no later operation may recreate a deleted catalog row.
    async fn state(&self, name: &str, place: HostId) -> anyhow::Result<PodmanRecipeState> {
        let (_, state) = self
            .0
            .store
            .tensorlake_recipe_row(name)
            .await?
            .context("the recipe no longer exists")?;
        Ok(state.podman.get(&place).cloned().unwrap_or_default())
    }

    /// Image capture and builder publication use this same ownership writer.
    /// Definition saves never participate in these transactional state writes.
    pub(super) async fn edit<F>(&self, name: &str, place: HostId, edit: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut PodmanRecipeState) + Send + 'static,
    {
        let expected = self
            .0
            .store
            .list_hosts()
            .await?
            .into_iter()
            .find(|row| row.id == place)
            .context("the container place was removed before its state could be recorded")?;
        self.edit_at(name, &expected, edit).await
    }

    /// The first helper ownership write uses the row paired with the observed
    /// route, before any pull/command. A winning removal or retarget refuses
    /// that exact write; later commands cannot silently adopt another endpoint.
    async fn edit_at<F>(&self, name: &str, expected: &HostRow, edit: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut PodmanRecipeState) + Send + 'static,
    {
        let place = expected.id;
        self.0
            .store
            .update_recipe_place_state(name, expected, move |_, state| {
                edit(state.podman.entry(place).or_default());
            })
            .await?
            .context("the recipe disappeared before its ownership could be saved")?;
        Ok(())
    }

    /// Reuse has the same age/version/source contract as Tensorlake, plus
    /// actual image existence because place storage can disappear independently.
    pub(super) async fn fresh_image(
        &self,
        name: &str,
        place: HostId,
    ) -> anyhow::Result<Option<PreparedImage>> {
        let state = self.state(name, place).await?;
        let (Some(prepared), Some(source)) = (state.prepared, state.image) else {
            return Ok(None);
        };
        if crate::environment::recipes::prepared_stale_reason(
            prepared.prepared_at,
            &prepared.farhelm_version,
            &prepared.source_image,
            now(),
            farhelm_proto::BUILD_VERSION,
            Some(&source.id),
        )
        .is_some()
        {
            return Ok(None);
        }
        if !self.exists(place, "image", &source.id).await?
            || !self.exists(place, "image", &prepared.id).await?
        {
            return Ok(None);
        }
        Ok(Some(prepared))
    }

    /// A reserved name is pending ownership until run returns a canonical ID.
    /// A crash between those writes keeps enough evidence for later cleanup.
    /// Only a completed refusal with confirmed absence can settle the name
    /// before the ambiguous-create window expires.
    async fn helper(
        &self,
        name: &str,
        expected: &HostRow,
        role: &str,
        image: &str,
        bootstrap: bool,
    ) -> anyhow::Result<String> {
        let place = expected.id;
        let reserved = format!(
            "farhelm-{role}-{}-{}",
            self.tag().await?,
            uuid::Uuid::new_v4().simple()
        );
        let pending = reserved.clone();
        let preparing = role == "prepare";
        self.edit_at(name, expected, move |state| {
            if preparing {
                state.preparer_container_id = Some(pending);
                state.prepare_started_at = Some(now());
            } else {
                state.builder_container_id = Some(pending);
                state.build_started_at = Some(now());
            }
        })
        .await?;
        let result = self
            .run(place, &reserved, role, name, image, bootstrap)
            .await;
        let id = match result {
            Ok(id) => id,
            Err(error) => {
                // A missing lookup alone says nothing about an unanswered
                // create. A completed refusal plus absence settles this exact
                // reserved owner, allowing the next build to proceed at once.
                if error.downcast_ref::<CreateRefused>().is_some()
                    && !self.exists(place, "container", &reserved).await?
                {
                    self.edit(name, place, move |state| {
                        if preparing
                            && state.preparer_container_id.as_deref() == Some(reserved.as_str())
                        {
                            state.preparer_container_id = None;
                            state.prepare_started_at = None;
                        } else if !preparing
                            && state.builder_container_id.as_deref() == Some(reserved.as_str())
                        {
                            state.builder_container_id = None;
                            state.build_started_at = None;
                        }
                    })
                    .await?;
                }
                return Err(error);
            }
        };
        let saved = id.clone();
        self.edit(name, place, move |state| {
            if preparing {
                state.preparer_container_id = Some(saved);
            } else {
                state.builder_container_id = Some(saved);
            }
        })
        .await?;
        Ok(id)
    }

    /// Removal confirmation precedes clearing both identity and send marker.
    async fn clear_helper(
        &self,
        name: &str,
        place: HostId,
        role: &str,
        id: &str,
    ) -> anyhow::Result<()> {
        self.remove_container(place, id, role).await?;
        let preparing = role == "prepare";
        self.edit(name, place, move |state| {
            if preparing {
                state.preparer_container_id = None;
                state.prepare_started_at = None;
            } else {
                state.builder_container_id = None;
                state.build_started_at = None;
            }
        })
        .await
    }

    /// Recent absent names remain ambiguous: a disconnected SSH command may
    /// still create them. Only the persisted start marker authorizes recovery.
    async fn reconcile_helpers(&self, name: &str, place: HostId) -> anyhow::Result<()> {
        let state = self.state(name, place).await?;
        for (role, id, sent) in [
            ("build", state.builder_container_id, state.build_started_at),
            (
                "prepare",
                state.preparer_container_id,
                state.prepare_started_at,
            ),
        ] {
            let Some(id) = id else {
                continue;
            };
            let canonical = if let Some(info) = self.inspect(place, &id).await? {
                self.owned(&info, role).await?;
                anyhow::ensure!(
                    info.labels.get("farhelm.recipe").map(String::as_str) == Some(name),
                    "the recorded helper belongs to another recipe; it was left untouched"
                );
                // A name authorizes lookup only. Once observed, pin the ID
                // durably before any further await or destructive command.
                let saved = info.id.clone();
                let preparing = role == "prepare";
                self.edit(name, place, move |state| {
                    if preparing {
                        state.preparer_container_id = Some(saved);
                    } else {
                        state.builder_container_id = Some(saved);
                    }
                })
                .await?;
                info.id
            } else {
                if canonical(&id).is_err() {
                    anyhow::ensure!(
                        sent.is_some_and(
                            |sent| now() >= sent + crate::tensorlake::build::CREATE_SETTLE_SECS
                        ),
                        "an interrupted image helper may still be starting; its ownership was retained"
                    );
                }
                id
            };
            self.clear_helper(name, place, role, &canonical).await?;
        }
        Ok(())
    }

    /// Every container counts as an image user, including another helm's.
    /// Removal never forces usage away and never includes user base images.
    async fn retire_images(&self, name: &str, place: HostId) -> anyhow::Result<()> {
        let _source = self.0.builds.lock_podman_use(name, place).await?;
        self.retire_images_unlocked(name, place).await
    }

    /// Callers hold source exclusion, including the guard-gated deletion path.
    /// Images still used by any container remain in the retirement record.
    /// A pending commit tag needs a positive observation before absence can
    /// settle it. A disconnected commit may publish after an earlier lookup.
    async fn retire_images_unlocked(&self, name: &str, place: HostId) -> anyhow::Result<()> {
        for mut id in self.state(name, place).await?.retired_images {
            if self.exists(place, "image", &id).await? {
                let text = self
                    .require(
                        place,
                        vec!["image".into(), "inspect".into(), "--".into(), id.clone()],
                        COMMAND,
                    )
                    .await?;
                let info: serde_json::Value = serde_json::from_str(&text)?;
                let labels = &info[0]["Config"]["Labels"];
                anyhow::ensure!(
                    labels["farhelm.helm"].as_str() == Some(self.tag().await?.as_str())
                        && labels["farhelm.recipe"].as_str() == Some(name),
                    "an image's ownership labels changed; it was left untouched"
                );
                if image_id(&id).is_err() {
                    let observed = info[0]["Id"]
                        .as_str()
                        .context("Podman returned no committed image identity")?
                        .to_owned();
                    image_id(&observed)?;
                    let pending = id.clone();
                    let saved = observed.clone();
                    self.edit(name, place, move |state| {
                        state.retired_images.retain(|id| id != &pending);
                        state.retired_images.push(saved);
                    })
                    .await?;
                    id = observed;
                }
                let using = self
                    .require(
                        place,
                        vec![
                            "ps".into(),
                            "--all".into(),
                            "--filter".into(),
                            format!("ancestor={id}"),
                            "--format".into(),
                            "{{.ID}}".into(),
                        ],
                        COMMAND,
                    )
                    .await?;
                if !using.trim().is_empty() {
                    continue;
                }
                self.require(
                    place,
                    vec!["image".into(), "rm".into(), "--".into(), id.clone()],
                    COMMAND,
                )
                .await?;
                anyhow::ensure!(
                    !self.exists(place, "image", &id).await?,
                    "the retired image still exists"
                );
            } else {
                anyhow::ensure!(
                    image_id(&id).is_ok(),
                    "an unanswered image capture has no confirmed outcome; its ownership tag was retained"
                );
            }
            self.edit(name, place, move |state| {
                state.retired_images.retain(|old| old != &id)
            })
            .await?;
        }
        Ok(())
    }

    /// Global admission closes first, then owned jobs finish their current
    /// command and cleanup. Each place retains every unconfirmed resource.
    pub(super) async fn clean_images(
        &self,
        name: &str,
        deletion: &crate::tensorlake::build::DeleteGuard,
    ) -> anyhow::Result<()> {
        self.0.builds.cancel_podman(name, deletion).await?;
        let (_, state) = self
            .0
            .store
            .tensorlake_recipe_row(name)
            .await?
            .context("the recipe no longer exists")?;
        let mut failures = Vec::new();
        for (place, _) in state.podman {
            let result = async {
                let _source = self.0.builds.lock_podman_cleanup(name, place, deletion).await?;
                self.reconcile_helpers(name, place).await?;
                self.edit(name, place, |state| {
                    if let Some(image) = state.image.take() { state.retired_images.push(image.id); }
                    if let Some(image) = state.prepared.take() { state.retired_images.push(image.id); }
                }).await?;
                self.retire_images_unlocked(name, place).await?;
                anyhow::ensure!(!self.state(name, place).await?.needs_cleanup(),
                    "owned images are still in use; delete their containers before deleting the recipe");
                self.0.store.update_tensorlake_recipe_state(name, move |_, state| { state.podman.remove(&place); }).await?;
                Ok::<_, anyhow::Error>(())
            }.await;
            if let Err(error) = result {
                failures.push(format!(
                    "container host {place}: {}",
                    crate::tensorlake::shell::shown(&error.to_string())
                ));
            }
        }
        anyhow::ensure!(
            failures.is_empty(),
            "recipe cleanup did not finish: {}",
            failures.join("; ")
        );
        Ok(())
    }

    /// Pull only a missing selected base, then reject installation checkpoints.
    /// This reference is never entered in the helm-owned image cleanup list.
    async fn pull_base(&self, place: HostId, reference: &str) -> anyhow::Result<()> {
        if !self.exists(place, "image", reference).await? {
            self.require(
                place,
                vec![
                    "pull".into(),
                    "--quiet".into(),
                    "--".into(),
                    reference.into(),
                ],
                IMAGE_WORK,
            )
            .await?;
        }
        let text = self
            .require(
                place,
                vec![
                    "image".into(),
                    "inspect".into(),
                    "--".into(),
                    reference.into(),
                ],
                COMMAND,
            )
            .await?;
        let info: serde_json::Value = serde_json::from_str(&text)?;
        anyhow::ensure!(
            info[0]["Config"]["Labels"]["farhelm.artifact"].as_str() != Some("prepared"),
            "a Farhelm installation checkpoint cannot be a recipe base; choose its original base instead"
        );
        Ok(())
    }
}
