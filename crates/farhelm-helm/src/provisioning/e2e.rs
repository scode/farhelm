//! The explicit filesystem-controlled backend lets browser tests exercise the
//! shipped provisioning orchestration without changing its HTTP or state path.

use super::backend::{
    ActionOutcome, BackendFailure, PreparedPayload, ProbeObservation, ProbeTarget,
    ProvisioningBackend, Reach, ReachOutcome,
};
use super::payloads::PayloadSource;
use super::plan::{DirectorySpec, PayloadArch, PayloadKind, ProvisioningTarget};
use anyhow::{Context as _, bail};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

pub(super) const E2E_BACKEND_ENV: &str = "FARHELM_E2E_PROVISIONING_BACKEND_DIR";
const E2E_BACKEND_MARKER: &str = "farhelm-e2e-provisioning-v1\n";

/// One explicit, filesystem-controlled backend used only by Playwright.
///
/// This is runtime gated because the E2E suite drives the ordinary debug
/// binary, not a `cfg(test)` executable. Enabling it requires both the
/// unmistakably test-named environment variable and a marker inside the
/// helm's private state directory. Startup prints a warning before serving.
/// The seam ends at [`ProvisioningBackend`]: HTTP routing, authentication,
/// one-use plans, registration, progress retention, and feed bumps remain the
/// shipped implementation.
pub(super) struct E2eProvisioningBackend {
    root: PathBuf,
    event_write: tokio::sync::Mutex<()>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
enum E2eProbeOutcome {
    #[default]
    Absent,
    Supervisor,
    Error,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
enum E2eInspectOutcome {
    #[default]
    Supported,
    Manual,
    Error,
}

/// Behavior selected independently per transport target by the E2E suite.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct E2eBehavior {
    probe: E2eProbeOutcome,
    inspect: E2eInspectOutcome,
    message: String,
    build_version: String,
    identity: Option<String>,
    dial_farhelm: String,
    dial_state_dir: Option<String>,
    home: String,
    user_unit_dir: String,
    needs_tmux: bool,
    hold_actions: bool,
    fail_action: Option<String>,
    action_delay_ms: u64,
}

impl Default for E2eBehavior {
    fn default() -> Self {
        Self {
            probe: E2eProbeOutcome::Absent,
            inspect: E2eInspectOutcome::Supported,
            message: "injected provisioning failure".to_string(),
            build_version: env!("CARGO_PKG_VERSION").to_string(),
            identity: None,
            dial_farhelm: "/opt/farhelm-e2e/farhelm".to_string(),
            dial_state_dir: Some("/var/lib/farhelm-e2e".to_string()),
            home: "/home/farhelm-e2e".to_string(),
            user_unit_dir: "/home/farhelm-e2e/.config/systemd/user".to_string(),
            needs_tmux: false,
            hold_actions: false,
            fail_action: None,
            action_delay_ms: 0,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct E2eBackendConfig {
    default: E2eBehavior,
    targets: HashMap<String, E2eBehavior>,
}

impl E2eProvisioningBackend {
    pub(super) fn new(root: PathBuf, helm_state_dir: &Path) -> anyhow::Result<Self> {
        if !root.is_absolute() || !root.starts_with(helm_state_dir) {
            bail!("{E2E_BACKEND_ENV} must name a directory inside the helm state directory");
        }
        let marker = std::fs::read_to_string(root.join("ENABLED"))
            .with_context(|| format!("reading the {E2E_BACKEND_ENV} marker"))?;
        if marker != E2E_BACKEND_MARKER {
            bail!("{E2E_BACKEND_ENV} has no valid E2E marker");
        }
        Ok(Self {
            root,
            event_write: tokio::sync::Mutex::new(()),
        })
    }

    fn target_key(target: &ProvisioningTarget) -> String {
        match target {
            ProvisioningTarget::Local => "local".to_string(),
            ProvisioningTarget::Ssh { destination } => format!("ssh:{destination}"),
        }
    }

    async fn behavior(&self, target: &ProvisioningTarget) -> Result<E2eBehavior, BackendFailure> {
        let path = self.root.join("config.json");
        let bytes = tokio::fs::read(&path).await.map_err(|error| {
            BackendFailure::new(
                format!("reading injected provisioning config {}", path.display()),
                error.to_string(),
            )
        })?;
        let config: E2eBackendConfig = serde_json::from_slice(&bytes).map_err(|error| {
            BackendFailure::new(
                format!("decoding injected provisioning config {}", path.display()),
                error.to_string(),
            )
        })?;
        Ok(config
            .targets
            .get(&Self::target_key(target))
            .cloned()
            .unwrap_or(config.default))
    }

    async fn record(&self, target: &ProvisioningTarget, event: &str) -> Result<(), BackendFailure> {
        let _guard = self.event_write.lock().await;
        let path = self.root.join("events.jsonl");
        let mut line = serde_json::to_vec(&serde_json::json!({
            "event": event,
            "target": Self::target_key(target),
        }))
        .expect("the injected event is serializable");
        line.push(b'\n');
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|error| {
                BackendFailure::new(
                    format!("opening injected provisioning events {}", path.display()),
                    error.to_string(),
                )
            })?;
        file.write_all(&line).await.map_err(|error| {
            BackendFailure::new(
                format!("recording injected provisioning event {event}"),
                error.to_string(),
            )
        })?;
        file.flush().await.map_err(|error| {
            BackendFailure::new(
                format!("flushing injected provisioning event {event}"),
                error.to_string(),
            )
        })
    }

    async fn action(
        &self,
        target: &ProvisioningTarget,
        label: &str,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.record(target, label).await?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        let behavior = loop {
            let behavior = self.behavior(target).await?;
            if !behavior.hold_actions {
                break behavior;
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(BackendFailure::new(
                    format!("waiting for injected {label} release"),
                    "the E2E control file held the action for 30 seconds",
                ));
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        if behavior.action_delay_ms != 0 {
            tokio::time::sleep(Duration::from_millis(behavior.action_delay_ms.min(30_000))).await;
        }
        if behavior.fail_action.as_deref() == Some(label) {
            return Err(BackendFailure::new(
                format!("injected {label} failure"),
                behavior.message,
            ));
        }
        Ok(ActionOutcome::Completed)
    }
}

#[async_trait]
impl ProvisioningBackend for E2eProvisioningBackend {
    async fn probe(&self, target: &ProbeTarget) -> Result<ProbeObservation, BackendFailure> {
        self.record(&target.transport, "probe").await?;
        let behavior = self.behavior(&target.transport).await?;
        match behavior.probe {
            E2eProbeOutcome::Absent => Ok(ProbeObservation::Absent),
            E2eProbeOutcome::Supervisor => Ok(ProbeObservation::Supervisor {
                build_version: behavior.build_version,
                host_identity: behavior.identity,
                dial_farhelm: PathBuf::from(behavior.dial_farhelm),
                dial_state_dir: behavior.dial_state_dir.map(PathBuf::from),
            }),
            E2eProbeOutcome::Error => Err(BackendFailure::new(
                "injected provisioning probe failure",
                behavior.message,
            )),
        }
    }

    async fn inspect(&self, target: &ProbeTarget) -> Result<ReachOutcome, BackendFailure> {
        self.record(&target.transport, "inspect").await?;
        let behavior = self.behavior(&target.transport).await?;
        match behavior.inspect {
            E2eInspectOutcome::Supported => Ok(ReachOutcome::Supported(Reach {
                home: PathBuf::from(behavior.home),
                user_unit_dir: PathBuf::from(behavior.user_unit_dir),
                arch: PayloadArch::X86_64,
                needs_tmux: behavior.needs_tmux,
                host_tmux: (!behavior.needs_tmux).then(|| PathBuf::from("/usr/bin/tmux")),
            })),
            E2eInspectOutcome::Manual => Ok(ReachOutcome::Manual(behavior.message)),
            E2eInspectOutcome::Error => Err(BackendFailure::new(
                "injected provisioning inspection failure",
                behavior.message,
            )),
        }
    }

    async fn ensure_directories(
        &self,
        target: &ProvisioningTarget,
        _directories: &[DirectorySpec],
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(target, "create-directories").await
    }

    async fn install_path(
        &self,
        target: &ProvisioningTarget,
        kind: PayloadKind,
        _payload: &PreparedPayload,
        _destination: &Path,
        _temporary: &Path,
        _mode: u32,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(
            target,
            match kind {
                PayloadKind::Farhelm => "install-farhelm",
                PayloadKind::Tmux => "install-tmux",
            },
        )
        .await
    }

    async fn install_bytes(
        &self,
        target: &ProvisioningTarget,
        _content: &[u8],
        _destination: &Path,
        _temporary: &Path,
        _mode: u32,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(target, "write-unit").await
    }

    async fn daemon_reload(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(target, "daemon-reload").await
    }

    async fn enable_now(
        &self,
        target: &ProvisioningTarget,
        _unit: &str,
        _unit_path: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(target, "enable-supervisor").await
    }

    async fn enable_linger(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(target, "enable-linger").await
    }

    async fn restart(
        &self,
        target: &ProvisioningTarget,
        _unit: &str,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(target, "restart-supervisor").await
    }

    /// The e2e fixture never stands in for a machine whose units
    /// `farhelm helm setup` owns: the browser suite drives the hosts
    /// panel, and a scripted refusal there would test the fixture rather
    /// than the panel. Answering "no such unit" keeps the local row on the
    /// path the suite exercises.
    async fn read_user_unit(&self, _name: &str) -> Result<Option<String>, BackendFailure> {
        Ok(None)
    }

    async fn injected_attach(
        &self,
        target: &ProvisioningTarget,
    ) -> Result<Option<ActionOutcome>, BackendFailure> {
        self.action(target, "attach-supervisor").await.map(Some)
    }
}

/// The E2E backend still exercises payload staging, but uses its small marker
/// as inert content so browser tests measure provisioning rather than copying
/// the running debug executable for every simulated host.
pub(super) struct E2ePayloads(pub(super) PathBuf);

#[async_trait]
impl PayloadSource for E2ePayloads {
    async fn path(&self, _payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
        Ok(self.0.clone())
    }
}
