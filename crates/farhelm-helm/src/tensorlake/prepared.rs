//! The second recipe snapshot contains Farhelm's installation, with no captured
//! supervisor state. Provisioning owns installation and verification; the recipe
//! engine owns the billed sandbox and snapshot lifecycle.

use async_trait::async_trait;
use farhelm_proto::tensorlake::recipes::{PreparedSnapshotView, PreparedStaleReason};
use serde::{Deserialize, Serialize};

use super::ssh::TensorlakeSshAccess;

// Existing snapshot consumers keep their import path for these shared facts.
pub(crate) use crate::environment::recipes::PreparedInstallFacts;

/// Tensorlake supplies its gateway access and logind setup before entering
/// the common route/home installer. Keeping this wrapper here preserves the
/// build engine's existing injected machine-preparation seam.
pub(crate) struct TensorlakeSnapshotPreparer {
    backend: std::sync::Arc<dyn crate::provisioning::ProvisioningBackend>,
    installer: std::sync::Arc<dyn crate::environment::EnvironmentPreparer>,
    access: TensorlakeSshAccess,
}

impl TensorlakeSnapshotPreparer {
    /// Freeze the same selected backend and key coordinates as provisioning.
    pub(crate) fn new(
        backend: std::sync::Arc<dyn crate::provisioning::ProvisioningBackend>,
        installer: std::sync::Arc<dyn crate::environment::EnvironmentPreparer>,
        state: &std::path::Path,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            backend,
            installer,
            access: TensorlakeSshAccess::for_state_dir(state)?,
        })
    }
}

#[async_trait]
impl SnapshotPreparer for TensorlakeSnapshotPreparer {
    async fn prepare(
        &self,
        sandbox: &str,
        access: &TensorlakeSshAccess,
    ) -> anyhow::Result<PreparedInstallFacts> {
        anyhow::ensure!(
            *access == self.access,
            "the snapshot preparer's SSH access differs from the selected helm access"
        );
        let route = super::ssh::route(super::ssh::destination(sandbox));
        let home = self
            .backend
            .prepare_tensorlake_sandbox(&crate::provisioning::ProvisioningTarget::Remote(
                route.clone(),
            ))
            .await?;
        self.installer.prepare(&route, home).await
    }
}

/// Install and leave a sandbox safe to checkpoint without delivering credentials.
///
/// Success requires a boot-enabled supervisor unit, a stopped supervisor, an
/// absent whole state directory (including supervisor.db), and an empty machine-id.
/// The implementation must refuse copied supervisor state before installation.
/// Inherited authentication belongs to the base author and is left unchanged.
/// Errors must be suitable for bounded display; never include credentials or raw
/// remote captures. Cancellation leaves vendor cleanup to the recipe engine.
#[async_trait]
pub(crate) trait SnapshotPreparer: Send + Sync {
    async fn prepare(
        &self,
        sandbox: &str,
        access: &TensorlakeSshAccess,
    ) -> anyhow::Result<PreparedInstallFacts>;
}

/// An explicit offline seam: tests unrelated to preparation must not accidentally
/// install software or silently pretend a safe checkpoint was produced.
pub(crate) struct UnavailablePreparer;

#[async_trait]
impl SnapshotPreparer for UnavailablePreparer {
    async fn prepare(
        &self,
        _: &str,
        _: &TensorlakeSshAccess,
    ) -> anyhow::Result<PreparedInstallFacts> {
        anyhow::bail!("snapshot preparation is unavailable on this helm")
    }
}

/// A durably owned installation checkpoint and the exact built source it used.
/// Recipe edits do not invalidate it until Rebuild publishes a different source.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct PreparedSnapshot {
    pub(crate) id: String,
    pub(crate) prepared_at: i64,
    pub(crate) farhelm_version: String,
    pub(crate) source_snapshot: String,
    pub(crate) disk_gb: u32,
    pub(crate) facts: PreparedInstallFacts,
}

impl PreparedSnapshot {
    /// Exactly three facts govern reuse. Version wins display precedence over
    /// source, then age; clock rollback adds no fourth invalidation rule.
    pub(crate) fn stale_reason(
        &self,
        now: i64,
        version: &str,
        source: Option<&str>,
    ) -> Option<PreparedStaleReason> {
        crate::environment::recipes::prepared_stale_reason(
            self.prepared_at,
            &self.farhelm_version,
            &self.source_snapshot,
            now,
            version,
            source,
        )
    }

    /// The public summary excludes install paths, which only clone admission uses.
    pub(crate) fn view(
        &self,
        now: i64,
        version: &str,
        source: Option<&str>,
    ) -> PreparedSnapshotView {
        PreparedSnapshotView {
            id: self.id.clone(),
            prepared_at: self.prepared_at,
            farhelm_version: self.farhelm_version.clone(),
            source_snapshot: self.source_snapshot.clone(),
            stale_reason: self.stale_reason(now, version, source),
        }
    }
}
