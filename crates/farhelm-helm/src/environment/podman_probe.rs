//! Place capabilities are cached observations, refreshed independently of UI reads.
//!
//! The complete result stays private because proxy values may hold credentials.
//! Public views expose readiness and its reason. A result belongs to the exact
//! current base route, so retargeting a host immediately invalidates its cache.
//!
//! Host entries carry each view's readiness (`container_check`), so a check's
//! start and end bump the fleet feed the hosts collection is re-read on.

use crate::access::{BaseRoute, RemoteCommand, base_route};
use crate::aggregate::host_display_name;
use crate::manager::{ConnectionManager, HostState};
use crate::store::{DialedAs, HelmStore, HostId, HostRow, PlaceClaim, PodmanPlaceIdentity};
use crate::units::{MANAGER_ENVIRONMENT_BUSCTL_ARGS, ManagerEnvironment, ManagerValue};
use anyhow::Context as _;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Launch consumers require a recent observation before using its tool path
/// and proxy inputs. Explicit Refresh ignores this age and only joins in-flight work.
const FRESH_FOR: Duration = Duration::from_secs(300);
/// Check the tools needed on this route before new work can depend on them.
/// Both identity fields come from one invocation.
pub(crate) const PLACE_IDENTITY_READ: &str = r#"
if command -v sha256sum >/dev/null && command -v tr >/dev/null; then
    cat /etc/machine-id && id -u
else
    printf '%s\n' farhelm-identity-tools-missing
fi
"#;
const IDENTITY_TOOLS_MISSING: &str = "This container host needs sha256sum and tr to verify its machine and account. Make these commands available on the host, then press Refresh.";
/// The command sees a single shell and a single SSH login. The private
/// marker is written only after a match, so an exit 1 before the guard runs
/// cannot be mistaken for Podman returning "absent". The expected operand
/// is a per-invocation digest so a moved SSH route does not see the old
/// machine ID.
const VERIFY_PLACE_AND_EXEC: &str = r#"
machine=$(cat /etc/machine-id 2>/dev/null) &&
    machine=$(printf '%s' "$machine" | tr 'ABCDEF' 'abcdef') &&
    uid=$(id -u 2>/dev/null) &&
    digest=$(printf '%s\n%s\n%s\n' "$2" "$machine" "$uid" | sha256sum) &&
    [ "${digest%% *}" = "$1" ] || exit 125
printf '%s\n' "$2" || exit 125
shift 2
exec "$@"
"#;
pub(super) const CONTAINER_PLACE_UNVERIFIED: &str = "Farhelm could not verify this container host's machine and account. Its SSH destination may have changed outside Farhelm. Restore the original destination, then try again.";
/// A single place's identity for command fakes that model one Podman store.
#[cfg(test)]
pub(crate) const TEST_PLACE_IDENTITY: &str = "fe001122aabbccddeeff001122aabbcc\n1000\n";
/// Describe a container command that never reached the host's place check,
/// keeping whatever the transport or the remote shell said about why.
fn container_host_unanswered(result: &crate::provisioning::CommandResult) -> String {
    let status = result.failure_status();
    let detail = result.stderr.trim();
    if detail.is_empty() {
        format!("Farhelm could not reach this container host ({status}).")
    } else {
        format!("Farhelm could not reach this container host ({status}): {detail}")
    }
}
const IDENTITY_UNREADABLE: &str = "Farhelm could not verify this container host's machine and account. Check its SSH destination and press Refresh.";
const PROXY_VARIABLES: [&str; 10] = [
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

/// Inputs used by the real provider. These never enter client JSON or logs.
#[derive(Clone)]
pub(crate) struct ReadyPlace {
    /// A failed identity check refuses admission while still allowing
    /// existing unguarded bindings to use the observed runtime.
    pub(crate) identity: Result<PodmanPlaceIdentity, &'static str>,
    pub(crate) architecture: String,
    pub(crate) version: String,
    #[allow(
        dead_code,
        reason = "the real provider consumes the observed executable next"
    )]
    pub(crate) podman: PathBuf,
    #[allow(
        dead_code,
        reason = "the real provider writes these exact values at creation"
    )]
    pub(crate) proxy: BTreeMap<String, String>,
}

/// A client-safe projection, including hosts never checked in this process.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ProbeView {
    pub(crate) checked_at: Option<u64>,
    pub(crate) refreshing: bool,
    pub(crate) usable: bool,
    pub(crate) reason: Option<String>,
    pub(crate) architecture: Option<String>,
    pub(crate) version: Option<String>,
}

impl ProbeView {
    /// The part of the view a host entry carries: readiness and its reason.
    /// Architecture and version stay on the per-host probe route, which the
    /// launcher reads when it needs them.
    pub(crate) fn container_check(&self) -> farhelm_proto::ui_channel::ContainerCheckView {
        farhelm_proto::ui_channel::ContainerCheckView {
            checked_at: self.checked_at,
            refreshing: self.refreshing,
            usable: self.usable,
            reason: self.reason.clone(),
        }
    }
}

/// A small command seam lets tests script each prerequisite without changing
/// PATH or any process environment, or touching a real user manager.
#[async_trait::async_trait]
pub(crate) trait PlaceCommands: Send + Sync {
    /// Probe text must come from a successful command within twenty seconds.
    /// Use the bounded result seam when nonzero statuses are meaningful data.
    async fn run(&self, route: &BaseRoute, argv: &[String]) -> anyhow::Result<String>;

    /// Lifecycle and image work needs the actual exit status and a bound
    /// appropriate to the operation. Probe-only fakes can retain their text
    /// seam; runtime fakes override this method to exercise nonzero exits.
    async fn run_bounded(
        &self,
        route: &BaseRoute,
        argv: &[String],
        timeout: Duration,
    ) -> anyhow::Result<crate::provisioning::CommandResult> {
        let stdout = tokio::time::timeout(timeout, self.run(route, argv)).await??;
        Ok(crate::provisioning::CommandResult {
            code: Some(0),
            signal: None,
            stdout: stdout.into_bytes(),
            stderr: String::new(),
        })
    }

    /// Trust a container command only after this invocation proves that the
    /// recorded endpoint was checked in its own shell. Keep argv positional
    /// and strip the proof before handing output to the Podman parser.
    async fn run_verified(
        &self,
        route: &BaseRoute,
        argv: &[String],
        identity: &PodmanPlaceIdentity,
        timeout: Duration,
    ) -> anyhow::Result<crate::provisioning::CommandResult> {
        let nonce = format!("farhelm-place-verified-{}", uuid::Uuid::new_v4().simple());
        let marker = format!("{nonce}\n");
        let mut wrapped = vec![
            "sh".into(),
            "-c".into(),
            VERIFY_PLACE_AND_EXEC.into(),
            "farhelm-container-place".into(),
            place_digest(identity, &nonce),
            nonce,
        ];
        wrapped.extend_from_slice(argv);
        let mut result = self.run_bounded(route, &wrapped, timeout).await?;
        if !result.stdout.starts_with(marker.as_bytes()) {
            // Without the proof line the command did not run. Exit 125 with
            // nothing printed is the guard itself: it ran and could not prove
            // the recorded machine and account (a different host, or one that
            // lacks what the check needs). Anything else, such as SSH failing
            // to connect to a host that is asleep, never reached the check, so
            // it is reported as the host not answering, with its own
            // diagnostic, rather than as a changed destination. Both stay
            // errors, so neither can be read as Podman's own answer.
            if result.code == Some(125) && result.stdout.is_empty() {
                anyhow::bail!("{CONTAINER_PLACE_UNVERIFIED}");
            }
            anyhow::bail!("{}", container_host_unanswered(&result));
        }
        result.stdout.drain(..marker.len());
        Ok(result)
    }

    /// Sensitive input shares ordinary argv lowering and bounded capture, but
    /// never becomes an argument at any shell or SSH layer. Fakes must opt in
    /// rather than silently dropping credential-bearing input.
    async fn run_with_input(
        &self,
        _route: &BaseRoute,
        _argv: &[String],
        _input: &[u8],
        _timeout: Duration,
    ) -> anyhow::Result<crate::provisioning::CommandResult> {
        anyhow::bail!("this place backend cannot send private command input")
    }
}

/// Bind the comparison to this command and the account without sending the
/// recorded machine ID to the endpoint being checked.
fn place_digest(identity: &PodmanPlaceIdentity, nonce: &str) -> String {
    let value = format!("{nonce}\n{}\n{}\n", identity.machine_id, identity.uid);
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Execute through the place's ordinary route while keeping the helm's private
/// variables, and those of any tmux the caller runs in, away from helpers that
/// Podman may leave running after the call.
struct SystemPlaceCommands {
    control_dir: PathBuf,
}

#[async_trait::async_trait]
impl PlaceCommands for SystemPlaceCommands {
    async fn run(&self, route: &BaseRoute, argv: &[String]) -> anyhow::Result<String> {
        let result = self
            .run_bounded(route, argv, Duration::from_secs(20))
            .await?;
        anyhow::ensure!(result.code == Some(0), "the place command was refused");
        String::from_utf8(result.stdout)
            .map_err(|_| anyhow::anyhow!("the place command returned invalid text"))
    }

    async fn run_bounded(
        &self,
        route: &BaseRoute,
        argv: &[String],
        timeout: Duration,
    ) -> anyhow::Result<crate::provisioning::CommandResult> {
        self.execute(route, argv, None, timeout).await
    }

    async fn run_with_input(
        &self,
        route: &BaseRoute,
        argv: &[String],
        input: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<crate::provisioning::CommandResult> {
        self.execute(route, argv, Some(input), timeout).await
    }
}

impl SystemPlaceCommands {
    /// Both input policies use the same route and scrubbing. Keeping this
    /// description separate lets tests inspect every actual argv layer without
    /// starting an SSH connection or a container.
    async fn process(
        &self,
        route: &BaseRoute,
        argv: &[String],
    ) -> anyhow::Result<crate::access::ProcessCommand> {
        // The remote login has its own environment too. Only validated variable
        // names are unset, and argv remains positional data through this shell.
        let mut clean = vec![
            "sh".into(),
            "-c".into(),
            crate::access::CLEAN_PLACE_EXEC.into(),
            "farhelm-place-command".into(),
        ];
        clean.extend_from_slice(argv);
        let mut process = route
            .command(&self.control_dir, RemoteCommand::Argv(&clean))
            .await?;
        if let BaseRoute::Ssh { .. } = route {
            // Quiet user config must not erase the mux refusal before a
            // failed fallback login. Other SSH sharing options stay intact.
            process
                .args
                .splice(0..0, ["-o".into(), "LogLevel=ERROR".into()]);
        }
        Ok(process)
    }

    /// Feed private bytes while the existing capture drains both output pipes.
    /// A bounded/cancelled command must also drop its feeder; a full stdin pipe
    /// cannot retain the secret or keep the operation alive after capture ends.
    async fn execute(
        &self,
        route: &BaseRoute,
        argv: &[String],
        input: Option<&[u8]>,
        timeout: Duration,
    ) -> anyhow::Result<crate::provisioning::CommandResult> {
        let mut command = self.process(route, argv).await?.command();
        command
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // A local probe can initialize shared Podman helpers. The helm's private
        // variables, and the caller's tmux ones, must not become those helpers'
        // inherited environment.
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("FARHELM_")
                || matches!(name.to_str(), Some("TMUX" | "TMUX_PANE" | "TMUX_TMPDIR"))
            {
                command.env_remove(name);
            }
        }
        crate::provisioning::isolate_process_group(&mut command);
        let mut child = command
            .spawn()
            .map_err(|_| anyhow::anyhow!("could not start the place command"))?;
        // As with the credential uploader, stdin is a byte stream with no
        // terminal. Cancellation drops this future and therefore its pipe too.
        // Move the pipe out before capture takes the child. The input future
        // owns no task, so cancellation cannot leave a detached writer behind.
        let stdin = child.stdin.take();
        let send = async move {
            if let (Some(input), Some(mut stdin)) = (input, stdin) {
                use tokio::io::AsyncWriteExt as _;
                stdin.write_all(input).await?;
                stdin.shutdown().await?;
            }
            Ok::<_, std::io::Error>(())
        };
        tokio::pin!(send);
        let capture = crate::provisioning::capture_child(child, timeout, "container place command");
        tokio::pin!(capture);
        let (result, sent) = tokio::select! {
            biased;
            sent = &mut send => (capture.await, sent),
            result = &mut capture => (result, Err(std::io::Error::other("command ended before private input completed"))),
        };
        let result =
            result.map_err(|_| anyhow::anyhow!("the place command failed or timed out"))?;
        if let BaseRoute::Ssh { destination, .. } = route
            && result.code == Some(255)
            && crate::transport::ssh_session_refused(result.stderr.as_bytes())
        {
            return Err(crate::transport::SessionLimitRefusal::at(destination).into());
        }
        sent.map_err(|_| anyhow::anyhow!("the place command did not receive its private input"))?;
        Ok(result)
    }
}

/// Provider tests inspect the exact system runner's complete process argv,
/// including SSH wrapping, without starting a process or using ambient PATH.
#[cfg(test)]
pub(super) async fn lowered_command_for_test(
    control_dir: &Path,
    route: &BaseRoute,
    argv: &[String],
) -> anyhow::Result<crate::access::ProcessCommand> {
    SystemPlaceCommands {
        control_dir: control_dir.into(),
    }
    .process(route, argv)
    .await
}

/// Model the shell's admission before passing argv to a scripted engine.
/// The production method still checks and removes the returned nonce.
#[cfg(test)]
pub(super) async fn simulate_verified_place<'a, F, Fut>(
    argv: &'a [String],
    observed: &str,
    execute: F,
) -> anyhow::Result<crate::provisioning::CommandResult>
where
    F: FnOnce(&'a [String]) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<crate::provisioning::CommandResult>>,
{
    if argv.get(2).map(String::as_str) != Some(VERIFY_PLACE_AND_EXEC) {
        return execute(argv).await;
    }
    let identity = parse_place_identity(observed).expect("a scripted endpoint has an identity");
    if argv[4] != place_digest(&identity, &argv[5]) {
        return Ok(crate::provisioning::CommandResult {
            code: Some(125),
            signal: None,
            stdout: Vec::new(),
            stderr: String::new(),
        });
    }
    let mut output = execute(&argv[6..]).await?;
    output.stdout.splice(..0, argv[5].bytes().chain(*b"\n"));
    Ok(output)
}

/// The stdin/temporary cleanup contract can be exercised without a container
/// engine. Only an owned local shell is started; the route and capture are real.
#[cfg(test)]
pub(super) async fn private_input_for_test(
    control_dir: &Path,
    argv: &[String],
    input: &[u8],
) -> anyhow::Result<crate::provisioning::CommandResult> {
    SystemPlaceCommands {
        control_dir: control_dir.into(),
    }
    .run_with_input(&BaseRoute::Local, argv, input, Duration::from_secs(10))
    .await
}

struct CachedProbe {
    route: BaseRoute,
    checked_at: u64,
    observed: Instant,
    result: Result<ReadyPlace, String>,
    /// A place can run commands for existing containers while refusing new work.
    admission_refusal: Option<String>,
}

#[derive(Default)]
struct Cache {
    probes: HashMap<HostId, CachedProbe>,
    /// The current task's permit. A retarget removes it so the next request
    /// starts a new probe even if the old route is still answering.
    refreshing: HashMap<HostId, Arc<()>>,
}

/// One helm's probe cache and in-flight admission. Refresh tasks retain the
/// service until they publish an observation, even if the requesting client goes away.
pub(crate) struct PodmanPlaces {
    commands: Arc<dyn PlaceCommands>,
    cache: Mutex<Cache>,
    /// Wakes the helm's own waiters ([`Self::runtime`], [`Self::fresh`]).
    changed: tokio::sync::Notify,
    /// Serialize claim writes with publishing their verdicts, so a probe
    /// cannot advertise a claim after another probe has transferred it.
    claim_publication: tokio::sync::Mutex<()>,
    /// Publish through the manager's feed and consult its ordinary host actors
    /// without keeping the manager alive. Before AppState adopts this cache,
    /// there is no GUI to wake. Its first snapshot reads the current check.
    manager: OnceLock<Weak<ConnectionManager>>,
}

impl PodmanPlaces {
    /// Production commands use this helm's ordinary shared SSH control directory.
    pub(crate) fn new(control_dir: &Path) -> Arc<Self> {
        Self::with_commands(Arc::new(SystemPlaceCommands {
            control_dir: control_dir.into(),
        }))
    }

    /// Tests inject command observations instead of process-global environment.
    pub(crate) fn with_commands(commands: Arc<dyn PlaceCommands>) -> Arc<Self> {
        Arc::new(Self {
            commands,
            cache: Mutex::new(Cache::default()),
            changed: tokio::sync::Notify::new(),
            claim_publication: tokio::sync::Mutex::new(()),
            manager: OnceLock::new(),
        })
    }

    /// Attach to the helm that owns these checks. One cache serves one helm,
    /// so a second AppState over the same delivery service cannot redirect it.
    pub(crate) fn publish_to(self: &Arc<Self>, manager: &Arc<ConnectionManager>) {
        if self.manager.set(Arc::downgrade(manager)).is_ok() {
            manager.set_podman_places(Arc::downgrade(self));
        }
    }

    /// Adoption holds this across the registry commit, probe invalidation
    /// and withdrawal of the old actor state.
    pub(crate) async fn lock_claim_publication(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.claim_publication.lock().await
    }

    /// Bump the feed for a change a host entry shows. Callers hold no cache
    /// lock: the re-read this wakes takes it.
    fn announce(&self) {
        if let Some(manager) = self.manager.get().and_then(Weak::upgrade) {
            manager.events().bump();
        }
    }

    /// Release a place claim and invalidate its probe before publishing the
    /// registry edit. A probe in flight during the change cannot reclaim the
    /// old endpoint if the entry is edited back to the same destination.
    pub(crate) async fn retarget(
        &self,
        store: &HelmStore,
        host: HostId,
        destination: &str,
    ) -> anyhow::Result<()> {
        let _published = self.claim_publication.lock().await;
        if store
            .update_ssh_destination_changed(host, destination)
            .await?
        {
            self.invalidate_check(host);
        }
        Ok(())
    }

    /// Serializes with probe publication just like a destination edit. Both can change the endpoint.
    pub(crate) async fn set_ssh_tailscale(
        &self,
        store: &HelmStore,
        row: &HostRow,
        tailscale: bool,
    ) -> anyhow::Result<()> {
        let _published = self.claim_publication.lock().await;
        if store.set_ssh_tailscale(row, tailscale).await? {
            self.invalidate_check(row.id);
        }
        Ok(())
    }

    /// Invalidate cached and pending checks under the claim-publication lock.
    pub(crate) fn invalidate_check(&self, host: HostId) {
        let mut cache = self.cache.lock().unwrap();
        cache.probes.remove(&host);
        cache.refreshing.remove(&host);
        self.changed.notify_waiters();
    }

    /// Read locally without waiting on a host. A changed endpoint has no current
    /// result, regardless of the timestamp of the previous endpoint's probe.
    pub(crate) fn view(&self, row: &HostRow) -> anyhow::Result<ProbeView> {
        let route = base_route(row)?;
        let ordinary_state = self.ordinary_state(row);
        let ordinary_refusal = ordinary_state.as_ref().and_then(ordinary_identity_refusal);
        let cache = self.cache.lock().unwrap();
        // The registry can show a new installation before Adopt clears the
        // old check. The old actor cannot vouch for a snapshot of that row.
        let current_actor = ordinary_state.is_some() || self.manager.get().is_none();
        let probe = cache
            .probes
            .get(&row.id)
            .filter(|probe| current_actor && probe.route == route);
        let ready = probe.and_then(|probe| probe.result.as_ref().ok());
        let reason = ordinary_refusal.map(str::to_owned).or_else(|| {
            probe.and_then(|probe| {
                probe
                    .result
                    .as_ref()
                    .err()
                    .cloned()
                    .or_else(|| probe.admission_refusal.clone())
            })
        });
        Ok(ProbeView {
            checked_at: probe.map(|probe| probe.checked_at),
            refreshing: cache.refreshing.contains_key(&row.id),
            usable: ready.is_some() && reason.is_none(),
            reason,
            architecture: ready.map(|ready| ready.architecture.clone()),
            version: ready.map(|ready| ready.version.clone()),
        })
    }

    /// Preparation consumes exactly the place observation that passed the probe.
    /// Stale results and retargeted endpoints require a background refresh first.
    pub(crate) fn ready(&self, row: &HostRow) -> anyhow::Result<ReadyPlace> {
        let route = base_route(row)?;
        let ordinary_state = self.ordinary_state(row);
        let ordinary_refusal = ordinary_state.as_ref().and_then(ordinary_identity_refusal);
        let cache = self.cache.lock().unwrap();
        let probe = cache
            .probes
            .get(&row.id)
            .filter(|probe| probe.route == route && probe.observed.elapsed() <= FRESH_FOR)
            .context("the container host needs a fresh Podman check")?;
        let ready = probe.result.clone().map_err(anyhow::Error::msg)?;
        if let Some(reason) = ordinary_refusal.or(probe.admission_refusal.as_deref()) {
            anyhow::bail!("{reason}");
        }
        Ok(ready)
    }

    /// Orphan discovery scans already observed capabilities without scheduling
    /// new checks or treating new-work refusal as loss of command access.
    pub(super) fn runtime_observed(&self, row: &HostRow) -> anyhow::Result<bool> {
        let route = base_route(row)?;
        Ok(self
            .cache
            .lock()
            .unwrap()
            .probes
            .get(&row.id)
            .is_some_and(|probe| probe.route == route && probe.result.is_ok()))
    }

    /// A host that runs Podman without a supervisor uses its place claim alone.
    /// An actor's refusal applies only to the registry entry it observed.
    fn ordinary_state(&self, row: &HostRow) -> Option<HostState> {
        self.manager.get()?.upgrade()?.state_for_registry_row(row)
    }

    /// Read the incumbent only on a collision. Its duplicate hello can
    /// identify the ordinary owner even when that owner's supervisor is
    /// unreachable. Any other refusal needs the challenger's own live hello
    /// before the store rechecks both rows and can transfer an unused claim.
    /// The initial collision keeps the prior claim until that decision so a
    /// resource registered between the two writes keeps its place.
    async fn settle_claim(
        &self,
        store: &HelmStore,
        row: &HostRow,
        identity: &PodmanPlaceIdentity,
        verified_supervisor: Option<String>,
    ) -> anyhow::Result<Option<PlaceClaim>> {
        let claim = store
            .try_claim_podman_place(row, identity.clone(), verified_supervisor.clone())
            .await?;
        let Some(PlaceClaim::Twin { owner, .. }) = &claim else {
            return Ok(claim);
        };
        let incumbent = current_row(store, *owner).await.ok();
        let verified = incumbent.as_ref().and_then(|incumbent| {
            match self.ordinary_state(incumbent) {
                Some(HostState::Duplicate { twin, identity }) if twin == row.id => Some(identity),
                // These refusals can show that the incumbent is barred from new work.
                // The challenger needs its own verified hello to take the claim.
                Some(
                    HostState::Duplicate { .. }
                    | HostState::IdentityMismatch { .. }
                    | HostState::IdentityUnverified { .. },
                ) => verified_supervisor.clone(),
                _ => None,
            }
        });
        let refused = incumbent.as_ref().filter(|_| verified.is_some());
        store
            .claim_podman_place(
                row,
                identity.clone(),
                verified.or(verified_supervisor),
                refused,
            )
            .await
    }

    /// Reconnection cannot depend on somebody opening the launcher. Runtime
    /// accepts an older success for the current endpoint, and refreshes only
    /// this place on a miss. The refresh survives a timed-out actor attempt.
    pub(crate) async fn runtime(
        self: &Arc<Self>,
        store: &HelmStore,
        host: HostId,
    ) -> anyhow::Result<(BaseRoute, ReadyPlace)> {
        tokio::time::timeout(Duration::from_secs(30), async {
            let mut requested = false;
            loop {
                let changed = self.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                let row = current_row(store, host).await?;
                let route = base_route(&row)?;
                let cached = {
                    self.cache
                        .lock()
                        .unwrap()
                        .probes
                        .get(&host)
                        .filter(|probe| probe.route == route)
                        .map(|probe| probe.result.clone())
                };
                if let Some(result) = cached {
                    match result {
                        Ok(ready) => return Ok((route, ready)),
                        Err(reason)
                            if requested
                                && !self.cache.lock().unwrap().refreshing.contains_key(&host) =>
                        {
                            return Err(anyhow::Error::msg(reason));
                        }
                        Err(_) => {}
                    }
                }
                self.refresh(store.clone(), host).await?;
                requested = true;
                changed.await;
            }
        })
        .await
        .context("the container host's Podman check did not finish in time")?
    }

    /// Launch/preparation needs recent capabilities. Refresh one stale place
    /// and join its current task without waiting on any unrelated host.
    pub(crate) async fn fresh(
        self: &Arc<Self>,
        store: &HelmStore,
        host: HostId,
    ) -> anyhow::Result<ReadyPlace> {
        if let Ok(ready) = self.confirm_cached_admission(store, host).await {
            return Ok(ready);
        }
        tokio::time::timeout(Duration::from_secs(30), async {
            self.refresh(store.clone(), host).await?;
            loop {
                let changed = self.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                if !self.cache.lock().unwrap().refreshing.contains_key(&host) {
                    return self.confirm_cached_admission(store, host).await;
                }
                changed.await;
            }
        })
        .await
        .context("the container host's fresh Podman check did not finish in time")?
    }

    /// An explicit route edit releases the claim in storage. Even if the
    /// route later returns to the same text, only a new probe can claim it.
    async fn confirm_cached_admission(
        &self,
        store: &HelmStore,
        host: HostId,
    ) -> anyhow::Result<ReadyPlace> {
        let _published = self.claim_publication.lock().await;
        let row = current_row(store, host).await?;
        let ready = self.ready(&row)?;
        let identity = ready.identity.clone().map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            store.owns_podman_place(row, identity).await?,
            "{CLAIM_NEEDS_REFRESH}"
        );
        Ok(ready)
    }

    /// A new session container inherits the place's environment at setup.
    /// Cached capabilities can outlive a change to the manager's settings.
    pub(super) async fn creation_proxy(
        &self,
        store: &HelmStore,
        host: HostId,
    ) -> anyhow::Result<BTreeMap<String, String>> {
        let route = base_route(&current_row(store, host).await?)?;
        read_proxy(self.commands.as_ref(), &route).await
    }

    /// All provider commands retain the probe's lowering, scrub and capture
    /// contract, with operation-specific bounds and truthful exit statuses.
    pub(crate) async fn command(
        &self,
        route: &BaseRoute,
        argv: &[String],
        identity: Option<&PodmanPlaceIdentity>,
        timeout: Duration,
    ) -> anyhow::Result<crate::provisioning::CommandResult> {
        match identity {
            Some(identity) => {
                self.commands
                    .run_verified(route, argv, identity, timeout)
                    .await
            }
            None => self.commands.run_bounded(route, argv, timeout).await,
        }
    }

    /// Private script bytes never enter the command description. Callers keep
    /// the same place route, output bounds and cleanup ownership as commands.
    pub(crate) async fn command_with_input(
        &self,
        route: &BaseRoute,
        argv: &[String],
        input: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<crate::provisioning::CommandResult> {
        self.commands
            .run_with_input(route, argv, input, timeout)
            .await
    }

    /// Schedule only this host and return immediately. The current row is read
    /// again before execution, and a stale observation cannot replace a result
    /// for a different route. Requests join the current check for a host.
    pub(crate) async fn refresh(
        self: &Arc<Self>,
        store: HelmStore,
        host: HostId,
    ) -> anyhow::Result<()> {
        let row = current_row(&store, host).await?;
        base_route(&row)?;
        let permit = {
            let mut cache = self.cache.lock().unwrap();
            let std::collections::hash_map::Entry::Vacant(slot) = cache.refreshing.entry(host)
            else {
                return Ok(());
            };
            let permit = Arc::new(());
            slot.insert(Arc::clone(&permit));
            permit
        };
        // Only an admitted check changes the view (`refreshing` turns on); a
        // joined one changes nothing and so announces nothing.
        self.announce();
        let service = Arc::clone(self);
        tokio::spawn(async move {
            let refreshing = Refreshing {
                service: Arc::clone(&service),
                host,
                permit,
            };
            let Ok(row) = current_row(&store, host).await else {
                return;
            };
            // A place check can start its own shared SSH connection. Wait for
            // a route edit, then check that this probe still owns its permit.
            let lock = service
                .manager
                .get()
                .and_then(Weak::upgrade)
                .map(|manager| manager.ssh_route_lock(host));
            let _routing = match (row.kind, lock) {
                (crate::store::HostKind::Ssh, Some(lock)) => Some(lock.read_owned().await),
                _ => None,
            };
            if !refreshing.is_current(&service.cache.lock().unwrap()) {
                return;
            }
            let Ok(row) = current_row(&store, host).await else {
                return;
            };
            let Ok(route) = base_route(&row) else { return };
            let dialed_as = DialedAs::of(&row);
            let result = probe(service.commands.as_ref(), &route).await;
            let _published = service.claim_publication.lock().await;
            if !refreshing.is_current(&service.cache.lock().unwrap()) {
                return;
            }
            let Ok(current) = current_row(&store, host).await else {
                return;
            };
            // Setup may have changed the supervisor's install coordinates
            // without changing the Podman route. That actor's result cannot
            // settle a claim for a newly configured entry.
            if row.entry_uuid != current.entry_uuid || dialed_as != DialedAs::of(&current) {
                return;
            }
            let row = current;
            let ordinary_state = service.ordinary_state(&row);
            // Setup can save the new paths before the manager replaces the
            // old actor. A matching Unreachable actor is enough for a
            // Podman-only host. An actor for different settings says nothing
            // about the host that would receive this claim.
            if ordinary_state.is_none() && service.manager.get().is_some() {
                return;
            }
            let ordinary_refusal = ordinary_state.as_ref().and_then(ordinary_identity_refusal);
            let verified_supervisor = match ordinary_state {
                Some(HostState::Connected { identity, .. }) => identity,
                _ => None,
            };
            let (admission_refusal, displaced) = match (&result, ordinary_refusal) {
                (Ok(_), Some(_)) => (Some(CLAIM_NEEDS_REFRESH.into()), None),
                (Ok(ReadyPlace { identity: Ok(identity), .. }), None) => match service.settle_claim(&store, &row, identity, verified_supervisor).await {
                    Ok(Some(PlaceClaim::Owned { displaced })) => (None, displaced),
                    Ok(Some(PlaceClaim::Twin { name, .. })) => (Some(twin_refusal(&name)), None),
                    Ok(Some(PlaceClaim::Changed)) => (Some(CHANGED_PLACE.into()), None),
                    Ok(None) => return,
                    Err(_) => (
                        Some("Farhelm could not record this container host's machine and account. Press Refresh to try again.".into()),
                        None,
                    ),
                },
                (Ok(ReadyPlace { identity: Err(reason), .. }), None) => (Some((*reason).into()), None),
                (Err(_), _) => (None, None),
            };
            // Publication is route-bound too. A host edited during the command
            // chain receives no stale success for its new endpoint.
            if let Some(displaced) = displaced {
                let mut cache = service.cache.lock().unwrap();
                if let Some(previous) = cache.probes.get_mut(&displaced) {
                    let name = host_display_name(
                        row.kind,
                        row.destination.as_deref(),
                        row.alias.as_deref(),
                    );
                    previous.admission_refusal = Some(twin_refusal(&name));
                }
            }
            if let Ok(current) = current_row(&store, host).await
                && row.entry_uuid == current.entry_uuid
                && dialed_as == DialedAs::of(&current)
                && base_route(&current).ok().as_ref() == Some(&route)
            {
                service.cache.lock().unwrap().probes.insert(
                    host,
                    CachedProbe {
                        route,
                        checked_at: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                        observed: Instant::now(),
                        result,
                        admission_refusal,
                    },
                );
            }
        });
        Ok(())
    }
}

const CHANGED_PLACE: &str = "This container host now reaches a different machine or account. Its SSH destination may have changed outside Farhelm. Restore it and press Refresh before starting containers.";
const CLAIM_NEEDS_REFRESH: &str = "This container host needs a fresh Podman check. Press Refresh.";

fn ordinary_identity_refusal(state: &HostState) -> Option<&'static str> {
    match state {
        HostState::Duplicate { .. } => Some(
            "Farhelm already connects to this installation through another host. Use that host for new containers.",
        ),
        HostState::IdentityMismatch { .. } => Some(
            "This host now reaches a different Farhelm installation. Check its SSH destination or adopt the new installation from Hosts before starting containers.",
        ),
        HostState::IdentityUnverified { .. } => Some(
            "Farhelm could not verify the installation on this host. Check its SSH destination and press Retry in Hosts before starting containers.",
        ),
        _ => None,
    }
}

/// Give the user the winning host and how to resolve two entries that reach
/// one Podman store, including the common case of a copied machine.
fn twin_refusal(name: &str) -> String {
    format!(
        "This host reaches the same machine and account as {name}, so new containers go through {name}. If these are different machines, a copied machine may still have the original machine ID. Give the copy a new machine ID and press Refresh. Otherwise, remove one entry or change its destination."
    )
}

/// A cancelled or panicking refresh must release its in-flight admission too.
///
/// Its drop is also where a check's end is announced, after the result (if
/// any) is stored and `refreshing` is off. Announcing at the store instead
/// would let the woken re-read see the new result still marked refreshing,
/// with nothing left to announce the flag clearing. A discarded task must
/// leave the slot alone if another check has already taken it.
struct Refreshing {
    service: Arc<PodmanPlaces>,
    host: HostId,
    permit: Arc<()>,
}

impl Refreshing {
    /// Compare permit identity so a task from an earlier route can neither
    /// publish nor clear the current task's slot.
    fn is_current(&self, cache: &Cache) -> bool {
        cache
            .refreshing
            .get(&self.host)
            .is_some_and(|permit| Arc::ptr_eq(permit, &self.permit))
    }
}

impl Drop for Refreshing {
    fn drop(&mut self) {
        {
            let mut cache = self.service.cache.lock().unwrap();
            if !self.is_current(&cache) {
                return;
            }
            cache.refreshing.remove(&self.host);
        }
        self.service.changed.notify_waiters();
        self.service.announce();
    }
}

/// Resolve the registry entry again at each asynchronous phase boundary.
/// The numeric key never authorizes reusing an earlier endpoint snapshot.
async fn current_row(store: &HelmStore, host: HostId) -> anyhow::Result<HostRow> {
    store
        .list_hosts()
        .await?
        .into_iter()
        .find(|row| row.id == host)
        .context("the container host is no longer registered")
}

/// Check prerequisites in dependency order so every failure has a specific,
/// bounded diagnosis. Raw command errors and environment values stay private.
async fn probe(commands: &dyn PlaceCommands, route: &BaseRoute) -> Result<ReadyPlace, String> {
    probe_inner(commands, route)
        .await
        .map_err(|error| error.to_string())
}

/// Run the production local capability check with a test's private Podman
/// arguments. Even `podman info` can initialize or refresh a Podman store.
#[cfg(all(target_os = "linux", feature = "test-seams"))]
pub async fn probe_local_podman_with_test_storage(
    control_dir: &Path,
    storage_args: &[String],
) -> Result<String, String> {
    struct OwnedStoreProbe<'a> {
        native: SystemPlaceCommands,
        storage_args: &'a [String],
    }

    #[async_trait::async_trait]
    impl PlaceCommands for OwnedStoreProbe<'_> {
        async fn run(&self, route: &BaseRoute, argv: &[String]) -> anyhow::Result<String> {
            if argv.get(1).is_some_and(|arg| arg == "info") {
                let mut args = argv.to_vec();
                args.splice(1..1, self.storage_args.iter().cloned());
                self.native.run(route, &args).await
            } else {
                self.native.run(route, argv).await
            }
        }
    }

    let commands = OwnedStoreProbe {
        native: SystemPlaceCommands {
            control_dir: control_dir.to_owned(),
        },
        storage_args,
    };
    probe(&commands, &BaseRoute::Local)
        .await
        .map(|ready| ready.version)
}

/// Refusals identify the failed prerequisite and discard raw command errors.
/// Manager output can contain proxy credentials, so none reaches public reasons.
async fn run(
    commands: &dyn PlaceCommands,
    route: &BaseRoute,
    argv: &[&str],
    reason: &'static str,
) -> anyhow::Result<String> {
    commands
        .run(
            route,
            &argv
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>(),
        )
        .await
        .map_err(|error| {
            if error.is::<crate::transport::SessionLimitRefusal>() {
                error
            } else {
                anyhow::anyhow!(reason)
            }
        })
}

/// Retain the exact executable and manager environment from this observation.
/// These inputs belong together and expire together before preparation consumes them.
async fn probe_inner(
    commands: &dyn PlaceCommands,
    route: &BaseRoute,
) -> anyhow::Result<ReadyPlace> {
    let os = run(
        commands,
        route,
        &["uname", "-s"],
        "could not identify the container host's operating system",
    )
    .await?;
    anyhow::ensure!(os.trim() == "Linux", "containers require a Linux host");
    let architecture = run(
        commands,
        route,
        &["uname", "-m"],
        "could not identify the container host's architecture",
    )
    .await?;
    let architecture = architecture.trim().to_string();
    anyhow::ensure!(
        matches!(architecture.as_str(), "x86_64" | "aarch64"),
        "the container host's architecture is unsupported"
    );
    let path = run(
        commands,
        route,
        &["sh", "-c", "command -v podman"],
        "Podman is not installed on this host",
    )
    .await?;
    let podman = PathBuf::from(path.trim());
    anyhow::ensure!(
        podman.is_absolute() && !path.trim().contains('\n'),
        "Podman did not resolve to an absolute executable path"
    );
    let info = run(
        commands,
        route,
        &[
            podman
                .to_str()
                .context("Podman's executable path is not text")?,
            "info",
            "--format=json",
        ],
        "Podman could not report its rootless configuration",
    )
    .await?;
    let version = validate_info(&info)?;
    let proxy = read_proxy(commands, route).await?;
    let unit = format!("farhelm-env-probe-{}.scope", uuid::Uuid::new_v4().simple());
    let scope = run(
        commands,
        route,
        &[
            "systemd-run",
            "--user",
            "--scope",
            "--collect",
            "--quiet",
            "--unit",
            &unit,
            "--",
            "true",
        ],
        "the container host cannot run a systemd user scope",
    )
    .await;
    // This nonce belongs only to this attempt. Stop is safe after a normal
    // collected exit and also cleans up a partially failed scope invocation.
    let _ = run(
        commands,
        route,
        &["systemctl", "--user", "stop", &unit],
        "could not clean up the probe scope",
    )
    .await;
    scope?;
    // Existing bindings can still need command access when the identity probe
    // fails. Only admission of new work requires a successful identity read.
    let observed = run(
        commands,
        route,
        &["sh", "-c", PLACE_IDENTITY_READ],
        IDENTITY_UNREADABLE,
    )
    .await;
    let identity = match observed {
        Ok(output) if output.trim_end() == "farhelm-identity-tools-missing" => {
            Err(IDENTITY_TOOLS_MISSING)
        }
        Ok(output) => parse_place_identity(&output).ok_or(IDENTITY_UNREADABLE),
        Err(_) => Err(IDENTITY_UNREADABLE),
    };
    Ok(ReadyPlace {
        identity,
        architecture,
        version,
        podman,
        proxy,
    })
}

/// Machine IDs are case-insensitive hex. Reject empty, uninitialized and
/// malformed results without including either raw identity field in errors.
pub(super) fn parse_place_identity(output: &str) -> Option<PodmanPlaceIdentity> {
    let mut lines = output.lines();
    let machine_id = lines.next()?;
    let uid = lines.next()?;
    if lines.next().is_some() || uid.is_empty() || !uid.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    PodmanPlaceIdentity::from_parts(machine_id, uid.parse().ok()?)
}

/// Keep exact proxy values, including empty ones, private to the caller.
/// The systemctl fallback supports hosts without structured busctl output.
async fn read_proxy(
    commands: &dyn PlaceCommands,
    route: &BaseRoute,
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut busctl = vec!["busctl".to_string(), "--user".into()];
    busctl.extend(MANAGER_ENVIRONMENT_BUSCTL_ARGS.map(str::to_string));
    let environment = match commands
        .run(route, &busctl)
        .await
        .ok()
        .and_then(|text| ManagerEnvironment::from_busctl_json(&text).ok())
    {
        Some(environment) => environment,
        None => {
            let text = run(
                commands,
                route,
                &["systemctl", "--user", "show-environment"],
                "could not read the container host's user manager environment",
            )
            .await?;
            ManagerEnvironment::from_show_environment_exact(&text).map_err(|_| {
                anyhow::anyhow!("the user manager returned an unreadable environment")
            })?
        }
    };
    let mut proxy = BTreeMap::new();
    for name in PROXY_VARIABLES {
        match environment.get_exact(name) {
            ManagerValue::Absent => {}
            ManagerValue::Value(value) => {
                proxy.insert(name.to_string(), value);
            }
            ManagerValue::Unreadable => {
                anyhow::bail!("the user manager returned an unreadable proxy value")
            }
        }
    }
    Ok(proxy)
}

/// Reject unknown configuration instead of inferring rootless/systemd from a
/// successful Podman invocation. The existing semantic-version parser also
/// handles build metadata without letting a 4.9.3 prerelease pass the floor.
fn validate_info(text: &str) -> anyhow::Result<String> {
    let info: serde_json::Value = serde_json::from_str(text)
        .map_err(|_| anyhow::anyhow!("Podman returned unreadable configuration"))?;
    let host = &info["host"];
    anyhow::ensure!(
        host["security"]["rootless"].as_bool() == Some(true),
        "Podman is not rootless"
    );
    anyhow::ensure!(
        host["cgroupVersion"].as_str() == Some("v2"),
        "Podman requires cgroup v2"
    );
    anyhow::ensure!(
        host["cgroupManager"].as_str() == Some("systemd"),
        "Podman requires the systemd cgroup manager"
    );
    let version = info["version"]["Version"]
        .as_str()
        .context("Podman did not report its version")?;
    let parsed = semver::Version::parse(version)
        .map_err(|_| anyhow::anyhow!("Podman reported an unreadable version"))?;
    anyhow::ensure!(
        parsed >= semver::Version::new(4, 9, 3),
        "Podman {version} is too old; version 4.9.3 or later is required"
    );
    Ok(version.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Manager environment commands may contain secrets in their failures,
    /// but a locally classified mux refusal must still identify the place.
    /// The distinction is typed rather than inferred from untrusted text.
    #[farhelm_testtrace::test]
    async fn probe_redacts_peer_errors_but_keeps_typed_session_limit_reason() {
        let route = BaseRoute::Ssh {
            destination: "place.example".into(),
            access: crate::ssh::SshAccessKind::UserConfig,
        };
        let commands = FakeCommands {
            replies: Mutex::new(VecDeque::from([
                (
                    "uname".into(),
                    Err(crate::transport::SessionLimitRefusal::at("place.example").into()),
                ),
                (
                    "uname".into(),
                    Err(anyhow::anyhow!("secret proxy credentials")),
                ),
            ])),
            seen: Mutex::new(Vec::new()),
        };
        let limit = probe(&commands, &route)
            .await
            .err()
            .expect("the command must fail");
        assert!(
            limit.contains("place.example") && limit.contains("SSH session limit"),
            "{limit}"
        );
        let redacted = probe(&commands, &route)
            .await
            .err()
            .expect("the command must fail");
        assert_eq!(
            redacted,
            "could not identify the container host's operating system"
        );
        assert!(commands.replies.lock().unwrap().is_empty());
    }

    /// Each answer belongs to one expected command. An unexpected command
    /// fails the fixture instead of silently obtaining a successful response.
    struct FakeCommands {
        replies: Mutex<VecDeque<(String, anyhow::Result<String>)>>,
        seen: Mutex<Vec<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl PlaceCommands for FakeCommands {
        async fn run(&self, _route: &BaseRoute, argv: &[String]) -> anyhow::Result<String> {
            self.seen.lock().unwrap().push(argv.to_vec());
            let (program, reply) = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected command");
            assert_eq!(argv[0], program);
            reply
        }
    }

    /// A successful capability check with a caller-chosen identity answer.
    /// Each requested answer is consumed only after the preceding probe ran.
    fn scripted_places(identities: &[&str]) -> Arc<FakeCommands> {
        let replies = identities.iter().flat_map(|identity| {
            [
                ("uname", "Linux"),
                ("uname", "x86_64"),
                ("sh", "/usr/bin/podman"),
                ("/usr/bin/podman", r#"{"version":{"Version":"4.9.3"},"host":{"security":{"rootless":true},"cgroupVersion":"v2","cgroupManager":"systemd"}}"#),
                ("busctl", r#"{"type":"as","data":[]}"#),
                ("systemd-run", ""),
                ("systemctl", ""),
                ("sh", *identity),
            ]
            .map(|(program, reply)| (program.into(), Ok(reply.into())))
        });
        Arc::new(FakeCommands {
            replies: Mutex::new(replies.collect()),
            seen: Mutex::new(Vec::new()),
        })
    }

    /// Duplicate place entries refuse new work. Their cached capability is
    /// still usable to check a container bound before it became a twin.
    #[farhelm_testtrace::test]
    async fn twin_admission_does_not_take_away_runtime_capabilities() {
        let directory = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&directory.path().join("helm.db"))
            .await
            .unwrap();
        let first = store
            .add_ssh_host("place-one.test", None, None)
            .await
            .unwrap();
        let second = store
            .add_ssh_host("place-two.test", None, None)
            .await
            .unwrap();
        let places = PodmanPlaces::with_commands(scripted_places(&[
            TEST_PLACE_IDENTITY,
            TEST_PLACE_IDENTITY,
        ]));

        assert!(places.fresh(&store, first).await.is_ok());
        let refusal = places
            .fresh(&store, second)
            .await
            .err()
            .expect("twin must refuse new work");
        assert!(
            refusal
                .to_string()
                .contains("same machine and account as place-one.test")
        );
        let second_row = current_row(&store, second).await.unwrap();
        assert!(!places.view(&second_row).unwrap().usable);
        let (_, ready) = places.runtime(&store, second).await.unwrap();
        assert_eq!(ready.podman, PathBuf::from("/usr/bin/podman"));
    }

    /// An ordinary supervisor can refuse its registered identity even when
    /// the Podman endpoint is otherwise usable and has no competing claim.
    #[farhelm_testtrace::test]
    async fn ordinary_identity_mismatch_refuses_container_admission() {
        use crate::rest_harness::{FleetBuilder, HostScript};
        use crate::store::DialedAs;

        let builder = FleetBuilder::new()
            .await
            .place_commands(scripted_places(&[TEST_PLACE_IDENTITY, TEST_PLACE_IDENTITY]));
        let (builder, usable) = builder
            .ssh(
                "place-without-supervisor.test",
                HostScript {
                    reachable: false,
                    ..HostScript::default()
                },
            )
            .await;
        let (builder, host) = builder
            .ssh(
                "place-reinstalled.test",
                HostScript {
                    identity: Some("new-install".into()),
                    ..HostScript::default()
                },
            )
            .await;
        let row = current_row(builder.store(), host).await.unwrap();
        builder
            .store()
            .record_first_contact(host, &DialedAs::of(&row), "previous-install")
            .await
            .unwrap();
        let harness = builder.start().await;
        harness
            .await_state(host, |state| {
                matches!(state, HostState::IdentityMismatch { .. })
            })
            .await;

        let places = &harness.state.podman_places;
        assert!(places.fresh(&harness.store, usable).await.is_ok());
        let refusal = places
            .fresh(&harness.store, host)
            .await
            .err()
            .expect("identity mismatch must block new work");
        assert!(
            refusal
                .to_string()
                .contains("different Farhelm installation")
        );
        assert!(!places.view(&row).unwrap().usable);
        assert!(places.runtime(&harness.store, host).await.is_ok());
        assert!(
            places.fresh(&harness.store, usable).await.is_ok(),
            "the refused actor did not displace the usable place"
        );
    }

    /// Adoption changes the accepted supervisor without changing the route.
    /// The previous installation's unused place claim and check must expire
    /// at that boundary even when no one has asked to check the new host.
    #[farhelm_testtrace::test]
    async fn adoption_clears_the_previous_podman_check_and_unused_claim() {
        use crate::rest_harness::{FleetBuilder, HostScript};

        let builder = FleetBuilder::new()
            .await
            .place_commands(scripted_places(&[TEST_PLACE_IDENTITY]));
        let (builder, host) = builder
            .ssh(
                "place.example",
                HostScript {
                    identity: Some("before".into()),
                    ..HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_state(host, HostState::is_connected).await;
        let places = &harness.state.podman_places;
        assert!(places.fresh(&harness.store, host).await.is_ok());

        harness
            .fleet
            .edit(host, |script| script.identity = Some("after".into()));
        harness.manager.retry_now(host).await.unwrap();
        harness
            .await_state(host, |state| {
                matches!(state, HostState::IdentityMismatch { .. })
            })
            .await;
        let before = current_row(&harness.store, host).await.unwrap();
        assert!(places.view(&before).unwrap().checked_at.is_some());
        assert!(
            harness
                .store
                .podman_place_identity(host)
                .await
                .unwrap()
                .is_some()
        );
        let mut adopted_row = before.clone();
        adopted_row.host_identity = Some("after".into());
        assert!(
            harness
                .manager
                .state_for_registry_row(&adopted_row)
                .is_none(),
            "a late refusal about the old installation cannot settle a check for the adopted row",
        );
        assert_eq!(places.view(&adopted_row).unwrap().checked_at, None);

        harness.manager.adopt(host, "after").await.unwrap();
        let after = current_row(&harness.store, host).await.unwrap();
        assert_eq!(after.host_identity.as_deref(), Some("after"));
        assert_eq!(places.view(&after).unwrap().checked_at, None);
        assert_eq!(
            harness.store.podman_place_identity(host).await.unwrap(),
            None
        );
    }

    /// A reinstall can leave an unused place claim on the old host entry
    /// while another entry connects to the new installation. The stale
    /// ordinary claim must not prevent both entries from settling on a
    /// single usable place.
    #[farhelm_testtrace::test]
    async fn verified_supervisor_takes_unused_place_from_refused_incumbent() {
        use crate::rest_harness::{FleetBuilder, HostScript};
        use crate::store::DialedAs;

        let builder = FleetBuilder::new()
            .await
            .place_commands(scripted_places(&[TEST_PLACE_IDENTITY, TEST_PLACE_IDENTITY]));
        let reinstalled = |reachable| HostScript {
            reachable,
            identity: Some("new-install".into()),
            ..HostScript::default()
        };
        let (builder, former) = builder.ssh("former-place.test", reinstalled(true)).await;
        let (builder, next) = builder.ssh("next-place.test", reinstalled(false)).await;
        let row = current_row(builder.store(), former).await.unwrap();
        let identity = parse_place_identity(TEST_PLACE_IDENTITY).unwrap();
        builder
            .store()
            .record_first_contact(former, &DialedAs::of(&row), "previous-install")
            .await
            .unwrap();
        assert_eq!(
            builder
                .store()
                .claim_podman_place(&row, identity.clone(), None, None)
                .await
                .unwrap(),
            Some(PlaceClaim::Owned { displaced: None })
        );
        let harness = builder.start().await;
        harness
            .await_state(former, |state| {
                matches!(state, HostState::IdentityMismatch { .. })
            })
            .await;
        // If the new entry wins the hello first, the former entry sees a
        // duplicate. Keep the new entry down until the mismatch is visible.
        harness.fleet.edit(next, |script| script.reachable = true);
        assert!(harness.manager.retry_now(next).await.unwrap());
        harness
            .await_state(next, |state| {
                matches!(state, HostState::Connected { identity: Some(id), .. } if id == "new-install")
            })
            .await;

        let places = &harness.state.podman_places;
        assert!(places.fresh(&harness.store, next).await.is_ok());
        assert!(
            !harness
                .store
                .owns_podman_place(row, identity)
                .await
                .unwrap()
        );
        assert!(places.fresh(&harness.store, former).await.is_err());
        assert!(places.runtime(&harness.store, former).await.is_ok());
    }

    /// A refused incumbent can point to a third registry entry. That hello
    /// cannot identify the challenger, but its own verified connection can
    /// take ownership of the unused Podman place.
    #[farhelm_testtrace::test]
    async fn verified_supervisor_takes_unused_place_from_third_host_duplicate() {
        use crate::rest_harness::{FleetBuilder, HostScript};

        let builder = FleetBuilder::new()
            .await
            .place_commands(scripted_places(&[TEST_PLACE_IDENTITY]));
        let install = |identity: &str| HostScript {
            identity: Some(identity.into()),
            ..HostScript::default()
        };
        let (builder, former) = builder
            .ssh("former-place.test", install("other-install"))
            .await;
        let (builder, other) = builder
            .ssh("other-place.test", install("other-install"))
            .await;
        let (builder, next) = builder
            .ssh("next-place.test", install("next-install"))
            .await;
        for (host, name) in [(former, "previous-install"), (other, "other-install")] {
            let row = current_row(builder.store(), host).await.unwrap();
            builder
                .store()
                .record_first_contact(host, &DialedAs::of(&row), name)
                .await
                .unwrap();
        }
        let former_row = current_row(builder.store(), former).await.unwrap();
        assert_eq!(
            former_row.host_identity.as_deref(),
            Some("previous-install")
        );
        let identity = parse_place_identity(TEST_PLACE_IDENTITY).unwrap();
        assert_eq!(
            builder
                .store()
                .claim_podman_place(&former_row, identity.clone(), None, None)
                .await
                .unwrap(),
            Some(PlaceClaim::Owned { displaced: None })
        );
        let harness = builder.start().await;
        harness.await_state(former, |state| {
            matches!(state, HostState::Duplicate { twin, identity } if *twin == other && identity == "other-install")
        }).await;
        harness.await_state(next, |state| {
            matches!(state, HostState::Connected { identity: Some(id), .. } if id == "next-install")
        }).await;
        harness
            .state
            .podman_places
            .fresh(&harness.store, next)
            .await
            .unwrap();
        let next_row = current_row(&harness.store, next).await.unwrap();
        assert!(
            harness
                .store
                .owns_podman_place(next_row, identity)
                .await
                .unwrap()
        );
        assert!(
            harness
                .store
                .podman_place_identity(former)
                .await
                .unwrap()
                .is_none()
        );
    }

    /// A duplicate hello can identify the ordinary owner even when that
    /// owner's supervisor is down. A Podman-only check can then reunite the
    /// claims if the duplicate has no recorded resources.
    #[farhelm_testtrace::test]
    async fn duplicate_hello_settles_place_for_unreachable_supervisor() {
        use crate::rest_harness::{FleetBuilder, HostScript};
        use crate::store::DialedAs;

        let builder = FleetBuilder::new()
            .await
            .place_commands(scripted_places(&[TEST_PLACE_IDENTITY]));
        let (builder, owner) = builder
            .ssh(
                "ordinary-owner.test",
                HostScript {
                    reachable: false,
                    ..HostScript::default()
                },
            )
            .await;
        let (builder, twin) = builder
            .ssh(
                "duplicate.test",
                HostScript {
                    identity: Some("ordinary-install".into()),
                    ..HostScript::default()
                },
            )
            .await;
        let row = current_row(builder.store(), owner).await.unwrap();
        builder
            .store()
            .record_first_contact(owner, &DialedAs::of(&row), "ordinary-install")
            .await
            .unwrap();
        let twin_row = current_row(builder.store(), twin).await.unwrap();
        let identity = parse_place_identity(TEST_PLACE_IDENTITY).unwrap();
        assert_eq!(
            builder
                .store()
                .claim_podman_place(&twin_row, identity.clone(), None, None)
                .await
                .unwrap(),
            Some(PlaceClaim::Owned { displaced: None })
        );
        let harness = builder.start().await;
        harness
            .await_state(owner, |state| {
                matches!(state, HostState::Unreachable { .. })
            })
            .await;
        harness
            .await_state(twin, |state| {
                matches!(state, HostState::Duplicate { twin, identity }
                if *twin == owner && identity == "ordinary-install")
            })
            .await;

        let places = &harness.state.podman_places;
        assert!(places.fresh(&harness.store, owner).await.is_ok());
        assert!(
            harness
                .store
                .owns_podman_place(row, identity)
                .await
                .unwrap()
        );
    }

    /// An unusable identity or missing verifier tools refuses only new work.
    /// Existing bindings can still get ordinary command access.
    #[farhelm_testtrace::test]
    async fn unreadable_identity_only_refuses_new_work() {
        let directory = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&directory.path().join("helm.db"))
            .await
            .unwrap();
        let host = store.list_hosts().await.unwrap()[0].id;
        for (output, reason) in [
            (
                "00000000000000000000000000000000\n1000\n",
                IDENTITY_UNREADABLE,
            ),
            ("farhelm-identity-tools-missing\n", IDENTITY_TOOLS_MISSING),
        ] {
            let places = PodmanPlaces::with_commands(scripted_places(&[output]));
            let refusal = places
                .fresh(&store, host)
                .await
                .err()
                .expect("unusable identity must block new work");
            assert_eq!(refusal.to_string(), reason);
            let (_, ready) = places.runtime(&store, host).await.unwrap();
            assert_eq!(ready.identity, Err(reason));
        }
    }

    /// The oldest supported Podman must pass, and each missing prerequisite
    /// must identify the actual refused feature without needing a real host.
    #[farhelm_testtrace::test]
    fn podman_info_requires_version_rootless_and_systemd_cgroup_v2() {
        let info = serde_json::json!({
            "version": {"Version": "4.9.3"},
            "host": {"security": {"rootless": true}, "cgroupVersion": "v2", "cgroupManager": "systemd"}
        });
        assert_eq!(validate_info(&info.to_string()).unwrap(), "4.9.3");
        for (pointer, value, reason) in [
            (
                "/host/security/rootless",
                serde_json::json!(false),
                "not rootless",
            ),
            ("/host/cgroupVersion", serde_json::json!("v1"), "cgroup v2"),
            (
                "/host/cgroupManager",
                serde_json::json!("cgroupfs"),
                "systemd",
            ),
            ("/version/Version", serde_json::json!("4.9.2"), "too old"),
        ] {
            let mut invalid = info.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(
                validate_info(&invalid.to_string())
                    .unwrap_err()
                    .to_string()
                    .contains(reason)
            );
        }
    }

    /// Structured manager values retain spaces, quotes, dollars, newlines and
    /// explicit empty assignments. Public status must never serialize them.
    #[farhelm_testtrace::test]
    async fn probe_preserves_proxy_values_and_collects_its_owned_scope() {
        let proxy = "https://user:p a'ss@$proxy.example/\nline";
        let replies = [
            ("uname", "Linux\n".to_string()),
            ("uname", "x86_64\n".to_string()),
            ("sh", "/usr/bin/podman\n".to_string()),
            ("/usr/bin/podman", serde_json::json!({
                "version": {"Version": "4.9.3"},
                "host": {"security": {"rootless": true}, "cgroupVersion": "v2", "cgroupManager": "systemd"}
            }).to_string()),
            ("busctl", serde_json::json!({"type": "as", "data": [
                format!("https_proxy={proxy}"), "HTTP_PROXY=".to_string()
            ]}).to_string()),
            ("systemd-run", String::new()),
            ("systemctl", String::new()),
            ("sh", TEST_PLACE_IDENTITY.into()),
        ];
        let commands = FakeCommands {
            replies: Mutex::new(
                replies
                    .into_iter()
                    .map(|(name, reply)| (name.into(), Ok(reply)))
                    .collect(),
            ),
            seen: Mutex::new(Vec::new()),
        };
        let ready = probe(&commands, &BaseRoute::Local).await.unwrap();
        assert_eq!(ready.proxy.get("https_proxy").unwrap(), proxy);
        assert_eq!(ready.proxy.get("HTTP_PROXY").unwrap(), "");
        let seen = commands.seen.lock().unwrap();
        let unit = seen[5].iter().find(|arg| arg.ends_with(".scope")).unwrap();
        assert!(unit.starts_with("farhelm-env-probe-"));
        assert_eq!(seen[6], ["systemctl", "--user", "stop", unit]);
        assert!(commands.replies.lock().unwrap().is_empty());
        let view = ProbeView {
            checked_at: Some(1),
            refreshing: false,
            usable: true,
            reason: None,
            architecture: Some(ready.architecture),
            version: Some(ready.version),
        };
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("proxy") && !json.contains("/usr/bin"));
    }

    /// Background refreshes must join per host and leave reads immediately
    /// available while commands are held. An installation change prevents an
    /// in-flight probe from settling an old actor's claim. A retarget discards
    /// both completed and in-flight observations, even across a text restore.
    #[farhelm_testtrace::test]
    async fn refresh_is_background_deduplicated_and_route_bound() {
        struct HeldCommands {
            entered: tokio::sync::Notify,
            release: tokio::sync::Notify,
            calls: std::sync::atomic::AtomicUsize,
            hold: std::sync::atomic::AtomicBool,
        }
        #[async_trait::async_trait]
        impl PlaceCommands for HeldCommands {
            async fn run(&self, _route: &BaseRoute, argv: &[String]) -> anyhow::Result<String> {
                self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let answer = match argv[0].as_str() {
                    "uname" if argv[1] == "-s" => {
                        if self.hold.load(std::sync::atomic::Ordering::Relaxed) {
                            self.entered.notify_one();
                            self.release.notified().await;
                        }
                        "Linux".to_string()
                    }
                    "uname" => "x86_64".to_string(),
                    "sh" if argv[2] == PLACE_IDENTITY_READ => TEST_PLACE_IDENTITY.into(),
                    "sh" => "/usr/bin/podman".to_string(),
                    "/usr/bin/podman" => serde_json::json!({
                        "version": {"Version": "4.9.3"},
                        "host": {"security": {"rootless": true}, "cgroupVersion": "v2", "cgroupManager": "systemd"}
                    }).to_string(),
                    "busctl" => r#"{"type":"as","data":[]}"#.to_string(),
                    "systemd-run" | "systemctl" => String::new(),
                    other => panic!("unexpected command {other}"),
                };
                Ok(answer)
            }
        }
        /// Wait on the completion signal shared by the real cache waiters.
        async fn wait_for_probe(service: &PodmanPlaces, row: &HostRow) {
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    let changed = service.changed.notified();
                    tokio::pin!(changed);
                    changed.as_mut().enable();
                    if !service.view(row).unwrap().refreshing {
                        break;
                    }
                    changed.await;
                }
            })
            .await
            .expect("the released fake probe still has a pending cache entry");
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("helm.db");
        let store = HelmStore::open(&path).await.unwrap();
        let host = store
            .add_ssh_host("before.example", None, None)
            .await
            .unwrap();
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        let commands = Arc::new(HeldCommands {
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            calls: std::sync::atomic::AtomicUsize::new(0),
            hold: std::sync::atomic::AtomicBool::new(true),
        });
        let service = PodmanPlaces::with_commands(commands.clone());
        service.refresh(store.clone(), host).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), commands.entered.notified())
            .await
            .unwrap();
        assert_eq!(commands.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // The first probe is held inside its command. A second task would
        // own a service reference as soon as it was queued, before it ran
        // any commands, so the command count alone cannot witness a join.
        let references = Arc::strong_count(&service);
        service.refresh(store.clone(), host).await.unwrap();
        assert_eq!(Arc::strong_count(&service), references);
        assert!(service.view(&row).unwrap().refreshing);
        assert!(!service.view(&row).unwrap().usable);
        assert_eq!(
            store
                .plant_remote_paths_for_tests(host, "/fixture/farhelm", "/fixture/state")
                .await,
            1,
        );
        let configured = current_row(&store, host).await.unwrap();
        assert_eq!(base_route(&row).unwrap(), base_route(&configured).unwrap());
        commands.release.notify_one();
        wait_for_probe(&service, &configured).await;
        assert!(!service.view(&configured).unwrap().usable);
        assert!(store.podman_place_identity(host).await.unwrap().is_none());
        let identity = parse_place_identity(TEST_PLACE_IDENTITY).unwrap();
        assert_eq!(
            store
                .claim_podman_place(&row, identity.clone(), None, None)
                .await
                .unwrap(),
            None,
            "the transaction must recheck install coordinates too",
        );
        let row = configured;
        service.refresh(store.clone(), host).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), commands.entered.notified())
            .await
            .expect("the newly configured entry must start a fresh check");
        commands.release.notify_one();
        wait_for_probe(&service, &row).await;
        assert!(service.view(&row).unwrap().usable);
        assert!(service.ready(&row).is_ok());
        service
            .retarget(&store, host, "before.example")
            .await
            .unwrap();
        assert!(
            service.view(&row).unwrap().usable,
            "a no-op retains the claim"
        );
        assert!(
            store
                .owns_podman_place(row.clone(), identity.clone())
                .await
                .unwrap()
        );
        service.refresh(store.clone(), host).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), commands.entered.notified())
            .await
            .expect("the second probe must already be in flight");
        let permit = service.cache.lock().unwrap().refreshing[&host].clone();
        service
            .retarget(&store, host, "after.example")
            .await
            .unwrap();
        let changed = current_row(&store, host).await.unwrap();
        assert!(!service.view(&changed).unwrap().usable);
        assert!(service.ready(&changed).is_err());
        service
            .retarget(&store, host, "before.example")
            .await
            .unwrap();
        let restored = current_row(&store, host).await.unwrap();
        assert!(!service.view(&restored).unwrap().usable);
        commands.release.notify_one();
        // Retarget removed the slot, so only the probe's own reference can
        // witness its exit. Wait for that before checking for publication.
        tokio::time::timeout(Duration::from_secs(2), async {
            while Arc::strong_count(&permit) > 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the abandoned probe must exit after its held command answers");
        assert!(!service.view(&restored).unwrap().usable);
        assert!(
            !store.owns_podman_place(restored, identity).await.unwrap(),
            "the pre-edit probe cannot re-claim the restored destination"
        );

        // Refresh on the edited host must start while the abandoned
        // route's probe is still waiting for its remote answer.
        service.refresh(store.clone(), host).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), commands.entered.notified())
            .await
            .expect("the check from the old route must be in flight");
        service
            .retarget(&store, host, "after.example")
            .await
            .unwrap();
        commands
            .hold
            .store(false, std::sync::atomic::Ordering::Relaxed);
        tokio::time::timeout(Duration::from_secs(2), service.fresh(&store, host))
            .await
            .expect("a fresh check must not wait for the abandoned route")
            .expect("the current endpoint passes its check");
        assert!(service.view(&changed).unwrap().usable);
        commands.release.notify_one();
    }

    /// Setup saves install coordinates before replacing the host actor. A
    /// probe in that interval must wait for a matching actor to settle a
    /// claim. An unreachable supervisor on a reconciled place remains valid.
    #[farhelm_testtrace::test]
    async fn refresh_waits_for_the_actor_matching_the_saved_install() {
        use crate::rest_harness::{FleetBuilder, HostScript};

        let builder = FleetBuilder::new()
            .await
            .place_commands(scripted_places(&[TEST_PLACE_IDENTITY, TEST_PLACE_IDENTITY]));
        let offline = |state: &HostState| matches!(state, HostState::Unreachable { .. });
        let (builder, host) = builder
            .ssh(
                "place.example",
                HostScript {
                    reachable: false,
                    ..HostScript::default()
                },
            )
            .await;
        let harness = builder.start().await;
        harness.await_state(host, offline).await;
        let places = &harness.state.podman_places;
        assert_eq!(
            harness
                .store
                .plant_remote_paths_for_tests(host, "/fixture/farhelm", "/fixture/state")
                .await,
            1,
        );
        let row = current_row(&harness.store, host).await.unwrap();
        assert!(harness.manager.state_for_registry_row(&row).is_none());
        assert!(places.fresh(&harness.store, host).await.is_err());
        assert!(
            harness
                .store
                .podman_place_identity(host)
                .await
                .unwrap()
                .is_none()
        );
        assert!(!places.view(&row).unwrap().usable);

        harness.manager.sync_registry().await.unwrap();
        harness.await_state(host, offline).await;
        assert!(
            harness
                .manager
                .state_for_registry_row(&row)
                .is_some_and(|state| offline(&state))
        );
        assert!(places.fresh(&harness.store, host).await.is_ok());
    }

    /// Run the production guard and capture layer with shell functions that
    /// supply a fixture identity. A real command's status 1 counts only
    /// after the guard evaluates and proves the expected account and machine.
    /// A failure before the guard runs (a shell exiting 1, or 255 standing in
    /// for SSH failing to connect) is reported as an unreachable host with its
    /// own stderr; only the guard's 125 reports a changed destination.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    async fn absence_needs_a_same_shell_place_proof() {
        struct ShellFixture {
            system: SystemPlaceCommands,
            before_guard: &'static str,
        }
        #[async_trait::async_trait]
        impl PlaceCommands for ShellFixture {
            async fn run(&self, _: &BaseRoute, _: &[String]) -> anyhow::Result<String> {
                unreachable!("a second identity command cannot prove the command's own route")
            }

            async fn run_bounded(
                &self,
                route: &BaseRoute,
                argv: &[String],
                timeout: Duration,
            ) -> anyhow::Result<crate::provisioning::CommandResult> {
                assert_eq!(argv[2], VERIFY_PLACE_AND_EXEC);
                assert!(
                    argv[4].len() == 64 && argv[4].bytes().all(|byte| byte.is_ascii_hexdigit())
                );
                let mut args = argv.to_vec();
                args[2] = format!("{}\n{}", self.before_guard, args[2]);
                self.system.run_bounded(route, &args, timeout).await
            }
        }

        let directory = tempfile::tempdir().unwrap();
        let mut commands = ShellFixture {
            system: SystemPlaceCommands {
                control_dir: directory.path().into(),
            },
            before_guard: "cat() { echo FE001122AABBCCDDEEFF001122AABBCC; }; id() { echo 1000; }",
        };
        let command = [
            "sh".into(),
            "-c".into(),
            "printf '%s' \"$1\"; exit 1".into(),
            "test-command".into(),
            "a quoted ' argument".into(),
        ];
        let mut expected = parse_place_identity(TEST_PLACE_IDENTITY).unwrap();
        let output = commands
            .run_verified(
                &BaseRoute::Local,
                &command,
                &expected,
                Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!((output.code, output.signal), (Some(1), None));
        assert_eq!(output.stdout, b"a quoted ' argument");

        expected.uid += 1;
        let error = commands
            .run_verified(
                &BaseRoute::Local,
                &command,
                &expected,
                Duration::from_secs(5),
            )
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), CONTAINER_PLACE_UNVERIFIED);
        let missed = [
            "sh".into(),
            "-c".into(),
            VERIFY_PLACE_AND_EXEC.into(),
            "test-guard".into(),
            place_digest(&expected, "unused-proof"),
            "unused-proof".into(),
            "sh".into(),
            "-c".into(),
            "echo unexpected".into(),
        ];
        let output = commands
            .run_bounded(&BaseRoute::Local, &missed, Duration::from_secs(5))
            .await
            .unwrap();
        assert!(
            output.stdout.is_empty(),
            "the mismatched shell must leave the command unsent"
        );
        assert_eq!(output.code, Some(125));

        // A shell can exit before any verification. Its 1 is not Podman
        // absence, and since the check never ran it is not reported as a
        // changed destination either.
        commands.before_guard = "echo 'no route to host' >&2; exit 1";
        expected.uid -= 1;
        let error = commands
            .run_verified(
                &BaseRoute::Local,
                &command,
                &expected,
                Duration::from_secs(5),
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Farhelm could not reach this container host (exit status 1): no route to host"
        );

        // SSH's own 255 for a host that cannot be reached is the common case.
        commands.before_guard = "exit 255";
        let error = commands
            .run_verified(
                &BaseRoute::Local,
                &command,
                &expected,
                Duration::from_secs(5),
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Farhelm could not reach this container host (exit status 255)."
        );
    }

    /// A missing busctl must not add a new place prerequisite. systemctl's
    /// escaped values still reach the provider exactly, including empty ones.
    #[farhelm_testtrace::test]
    async fn probe_falls_back_to_exact_systemctl_environment() {
        let info = serde_json::json!({
            "version": {"Version": "4.9.3"},
            "host": {"security": {"rootless": true}, "cgroupVersion": "v2", "cgroupManager": "systemd"}
        }).to_string();
        let replies = vec![
            ("uname".into(), Ok("Linux".into())),
            ("uname".into(), Ok("x86_64".into())),
            ("sh".into(), Ok("/usr/bin/podman".into())),
            ("/usr/bin/podman".into(), Ok(info)),
            ("busctl".into(), Err(anyhow::anyhow!("unavailable"))),
            (
                "systemctl".into(),
                Ok("https_proxy=$'space \\'quote\\' $dollar\\nline'\nHTTP_PROXY=\n".into()),
            ),
            ("systemd-run".into(), Ok(String::new())),
            ("systemctl".into(), Ok(String::new())),
            ("sh".into(), Ok(TEST_PLACE_IDENTITY.into())),
        ];
        let commands = FakeCommands {
            replies: Mutex::new(replies.into()),
            seen: Mutex::new(Vec::new()),
        };
        let ready = probe(&commands, &BaseRoute::Local).await.unwrap();
        assert_eq!(ready.proxy["https_proxy"], "space 'quote' $dollar\nline");
        assert_eq!(ready.proxy["HTTP_PROXY"], "");
        assert!(commands.replies.lock().unwrap().is_empty());
    }
}
