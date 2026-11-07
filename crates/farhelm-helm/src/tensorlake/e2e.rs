//! A filesystem-controlled stand-in for Tensorlake, used only by the browser
//! end-to-end suite: its API and the shell a snapshot build runs in a
//! sandbox.
//!
//! The browser suite drives the ordinary debug binary, not a `cfg(test)`
//! build, so this fake is selected at runtime, the same way the injected
//! provisioning backend is (`provisioning/e2e.rs`): an unmistakably
//! test-named environment variable read once at helm startup, naming a
//! directory that must lie inside the helm's private state directory and
//! carry a marker file. Startup prints a warning before serving. Everything
//! above the [`TensorlakeApi`] and [`SandboxShell`] seams (the routes, the
//! stored key, the build engine) stays the shipped code.
//!
//! The directory is the control plane:
//!
//! - `config.json`, written by the spec and read afresh on every call (a spec
//!   changes behavior mid-run by replacing it atomically): the account's
//!   extra sandboxes, an optional key the fake accepts (any other gets a
//!   401, which is how a spec proves the helm sends the key it saved without
//!   the fake ever writing a key down), failures to inject per operation,
//!   how many polls a snapshot takes, and the shell's behavior (hold every
//!   command, fail a step with given stderr, answer a step with given
//!   stdout).
//! - `state.json`, written by the fake: the sandboxes and snapshots the helm
//!   created, terminated and deleted, so a spec can assert what is left
//!   billing. A spec may seed it before acting.
//! - `events.jsonl`, appended by the fake: one line per call, API and shell.
//!
//! The model itself, [`FakeModel`], is plain data with Tensorlake's rules
//! (name uniqueness, 404s, a snapshot that completes after some polls); the
//! helm's unit tests drive the same model in memory. Suspend and resume
//! preserve the sandbox's identity and resources, as the real API does.

use super::api::{
    ApiKey, CreateSandbox, CreatedSandbox, CreatedSnapshot, Resources, SandboxInfo, SandboxPage,
    SandboxSource, SandboxStatus, SnapshotInfo, SnapshotList, SnapshotStatus, SnapshotType,
    TensorlakeApi, TensorlakeError,
};
use super::shell::{SandboxShell, ShellFailure, ShellOutput};
use super::ssh::TensorlakeSshAccess;
use anyhow::{Context as _, bail};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

pub(crate) const E2E_TENSORLAKE_ENV: &str = "FARHELM_E2E_TENSORLAKE_DIR";
const E2E_TENSORLAKE_MARKER: &str = "farhelm-e2e-tensorlake-v1\n";

/// How often a held shell command re-reads `config.json` to see whether the
/// spec released it.
const HOLD_POLL: Duration = Duration::from_millis(100);

/// A failure to answer instead of an operation's normal reply.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct InjectedFailure {
    status: u16,
    #[serde(default)]
    message: String,
}

/// How the fake shell answers. Keys of `fail` and `stdout` are the build
/// step labels (`ready`, `packages`, `claude`, `codex`, `prepare`).
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ShellConfig {
    /// While true, every command waits (up to its own timeout) before doing
    /// anything else, so a spec can see a build mid-step.
    hold: bool,
    /// Step label to the stderr it fails with (exit status 1).
    fail: HashMap<String, String>,
    /// Step label to its stdout, replacing the default.
    stdout: HashMap<String, String>,
}

/// The whole control file. Unknown fields are refused, so a spec that
/// misspells a field fails loudly instead of testing the default.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FakeConfig {
    /// API operations parked until the control file releases them. Tests
    /// use this boundary to close a browser with cleanup still in flight.
    hold_operations: std::collections::HashSet<String>,
    /// When set, only this key is accepted; any other answers 401.
    accepted_key: Option<String>,
    /// Sandboxes the account has besides the ones the helm created, as
    /// `GET /sandboxes` lists them, in the API's own shape.
    sandboxes: Vec<SandboxInfo>,
    /// Operation name (`list_sandboxes`, ...) to the failure it answers.
    fail: HashMap<String, InjectedFailure>,
    /// How many `get_snapshot` calls a new snapshot answers before it is
    /// completed. Default 1.
    snapshot_polls: Option<u32>,
    shell: ShellConfig,
}

/// One sandbox in the model.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct FakeSandbox {
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    /// `running`, `suspended` or `terminated`; transitions settle immediately.
    pub(crate) status: String,
    pub(crate) image: Option<String>,
    pub(crate) snapshot_id: Option<String>,
    pub(crate) resources: Resources,
    pub(crate) timeout_secs: u64,
    pub(crate) auto_resume: bool,
}

/// One snapshot in the model.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct FakeSnapshot {
    pub(crate) id: String,
    pub(crate) sandbox_id: String,
    /// `in_progress`, `completed` or `failed`.
    pub(crate) status: String,
    /// `get_snapshot` calls left before an in-progress snapshot settles.
    pub(crate) polls_left: u32,
}

/// Tensorlake's sandboxes and snapshots as the fake knows them. Plain data
/// plus the rules the build engine relies on; persisted as `state.json` by
/// the e2e fake and held in memory by the unit tests.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub(crate) struct FakeModel {
    pub(crate) sandboxes: Vec<FakeSandbox>,
    pub(crate) snapshots: Vec<FakeSnapshot>,
    /// Makes new snapshots settle as `failed` instead of `completed`.
    pub(crate) snapshots_fail: bool,
    /// The last ID number handed out.
    pub(crate) counter: u64,
}

impl FakeModel {
    /// A new ID shaped like Tensorlake's: 21 lowercase letters and digits.
    fn next_id(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!(
            "{prefix}{:0width$}",
            self.counter,
            width = 21 - prefix.len()
        )
    }

    /// The sandbox an argument names, by ID or by name. A terminated sandbox
    /// still answers to its ID, but no longer to its name: a name belongs to
    /// the live sandbox holding it, and Farhelm must not count on finding a
    /// terminated one by name (which is why builders' IDs are recorded).
    fn find(&self, sandbox: &str) -> Option<usize> {
        let by_id = self.sandboxes.iter().position(|s| s.id == sandbox);
        by_id.or_else(|| {
            self.sandboxes
                .iter()
                .rposition(|s| s.name.as_deref() == Some(sandbox) && s.status != "terminated")
        })
    }

    fn info(sandbox: &FakeSandbox) -> SandboxInfo {
        SandboxInfo {
            id: sandbox.id.clone(),
            name: sandbox.name.clone(),
            status: match sandbox.status.as_str() {
                "running" => SandboxStatus::Running,
                "suspended" => SandboxStatus::Suspended,
                _ => SandboxStatus::Terminated,
            },
            created_at: None,
            image: sandbox.image.clone(),
            resources: Some(sandbox.resources.clone()),
            snapshot_id: None,
            error_details: None,
            termination_reason: None,
            pending_reason: None,
            auto_resume: Some(sandbox.auto_resume),
        }
    }

    fn not_found() -> TensorlakeError {
        TensorlakeError::NotFound {
            message: "not found".into(),
        }
    }

    /// Preserve the same machine across suspend/resume. A terminated sandbox
    /// cannot be revived; repeated requests for the current state succeed.
    pub(crate) fn set_suspended(
        &mut self,
        sandbox: &str,
        suspended: bool,
    ) -> Result<(), TensorlakeError> {
        let index = self.find(sandbox).ok_or_else(Self::not_found)?;
        if self.sandboxes[index].status == "terminated" {
            return Err(Self::not_found());
        }
        self.sandboxes[index].status = if suspended { "suspended" } else { "running" }.into();
        Ok(())
    }

    /// The live sandboxes, as a listing shows them.
    pub(crate) fn live_sandboxes(&self) -> Vec<SandboxInfo> {
        self.sandboxes
            .iter()
            .filter(|s| s.status != "terminated")
            .map(Self::info)
            .collect()
    }

    pub(crate) fn get(&self, sandbox: &str) -> Result<SandboxInfo, TensorlakeError> {
        self.find(sandbox)
            .map(|index| Self::info(&self.sandboxes[index]))
            .ok_or_else(Self::not_found)
    }

    /// Create a running sandbox; a live sandbox with the same name is a 409.
    pub(crate) fn create(
        &mut self,
        request: &CreateSandbox,
    ) -> Result<CreatedSandbox, TensorlakeError> {
        if self
            .sandboxes
            .iter()
            .any(|s| s.name.as_deref() == Some(&request.name) && s.status != "terminated")
        {
            return Err(TensorlakeError::Conflict {
                message: format!("a sandbox named {} already exists", request.name),
            });
        }
        let id = self.next_id("fakesbx");
        let (image, snapshot_id) = match &request.source {
            SandboxSource::Image(image) => (Some(image.clone()), None),
            SandboxSource::Snapshot(snapshot) => {
                if !self.snapshots.iter().any(|s| &s.id == snapshot) {
                    return Err(Self::not_found());
                }
                (None, Some(snapshot.clone()))
            }
        };
        self.sandboxes.push(FakeSandbox {
            id: id.clone(),
            name: Some(request.name.clone()),
            status: "running".into(),
            image,
            snapshot_id,
            resources: request.resources.clone(),
            timeout_secs: request.timeout_secs,
            auto_resume: request.auto_resume,
        });
        Ok(CreatedSandbox {
            sandbox_id: id,
            status: SandboxStatus::Running,
        })
    }

    /// Terminate by ID or name; already terminated is success, unknown 404.
    pub(crate) fn terminate(&mut self, sandbox: &str) -> Result<(), TensorlakeError> {
        let index = self.find(sandbox).ok_or_else(Self::not_found)?;
        self.sandboxes[index].status = "terminated".into();
        Ok(())
    }

    /// Start a snapshot of a running sandbox; it settles after `polls`
    /// `get_snapshot` calls.
    pub(crate) fn snapshot(
        &mut self,
        sandbox: &str,
        polls: u32,
    ) -> Result<CreatedSnapshot, TensorlakeError> {
        let index = self.find(sandbox).ok_or_else(Self::not_found)?;
        if self.sandboxes[index].status != "running" {
            return Err(TensorlakeError::Conflict {
                message: "the sandbox is not running".into(),
            });
        }
        let sandbox_id = self.sandboxes[index].id.clone();
        let id = self.next_id("fakesnap");
        self.snapshots.push(FakeSnapshot {
            id: id.clone(),
            sandbox_id,
            status: "in_progress".into(),
            polls_left: polls,
        });
        Ok(CreatedSnapshot { snapshot_id: id })
    }

    /// Plant a settled snapshot of `sandbox_id` (a suspend snapshot, say),
    /// for tests that seed leftovers.
    #[cfg(test)]
    pub(crate) fn plant_snapshot(&mut self, sandbox_id: &str) -> String {
        let id = self.next_id("fakesnap");
        self.snapshots.push(FakeSnapshot {
            id: id.clone(),
            sandbox_id: sandbox_id.into(),
            status: "completed".into(),
            polls_left: 0,
        });
        id
    }

    fn snapshot_info(snapshot: &FakeSnapshot) -> SnapshotInfo {
        SnapshotInfo {
            id: snapshot.id.clone(),
            sandbox_id: Some(snapshot.sandbox_id.clone()),
            status: match snapshot.status.as_str() {
                "completed" => SnapshotStatus::Completed,
                "failed" => SnapshotStatus::Failed,
                _ => SnapshotStatus::Unknown,
            },
            snapshot_type: Some(SnapshotType::Filesystem),
            size_bytes: None,
            created_at: None,
        }
    }

    /// One poll of a snapshot, which counts down its settling.
    pub(crate) fn poll_snapshot(&mut self, id: &str) -> Result<SnapshotInfo, TensorlakeError> {
        let fail = self.snapshots_fail;
        let snapshot = self
            .snapshots
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(Self::not_found)?;
        if snapshot.status == "in_progress" {
            if snapshot.polls_left == 0 {
                snapshot.status = if fail { "failed" } else { "completed" }.into();
            } else {
                snapshot.polls_left -= 1;
            }
        }
        Ok(Self::snapshot_info(snapshot))
    }

    pub(crate) fn list_snapshots(&self) -> SnapshotList {
        SnapshotList {
            snapshots: self.snapshots.iter().map(Self::snapshot_info).collect(),
        }
    }

    pub(crate) fn delete_snapshot(&mut self, id: &str) -> Result<(), TensorlakeError> {
        let before = self.snapshots.len();
        self.snapshots.retain(|s| s.id != id);
        if self.snapshots.len() == before {
            return Err(Self::not_found());
        }
        Ok(())
    }

    /// Whether the sandbox named `sandbox` is live and running, which is
    /// what an ssh connection needs.
    pub(crate) fn is_running(&self, sandbox: &str) -> bool {
        self.find(sandbox)
            .is_some_and(|index| self.sandboxes[index].status == "running")
    }
}

/// The stdout a step answers when the config names none: the agent steps
/// answer a version line, as the real scripts do.
pub(crate) fn default_stdout(label: &str) -> &'static str {
    match label {
        "claude" => "1.0.0 (Claude Code)\n",
        "codex" => "codex-cli 0.1.0\n",
        _ => "",
    }
}

/// One explicit fake per helm, rooted in the control directory.
pub(crate) struct E2eTensorlake {
    root: PathBuf,
    event_write: tokio::sync::Mutex<()>,
    /// Serializes read-modify-write of `state.json`.
    state_write: tokio::sync::Mutex<()>,
}

impl E2eTensorlake {
    /// Simulate a long-lived tunnel with failure injection. The production
    /// lease still owns cancellation and retry; no real listener is claimed.
    pub(crate) async fn run_tunnels(
        &self,
        _sandbox: &str,
        tx: &tokio::sync::watch::Sender<farhelm_proto::tensorlake::TunnelView>,
        events: &crate::feed::FleetEvents,
    ) -> Result<(), String> {
        loop {
            let config = self.config().await.map_err(|error| error.to_string())?;
            if let Some(failure) = config.fail.get("tunnels") {
                return Err(failure.message.clone());
            }
            if tx.borrow().state != farhelm_proto::tensorlake::TunnelState::Up {
                tx.send_modify(|view| view.state = farhelm_proto::tensorlake::TunnelState::Up);
                events.bump();
            }
            tokio::time::sleep(HOLD_POLL).await;
        }
    }

    /// Accept the directory only if it REALLY lies inside the helm's private
    /// state directory and carries the marker. Both paths are canonicalized
    /// first, for the reason `provisioning/e2e.rs` records: a lexical prefix
    /// check accepts `..` components and symlinks that point anywhere.
    pub(crate) fn new(root: PathBuf, helm_state_dir: &Path) -> anyhow::Result<Self> {
        if !root.is_absolute() {
            bail!("{E2E_TENSORLAKE_ENV} must name a directory inside the helm state directory");
        }
        let root = std::fs::canonicalize(&root)
            .with_context(|| format!("resolving {E2E_TENSORLAKE_ENV} {}", root.display()))?;
        let state = std::fs::canonicalize(helm_state_dir).with_context(|| {
            format!(
                "resolving the helm state directory {}",
                helm_state_dir.display()
            )
        })?;
        if !root.starts_with(&state) {
            bail!("{E2E_TENSORLAKE_ENV} must name a directory inside the helm state directory");
        }
        let marker = std::fs::read_to_string(root.join("ENABLED"))
            .with_context(|| format!("reading the {E2E_TENSORLAKE_ENV} marker"))?;
        if marker != E2E_TENSORLAKE_MARKER {
            bail!("{E2E_TENSORLAKE_ENV} has no valid E2E marker");
        }
        Ok(Self {
            root,
            event_write: tokio::sync::Mutex::new(()),
            state_write: tokio::sync::Mutex::new(()),
        })
    }

    /// Read the control file. A missing file is the default (any key, no
    /// sandboxes); a malformed one fails the call loudly, as a 500.
    async fn config(&self) -> Result<FakeConfig, TensorlakeError> {
        match tokio::fs::read(self.root.join("config.json")).await {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| TensorlakeError::Status {
                status: 500,
                message: format!("the e2e Tensorlake fake's config.json is invalid: {error}"),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FakeConfig::default()),
            Err(error) => Err(TensorlakeError::Status {
                status: 500,
                message: format!("the e2e Tensorlake fake could not read config.json: {error}"),
            }),
        }
    }

    /// Read `state.json`; missing is the empty model.
    async fn read_model(&self) -> Result<FakeModel, TensorlakeError> {
        match tokio::fs::read(self.root.join("state.json")).await {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| TensorlakeError::Status {
                status: 500,
                message: format!("the e2e Tensorlake fake's state.json is invalid: {error}"),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FakeModel::default()),
            Err(error) => Err(TensorlakeError::Status {
                status: 500,
                message: format!("the e2e Tensorlake fake could not read state.json: {error}"),
            }),
        }
    }

    /// Apply `change` to the model and write it back atomically (a temporary
    /// file renamed over `state.json`), so a spec never reads half a file.
    async fn change_model<T>(
        &self,
        change: impl FnOnce(&mut FakeModel) -> Result<T, TensorlakeError>,
    ) -> Result<T, TensorlakeError> {
        let _guard = self.state_write.lock().await;
        let mut model = self.read_model().await?;
        let result = change(&mut model)?;
        let write = async {
            let next = self.root.join("state.json.next");
            tokio::fs::write(&next, serde_json::to_vec_pretty(&model)?).await?;
            tokio::fs::rename(&next, self.root.join("state.json")).await?;
            anyhow::Ok(())
        };
        write.await.map_err(|error| TensorlakeError::Status {
            status: 500,
            message: format!("the e2e Tensorlake fake could not write state.json: {error}"),
        })?;
        Ok(result)
    }

    /// Record one call, then decide whether it gets past authentication and
    /// injected failures. The event names the operation and whether the key
    /// matched, never the key.
    async fn admit(&self, operation: &str, key: &ApiKey) -> Result<FakeConfig, TensorlakeError> {
        let mut config = self.config().await?;
        let key_accepted = config
            .accepted_key
            .as_deref()
            .is_none_or(|accepted| accepted == key.expose());
        self.record(serde_json::json!({ "op": operation, "key_accepted": key_accepted }))
            .await;
        if !key_accepted {
            return Err(TensorlakeError::Auth {
                status: 401,
                message: "the e2e Tensorlake fake does not accept this key".into(),
            });
        }
        while config.hold_operations.contains(operation) {
            tokio::time::sleep(HOLD_POLL).await;
            config = self.config().await?;
        }
        if let Some(failure) = config.fail.get(operation) {
            return Err(TensorlakeError::from_status(
                failure.status,
                failure.message.clone(),
            ));
        }
        Ok(config)
    }

    /// Append one event line. Serialized so concurrent calls cannot
    /// interleave partial lines; a write failure is ignored, since the event
    /// log is the spec's evidence, not the helm's.
    async fn record(&self, event: serde_json::Value) {
        let _guard = self.event_write.lock().await;
        let Ok(mut file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("events.jsonl"))
            .await
        else {
            return;
        };
        // tokio's File finishes a write in the background; without the flush
        // the line could land after the reply that a spec reads it after.
        let _ = file.write_all(format!("{event}\n").as_bytes()).await;
        let _ = file.flush().await;
    }
}

#[async_trait]
impl TensorlakeApi for E2eTensorlake {
    async fn list_sandboxes(
        &self,
        key: &ApiKey,
        _cursor: Option<&str>,
    ) -> Result<SandboxPage, TensorlakeError> {
        let config = self.admit("list_sandboxes", key).await?;
        let mut sandboxes = config.sandboxes;
        sandboxes.extend(self.read_model().await?.live_sandboxes());
        Ok(SandboxPage {
            sandboxes,
            next_cursor: None,
        })
    }

    async fn get_sandbox(
        &self,
        key: &ApiKey,
        sandbox: &str,
    ) -> Result<SandboxInfo, TensorlakeError> {
        self.admit("get_sandbox", key).await?;
        self.read_model().await?.get(sandbox)
    }

    async fn create_sandbox(
        &self,
        key: &ApiKey,
        request: &CreateSandbox,
    ) -> Result<CreatedSandbox, TensorlakeError> {
        self.admit("create_sandbox", key).await?;
        self.change_model(|model| model.create(request)).await
    }

    async fn suspend_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError> {
        self.admit("suspend_sandbox", key).await?;
        self.change_model(|model| model.set_suspended(sandbox, true))
            .await
    }

    async fn resume_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError> {
        self.admit("resume_sandbox", key).await?;
        self.change_model(|model| model.set_suspended(sandbox, false))
            .await
    }

    async fn terminate_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError> {
        self.admit("terminate_sandbox", key).await?;
        self.change_model(|model| model.terminate(sandbox)).await
    }

    async fn create_snapshot(
        &self,
        key: &ApiKey,
        sandbox: &str,
        _snapshot_type: SnapshotType,
    ) -> Result<CreatedSnapshot, TensorlakeError> {
        let config = self.admit("create_snapshot", key).await?;
        let polls = config.snapshot_polls.unwrap_or(1);
        self.change_model(|model| model.snapshot(sandbox, polls))
            .await
    }

    async fn get_snapshot(
        &self,
        key: &ApiKey,
        snapshot: &str,
    ) -> Result<SnapshotInfo, TensorlakeError> {
        self.admit("get_snapshot", key).await?;
        self.change_model(|model| model.poll_snapshot(snapshot))
            .await
    }

    async fn list_snapshots(&self, key: &ApiKey) -> Result<SnapshotList, TensorlakeError> {
        self.admit("list_snapshots", key).await?;
        Ok(self.read_model().await?.list_snapshots())
    }

    async fn delete_snapshot(&self, key: &ApiKey, snapshot: &str) -> Result<(), TensorlakeError> {
        self.admit("delete_snapshot", key).await?;
        self.change_model(|model| model.delete_snapshot(snapshot))
            .await
    }
}

/// The fake shell: never runs ssh. It records the step, waits while the
/// config holds commands, refuses a sandbox the model does not have running
/// (which catches a build that connects before its sandbox runs), and then
/// fails or answers as the config says.
#[async_trait]
impl SandboxShell for E2eTensorlake {
    async fn run(
        &self,
        _access: &TensorlakeSshAccess,
        sandbox: &str,
        label: &str,
        _script: &str,
        timeout: Duration,
    ) -> Result<ShellOutput, ShellFailure> {
        let unavailable = |error: TensorlakeError| ShellFailure::Unavailable(error.to_string());
        self.record(serde_json::json!({ "op": "shell", "sandbox": sandbox, "label": label }))
            .await;
        let deadline = tokio::time::Instant::now() + timeout;
        let config = loop {
            let config = self.config().await.map_err(unavailable)?;
            if !config.shell.hold {
                break config;
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(ShellFailure::TimedOut(timeout));
            }
            tokio::time::sleep(HOLD_POLL).await;
        };
        if !self
            .read_model()
            .await
            .map_err(unavailable)?
            .is_running(sandbox)
        {
            return Err(ShellFailure::Unreachable(format!(
                "the e2e fake has no running sandbox named {sandbox}"
            )));
        }
        if let Some(stderr) = config.shell.fail.get(label) {
            return Err(ShellFailure::Exited {
                code: 1,
                stderr: stderr.clone(),
            });
        }
        Ok(ShellOutput {
            stdout: config
                .shell
                .stdout
                .get(label)
                .cloned()
                .unwrap_or_else(|| default_stdout(label).to_string()),
        })
    }
}
