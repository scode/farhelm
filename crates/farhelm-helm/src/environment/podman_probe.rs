//! Place capabilities are cached observations, refreshed independently of UI reads.
//!
//! The complete result stays private because proxy values may hold credentials.
//! Public views expose readiness and its reason. A result belongs to the exact
//! current base route, so retargeting a host immediately invalidates its cache.

use crate::access::{BaseRoute, RemoteCommand, base_route};
use crate::store::{HelmStore, HostId, HostRow};
use crate::units::{MANAGER_ENVIRONMENT_BUSCTL_ARGS, ManagerEnvironment, ManagerValue};
use anyhow::Context as _;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Launch consumers require a recent observation before using its tool path
/// and proxy inputs. Explicit Refresh ignores this age and only joins in-flight work.
const FRESH_FOR: Duration = Duration::from_secs(300);
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

/// Inputs used by the real provider. These never enter browser JSON or logs.
#[derive(Clone)]
pub(crate) struct ReadyPlace {
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

/// A browser-safe projection, including hosts never checked in this process.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ProbeView {
    pub(crate) checked_at: Option<u64>,
    pub(crate) refreshing: bool,
    pub(crate) usable: bool,
    pub(crate) reason: Option<String>,
    pub(crate) architecture: Option<String>,
    pub(crate) version: Option<String>,
}

/// A small command seam lets tests script each prerequisite without changing
/// PATH or any process environment, or touching a real user manager.
#[async_trait::async_trait]
pub(crate) trait PlaceCommands: Send + Sync {
    async fn run(&self, route: &BaseRoute, argv: &[String]) -> anyhow::Result<String>;
}

struct SystemPlaceCommands {
    control_dir: PathBuf,
}

#[async_trait::async_trait]
impl PlaceCommands for SystemPlaceCommands {
    async fn run(&self, route: &BaseRoute, argv: &[String]) -> anyhow::Result<String> {
        // The remote login has its own environment too. Only validated variable
        // names are unset, and argv remains positional data through this shell.
        let mut clean = vec!["sh".into(), "-c".into(),
            "for name in $(env | sed -n 's/^\\(FARHELM_[A-Za-z0-9_]*\\)=.*/\\1/p'); do unset \"$name\"; done; unset TMUX TMUX_PANE; exec \"$@\"".into(),
            "farhelm-place-command".into()];
        clean.extend_from_slice(argv);
        let process = route.command(&self.control_dir, RemoteCommand::Argv(&clean))?;
        let mut command = process.command();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // A local probe can initialize shared Podman helpers. Private helm and
        // tmux inputs must not become those helpers' inherited environment.
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("FARHELM_")
                || matches!(name.to_str(), Some("TMUX" | "TMUX_PANE"))
            {
                command.env_remove(name);
            }
        }
        crate::provisioning::isolate_process_group(&mut command);
        let child = command
            .spawn()
            .map_err(|_| anyhow::anyhow!("could not start the place command"))?;
        let result = crate::provisioning::capture_child(
            child,
            Duration::from_secs(20),
            "container place probe",
        )
        .await
        .map_err(|_| anyhow::anyhow!("the place command failed or timed out"))?;
        anyhow::ensure!(result.code == Some(0), "the place command was refused");
        String::from_utf8(result.stdout)
            .map_err(|_| anyhow::anyhow!("the place command returned invalid text"))
    }
}

struct CachedProbe {
    route: BaseRoute,
    checked_at: u64,
    observed: Instant,
    result: Result<ReadyPlace, String>,
}

#[derive(Default)]
struct Cache {
    probes: HashMap<HostId, CachedProbe>,
    refreshing: HashSet<HostId>,
}

/// One helm's probe cache and in-flight admission. Refresh tasks retain the
/// service until they publish an observation, even if the requesting page closes.
pub(crate) struct PodmanPlaces {
    commands: Arc<dyn PlaceCommands>,
    cache: Mutex<Cache>,
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
        })
    }

    /// Read locally without waiting on a host. A changed endpoint has no current
    /// result, regardless of the timestamp of the previous endpoint's probe.
    pub(crate) fn view(&self, row: &HostRow) -> anyhow::Result<ProbeView> {
        let route = base_route(row)?;
        let cache = self.cache.lock().unwrap();
        let probe = cache
            .probes
            .get(&row.id)
            .filter(|probe| probe.route == route);
        let ready = probe.and_then(|probe| probe.result.as_ref().ok());
        Ok(ProbeView {
            checked_at: probe.map(|probe| probe.checked_at),
            refreshing: cache.refreshing.contains(&row.id),
            usable: ready.is_some(),
            reason: probe.and_then(|probe| probe.result.as_ref().err().cloned()),
            architecture: ready.map(|ready| ready.architecture.clone()),
            version: ready.map(|ready| ready.version.clone()),
        })
    }

    /// Preparation consumes exactly the place observation that passed the probe.
    /// Stale results and retargeted endpoints require a background refresh first.
    #[allow(
        dead_code,
        reason = "the next provider unit consumes prepared place inputs"
    )]
    pub(crate) fn ready(&self, row: &HostRow) -> anyhow::Result<ReadyPlace> {
        let route = base_route(row)?;
        let cache = self.cache.lock().unwrap();
        let probe = cache
            .probes
            .get(&row.id)
            .filter(|probe| probe.route == route && probe.observed.elapsed() <= FRESH_FOR)
            .context("the container host needs a fresh Podman check")?;
        probe.result.clone().map_err(anyhow::Error::msg)
    }

    /// Schedule only this host and return immediately. The current row is read
    /// again before execution, and a stale observation cannot replace a result
    /// for a different route. Each host has at most one in-flight command chain.
    pub(crate) async fn refresh(
        self: &Arc<Self>,
        store: HelmStore,
        host: HostId,
    ) -> anyhow::Result<()> {
        let row = current_row(&store, host).await?;
        base_route(&row)?;
        if !self.cache.lock().unwrap().refreshing.insert(host) {
            return Ok(());
        }
        let service = Arc::clone(self);
        tokio::spawn(async move {
            let _refreshing = Refreshing {
                service: Arc::clone(&service),
                host,
            };
            let Ok(row) = current_row(&store, host).await else {
                return;
            };
            let Ok(route) = base_route(&row) else { return };
            let result = probe(service.commands.as_ref(), &route).await;
            // Publication is route-bound too. A host edited during the command
            // chain receives no stale success for its new endpoint.
            if let Ok(current) = current_row(&store, host).await
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
                    },
                );
            }
        });
        Ok(())
    }
}

/// A cancelled or panicking refresh must release its in-flight admission too.
struct Refreshing {
    service: Arc<PodmanPlaces>,
    host: HostId,
}

impl Drop for Refreshing {
    fn drop(&mut self) {
        self.service
            .cache
            .lock()
            .unwrap()
            .refreshing
            .remove(&self.host);
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
        .map_err(|_| anyhow::anyhow!(reason))
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
    Ok(ReadyPlace {
        architecture,
        version,
        podman,
        proxy,
    })
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
    /// available while commands are held. This exercises the cache's actual
    /// consumer seam without starting a process or changing its environment.
    #[farhelm_testtrace::test]
    async fn refresh_is_background_deduplicated_and_route_bound() {
        struct HeldCommands {
            entered: tokio::sync::Notify,
            release: tokio::sync::Notify,
            calls: std::sync::atomic::AtomicUsize,
        }
        #[async_trait::async_trait]
        impl PlaceCommands for HeldCommands {
            async fn run(&self, _route: &BaseRoute, argv: &[String]) -> anyhow::Result<String> {
                self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let answer = match argv[0].as_str() {
                    "uname" if argv[1] == "-s" => {
                        self.entered.notify_one();
                        self.release.notified().await;
                        "Linux".to_string()
                    }
                    "uname" => "x86_64".to_string(),
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
        });
        let service = PodmanPlaces::with_commands(commands.clone());
        service.refresh(store.clone(), host).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), commands.entered.notified())
            .await
            .unwrap();
        service.refresh(store.clone(), host).await.unwrap();
        assert_eq!(commands.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(service.view(&row).unwrap().refreshing);
        assert!(!service.view(&row).unwrap().usable);
        commands.release.notify_one();
        tokio::time::timeout(Duration::from_secs(2), async {
            while service.view(&row).unwrap().refreshing {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(service.view(&row).unwrap().usable);
        assert!(service.ready(&row).is_ok());
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute(
            "UPDATE hosts SET destination = 'after.example' WHERE id = ?1",
            [host],
        )
        .unwrap();
        let changed = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert!(!service.view(&changed).unwrap().usable);
        assert!(service.ready(&changed).is_err());
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
