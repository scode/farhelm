//! Explicit cleanup of reserved-name sandboxes this helm does not own.
//!
//! Names are the only ownership tag Tensorlake supplies. Host rows and all
//! saved recipes exclude their sandboxes, including a builder or preparer known
//! only by its deterministic name after an interrupted create. Nothing is reclaimed
//! automatically; another helm's tag is a warning, not permission to delete.

use super::{
    Tensorlake,
    api::{SandboxStatus, TensorlakeError, list_all_sandboxes_by},
    build::{BUILDER_PREFIX, PREPARER_PREFIX, builder_name, preparer_name},
    config::helm_tag,
};
use crate::AppState;
use anyhow::Context as _;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use farhelm_proto::tensorlake::{OrphanCleanup, OrphanDiscovery, OrphanSandbox, OrphanSandboxList};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

pub(crate) const HOST_PREFIX: &str = "farhelm-host-";
pub(crate) const SESSION_PREFIX: &str = "farhelm-sess-";

/// How long a deletion's sandbox lookup may take before its cleanup is
/// recorded. Half the host reply bound, so that a Tensorlake that does not
/// answer it fails the request with its own reason well before the route's
/// timeout reply could promise a cleanup nothing has recorded; the local
/// reads after it are milliseconds in practice. See
/// [`Tensorlake::delete_orphan`].
const ADMISSION_LOOKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// In-flight and failed explicit deletions, keyed by canonical vendor ID.
/// This is operation state for one helm lifetime, not an ownership ledger
/// or a queue that reclaims resources without another user request.
#[derive(Default)]
pub(super) struct Cleanups(Mutex<HashMap<String, AdmittedCleanup>>);

/// Only the key that admitted cleanup can resume it. A missing sandbox under another key says nothing about
/// whether the original account's snapshots are gone.
#[derive(Clone)]
struct AdmittedCleanup {
    key_digest: [u8; 32],
    row: OrphanSandbox,
}

impl AdmittedCleanup {
    /// Check before using retained admission or replacing it after a lookup.
    fn require_key(&self, digest: [u8; 32]) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.key_digest == digest,
            "This cleanup belongs to a different Tensorlake API key. Restore the key that started it to retry."
        );
        Ok(())
    }
}

impl Cleanups {
    /// Read without lending the operation mutex across any vendor call.
    fn snapshot(&self) -> Vec<OrphanSandbox> {
        self.0
            .lock()
            .expect("orphan cleanups poisoned")
            .values()
            .map(|cleanup| cleanup.row.clone())
            .collect()
    }
}

/// Return the creator tag only for the product resource namespaces. The
/// Tensorlake test harness's farhelm-sbx names must never become cleanup candidates.
fn creator(name: &str) -> Option<&str> {
    let suffix = name
        .strip_prefix(HOST_PREFIX)
        .or_else(|| name.strip_prefix(SESSION_PREFIX))
        .or_else(|| name.strip_prefix(BUILDER_PREFIX))
        .or_else(|| name.strip_prefix(PREPARER_PREFIX))?;
    let (tag, rest) = suffix.split_once('-')?;
    (!tag.is_empty() && !rest.is_empty()).then_some(tag)
}

/// The namespace, rather than today's tag length, is the vendor-side
/// ownership signal. An older helm's tag spelling must not hide a leftover.
fn reserved(name: &str) -> bool {
    name.starts_with(HOST_PREFIX)
        || name.starts_with(SESSION_PREFIX)
        || name.starts_with(BUILDER_PREFIX)
        || name.starts_with(PREPARER_PREFIX)
}

impl Tensorlake {
    /// Current host names and deterministic builders/preparers for every recipe,
    /// not just a build whose create has already returned its ID. A host
    /// reserves its name even with schema 72's create marker still pending
    /// and no sandbox ID or admitted install coordinates yet.
    async fn known_sandboxes(&self) -> anyhow::Result<(String, HashSet<String>)> {
        let tag = helm_tag(&self.0.store.registry_namespace().await?);
        let mut known: HashSet<_> = self
            .0
            .store
            .list_hosts()
            .await?
            .into_iter()
            .filter_map(|row| row.tensorlake.map(|binding| binding.sandbox))
            .collect();
        for (name, _, _) in self.0.store.tensorlake_recipe_rows().await? {
            known.insert(builder_name(&tag, &name));
            known.insert(preparer_name(&tag, &name));
        }
        Ok((tag, known))
    }

    /// Keep cleanup state authoritative even if discovery fails. Snapshot it after discovery so a cleanup that
    /// finished during the read disappears from the response, including during a provider outage.
    pub(crate) async fn orphan_sandboxes_by(
        &self,
        reply_by: tokio::time::Instant,
    ) -> OrphanSandboxList {
        let discovery = match self.discover_orphan_sandboxes_by(reply_by).await {
            Ok(sandboxes) => OrphanDiscovery::Ok { sandboxes },
            Err(error) => OrphanDiscovery::Failed {
                reason: error.to_string(),
            },
        };
        let mut cleanups = self.0.orphan_cleanups.snapshot();
        cleanups.sort_by(|a, b| a.name.cmp(&b.name));
        OrphanSandboxList {
            discovery,
            cleanups,
        }
    }

    /// Give local and provider reads the same deadline. A local stall must still let Settings get the cleanup snapshot.
    /// Read ownership after listing because a host reserves its name before Create. Reading ownership first could list
    /// a sandbox created during the vendor read as a leftover.
    async fn discover_orphan_sandboxes_by(
        &self,
        reply_by: tokio::time::Instant,
    ) -> anyhow::Result<Vec<OrphanSandbox>> {
        let key = tokio::time::timeout_at(reply_by, self.api_key())
            .await
            .context("Farhelm timed out reading its Tensorlake settings")?
            .map_err(anyhow::Error::msg)?;
        let sandboxes = list_all_sandboxes_by(reply_by, self.api(), &key).await?;
        let (tag, known) = tokio::time::timeout_at(reply_by, self.known_sandboxes())
            .await
            .context("Farhelm timed out checking which Tensorlake sandboxes it owns")??;
        let mut by_id = HashMap::new();
        for sandbox in sandboxes {
            let Some(name) = sandbox.name else {
                continue;
            };
            if !reserved(&name) {
                continue;
            }
            if known.contains(&name) {
                continue;
            }
            by_id.insert(
                sandbox.id.clone(),
                OrphanSandbox {
                    id: sandbox.id,
                    from_another_helm: creator(&name).is_some_and(|other| other != tag.as_str()),
                    name,
                    created_at: sandbox.created_at,
                    state: status(sandbox.status).into(),
                    cleanup: None,
                },
            );
        }
        let mut orphans: Vec<_> = by_id.into_values().collect();
        orphans.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(orphans)
    }

    /// [`Self::orphan_sandboxes_by`] with its bound starting now.
    #[cfg(test)]
    pub(crate) async fn orphan_sandboxes(&self) -> OrphanSandboxList {
        self.orphan_sandboxes_by(tokio::time::Instant::now() + crate::sessions::HOST_REPLY_BOUND)
            .await
    }

    /// Recheck the tag and local ownership at deletion time. A stale UI list
    /// cannot delete a sandbox that this helm has since adopted.
    ///
    /// Everything before the cleanup is recorded as running (the lookup and
    /// the ownership checks) has to finish well inside the host reply bound:
    /// the route's timeout reply says the cleanup carries on and the list
    /// shows how it ends, which is only true once it is recorded. So the
    /// lookup gets [`ADMISSION_LOOKUP_TIMEOUT`], and a Tensorlake that does
    /// not answer it ends the request with its own failure first.
    async fn delete_orphan(&self, id: &str) -> anyhow::Result<()> {
        let key = self.api_key().await.map_err(anyhow::Error::msg)?;
        let key_digest = key.cleanup_digest();
        let previous = self
            .0
            .orphan_cleanups
            .0
            .lock()
            .expect("orphan cleanups poisoned")
            .get(id)
            .cloned();
        let mut row = match previous {
            Some(cleanup) => {
                cleanup.require_key(key_digest)?;
                cleanup.row
            }
            None => {
                let sandbox = super::api::bounded_by(
                    ADMISSION_LOOKUP_TIMEOUT,
                    self.api().get_sandbox(&key, id),
                )
                .await?;
                let name = sandbox
                    .name
                    .ok_or_else(|| anyhow::anyhow!("This sandbox has no Farhelm name"))?;
                let tag = helm_tag(&self.0.store.registry_namespace().await?);
                OrphanSandbox {
                    id: sandbox.id,
                    from_another_helm: creator(&name).is_some_and(|other| other != tag.as_str()),
                    name,
                    created_at: sandbox.created_at,
                    state: status(sandbox.status).into(),
                    cleanup: None,
                }
            }
        };
        let name = row.name.as_str();
        if !reserved(name) {
            anyhow::bail!("This sandbox was not created by Farhelm");
        }
        let (_, known) = self.known_sandboxes().await?;
        if known.contains(name) {
            anyhow::bail!("This sandbox belongs to a host or recipe on this helm; delete it there");
        }
        {
            let mut operations = self
                .0
                .orphan_cleanups
                .0
                .lock()
                .expect("orphan cleanups poisoned");
            // Another request could have recorded the canonical ID during the lookup.
            if let Some(cleanup) = operations.get(&row.id) {
                cleanup.require_key(key_digest)?;
                anyhow::ensure!(
                    cleanup.row.cleanup != Some(OrphanCleanup::Running),
                    "Cleanup of this sandbox is already running"
                );
            }
            row.cleanup = Some(OrphanCleanup::Running);
            row.state = "Cleanup in progress".into();
            operations.insert(
                row.id.clone(),
                AdmittedCleanup {
                    key_digest,
                    row: row.clone(),
                },
            );
        }
        let result = self.sweep_orphan(&key, &row).await;
        let mut operations = self
            .0
            .orphan_cleanups
            .0
            .lock()
            .expect("orphan cleanups poisoned");
        match &result {
            Ok(()) => {
                operations.remove(&row.id);
            }
            Err(error) => {
                row.cleanup = Some(OrphanCleanup::Failed {
                    reason: error.to_string(),
                });
                row.state = "Cleanup unfinished".into();
                operations.insert(row.id.clone(), AdmittedCleanup { key_digest, row });
            }
        }
        result
    }

    /// The destructive part of an already admitted operation. Every retry
    /// uses the retained canonical ID, even if the vendor forgets the name
    /// or the terminated sandbox itself before its snapshots are swept.
    async fn sweep_orphan(
        &self,
        key: &super::api::ApiKey,
        row: &OrphanSandbox,
    ) -> anyhow::Result<()> {
        let id = row.id.as_str();
        match super::hosts::bounded(self.api().terminate_sandbox(key, id)).await {
            Ok(()) | Err(TensorlakeError::NotFound { .. }) => {}
            Err(error) => return Err(error.into()),
        }
        super::hosts::wait_until_terminated(self.api(), key, &row.name, id).await?;
        let snapshots = super::hosts::bounded(self.api().list_snapshots(key)).await?;
        for snapshot in snapshots
            .snapshots
            .into_iter()
            .filter(|snapshot| snapshot.sandbox_id.as_deref() == Some(id))
        {
            match super::hosts::bounded(self.api().delete_snapshot(key, &snapshot.id)).await {
                Ok(()) | Err(TensorlakeError::NotFound { .. }) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

/// Stable display words for vendor states, including an unknown future one.
fn status(status: SandboxStatus) -> &'static str {
    match status {
        SandboxStatus::Pending => "pending",
        SandboxStatus::Running => "running",
        SandboxStatus::Snapshotting => "snapshotting",
        SandboxStatus::Suspending => "suspending",
        SandboxStatus::Suspended => "suspended",
        SandboxStatus::Terminated => "terminated",
        SandboxStatus::ResumeFailed => "resume failed",
        SandboxStatus::Unknown => "unknown",
    }
}

/// Give Settings the result of bounded discovery and the current cleanup rows in the same reply.
/// A provider timeout is a discovery failure, so Settings can keep the last discovered rows and apply current cleanup.
pub(crate) async fn list(State(state): State<Arc<AppState>>) -> Response {
    let reply_by = tokio::time::Instant::now() + crate::sessions::HOST_REPLY_BOUND;
    axum::Json(state.tensorlake.orphan_sandboxes_by(reply_by).await).into_response()
}

/// Helm-owned deletion survives the confirming client closing mid-request,
/// and answers within the host reply bound (`crate::sessions::HOST_REPLY_BOUND`).
///
/// A deletion makes several Tensorlake calls of up to 20 seconds each, waits
/// up to 30 seconds for the termination, and deletes the sandbox's snapshots
/// one call at a time, so it can take minutes, far longer than the GUI's
/// one-at-a-time channel lets a command wait. Its work runs on a task of its
/// own, which also keeps it going when the caller leaves, and carries on past
/// a timeout reply. Its outcome shows in the listing: the sandbox stays
/// "Cleanup in progress" until the work ends, and then is gone or offers a
/// retry with its reason, and the feed bump at the end makes Settings read
/// the listing again. In practice the timeout reply comes only once the
/// cleanup is recorded: before that, the lookup gets half the bound and the
/// local reads are not bounded but take milliseconds (see
/// [`Tensorlake::delete_orphan`]).
pub(crate) async fn delete(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    crate::sessions::within_host_reply_bound_saying(
        async move {
            let result = state.tensorlake.delete_orphan(&id).await;
            state.manager.events().bump();
            match result {
                Ok(()) => StatusCode::NO_CONTENT.into_response(),
                Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
            }
        },
        format!(
            "Tensorlake did not finish within {} seconds; the cleanup carries on, and Sandboxes \
             not on this helm shows how it ends",
            crate::sessions::HOST_REPLY_BOUND.as_secs()
        ),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        store::HelmStore,
        tensorlake::{
            api::{ApiKey, CreateSandbox, Resources, SandboxSource, SnapshotType, TensorlakeApi},
            e2e::E2eTensorlake,
        },
    };
    use farhelm_proto::tensorlake::{
        SaveSettings,
        recipes::{RecipeBase, RecipeSave, RecipeSize},
    };

    /// A private, marked Tensorlake fake exercises the same ownership and
    /// snapshot calls as production without changing ambient configuration.
    async fn fixture() -> (tempfile::TempDir, HelmStore, Tensorlake, Arc<E2eTensorlake>) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("fake");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("ENABLED"), "farhelm-e2e-tensorlake-v1\n").unwrap();
        let fake = Arc::new(E2eTensorlake::new(root, dir.path()).unwrap());
        let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
        assert!(
            store
                .save_tensorlake_config(SaveSettings {
                    api_key: Some("test-key".into()),
                    ..Default::default()
                })
                .await
                .is_ok()
        );
        let service = Tensorlake::detached(store.clone()).with_api(fake.clone());
        (dir, store, service, fake)
    }

    /// Use the API's real create shape so a test cannot accidentally plant
    /// an impossible ownership premise in the model.
    async fn create(fake: &E2eTensorlake, name: &str) -> String {
        fake.create_sandbox(
            &ApiKey::new("test-key".into()),
            &CreateSandbox {
                name: name.into(),
                source: SandboxSource::Image("tensorlake/ubuntu-systemd".into()),
                resources: Resources {
                    cpus: 1.0,
                    memory_mb: 1024,
                    disk_mb: 10240,
                },
                timeout_secs: 900,
                auto_resume: false,
                max_pending_secs: 900,
            },
        )
        .await
        .unwrap()
        .sandbox_id
    }
    /// The product namespaces carry a helm tag; the Tensorlake test harness and
    /// unrelated sandbox names must never be offered for deletion.
    #[test]
    fn only_product_names_have_creator_tags() {
        assert_eq!(creator("farhelm-host-k7q2-dev-m3x9"), Some("k7q2"));
        assert_eq!(creator("farhelm-build-k7q2-rust"), Some("k7q2"));
        assert_eq!(creator("farhelm-prepare-k7q2-rust"), Some("k7q2"));
        assert!(reserved("farhelm-prepare-k7q2-rust"));
        for name in [
            "farhelm-sbx-k7q2-1",
            "other",
            "farhelm-host--dev",
            "farhelm-build-k7q2",
        ] {
            assert_eq!(creator(name), None, "{name}");
        }
    }

    /// A local host and every saved recipe reserve their names, including
    /// an unsettled host create and a builder with no recorded ID. Only a
    /// known foreign tag marks another helm. The test harness's sandboxes
    /// and unrelated ones are never offered.
    #[farhelm_testtrace::test]
    async fn list_excludes_local_owners_and_harness_and_marks_other_helms() {
        let (_dir, store, service, fake) = fixture().await;
        let tag = helm_tag(&store.registry_namespace().await.unwrap());
        let owned = format!("farhelm-host-{tag}-owned-abcd");
        let orphan = format!("farhelm-host-{tag}-orphan-abcd");
        let other_tag = if tag == "zzzz" { "yyyy" } else { "zzzz" };
        let other = format!("farhelm-build-{other_tag}-orphan");
        let untagged = "farhelm-host-untagged";
        for name in [
            &owned,
            &orphan,
            &other,
            untagged,
            &builder_name(&tag, "saved"),
            &preparer_name(&tag, "saved"),
            "farhelm-sbx-test-1",
            "my-sandbox",
        ] {
            create(&fake, name).await;
        }
        let host =
            super::super::hosts::test_support::plant(&store, "owned", &owned, "saved", None).await;
        assert!(
            store
                .set_tensorlake_pending_create(host, Some(super::super::recipes::now_secs()))
                .await
                .unwrap()
        );
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        let binding = row.tensorlake.unwrap();
        assert!(
            binding.pending_create_at.is_some(),
            "premise: create is unsettled"
        );
        assert!(
            binding.sandbox_id.is_none()
                && row.remote_farhelm.is_none()
                && row.remote_state_dir.is_none()
        );
        store
            .create_tensorlake_recipe(
                "saved".into(),
                RecipeSave {
                    size: RecipeSize {
                        cpus: 1,
                        memory_gb: 1,
                        disk_gb: 10,
                    },
                    base: RecipeBase::ManagedUbuntu,
                    packages: vec![],
                    claude: None,
                    codex: None,
                    github_token_credential: None,
                    claude_login_credential: None,
                    codex_login_credential: None,
                    tunnels: vec![],
                },
            )
            .await
            .unwrap()
            .unwrap();
        let OrphanSandboxList {
            discovery: OrphanDiscovery::Ok { sandboxes: list },
            cleanups,
        } = service.orphan_sandboxes().await
        else {
            panic!("expected successful discovery");
        };
        assert!(cleanups.is_empty());
        assert_eq!(list.len(), 3, "{list:?}");
        assert!(
            !list
                .iter()
                .find(|s| s.name == orphan)
                .unwrap()
                .from_another_helm
        );
        assert!(
            list.iter()
                .find(|s| s.name == other)
                .unwrap()
                .from_another_helm
        );
        assert!(
            !list
                .iter()
                .find(|s| s.name == untagged)
                .unwrap()
                .from_another_helm
        );
        let id = fake
            .get_sandbox(&ApiKey::new("test-key".into()), &owned)
            .await
            .unwrap()
            .id;
        assert!(
            service
                .delete_orphan(&id)
                .await
                .unwrap_err()
                .to_string()
                .contains("belongs to a host")
        );
        assert_eq!(
            fake.get_sandbox(&ApiKey::new("test-key".into()), &id)
                .await
                .unwrap()
                .status,
            SandboxStatus::Running
        );
    }

    /// Destruction must sweep snapshots by exact source ID and leave other
    /// resources alone. The reserved prefix is rechecked on the mutation,
    /// not trusted merely because a caller supplied an ID.
    #[farhelm_testtrace::test]
    async fn deletion_sweeps_only_its_source_and_refuses_non_farhelm_names() {
        let (_dir, _store, service, fake) = fixture().await;
        let key = ApiKey::new("test-key".into());
        let orphan = create(&fake, "farhelm-host-k7q2-orphan-abcd").await;
        let other = create(&fake, "farhelm-sbx-test-1").await;
        fake.create_snapshot(&key, &orphan, SnapshotType::Filesystem)
            .await
            .unwrap();
        let kept = fake
            .create_snapshot(&key, &other, SnapshotType::Filesystem)
            .await
            .unwrap();
        assert_eq!(
            fake.list_snapshots(&key).await.unwrap().snapshots.len(),
            2,
            "fixture snapshots exist before delete"
        );
        service
            .delete_orphan("farhelm-host-k7q2-orphan-abcd")
            .await
            .unwrap();
        assert_eq!(
            fake.get_sandbox(&key, &orphan).await.unwrap().status,
            SandboxStatus::Terminated
        );
        let snapshots = fake.list_snapshots(&key).await.unwrap().snapshots;
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, kept.snapshot_id);
        service
            .delete_orphan(&orphan)
            .await
            .expect("retry by retained ID");
        assert!(service.delete_orphan(&other).await.is_err());
        assert_eq!(
            fake.get_sandbox(&key, &other).await.unwrap().status,
            SandboxStatus::Running
        );
    }

    /// A failed snapshot sweep must survive loss of the client and even
    /// the vendor's terminated sandbox record. The canonical ID retained by
    /// the helm is enough to finish exact-source cleanup on explicit retry.
    /// A different API key cannot use that retained admission or finish the original key's retry.
    #[farhelm_testtrace::test]
    async fn unfinished_cleanup_is_listed_and_retried_after_the_sandbox_is_gone() {
        let (dir, store, service, fake) = fixture().await;
        let key = ApiKey::new("test-key".into());
        let id = create(&fake, "farhelm-host-untagged").await;
        fake.create_snapshot(&key, &id, SnapshotType::Filesystem)
            .await
            .unwrap();
        let config = dir.path().join("fake/config.json");
        std::fs::write(
            &config,
            r#"{"fail":{"delete_snapshot":{"status":503,"message":"sweep refused"}}}"#,
        )
        .unwrap();
        assert!(service.delete_orphan(&id).await.is_err());
        assert!(
            fake.list_sandboxes(&key, None)
                .await
                .unwrap()
                .sandboxes
                .is_empty(),
            "termination removed the sandbox from live discovery"
        );
        let listed = service.orphan_sandboxes().await;
        assert_eq!(
            listed.discovery,
            OrphanDiscovery::Ok {
                sandboxes: Vec::new()
            }
        );
        assert_eq!(listed.cleanups.len(), 1);
        assert!(
            !listed.cleanups[0].from_another_helm,
            "cleanup admission uses the same label rule as discovery"
        );
        assert!(
            matches!(&listed.cleanups[0].cleanup, Some(OrphanCleanup::Failed { reason }) if reason.contains("sweep refused"))
        );
        let events = dir.path().join("fake/events.jsonl");
        let before = std::fs::read(&events).unwrap();
        let save_key = |key: &str| {
            store.save_tensorlake_config(SaveSettings {
                api_key: Some(key.into()),
                ..Default::default()
            })
        };
        assert!(
            save_key("second-test-key").await.is_ok(),
            "switch fixture key"
        );
        let refusal = service.delete_orphan(&id).await.unwrap_err().to_string();
        assert!(
            refusal.contains("Restore the key that started it"),
            "{refusal}"
        );
        assert_eq!(
            std::fs::read(&events).unwrap(),
            before,
            "mismatch makes no provider calls"
        );
        assert!(save_key("test-key").await.is_ok(), "restore fixture key");
        std::fs::write(
            &config,
            r#"{"fail":{"list_sandboxes":{"status":503,"message":"discovery unavailable"}}}"#,
        )
        .unwrap();
        let unavailable = service.orphan_sandboxes().await;
        assert_eq!(
            unavailable.cleanups, listed.cleanups,
            "an outage leaves the cleanup's state and error intact"
        );
        assert!(
            matches!(&unavailable.discovery, OrphanDiscovery::Failed { reason } if reason.contains("discovery unavailable"))
        );
        let state = dir.path().join("fake/state.json");
        let mut model: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
        model["sandboxes"] = serde_json::json!([]);
        std::fs::write(state, serde_json::to_vec(&model).unwrap()).unwrap();
        assert!(
            matches!(
                fake.get_sandbox(&key, &id).await,
                Err(TensorlakeError::NotFound { .. })
            ),
            "vendor no longer knows the terminated sandbox"
        );
        service.delete_orphan(&id).await.unwrap();
        let completed = service.orphan_sandboxes().await;
        assert_eq!(completed.discovery, unavailable.discovery);
        assert!(
            completed.cleanups.is_empty(),
            "the retry finished while discovery was still unavailable"
        );
        std::fs::write(&config, "{}").unwrap();
        assert_eq!(
            service.orphan_sandboxes().await,
            OrphanSandboxList {
                discovery: OrphanDiscovery::Ok {
                    sandboxes: Vec::new()
                },
                cleanups: Vec::new()
            }
        );
        assert!(
            fake.list_snapshots(&key)
                .await
                .unwrap()
                .snapshots
                .is_empty()
        );
    }
}
