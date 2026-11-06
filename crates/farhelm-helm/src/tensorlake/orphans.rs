//! Explicit cleanup of reserved-name sandboxes this helm does not own.
//!
//! Names are the only ownership tag Tensorlake supplies. Host rows and all
//! saved recipes exclude their sandboxes, including a builder known only by
//! its deterministic name after an interrupted create. Nothing is reclaimed
//! automatically; another helm's tag is a warning, not permission to delete.

use super::{
    Tensorlake,
    api::{SandboxStatus, TensorlakeError, list_all_sandboxes},
    build::{BUILDER_PREFIX, builder_name},
    config::helm_tag,
};
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use farhelm_proto::tensorlake::{OrphanCleanup, OrphanSandbox};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

const HOST_PREFIX: &str = "farhelm-host-";

/// In-flight and failed explicit deletions, keyed by canonical vendor ID.
/// This is operation state for one helm lifetime, not an ownership ledger
/// or a queue that reclaims resources without another user request.
#[derive(Default)]
pub(super) struct Cleanups(Mutex<HashMap<String, OrphanSandbox>>);

impl Cleanups {
    /// Read without lending the operation mutex across any vendor call.
    fn snapshot(&self) -> Vec<OrphanSandbox> {
        self.0
            .lock()
            .expect("orphan cleanups poisoned")
            .values()
            .cloned()
            .collect()
    }
}

/// Return the creator tag only for the two product namespaces. The
/// browser harness's farhelm-sbx names must never become cleanup candidates.
fn creator(name: &str) -> Option<&str> {
    let suffix = name
        .strip_prefix(HOST_PREFIX)
        .or_else(|| name.strip_prefix(BUILDER_PREFIX))?;
    let (tag, rest) = suffix.split_once('-')?;
    (!tag.is_empty() && !rest.is_empty()).then_some(tag)
}

/// The namespace, rather than today's tag length, is the vendor-side
/// ownership signal. An older helm's tag spelling must not hide a leftover.
fn reserved(name: &str) -> bool {
    name.starts_with(HOST_PREFIX) || name.starts_with(BUILDER_PREFIX)
}

impl Tensorlake {
    /// Current host names and deterministic builders for every saved recipe,
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
        }
        Ok((tag, known))
    }

    /// Join live discovery with unfinished explicit cleanup. Termination
    /// removes a sandbox from Tensorlake's list, so the helm retains its
    /// canonical ID and display facts until snapshot cleanup succeeds.
    pub(crate) async fn orphan_sandboxes(&self) -> anyhow::Result<Vec<OrphanSandbox>> {
        let discovered = async {
            let key = self.api_key().await.map_err(anyhow::Error::msg)?;
            Ok::<_, anyhow::Error>(list_all_sandboxes(self.api(), &key).await?)
        }
        .await;
        let sandboxes = match discovered {
            Ok(sandboxes) => sandboxes,
            Err(error) => {
                let mut unfinished = self.0.orphan_cleanups.snapshot();
                if unfinished.is_empty() {
                    return Err(error);
                }
                // A vendor outage must not hide work the user already
                // requested. Say that discovery is incomplete on these rows.
                for row in &mut unfinished {
                    row.state = format!("Sandbox list unavailable: {error}");
                }
                unfinished.sort_by(|a, b| a.name.cmp(&b.name));
                return Ok(unfinished);
            }
        };
        let (tag, known) = self.known_sandboxes().await?;
        let mut orphans = vec![];
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
            orphans.push(OrphanSandbox {
                id: sandbox.id,
                from_another_helm: creator(&name) != Some(tag.as_str()),
                name,
                created_at: sandbox.created_at,
                state: status(sandbox.status).into(),
                cleanup: None,
            });
        }
        let mut by_id: HashMap<_, _> = orphans
            .into_iter()
            .map(|row| (row.id.clone(), row))
            .collect();
        for row in self.0.orphan_cleanups.snapshot() {
            by_id.insert(row.id.clone(), row);
        }
        let mut orphans: Vec<_> = by_id.into_values().collect();
        orphans.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(orphans)
    }

    /// Recheck the tag and local ownership at deletion time. A stale UI list
    /// cannot delete a sandbox that this helm has since adopted.
    async fn delete_orphan(&self, id: &str) -> anyhow::Result<()> {
        let key = self.api_key().await.map_err(anyhow::Error::msg)?;
        let previous = self
            .0
            .orphan_cleanups
            .0
            .lock()
            .expect("orphan cleanups poisoned")
            .get(id)
            .cloned();
        let mut row = match previous {
            Some(row) => row,
            None => {
                let sandbox = super::hosts::bounded(self.api().get_sandbox(&key, id)).await?;
                let name = sandbox
                    .name
                    .ok_or_else(|| anyhow::anyhow!("This sandbox has no Farhelm name"))?;
                let tag = helm_tag(&self.0.store.registry_namespace().await?);
                OrphanSandbox {
                    id: sandbox.id,
                    from_another_helm: creator(&name) != Some(tag.as_str()),
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
            if operations
                .get(&row.id)
                .is_some_and(|row| row.cleanup == Some(OrphanCleanup::Running))
            {
                anyhow::bail!("Cleanup of this sandbox is already running");
            }
            row.cleanup = Some(OrphanCleanup::Running);
            row.state = "Cleanup in progress".into();
            operations.insert(row.id.clone(), row.clone());
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
                operations.insert(row.id.clone(), row);
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

/// Protected Settings read. Failures remain visible, never an empty list.
pub(crate) async fn list(State(state): State<Arc<AppState>>) -> Response {
    match state.tensorlake.orphan_sandboxes().await {
        Ok(list) => axum::Json(list).into_response(),
        Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    }
}

/// Helm-owned deletion survives the confirming browser closing mid-request.
pub(crate) async fn delete(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    crate::run_owned(async move {
        let result = state.tensorlake.delete_orphan(&id).await;
        state.manager.events().bump();
        match result {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
        }
    })
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
        recipes::{RecipeSave, RecipeSize, RecipeSource},
    };

    /// A private, marked browser fake exercises the same ownership and
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
    /// The product namespaces carry a helm tag; the browser harness and
    /// unrelated sandbox names must never be offered for deletion.
    #[test]
    fn only_product_names_have_creator_tags() {
        assert_eq!(creator("farhelm-host-k7q2-dev-m3x9"), Some("k7q2"));
        assert_eq!(creator("farhelm-build-k7q2-rust"), Some("k7q2"));
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
    /// an unsettled host create and a builder with no recorded ID. Another
    /// helm is visible but marked;
    /// browser harness resources and unrelated sandboxes are never offered.
    #[farhelm_testtrace::test]
    async fn list_excludes_local_owners_and_harness_and_marks_other_helms() {
        let (_dir, store, service, fake) = fixture().await;
        let tag = helm_tag(&store.registry_namespace().await.unwrap());
        let owned = format!("farhelm-host-{tag}-owned-abcd");
        let orphan = format!("farhelm-host-{tag}-orphan-abcd");
        let other_tag = if tag == "zzzz" { "yyyy" } else { "zzzz" };
        let other = format!("farhelm-build-{other_tag}-orphan");
        for name in [
            &owned,
            &orphan,
            &other,
            &builder_name(&tag, "saved"),
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
                    source: RecipeSource::Packages { packages: vec![] },
                    claude: None,
                    codex: None,
                    github_token_credential: None,
                    tunnels: vec![],
                },
            )
            .await
            .unwrap()
            .unwrap();
        let list = service.orphan_sandboxes().await.unwrap();
        assert_eq!(list.len(), 2, "{list:?}");
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

    /// A failed snapshot sweep must survive loss of the browser and even
    /// the vendor's terminated sandbox record. The canonical ID retained by
    /// the helm is enough to finish exact-source cleanup on explicit retry.
    #[farhelm_testtrace::test]
    async fn unfinished_cleanup_is_listed_and_retried_after_the_sandbox_is_gone() {
        let (dir, _store, service, fake) = fixture().await;
        let key = ApiKey::new("test-key".into());
        let id = create(&fake, "farhelm-host-k7q2-orphan-abcd").await;
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
        let listed = service.orphan_sandboxes().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert!(
            matches!(&listed[0].cleanup, Some(OrphanCleanup::Failed { reason }) if reason.contains("sweep refused"))
        );
        std::fs::write(
            &config,
            r#"{"fail":{"list_sandboxes":{"status":503,"message":"discovery unavailable"}}}"#,
        )
        .unwrap();
        let unavailable = service.orphan_sandboxes().await.unwrap();
        assert_eq!(
            unavailable[0].id, id,
            "discovery failure cannot hide an unfinished operation"
        );
        assert!(unavailable[0].state.contains("discovery unavailable"));
        std::fs::write(&config, "{}").unwrap();
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
        assert!(service.orphan_sandboxes().await.unwrap().is_empty());
        assert!(
            fake.list_snapshots(&key)
                .await
                .unwrap()
                .snapshots
                .is_empty()
        );
    }
}
