//! A filesystem-controlled stand-in for Tensorlake's API, used only by the
//! browser end-to-end suite.
//!
//! The browser suite drives the ordinary debug binary, not a `cfg(test)`
//! build, so this fake is selected at runtime, the same way the injected
//! provisioning backend is (`provisioning/e2e.rs`): an unmistakably
//! test-named environment variable read once at helm startup, naming a
//! directory that must lie inside the helm's private state directory and
//! carry a marker file. Startup prints a warning before serving. Everything
//! above the [`TensorlakeApi`] seam (the routes, the stored key, the Test
//! button's handling) stays the shipped code.
//!
//! The directory is the control plane. A spec writes `config.json` (read
//! afresh on every call, so a spec can change behavior mid-run by replacing
//! it atomically) and reads `events.jsonl`, one line per call. The config
//! names the account's sandboxes, an optional key the fake accepts (any
//! other key gets a 401, which is how a spec proves the helm sends the key
//! it saved without the fake ever writing a key down), and failures to
//! inject per operation. Operations this PR does not exercise answer 501;
//! later PRs extend the config with what creating, suspending and removing
//! sandboxes need.

use super::api::{
    ApiKey, CreateSandbox, CreatedSandbox, CreatedSnapshot, SandboxInfo, SandboxPage, SnapshotInfo,
    SnapshotList, SnapshotType, TensorlakeApi, TensorlakeError,
};
use anyhow::{Context as _, bail};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

pub(crate) const E2E_TENSORLAKE_ENV: &str = "FARHELM_E2E_TENSORLAKE_DIR";
const E2E_TENSORLAKE_MARKER: &str = "farhelm-e2e-tensorlake-v1\n";

/// A failure to answer instead of an operation's normal reply.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct InjectedFailure {
    status: u16,
    #[serde(default)]
    message: String,
}

/// The whole control file. Unknown fields are refused, so a spec that
/// misspells a field fails loudly instead of testing the default.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FakeConfig {
    /// When set, only this key is accepted; any other answers 401.
    accepted_key: Option<String>,
    /// The account's live sandboxes, as `GET /sandboxes` lists them, in the
    /// API's own shape.
    sandboxes: Vec<SandboxInfo>,
    /// Operation name (`list_sandboxes`, ...) to the failure it answers.
    fail: HashMap<String, InjectedFailure>,
}

/// One explicit fake per helm, rooted in the control directory.
pub(crate) struct E2eTensorlake {
    root: PathBuf,
    event_write: tokio::sync::Mutex<()>,
}

impl E2eTensorlake {
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

    /// Record one call, then decide whether it gets past authentication and
    /// injected failures. The event names the operation and whether the key
    /// matched, never the key.
    async fn admit(&self, operation: &str, key: &ApiKey) -> Result<FakeConfig, TensorlakeError> {
        let config = self.config().await?;
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

    /// The reply for an operation this fake does not implement yet.
    async fn unimplemented<T>(&self, operation: &str, key: &ApiKey) -> Result<T, TensorlakeError> {
        self.admit(operation, key).await?;
        Err(TensorlakeError::Status {
            status: 501,
            message: format!("the e2e Tensorlake fake does not implement {operation}"),
        })
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
        Ok(SandboxPage {
            sandboxes: config.sandboxes,
            next_cursor: None,
        })
    }

    async fn get_sandbox(
        &self,
        key: &ApiKey,
        _sandbox: &str,
    ) -> Result<SandboxInfo, TensorlakeError> {
        self.unimplemented("get_sandbox", key).await
    }

    async fn create_sandbox(
        &self,
        key: &ApiKey,
        _request: &CreateSandbox,
    ) -> Result<CreatedSandbox, TensorlakeError> {
        self.unimplemented("create_sandbox", key).await
    }

    async fn suspend_sandbox(&self, key: &ApiKey, _sandbox: &str) -> Result<(), TensorlakeError> {
        self.unimplemented("suspend_sandbox", key).await
    }

    async fn resume_sandbox(&self, key: &ApiKey, _sandbox: &str) -> Result<(), TensorlakeError> {
        self.unimplemented("resume_sandbox", key).await
    }

    async fn terminate_sandbox(&self, key: &ApiKey, _sandbox: &str) -> Result<(), TensorlakeError> {
        self.unimplemented("terminate_sandbox", key).await
    }

    async fn create_snapshot(
        &self,
        key: &ApiKey,
        _sandbox: &str,
        _snapshot_type: SnapshotType,
    ) -> Result<CreatedSnapshot, TensorlakeError> {
        self.unimplemented("create_snapshot", key).await
    }

    async fn get_snapshot(
        &self,
        key: &ApiKey,
        _snapshot: &str,
    ) -> Result<SnapshotInfo, TensorlakeError> {
        self.unimplemented("get_snapshot", key).await
    }

    async fn list_snapshots(&self, key: &ApiKey) -> Result<SnapshotList, TensorlakeError> {
        self.unimplemented("list_snapshots", key).await
    }

    async fn delete_snapshot(&self, key: &ApiKey, _snapshot: &str) -> Result<(), TensorlakeError> {
        self.unimplemented("delete_snapshot", key).await
    }
}
