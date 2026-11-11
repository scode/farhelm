//! Rootless containers are ordinary supervisor hosts reached through one route.
//!
//! Podman storage belongs to the place account, not this helm. Every mutation
//! therefore uses this helm's labels and canonical identities. An unanswered
//! create retains its row/name; a retry never removes possible session work.

use super::podman_probe::{PodmanPlaces, ReadyPlace};
use super::recipes::{BuiltImage, PreparedImage, Recipe};
use super::*;
use crate::access::{BaseRoute, ContainerUser, RemoteCommand, RemoteRoute};
use crate::provisioning::{CommandResult, ProvisioningTarget};
use crate::store::HelmStore;
use crate::tensorlake::build::{PodmanProgress, RecipeBuilds};
use anyhow::Context as _;
use farhelm_proto::recipes::RecipeBase;
use std::collections::BTreeMap;
use std::time::Duration;

mod build;

#[cfg(test)]
// The command model is shared with provisioning's private fixture assembly.
pub(crate) mod failure_tests;

const COMMAND: Duration = Duration::from_secs(30);
const IMAGE_WORK: Duration = Duration::from_secs(1800);
const UBUNTU: &str = "docker.io/library/ubuntu@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852";
const PROXY_NAMES: [&str; 10] = [
    "http_proxy",
    "https_proxy",
    "ftp_proxy",
    "all_proxy",
    "no_proxy",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "FTP_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
];

/// Only this wrapper enters argv. The script itself may contain proxy
/// credentials, so receive it through stdin into a mode-0600 temporary and
/// remove it before reporting success or capturing an image. A script file
/// also keeps package tools from consuming the remaining script as their stdin.
const PRIVATE_SCRIPT: &str = r#"
umask 077
file=$(mktemp /tmp/farhelm-private-script.XXXXXXXXXX) || exit 1
trap 'rm -f -- "$file"' EXIT
trap 'exit 1' HUP INT TERM
cat > "$file" || exit 1
sh "$file" </dev/null
result=$?
rm -f -- "$file" || exit 1
trap - EXIT
exit "$result"
"#;

/// A completed engine refusal is distinct from transport loss or a signal.
/// Absence must still be confirmed separately before discarding ownership.
#[derive(Debug, thiserror::Error)]
#[error("{reason}")]
struct CreateRefused {
    reason: String,
}

/// The artifact cannot be constructed without the provider's source guard.
pub(crate) struct PreparedContainer {
    image: PreparedImage,
    guard: tokio::sync::OwnedMutexGuard<()>,
}

/// Successful cleanup is attested only by this module after confirmed absence.
#[derive(Debug)]
pub(crate) struct ContainerDeleted {
    host: HostId,
}

impl ContainerDeleted {
    /// A never-sent registration owns no resource, even if its name collides.
    pub(crate) fn never_created(host: HostId) -> Self {
        Self { host }
    }
    fn attest(host: HostId) -> Self {
        Self { host }
    }
    pub(crate) fn host(&self) -> HostId {
        self.host
    }
}

/// Shared production seams are selected before the first actor starts.
#[derive(Clone)]
pub(crate) struct Podman(Arc<Inner>);

struct Inner {
    store: HelmStore,
    places: Arc<PodmanPlaces>,
    delivery: Arc<crate::provisioning::credential_delivery::CredentialDelivery>,
    builds: Arc<RecipeBuilds>,
    preparer: Arc<dyn EnvironmentPreparer>,
    tunnels: crate::tensorlake::tunnels::Tunnels,
}

/// Inspect data is interpreted here, never by the shared actor or launcher.
struct ContainerInfo {
    id: String,
    name: String,
    status: String,
    labels: BTreeMap<String, String>,
}

impl Podman {
    /// Observe the current connected lease without retaining its listener or
    /// client. The common host projection exposes this alongside credentials.
    pub(crate) fn tunnel_view(
        &self,
        host: HostId,
    ) -> Option<farhelm_proto::tensorlake::TunnelView> {
        self.0.tunnels.view(host)
    }

    /// Production passes the already-selected cache, credentials, build
    /// registry and installer. Construction performs no place or image I/O.
    pub(crate) fn new(
        store: HelmStore,
        places: Arc<PodmanPlaces>,
        delivery: Arc<crate::provisioning::credential_delivery::CredentialDelivery>,
        builds: Arc<RecipeBuilds>,
        preparer: Arc<dyn EnvironmentPreparer>,
    ) -> Self {
        Self(Arc::new(Inner {
            store,
            places,
            delivery,
            builds,
            preparer,
            tunnels: Default::default(),
        }))
    }

    /// The durable registry namespace separates helms sharing a unix account.
    async fn tag(&self) -> anyhow::Result<String> {
        Ok(crate::tensorlake::config::helm_tag(
            &self.0.store.registry_namespace().await?,
        ))
    }

    /// Runtime reuse accepts a current endpoint's older successful probe.
    /// Launch admission separately requires recent prerequisites.
    async fn place(&self, host: HostId) -> anyhow::Result<(HostRow, BaseRoute, ReadyPlace)> {
        let (route, ready) = self.0.places.runtime(&self.0.store, host).await?;
        let row = self
            .0
            .store
            .list_hosts()
            .await?
            .into_iter()
            .find(|row| row.id == host)
            .context("the container host is no longer registered")?;
        anyhow::ensure!(
            crate::access::base_route(&row)? == route,
            "the container place changed after its Podman observation; retry at the current place"
        );
        Ok((row, route, ready))
    }

    /// Resolve the current route and executable at each operation boundary.
    /// argv is data, including user-selected image references and accounts.
    async fn command(
        &self,
        place: HostId,
        args: Vec<String>,
        bound: Duration,
    ) -> anyhow::Result<CommandResult> {
        let (_, route, ready) = self.place(place).await?;
        let mut argv = vec![
            ready
                .podman
                .to_str()
                .context("Podman's executable path is not text")?
                .to_owned(),
        ];
        argv.extend(args);
        self.0.places.command(&route, &argv, bound).await
    }

    /// Only a zero exit is a successful mutation or observation. Existence
    /// commands use their narrower 0/1 interpretation below instead.
    async fn require(
        &self,
        place: HostId,
        args: Vec<String>,
        bound: Duration,
    ) -> anyhow::Result<String> {
        let output = self.command(place, args, bound).await?;
        anyhow::ensure!(
            output.code == Some(0),
            "container command failed (status {:?}): {}",
            output.code,
            crate::tensorlake::shell::shown(&output.stderr)
        );
        String::from_utf8(output.stdout).context("Podman returned invalid text")
    }

    /// Podman exists=1 is absence; a transport or storage error is unanswered.
    async fn exists(&self, place: HostId, kind: &str, id: &str) -> anyhow::Result<bool> {
        let output = self
            .command(
                place,
                vec![kind.into(), "exists".into(), "--".into(), id.into()],
                COMMAND,
            )
            .await?;
        match (output.code, output.signal) {
            (Some(0), None) => Ok(true),
            (Some(1), None) => Ok(false),
            _ => anyhow::bail!(
                "Podman could not establish {kind} existence: {}",
                crate::tensorlake::shell::shown(&output.stderr)
            ),
        }
    }

    /// An inspect failure never proves absence. Canonical IDs are validated
    /// before their use as durable cleanup or execution authority.
    async fn inspect(&self, place: HostId, id: &str) -> anyhow::Result<Option<ContainerInfo>> {
        if !self.exists(place, "container", id).await? {
            return Ok(None);
        }
        let text = self
            .require(
                place,
                vec!["container".into(), "inspect".into(), "--".into(), id.into()],
                COMMAND,
            )
            .await?;
        let data: serde_json::Value = serde_json::from_str(&text)?;
        let item = data
            .as_array()
            .and_then(|items| items.first())
            .context("Podman returned no inspected container")?;
        let observed = item["Id"]
            .as_str()
            .context("Podman returned no container ID")?
            .to_owned();
        canonical(&observed)?;
        let name = item["Name"]
            .as_str()
            .context("Podman returned no container name")?
            .trim_start_matches('/')
            .to_owned();
        anyhow::ensure!(
            if canonical(id).is_ok() {
                observed == id
            } else {
                name == id
            },
            "Podman inspection returned a different container identity; it was left untouched"
        );
        Ok(Some(ContainerInfo {
            id: observed,
            name,
            status: item["State"]["Status"]
                .as_str()
                .context("Podman returned no container state")?
                .into(),
            labels: serde_json::from_value(item["Config"]["Labels"].clone()).unwrap_or_default(),
        }))
    }

    /// Reserved names alone cannot authorize touching shared Podman storage.
    async fn owned(&self, info: &ContainerInfo, role: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            info.labels.get("farhelm.helm") == Some(&self.tag().await?)
                && info.labels.get("farhelm.role").map(String::as_str) == Some(role),
            "the container's ownership labels do not match this helm; it was left untouched"
        );
        Ok(())
    }

    /// Scope placement keeps conmon/network helpers outside the helm or SSH
    /// login cgroup. A nonce avoids colliding with a still-collected old scope.
    async fn scoped(&self, place: HostId, name: &str, args: Vec<String>) -> anyhow::Result<String> {
        let output = self.scoped_output(place, name, args).await?;
        anyhow::ensure!(
            output.code == Some(0),
            "the container could not start: {}",
            crate::tensorlake::shell::shown(&output.stderr)
        );
        Ok(String::from_utf8(output.stdout)?)
    }

    /// Keep the completed outcome available to create before text parsing.
    /// A lost capture or an invalid successful reply must retain its owner.
    async fn scoped_output(
        &self,
        place: HostId,
        name: &str,
        args: Vec<String>,
    ) -> anyhow::Result<CommandResult> {
        let (_, route, ready) = self.place(place).await?;
        let mut argv = vec![
            "systemd-run".into(),
            "--user".into(),
            "--scope".into(),
            "--collect".into(),
            "--quiet".into(),
            "--unit".into(),
            format!("farhelm-env-{name}-{}.scope", uuid::Uuid::new_v4().simple()),
            "--".into(),
            ready
                .podman
                .to_str()
                .context("Podman's executable path is not text")?
                .into(),
        ];
        argv.extend(args);
        self.0
            .places
            .command(&route, &argv, Duration::from_secs(120))
            .await
    }

    /// Builders without host rows still use the ordinary command lowering.
    async fn route(
        &self,
        place: HostId,
        container: &str,
        user: ContainerUser,
    ) -> anyhow::Result<RemoteRoute> {
        let (_, via, ready) = self.place(place).await?;
        Ok(RemoteRoute::PodmanExec {
            via,
            podman: ready.podman,
            container: container.into(),
            user,
        })
    }

    /// Direct exec inherits no login-manager environment. Supply the exact
    /// observed place proxies here as well as in the future boot environment.
    /// They travel only through private stdin, never a nested shell's argv.
    /// A failure shows the script's error output, because that is where a
    /// build step says what went wrong (the base's missing requirement, the
    /// package manager's last lines), with every proxy value masked first,
    /// because a tool can echo its proxy and a proxy URL can carry a password.
    async fn script(
        &self,
        place: HostId,
        container: &str,
        user: ContainerUser,
        script: &str,
        bound: Duration,
    ) -> anyhow::Result<String> {
        let (_, _, ready) = self.place(place).await?;
        let script = format!("{}\n{script}", build::proxy_exports(&ready.proxy));
        let route = self.route(place, container, user).await?;
        let RemoteRoute::PodmanExec { via, .. } = &route else {
            unreachable!()
        };
        let output = self
            .0
            .places
            .command_with_input(
                via,
                &route.container_argv(RemoteCommand::Script(PRIVATE_SCRIPT))?,
                script.as_bytes(),
                bound,
            )
            .await
            .map_err(|error| {
                if error.is::<crate::transport::SessionLimitRefusal>() {
                    error
                } else {
                    anyhow::anyhow!("container setup command failed or timed out")
                }
            })?;
        if output.code != Some(0) {
            // A step killed outright (out of memory, say) or one that exits
            // without a word still needs a reason on record: its status.
            let shown = script_failure_output(&output.stderr, &ready.proxy);
            anyhow::bail!(
                "container setup failed: {}",
                if shown.is_empty() {
                    match (output.code, output.signal) {
                        (Some(code), _) => format!("exit status {code}, with no error output"),
                        (None, Some(signal)) => format!("terminated by signal {signal}"),
                        (None, None) => "terminated without an exit status".to_string(),
                    }
                } else {
                    shown
                }
            );
        }
        Ok(String::from_utf8(output.stdout)?)
    }

    /// Pause does not prevent whole-resource deletion. Revalidate ownership
    /// and confirm absence before callers discard any durable cleanup record.
    async fn remove_container(&self, place: HostId, id: &str, role: &str) -> anyhow::Result<()> {
        if let Some(info) = self.inspect(place, id).await? {
            self.owned(&info, role).await?;
            self.require(
                place,
                vec!["rm".into(), "--force".into(), "--".into(), info.id],
                COMMAND,
            )
            .await?;
        }
        anyhow::ensure!(
            !self.exists(place, "container", id).await?,
            "Podman still has the container; its ownership record was retained"
        );
        Ok(())
    }

    /// All starts use owned scopes and explicit systemd/proxy policy. The
    /// caller must durably record the fresh reserved name before entering.
    /// A typed refusal still needs confirmed absence to settle that owner;
    /// transport loss and successful replies that cannot be parsed stay pending.
    async fn run(
        &self,
        place: HostId,
        name: &str,
        role: &str,
        recipe: &str,
        image: &str,
        bootstrap: bool,
    ) -> anyhow::Result<String> {
        let mut args = vec![
            "run".into(),
            "--detach".into(),
            "--name".into(),
            name.into(),
            "--http-proxy=false".into(),
            "--systemd=always".into(),
            "--pull=never".into(),
            "--label".into(),
            format!("farhelm.helm={}", self.tag().await?),
            "--label".into(),
            format!("farhelm.role={role}"),
            "--label".into(),
            format!("farhelm.recipe={recipe}"),
            "--entrypoint".into(),
            if bootstrap {
                "/bin/sh".into()
            } else {
                "/sbin/init".into()
            },
            "--".into(),
            image.into(),
        ];
        if bootstrap {
            args.extend(["-c".into(), "exec sleep infinity".into()]);
        }
        let output = self.scoped_output(place, name, args).await?;
        if output.code != Some(0) {
            let reason = format!(
                "the container could not start: {}",
                crate::tensorlake::shell::shown(&output.stderr)
            );
            return Err(if definite_refusal(&output) {
                anyhow::anyhow!(CreateRefused { reason })
            } else {
                anyhow::anyhow!(reason)
            });
        }
        let id = String::from_utf8(output.stdout)?.trim().to_owned();
        canonical(&id)?;
        Ok(id)
    }

    /// Capture only a cleanly stopped helper. Committing a running user
    /// manager can preserve half-written state or a nonempty machine identity.
    async fn commit(
        &self,
        recipe: &str,
        place: HostId,
        id: &str,
        tag: &str,
        artifact: &str,
    ) -> anyhow::Result<String> {
        // No forced-kill fallback can certify a clean checkpoint. The command
        // has its own outer bound; timeout retains the helper for cleanup.
        self.require(
            place,
            vec![
                "stop".into(),
                "--time".into(),
                "-1".into(),
                "--".into(),
                id.into(),
            ],
            Duration::from_secs(60),
        )
        .await?;
        let info = self
            .inspect(place, id)
            .await?
            .context("the image helper disappeared before commit")?;
        anyhow::ensure!(
            matches!(info.status.as_str(), "exited" | "stopped"),
            "the image helper did not stop cleanly"
        );
        // A lost reply can leave a real image. Record its fresh lookup tag
        // before sending commit, then replace it with immutable identity.
        let pending = tag.to_owned();
        self.edit(recipe, place, move |state| {
            state.retired_images.push(pending)
        })
        .await?;
        let output = self
            .command(
                place,
                vec![
                    "commit".into(),
                    "--change".into(),
                    format!("LABEL farhelm.artifact={artifact}"),
                    "--".into(),
                    id.into(),
                    tag.into(),
                ],
                IMAGE_WORK,
            )
            .await?;
        if output.code != Some(0) {
            // SSH 255 or a signalled/capture failure leaves the remote outcome
            // unknown. A completed Podman refusal can clear an absent tag.
            if definite_refusal(&output) && !self.exists(place, "image", tag).await? {
                let pending = tag.to_owned();
                self.edit(recipe, place, move |state| {
                    state.retired_images.retain(|id| id != &pending)
                })
                .await?;
            }
            anyhow::bail!(
                "image capture failed (status {:?}): {}",
                output.code,
                crate::tensorlake::shell::shown(&output.stderr)
            );
        }
        let image = String::from_utf8(output.stdout)?.trim().to_owned();
        image_id(&image)?;
        let saved = image.clone();
        let pending = tag.to_owned();
        self.edit(recipe, place, move |state| {
            state.retired_images.retain(|id| id != &pending);
            state.retired_images.push(saved);
        })
        .await?;
        Ok(image)
    }

    /// Install exact manager proxy values for future boots and update the
    /// already-running manager. PID1's environment alone is insufficient.
    /// The credential installer owns private file transfer and replacement;
    /// the running manager imports values from the shell's environment by name.
    async fn proxies(
        &self,
        place: HostId,
        id: &str,
        user: &ContainerUser,
        values: &BTreeMap<String, String>,
    ) -> anyhow::Result<()> {
        let environment = self
            .script(
                place,
                id,
                user.clone(),
                "systemctl --user show-environment",
                COMMAND,
            )
            .await?;
        let manager = crate::units::ManagerEnvironment::from_show_environment_exact(&environment)?;
        let directory = match manager.get_exact("XDG_CONFIG_HOME") {
            crate::units::ManagerValue::Value(value)
                if std::path::Path::new(&value).is_absolute() =>
            {
                value
            }
            crate::units::ManagerValue::Absent | crate::units::ManagerValue::Value(_) => {
                format!("{}/.config", user.home)
            }
            crate::units::ManagerValue::Unreadable => anyhow::bail!(
                "the container user manager returned an unreadable configuration path"
            ),
        };
        let file = format!("{directory}/environment.d/90-farhelm-proxy.conf");
        let text = proxy_file(values);
        let backend = self
            .0
            .delivery
            .backend()
            .context("this helm has no provisioning transport")?;
        let target = ProvisioningTarget::Remote(self.route(place, id, user.clone()).await?);
        self.script(
            place,
            id,
            user.clone(),
            &format!(
                "umask 077\nmkdir -p -- {} || exit 1\n",
                crate::ssh::shell_quote(&format!("{directory}/environment.d"))
            ),
            COMMAND,
        )
        .await?;
        let path = std::path::Path::new(&file);
        let temporary = path.with_file_name(format!(
            ".90-farhelm-proxy.conf.farhelm-tmp-{}",
            uuid::Uuid::new_v4()
        ));
        backend
            .install_bytes(&target, text.as_bytes(), path, &temporary, 0o600)
            .await
            .map_err(|error| {
                anyhow::anyhow!(error.redacted_reason("could not install the proxy settings"))
            })?;
        // import-environment reads values from this process's environment. Only
        // names enter systemctl's argv; set-environment NAME=value would expose
        // passwords even when the enclosing script arrived over private stdin.
        let mut script = format!(
            "{}\nsystemctl --user unset-environment {} || exit 1\n",
            build::proxy_exports(values),
            PROXY_NAMES.join(" ")
        );
        if !values.is_empty() {
            script.push_str(&format!(
                "systemctl --user import-environment {} || exit 1\n",
                values
                    .keys()
                    .map(|name| crate::ssh::shell_quote(name))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        self.script(place, id, user.clone(), &script, COMMAND)
            .await?;
        Ok(())
    }

    /// Establish current canonical ownership before withdrawing exec leases.
    /// The common hold then closes this helm's exec before a real pause.
    async fn change(
        &self,
        manager: &crate::manager::ConnectionManager,
        host: HostId,
        resume: bool,
    ) -> anyhow::Result<()> {
        let _lock = manager
            .try_host_provision_lock(host)
            .context("This host is busy; try again after its current operation finishes")?;
        let row = manager
            .store()
            .list_hosts()
            .await?
            .into_iter()
            .find(|row| row.id == host)
            .context("the container is no longer registered")?;
        let binding = row
            .podman
            .as_ref()
            .context("the host is no longer a container")?;
        let id = binding
            .container_id
            .as_deref()
            .context("the container create has no confirmed identity")?;
        let info = self
            .inspect(binding.place_host, id)
            .await?
            .context("the container was deleted outside Farhelm")?;
        self.owned(&info, "session").await?;
        let verb = lifecycle_verb(&info.status, resume)?;
        let changing = crate::manager::HostState::TensorlakeChanging {
            starting: verb == "start",
            direction: if resume {
                farhelm_proto::tensorlake::LifecycleDirection::Resuming
            } else {
                farhelm_proto::tensorlake::LifecycleDirection::Suspending
            },
            requested_at: now_ms(),
        };
        super::lifecycle::perform(
            manager,
            &row,
            resume,
            changing,
            async {
                if verb.is_empty() {
                    return Ok(false);
                }
                if verb == "start" {
                    self.scoped(
                        binding.place_host,
                        &binding.name,
                        vec!["start".into(), "--".into(), id.into()],
                    )
                    .await?;
                } else {
                    self.require(
                        binding.place_host,
                        vec![verb.into(), "--".into(), id.into()],
                        COMMAND,
                    )
                    .await?;
                }
                let status = self
                    .inspect(binding.place_host, id)
                    .await?
                    .context("the container disappeared during its lifecycle operation")?
                    .status;
                anyhow::ensure!(
                    status == if resume { "running" } else { "paused" },
                    "Podman did not confirm the requested container state"
                );
                Ok(true)
            },
            |_| async { Ok(None) },
        )
        .await
    }

    /// The Containers section may delete an unrecorded leftover only. A
    /// recorded row goes through ordinary host removal and its actor hold;
    /// recorded recipe helpers go through guarded recipe cleanup instead.
    pub(crate) async fn delete_leftover(&self, place: HostId, id: &str) -> anyhow::Result<()> {
        canonical(id)?;
        let Some(info) = self.inspect(place, id).await? else {
            return Ok(());
        };
        let role = info
            .labels
            .get("farhelm.role")
            .map(String::as_str)
            .unwrap_or("");
        anyhow::ensure!(
            matches!(role, "session" | "build" | "prepare"),
            "the container is not a Farhelm environment resource"
        );
        self.owned(&info, role).await?;
        let rows = self.0.store.list_hosts().await?;
        let recorded_host = rows
            .iter()
            .filter_map(|row| row.podman.as_ref())
            .any(|binding| {
                binding.place_host == place
                    && (binding.container_id.as_deref() == Some(id) || binding.name == info.name)
            });
        let recipes = self.0.store.tensorlake_recipe_rows().await?;
        let recorded_recipe = recipes.iter().any(|(_, _, state)| {
            state.podman.get(&place).is_some_and(|state| {
                [&state.builder_container_id, &state.preparer_container_id]
                    .into_iter()
                    .flatten()
                    .any(|recorded| recorded == id || recorded == &info.name)
            })
        });
        let live = info
            .labels
            .get("farhelm.recipe")
            .is_some_and(|recipe| self.0.builds.podman_job_active(recipe, place));
        anyhow::ensure!(
            !recorded_host && !recorded_recipe && !live,
            "this container belongs to a host or recipe on this helm; delete it there"
        );
        self.remove_container(place, id, role).await
    }

    /// Include unfinished session-owned rows: an interrupted launch can leave
    /// a real container without ever creating a session. Recipe helpers remain
    /// recipe cleanup's responsibility, including after an interrupted build.
    async fn leftovers(
        &self,
    ) -> anyhow::Result<Vec<farhelm_proto::environment::ContainerLeftoverView>> {
        let rows = self.0.store.list_hosts().await?;
        let recipes = self.0.store.tensorlake_recipe_rows().await?;
        let mut places = std::collections::BTreeSet::new();
        for row in &rows {
            if row.kind.can_host_environments() && self.0.places.view(row)?.usable {
                places.insert(row.id);
            }
            if let Some(binding) = &row.podman {
                places.insert(binding.place_host);
            }
        }
        for (_, _, state) in &recipes {
            places.extend(
                state
                    .podman
                    .iter()
                    .filter_map(|(place, state)| state.needs_cleanup().then_some(*place)),
            );
        }
        let mut result = Vec::new();
        for place in places {
            let owned = self.list_owned(Place::PodmanHost(place)).await?;
            // Read ownership after discovery: a helper created while listing
            // must not briefly appear as an unrecorded deletion candidate.
            let recipes = self.0.store.tensorlake_recipe_rows().await?;
            let rows = self.0.store.list_hosts().await?;
            for item in owned {
                let recipe_helper = recipes.iter().any(|(_, _, state)| {
                    state.podman.get(&place).is_some_and(|state| {
                        [&state.builder_container_id, &state.preparer_container_id]
                            .into_iter()
                            .flatten()
                            .any(|id| id == &item.id || id == &item.name)
                    })
                });
                if recipe_helper {
                    continue;
                }
                let host = rows
                    .iter()
                    .find(|row| {
                        row.podman.as_ref().is_some_and(|binding| {
                            binding.place_host == place
                                && (binding.container_id.as_deref() == Some(&item.id)
                                    || binding.name == item.name)
                        })
                    })
                    .map(|row| row.id);
                if let Some(host) = host
                    && !self.0.store.cached_sessions(host).await?.is_empty()
                {
                    continue;
                }
                result.push(farhelm_proto::environment::ContainerLeftoverView {
                    id: item.id,
                    name: item.name,
                    place: item.place,
                    role: item.role,
                    host,
                });
            }
        }
        Ok(result)
    }
}

/// The provider-owned Containers adapter uses the same concrete service
/// installed in the manager registry. Injected assemblies without it answer
/// Unavailable rather than constructing another cache or provider instance.
pub(crate) async fn list_leftovers(
    axum::extract::State(state): axum::extract::State<Arc<crate::AppState>>,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    let Some(podman) = state.podman.as_ref() else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "this helm has no Podman service",
        )
            .into_response();
    };
    match podman.leftovers().await {
        Ok(items) => axum::Json(items).into_response(),
        Err(error) => (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("could not list this helm's container leftovers: {error:#}"),
        )
            .into_response(),
    }
}

/// Only place and canonical identity cross the public deletion boundary.
/// Ownership is revalidated by the provider immediately before removal.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeleteLeftover {
    place: Place,
    id: String,
}

/// Row-backed entries must use ordinary host removal and its connection hold.
/// A dropped HTTP request leaves this explicit cleanup attempt owned until its
/// absence check completes; it never becomes an automatic cleanup queue.
pub(crate) async fn delete_leftover(
    axum::extract::State(state): axum::extract::State<Arc<crate::AppState>>,
    axum::Json(request): axum::Json<DeleteLeftover>,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    let Some(podman) = state.podman.clone() else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "this helm has no Podman service",
        )
            .into_response();
    };
    let Place::PodmanHost(place) = request.place else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "the selected place is not a container host",
        )
            .into_response();
    };
    if let Err(error) = canonical(&request.id) {
        return (axum::http::StatusCode::BAD_REQUEST, error.to_string()).into_response();
    }
    match crate::run_owned(async move { podman.delete_leftover(place, &request.id).await }).await {
        Ok(()) => {
            state.manager.events().bump();
            axum::Json(serde_json::json!({})).into_response()
        }
        Err(error) => (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("container cleanup did not finish: {error:#}"),
        )
            .into_response(),
    }
}

#[async_trait::async_trait]
impl EnvironmentProvider for Podman {
    async fn rebuild_recipe(
        &self,
        name: &str,
        place: Place,
        events: Arc<FleetEvents>,
    ) -> anyhow::Result<()> {
        let Place::PodmanHost(place) = place else {
            anyhow::bail!("the place belongs to another provider")
        };
        let (recipe, _) = self
            .0
            .store
            .tensorlake_recipe_row(name)
            .await?
            .context("the recipe no longer exists")?;
        anyhow::ensure!(
            recipe.base.supports(ProviderKind::Podman),
            "this recipe's base cannot run in a container"
        );
        let owner = self.clone();
        let recipe_name = name.to_owned();
        self.0
            .builds
            .start_podman(name, place, true, move |progress, cancel| async move {
                owner.0.places.fresh(&owner.0.store, place).await?;
                owner
                    .prepare_images(&recipe_name, &recipe, place, progress, events, cancel, true)
                    .await
            })?;
        Ok(())
    }

    async fn clean_recipe(
        &self,
        name: &str,
        deletion: &crate::tensorlake::build::DeleteGuard,
    ) -> anyhow::Result<()> {
        self.clean_images(name, deletion).await
    }
    async fn places(&self) -> anyhow::Result<Vec<EnvironmentPlace>> {
        Ok(self
            .0
            .store
            .list_hosts()
            .await?
            .into_iter()
            .filter(|row| row.kind.can_host_environments())
            .map(|row| {
                let reason = match self.0.places.view(&row) {
                    Ok(view) if view.usable => None,
                    Ok(view) => Some(
                        view.reason
                            .unwrap_or_else(|| "this host needs a Podman check".into()),
                    ),
                    Err(_) => Some("this host cannot run containers".into()),
                };
                EnvironmentPlace {
                    place: Place::PodmanHost(row.id),
                    reason,
                }
            })
            .collect())
    }

    async fn ensure_prepared(
        &self,
        name: &str,
        recipe: &Recipe,
        place: Place,
        events: Arc<FleetEvents>,
        progress: &dyn PreparationProgress,
    ) -> anyhow::Result<PreparedArtifact> {
        let Place::PodmanHost(place) = place else {
            anyhow::bail!("the place belongs to another provider")
        };
        anyhow::ensure!(
            recipe.base.supports(ProviderKind::Podman),
            "this recipe's base cannot run in a container"
        );
        self.0.places.fresh(&self.0.store, place).await?;
        loop {
            if self.fresh_image(name, place).await?.is_none() {
                progress.needed().await;
                let owner = self.clone();
                let name_owned = name.to_owned();
                let recipe = recipe.clone();
                let events = events.clone();
                let mut job = self.0.builds.start_podman(
                    name,
                    place,
                    false,
                    move |view, cancel| async move {
                        owner
                            .prepare_images(
                                &name_owned,
                                &recipe,
                                place,
                                view,
                                events,
                                cancel,
                                false,
                            )
                            .await
                    },
                )?;
                loop {
                    let view = job.borrow_and_update().clone();
                    if !view.line.is_empty() {
                        progress.step(view.line).await;
                    }
                    if let Some(result) = view.result {
                        result.map_err(anyhow::Error::msg)?;
                        break;
                    }
                    job.changed().await.context(
                        "container preparation ended without a result; retry the launch",
                    )?;
                }
            }
            let guard = self.0.builds.lock_podman_use(name, place).await?;
            if let Some(image) = self.fresh_image(name, place).await? {
                return Ok(PreparedArtifact::Podman(PreparedContainer { image, guard }));
            }
        }
    }

    async fn create(
        &self,
        binding: &EnvironmentBinding,
        prepared: Option<PreparedArtifact>,
        _progress: &dyn PreparationProgress,
    ) -> anyhow::Result<CreatedEnvironment> {
        let ProviderHandle::Podman(expected) = &binding.handle else {
            anyhow::bail!("the registry binding belongs to another provider")
        };
        let row = self
            .0
            .store
            .list_hosts()
            .await?
            .into_iter()
            .find(|row| row.id == binding.host)
            .context("the container row disappeared before creation")?;
        let current = row
            .podman
            .as_ref()
            .context("the row is no longer a container")?;
        anyhow::ensure!(
            current.name == expected.name && current.place_host == expected.place_host,
            "the container binding changed before creation"
        );
        let selected = match prepared {
            Some(PreparedArtifact::Podman(image)) => Some(image),
            Some(PreparedArtifact::Tensorlake(_)) => {
                anyhow::bail!("the prepared artifact belongs to another provider")
            }
            None => None,
        };
        let target = current.container_id.as_deref().unwrap_or(&current.name);
        let mut created = false;
        let info = if let Some(info) = self.inspect(current.place_host, target).await? {
            anyhow::ensure!(
                current.container_id.is_some() || current.pending_create_at.is_some(),
                "the reserved container name is already in use; the existing container was left untouched"
            );
            self.owned(&info, "session").await?;
            anyhow::ensure!(
                current
                    .container_id
                    .as_deref()
                    .is_none_or(|id| id == info.id)
                    && (current.container_id.is_some() || info.name == current.name),
                "the recorded container identity changed; it was left untouched"
            );
            info
        } else {
            anyhow::ensure!(
                current.container_id.is_none(),
                "the recorded container was deleted outside Farhelm; Delete its entry and launch again"
            );
            anyhow::ensure!(current.pending_create_at.is_none_or(|sent| now() >= sent + crate::tensorlake::build::CREATE_SETTLE_SECS), "an unanswered container create may still be running; its row is retained for a later explicit retry");
            let selected = selected
                .as_ref()
                .context("the container disappeared before setup; launch again")?;
            if current.login.is_some() {
                anyhow::ensure!(
                    current.login_user()? == selected.image.login,
                    "the prepared image's login account differs from the registered container"
                );
            }
            let sent = now();
            anyhow::ensure!(
                self.0
                    .store
                    .set_podman_prepared_create(
                        crate::store::PodmanInstallGuard::of(&row)?,
                        sent,
                        &selected.image.facts,
                        &selected.image.login
                    )
                    .await?,
                "the container binding changed before create"
            );
            let result = self
                .run(
                    current.place_host,
                    &current.name,
                    "session",
                    &current.recipe,
                    &selected.image.id,
                    false,
                )
                .await;
            let info = self.inspect(current.place_host, &current.name).await?;
            let info = match (result, info) {
                (Ok(id), Some(info)) => {
                    anyhow::ensure!(info.id == id, "the created container identity changed");
                    created = true;
                    info
                }
                (Err(_), Some(info)) => info,
                (Err(error), None) => {
                    if error.downcast_ref::<CreateRefused>().is_some() {
                        anyhow::ensure!(
                            self.0
                                .store
                                .clear_refused_podman_create(
                                    crate::store::PodmanInstallGuard::of(&row)?,
                                    sent,
                                )
                                .await?,
                            "the refused container create's ownership changed before it could be settled"
                        );
                    }
                    return Err(error);
                }
                (Ok(_), None) => anyhow::bail!(
                    "the created container disappeared before its identity could be recorded"
                ),
            };
            self.owned(&info, "session").await?;
            anyhow::ensure!(
                info.name == current.name,
                "the pending create resolved to another container; it was left untouched"
            );
            info
        };
        anyhow::ensure!(
            self.0
                .store
                .set_podman_container_id(binding.host, &info.id)
                .await?,
            "the canonical container identity could not be recorded"
        );
        anyhow::ensure!(
            info.status == "running",
            "the container is {}; use Resume if it was stopped or paused",
            info.status
        );
        // First Running ends exclusion even though proxy installation follows.
        // Explicitly consume the guard so its lifetime is visible at this boundary.
        if let Some(selected) = selected {
            drop(selected.guard);
        }
        let observed = self
            .0
            .store
            .list_hosts()
            .await?
            .into_iter()
            .find(|row| row.id == binding.host)
            .context("the container binding disappeared after creation")?;
        match (&observed.remote_farhelm, &observed.remote_state_dir) {
            (None, None) => {
                // A canonical ID, including one recovered after a lost run
                // reply, does not admit a supervisor. Its prepared image may
                // have booted that process with older proxies. Install this
                // launch's values and restart it before admission can attach
                // or create user work. Do not clear its identity or state.
                let login = observed
                    .podman
                    .as_ref()
                    .context("the host is no longer a container")?
                    .login_user()?;
                let (_, _, ready) = self.place(current.place_host).await?;
                self.script(
                    current.place_host,
                    &info.id,
                    login.clone(),
                    crate::tensorlake::build_script::user_manager_ready(),
                    Duration::from_secs(45),
                )
                .await?;
                self.proxies(current.place_host, &info.id, &login, &ready.proxy)
                    .await?;
                let backend = self
                    .0
                    .delivery
                    .backend()
                    .context("this helm has no provisioning transport")?;
                let target = ProvisioningTarget::Remote(
                    self.route(current.place_host, &info.id, login).await?,
                );
                backend
                    .restart(&target, crate::units::SUPERVISOR_UNIT_NAME)
                    .await
                    .map_err(|error| {
                        anyhow::anyhow!(error.redacted_reason(
                            "could not restart Farhelm with the container's proxy settings"
                        ))
                    })?;
            }
            (Some(_), Some(_)) => {
                // An admitted environment may already contain user work.
                // Its creation-time proxy snapshot must survive later probes,
                // retries and reboots; even an import without restart changes it.
            }
            _ => anyhow::bail!(
                "the container has incomplete admitted installation coordinates; setup was refused"
            ),
        }
        Ok(CreatedEnvironment { created })
    }

    async fn status(
        &self,
        binding: &EnvironmentBinding,
    ) -> Result<EnvironmentStatus, StatusUnanswered> {
        let answer = async {
            let ProviderHandle::Podman(binding) = &binding.handle else {
                anyhow::bail!("the registry binding belongs to another provider")
            };
            let id = binding
                .container_id
                .as_deref()
                .context("the container create has no confirmed identity")?;
            let Some(info) = self.inspect(binding.place_host, id).await? else {
                return Ok(EnvironmentStatus::Missing(
                    "the container was deleted outside Farhelm; Delete its entry to remove it"
                        .into(),
                ));
            };
            self.owned(&info, "session").await?;
            Ok(match info.status.as_str() {
                "running" => EnvironmentStatus::Running,
                "paused" => EnvironmentStatus::Suspended {
                    resume_failed: None,
                },
                "exited" | "stopped" | "created" => EnvironmentStatus::Stopped,
                "removing" | "stopping" | "configured" => EnvironmentStatus::Changing(
                    "the container is changing state; Farhelm connects once it is running".into(),
                ),
                other => EnvironmentStatus::Unknown(format!(
                    "Podman reported unknown container state {}",
                    crate::manager::diagnostic_excerpt(other, 128)
                )),
            })
        }
        .await;
        answer.map_err(|error: anyhow::Error| {
            StatusUnanswered(format!(
                "Farhelm could not ask the container host whether the container is running: {error}"
            ))
        })
    }

    async fn suspend(
        &self,
        manager: &crate::manager::ConnectionManager,
        binding: &EnvironmentBinding,
    ) -> anyhow::Result<()> {
        self.change(manager, binding.host, false).await
    }
    async fn resume(
        &self,
        manager: &crate::manager::ConnectionManager,
        binding: &EnvironmentBinding,
    ) -> anyhow::Result<()> {
        self.change(manager, binding.host, true).await
    }

    async fn destroy(&self, binding: &EnvironmentBinding) -> anyhow::Result<Destroyed> {
        let ProviderHandle::Podman(current) = &binding.handle else {
            anyhow::bail!("the registry binding belongs to another provider")
        };
        if current.container_id.is_none() && current.pending_create_at.is_none() {
            return Ok(Destroyed::Podman(ContainerDeleted::never_created(
                binding.host,
            )));
        }
        let id = if let Some(id) = &current.container_id {
            id.clone()
        } else {
            match self.inspect(current.place_host, &current.name).await? {
                Some(info) => {
                    self.owned(&info, "session").await?;
                    anyhow::ensure!(
                        info.name == current.name,
                        "the pending create resolved to another container"
                    );
                    anyhow::ensure!(
                        self.0
                            .store
                            .set_podman_container_id(binding.host, &info.id)
                            .await?,
                        "the recovered container ID could not be saved; nothing was removed"
                    );
                    info.id
                }
                None => {
                    anyhow::ensure!(
                        current.pending_create_at.is_some_and(
                            |sent| now() >= sent + crate::tensorlake::build::CREATE_SETTLE_SECS
                        ),
                        "an unanswered container create may still be running; the row was retained"
                    );
                    return Ok(Destroyed::Podman(ContainerDeleted::attest(binding.host)));
                }
            }
        };
        self.remove_container(current.place_host, &id, "session")
            .await?;
        Ok(Destroyed::Podman(ContainerDeleted::attest(binding.host)))
    }

    async fn list_owned(&self, place: Place) -> anyhow::Result<Vec<OwnedEnvironment>> {
        let Place::PodmanHost(host) = place else {
            anyhow::bail!("the place belongs to another provider")
        };
        let text = self
            .require(
                host,
                vec![
                    "ps".into(),
                    "--all".into(),
                    "--no-trunc".into(),
                    "--filter".into(),
                    format!("label=farhelm.helm={}", self.tag().await?),
                    "--format=json".into(),
                ],
                COMMAND,
            )
            .await?;
        let items: Vec<serde_json::Value> = serde_json::from_str(&text)?;
        let mut owned = Vec::new();
        for item in items {
            let id = item["Id"]
                .as_str()
                .or_else(|| item["ID"].as_str())
                .context("Podman listed a container without an ID")?;
            if let Some(info) = self.inspect(host, id).await? {
                let role = info
                    .labels
                    .get("farhelm.role")
                    .map(String::as_str)
                    .unwrap_or("");
                if matches!(role, "session" | "build" | "prepare") {
                    self.owned(&info, role).await?;
                    owned.push(OwnedEnvironment {
                        id: info.id,
                        name: info.name,
                        place,
                        role: role.into(),
                    });
                }
            }
        }
        Ok(owned)
    }

    async fn connected(
        &self,
        row: &HostRow,
        client: Arc<SupervisorClient>,
        events: Arc<FleetEvents>,
    ) -> ConnectedResources {
        let Some(binding) = &row.podman else {
            return ConnectedResources::default();
        };
        let lease = self.0.delivery.enter(row);
        self.0.delivery.connect(row, &events).await;
        ConnectedResources {
            _credentials: Some(lease),
            _tunnels: self
                .0
                .tunnels
                .start(&self.0.store, row.id, &binding.recipe, client, events)
                .await,
        }
    }
}

fn now() -> i64 {
    crate::tensorlake::recipes::now_secs()
}
fn now_ms() -> i64 {
    now().saturating_mul(1000)
}

/// Only a completed nonzero engine exit can settle absent create/commit work.
/// SSH 255, signals and failed capture leave the remote outcome unknown.
fn definite_refusal(output: &CommandResult) -> bool {
    output.signal.is_none() && output.code.is_some_and(|code| code != 0 && code != 255)
}

/// Resume continues paused processes, but starts a stopped filesystem. The
/// distinction controls both the actual command and the changing-state wording;
/// a no-op never withdraws an already usable connection.
fn lifecycle_verb(status: &str, resume: bool) -> anyhow::Result<&'static str> {
    if resume {
        match status {
            "paused" => Ok("unpause"),
            "exited" | "stopped" | "created" => Ok("start"),
            "running" => Ok(""),
            _ => anyhow::bail!("the container is changing state; try again"),
        }
    } else {
        match status {
            "paused" => Ok(""),
            "running" => Ok("pause"),
            _ => anyhow::bail!("the container is not running; use Resume to start it"),
        }
    }
}

/// Container IDs are immutable authority; short-name aliases are not.
fn canonical(id: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        id.len() == 64
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "Podman returned a noncanonical container ID"
    );
    Ok(())
}

/// Podman versions may include the digest algorithm in commit output.
fn image_id(id: &str) -> anyhow::Result<()> {
    canonical(id.strip_prefix("sha256:").unwrap_or(id))
}

/// environment.d expands variables even inside quotes. Dollar doubling,
/// quoted newlines and escaped quotes/backslashes preserve literal values.
fn proxy_file(values: &BTreeMap<String, String>) -> String {
    values
        .iter()
        .map(|(name, value)| {
            format!(
                "{name}=\"{}\"\n",
                value
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('$', "$$")
            )
        })
        .collect()
}

/// The shown tail of a failed container setup script's error output, with
/// every proxy value the script was given masked.
///
/// A build step's error output is the only place it says why it stopped (the
/// base's missing requirement, which SPEC.md says the build names, or apt's
/// last lines), so it is shown. But the script ran with the container host's
/// proxies exported, and a tool can print them (curl and apt both do in some
/// failures), and a proxy URL may carry a user name and password. So each
/// non-empty value is masked whole, and so is its URL user information and
/// that part's password on their own, longest first, in case a tool prints
/// only the credentials. A password shorter than four characters is not
/// masked on its own: masking every occurrence of so short a string would
/// garble the message, and the whole value and user information still are.
/// This does not catch a value a tool prints transformed (percent-decoded,
/// say); the masking narrows what reaches the host's details, it does not
/// make error output safe to publish.
fn script_failure_output(stderr: &str, proxies: &BTreeMap<String, String>) -> String {
    const MASK: &str = "[proxy setting hidden]";
    let mut secrets: Vec<&str> = Vec::new();
    for value in proxies.values().filter(|value| !value.is_empty()) {
        secrets.push(value);
        let authority = value
            .split_once("://")
            .map_or(value.as_str(), |(_, rest)| rest);
        let authority = authority.split('/').next().unwrap_or(authority);
        if let Some((userinfo, _)) = authority.rsplit_once('@') {
            if !userinfo.is_empty() {
                secrets.push(userinfo);
            }
            if let Some((_, password)) = userinfo.split_once(':')
                && password.chars().count() >= 4
            {
                secrets.push(password);
            }
        }
    }
    secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    let mut masked = stderr.to_string();
    for secret in secrets {
        masked = masked.replace(secret, MASK);
    }
    crate::tensorlake::shell::shown(&masked)
}
