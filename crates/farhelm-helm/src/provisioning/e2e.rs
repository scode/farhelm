//! The explicit filesystem-controlled backend lets browser tests exercise the
//! shipped provisioning orchestration without changing its HTTP or state path.

use super::backend::{
    ActionOutcome, BackendFailure, HostPath, PreparedPayload, ProbeObservation, ProbeTarget,
    ProvisioningBackend, Reach, ReachOutcome, UninstallInspection,
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
    /// Enable the simulated backend only for a directory that REALLY lies
    /// inside the helm's private state directory and carries the marker.
    ///
    /// Both paths are canonicalized before the containment check.
    /// `Path::starts_with` compares components without resolving them, so a
    /// lexical check accepted `<state>/../../tmp/x` and a symlink inside the
    /// state directory pointing anywhere, letting a directory outside the
    /// private boundary choose which remote paths this user's ssh runs. The
    /// resolved root is what the backend keeps using afterwards.
    pub(super) fn new(root: PathBuf, helm_state_dir: &Path) -> anyhow::Result<Self> {
        if !root.is_absolute() {
            bail!("{E2E_BACKEND_ENV} must name a directory inside the helm state directory");
        }
        let root = std::fs::canonicalize(&root)
            .with_context(|| format!("resolving {E2E_BACKEND_ENV} {}", root.display()))?;
        let state = std::fs::canonicalize(helm_state_dir).with_context(|| {
            format!(
                "resolving the helm state directory {}",
                helm_state_dir.display()
            )
        })?;
        if !root.starts_with(&state) {
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
                distro_id: "ubuntu".to_string(),
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

    async fn upload_path(
        &self,
        target: &ProvisioningTarget,
        kind: PayloadKind,
        _payload: &PreparedPayload,
        _destination: &Path,
        _temporary: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(
            target,
            match kind {
                PayloadKind::Farhelm => "upload-farhelm",
                PayloadKind::Tmux => "upload-tmux",
            },
        )
        .await
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

    /// Every removable file is reported present, not a symlink, and at its
    /// own canonical path, and the unit running under provisioning's kill
    /// policy, so an injected UNINSTALL plans its full step list. The steps
    /// themselves are simulated like every other action, which is what keeps
    /// a browser spec's real remote supervisor running while the panel
    /// uninstalls it.
    async fn inspect_uninstall(
        &self,
        target: &ProvisioningTarget,
        _unit: &str,
        paths: &[&Path],
    ) -> Result<UninstallInspection, BackendFailure> {
        self.record(target, "inspect-uninstall").await?;
        let behavior = self.behavior(target).await?;
        let present = |path: PathBuf| HostPath {
            canonical: Some(path.clone()),
            path,
            exists: true,
            symlink: false,
        };
        Ok(UninstallInspection {
            paths: paths
                .iter()
                .map(|path| present(path.to_path_buf()))
                .collect(),
            default_state_dir: present(PathBuf::from(behavior.home).join(".local/state/farhelm")),
            unit_active_state: "active".to_string(),
            // The unit loaded from the planned path, which is the first one
            // asked about.
            unit_fragment: paths.first().map(|path| present(path.to_path_buf())),
        })
    }

    async fn disable(
        &self,
        target: &ProvisioningTarget,
        _unit: &str,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(target, "disable-supervisor").await
    }

    async fn remove_unit(
        &self,
        target: &ProvisioningTarget,
        _destination: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(target, "remove-unit").await
    }

    async fn stop(
        &self,
        target: &ProvisioningTarget,
        _unit: &str,
        _tmux_program: &Path,
        _tmux_socket: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(target, "stop-supervisor").await
    }

    async fn remove_directory(
        &self,
        target: &ProvisioningTarget,
        _path: &Path,
    ) -> Result<ActionOutcome, BackendFailure> {
        self.action(target, "remove-directory").await
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
#[derive(Debug)]
pub(super) struct E2ePayloads(pub(super) PathBuf);

#[async_trait]
impl PayloadSource for E2ePayloads {
    async fn path(&self, _payload: PayloadKind, _arch: PayloadArch) -> anyhow::Result<PathBuf> {
        Ok(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write the enabling marker into `dir`.
    fn enable(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("ENABLED"), E2E_BACKEND_MARKER).unwrap();
    }

    /// The test-only backend gate accepts only a directory that really lies
    /// inside the helm state directory.
    ///
    /// Why: the check used to be lexical, so a `..` path or a symlink inside
    /// the state directory could point the simulated backend at a directory
    /// outside the helm's private boundary, whose config then chose the
    /// remote paths real ssh runs. Spec: a marked subdirectory is accepted;
    /// the same marked outside directory is refused whether reached by `..`
    /// or through a symlink.
    #[farhelm_testtrace::test]
    fn the_backend_gate_resolves_paths_before_the_containment_check() {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state");
        let inside = state.join("e2e");
        let outside = root.path().join("outside");
        enable(&inside);
        enable(&outside);

        assert!(E2eProvisioningBackend::new(inside.clone(), &state).is_ok());

        let dotted = state.join("..").join("outside");
        assert!(
            dotted.starts_with(&state),
            "fixture premise: the dotted path passes a lexical prefix check"
        );
        assert!(E2eProvisioningBackend::new(dotted, &state).is_err());

        let link = state.join("link");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        assert!(E2eProvisioningBackend::new(link, &state).is_err());
    }
}
