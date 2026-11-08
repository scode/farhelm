//! Removal, Suspend/Resume and the status check against a scripted Tensorlake.
//!
//! [`FakeApi`] keeps sandboxes and snapshots in memory and records every
//! call, so a test can say both what Tensorlake ended up holding and in what
//! order Farhelm asked. Rows are planted through the store's own
//! registration ([`super::test_support::plant`]); no test touches the
//! network.

use super::test_support::plant;
use super::*;
use crate::manager::HostState;
use crate::store::{HelmStore, HostStoreError};
use crate::tensorlake::api::{
    CreateSandbox, CreatedSandbox, CreatedSnapshot, SandboxInfo, SandboxPage, SnapshotInfo,
    SnapshotList, SnapshotStatus, SnapshotType,
};
use farhelm_proto::tensorlake::{LifecycleDirection, SaveSettings};
use std::sync::{Arc, Mutex};

const KEY: &str = "tl_test_key";

/// One scripted failure: the call named `call` (as recorded in
/// [`FakeState::calls`]) answers `error` instead of doing its work.
type Failure = (&'static str, TensorlakeError);

#[derive(Default)]
struct FakeState {
    /// id → (name, status)
    sandboxes: Vec<(String, String, SandboxStatus)>,
    snapshots: Vec<SnapshotInfo>,
    /// Every call, as `"<call> <argument>"`, in order.
    calls: Vec<String>,
    failures: Vec<Failure>,
    /// When set, terminate leaves the sandbox in this state instead of
    /// `Terminated`, standing in for a termination Tensorlake has not
    /// finished.
    terminate_leaves: Option<SandboxStatus>,
    /// When set, terminate announces itself and waits for the test before
    /// doing anything, so a test can look at the helm mid-removal.
    terminate_gate: Option<Arc<Gate>>,
    /// Hold Suspend at its API boundary to inspect routing and admission.
    suspend_gate: Option<Arc<Gate>>,
    /// Hold Resume before changing status, so the actor and API still agree
    /// that the sandbox is suspended throughout the observed request.
    resume_gate: Option<Arc<Gate>>,
    /// A separate marked fake owns only tunnel execution; FakeApi still
    /// supplies the lifecycle status. It holds no FakeApi, so
    /// keeping this service here cannot form an ownership cycle.
    tunnels: Option<super::super::Tensorlake>,
    /// Park resource acquisition before it owns a lease, to prove a hold
    /// cancels a late recipe read rather than starting forwards afterwards.
    connected_gate: Option<Arc<Gate>>,
}

/// A pause point inside a fake call: the call notifies `entered`, then waits
/// for `release`. `Notify` keeps a permit, so neither side can miss the other.
#[derive(Default)]
struct Gate {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

/// An in-memory Tensorlake for removal, lifecycle and status checks.
/// Sandboxes are found by id or name, as the real API finds them.
#[derive(Clone, Default)]
struct FakeApi(Arc<Mutex<FakeState>>);

/// Lifecycle tests need the actor to observe the same sandbox the REST
/// service mutates. A separate offline status source would dial blindly
/// after a successful Suspend and hide the persistent-state contract.
#[async_trait::async_trait]
impl SandboxStatusSource for FakeApi {
    async fn connected(
        &self,
        row: &HostRow,
        client: Arc<crate::client::SupervisorClient>,
        events: Arc<crate::feed::FleetEvents>,
    ) -> crate::manager::ConnectedResources {
        let (service, gate) = {
            let state = self.0.lock().unwrap();
            (state.tunnels.clone(), state.connected_gate.clone())
        };
        let Some(service) = service else {
            return crate::manager::ConnectedResources::default();
        };
        if let Some(gate) = gate {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
        SandboxStatusSource::connected(&service, row, client, events).await
    }

    async fn check(&self, sandbox: &str) -> SandboxCheck {
        match self.get_sandbox(&ApiKey::new(KEY.into()), sandbox).await {
            Ok(info) => SandboxCheck::Status {
                status: info.status,
                error_details: info.error_details,
            },
            Err(TensorlakeError::NotFound { .. }) => SandboxCheck::Missing,
            Err(error) => SandboxCheck::Unavailable(error.to_string()),
        }
    }
}

impl FakeApi {
    fn with_sandbox(self, id: &str, name: &str, status: SandboxStatus) -> Self {
        self.0
            .lock()
            .unwrap()
            .sandboxes
            .push((id.into(), name.into(), status));
        self
    }

    fn with_snapshot(self, id: &str, sandbox_id: &str) -> Self {
        self.0.lock().unwrap().snapshots.push(SnapshotInfo {
            id: id.into(),
            sandbox_id: Some(sandbox_id.into()),
            status: SnapshotStatus::Completed,
            snapshot_type: Some(SnapshotType::Filesystem),
            size_bytes: None,
            created_at: None,
        });
        self
    }

    fn failing(self, call: &'static str, error: TensorlakeError) -> Self {
        self.0.lock().unwrap().failures.push((call, error));
        self
    }

    fn calls(&self) -> Vec<String> {
        self.0.lock().unwrap().calls.clone()
    }

    fn snapshot_ids(&self) -> Vec<String> {
        let state = self.0.lock().unwrap();
        state.snapshots.iter().map(|s| s.id.clone()).collect()
    }

    /// Record the call and answer a scripted failure for it, if any.
    fn enter(
        &self,
        call: &'static str,
        argument: &str,
        key: &ApiKey,
    ) -> Result<(), TensorlakeError> {
        assert_eq!(key.expose(), KEY, "every call carries the saved key");
        let mut state = self.0.lock().unwrap();
        state.calls.push(format!("{call} {argument}"));
        match state.failures.iter().position(|(name, _)| *name == call) {
            Some(index) => Err(state.failures.remove(index).1),
            None => Ok(()),
        }
    }
}

fn not_found() -> TensorlakeError {
    TensorlakeError::NotFound {
        message: "not found".into(),
    }
}

#[async_trait::async_trait]
impl TensorlakeApi for FakeApi {
    async fn list_sandboxes(
        &self,
        _: &ApiKey,
        _: Option<&str>,
    ) -> Result<SandboxPage, TensorlakeError> {
        unimplemented!("removal never lists sandboxes")
    }

    async fn get_sandbox(
        &self,
        key: &ApiKey,
        sandbox: &str,
    ) -> Result<SandboxInfo, TensorlakeError> {
        self.enter("get", sandbox, key)?;
        let state = self.0.lock().unwrap();
        let (id, name, status) = state
            .sandboxes
            .iter()
            .find(|(id, name, _)| id == sandbox || name == sandbox)
            .cloned()
            .ok_or_else(not_found)?;
        Ok(SandboxInfo {
            id,
            name: Some(name),
            status,
            created_at: None,
            image: None,
            resources: None,
            snapshot_id: None,
            error_details: (status == SandboxStatus::ResumeFailed)
                .then(|| "the snapshot could not be restored".to_string()),
            termination_reason: None,
            pending_reason: None,
            auto_resume: None,
        })
    }

    async fn create_sandbox(
        &self,
        _: &ApiKey,
        _: &CreateSandbox,
    ) -> Result<CreatedSandbox, TensorlakeError> {
        unimplemented!("removal never creates")
    }

    async fn suspend_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError> {
        let gate = self.0.lock().unwrap().suspend_gate.clone();
        if let Some(gate) = gate {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
        self.enter("suspend", sandbox, key)?;
        self.0
            .lock()
            .unwrap()
            .sandboxes
            .iter_mut()
            .find(|(id, name, _)| id == sandbox || name == sandbox)
            .ok_or_else(not_found)?
            .2 = SandboxStatus::Suspended;
        Ok(())
    }

    async fn resume_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError> {
        let gate = self.0.lock().unwrap().resume_gate.clone();
        if let Some(gate) = gate {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
        self.enter("resume", sandbox, key)?;
        self.0
            .lock()
            .unwrap()
            .sandboxes
            .iter_mut()
            .find(|(id, name, _)| id == sandbox || name == sandbox)
            .ok_or_else(not_found)?
            .2 = SandboxStatus::Running;
        Ok(())
    }

    async fn terminate_sandbox(&self, key: &ApiKey, sandbox: &str) -> Result<(), TensorlakeError> {
        let gate = self.0.lock().unwrap().terminate_gate.clone();
        if let Some(gate) = gate {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
        self.enter("terminate", sandbox, key)?;
        let mut state = self.0.lock().unwrap();
        let leaves = state.terminate_leaves.unwrap_or(SandboxStatus::Terminated);
        let entry = state
            .sandboxes
            .iter_mut()
            .find(|(id, name, _)| id == sandbox || name == sandbox)
            .ok_or_else(not_found)?;
        entry.2 = leaves;
        Ok(())
    }

    async fn create_snapshot(
        &self,
        _: &ApiKey,
        _: &str,
        _: SnapshotType,
    ) -> Result<CreatedSnapshot, TensorlakeError> {
        unimplemented!("removal never snapshots")
    }

    async fn get_snapshot(&self, _: &ApiKey, _: &str) -> Result<SnapshotInfo, TensorlakeError> {
        unimplemented!("removal lists snapshots instead")
    }

    async fn list_snapshots(&self, key: &ApiKey) -> Result<SnapshotList, TensorlakeError> {
        self.enter("list-snapshots", "", key)?;
        Ok(SnapshotList {
            snapshots: self.0.lock().unwrap().snapshots.clone(),
        })
    }

    async fn delete_snapshot(&self, key: &ApiKey, snapshot: &str) -> Result<(), TensorlakeError> {
        self.enter("delete-snapshot", snapshot, key)?;
        let mut state = self.0.lock().unwrap();
        let before = state.snapshots.len();
        state.snapshots.retain(|s| s.id != snapshot);
        if state.snapshots.len() == before {
            return Err(not_found());
        }
        Ok(())
    }
}

/// A store with an API key saved, its directory, and a service over `api`.
async fn service(api: &FakeApi) -> (tempfile::TempDir, HelmStore, Tensorlake) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = HelmStore::open(&dir.path().join("helm.db"))
        .await
        .expect("open a fresh store");
    store
        .save_tensorlake_config(SaveSettings {
            api_key: Some(KEY.into()),
            clear_api_key: false,
            create_without_asking: None,
        })
        .await
        .map_err(|_| "save failed")
        .expect("save the API key");
    let tensorlake = Tensorlake::detached(store.clone()).with_api(Arc::new(api.clone()));
    (dir, store, tensorlake)
}

/// The stored row for `host`.
async fn row(store: &HelmStore, host: crate::store::HostId) -> HostRow {
    store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .expect("the host row exists")
}

/// Why: removing a Tensorlake host must stop everything it bills for, and
/// suspend snapshots outlive their sandbox (D6, goal criterion 5).
///
/// Spec: with the sandbox id recorded, removal terminates the sandbox, waits
/// until Tensorlake reports it terminated, then deletes exactly the
/// snapshots whose source is that sandbox, in that order, and the proof it
/// returns lets the store forget the row. Another sandbox's snapshot stays.
#[farhelm_testtrace::test]
async fn removal_terminates_then_sweeps_the_sandboxes_snapshots() {
    let api = FakeApi::default()
        .with_sandbox(
            "sbx-1",
            "farhelm-host-k7q2-dev-m3x9",
            SandboxStatus::Running,
        )
        .with_snapshot("snap-a", "sbx-1")
        .with_snapshot("snap-other", "sbx-2")
        .with_snapshot("snap-b", "sbx-1");
    let (_dir, store, tensorlake) = service(&api).await;
    let host = plant(
        &store,
        "dev",
        "farhelm-host-k7q2-dev-m3x9",
        "rust",
        Some("sbx-1"),
    )
    .await;
    let proof = tensorlake
        .delete_sandbox(&row(&store, host).await)
        .await
        .expect("the sandbox is deleted");
    assert_eq!(
        api.calls(),
        [
            "terminate sbx-1",
            "get sbx-1",
            "list-snapshots ",
            "delete-snapshot snap-a",
            "delete-snapshot snap-b",
        ]
    );
    assert_eq!(api.snapshot_ids(), ["snap-other"]);
    store
        .remove_host_row(host, Some(&proof))
        .await
        .expect("the proof lets the store forget the row");
    assert!(
        store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .all(|r| r.id != host)
    );
}

/// Why: a removal that crashed or failed partway is finished by removing
/// again (goal criterion 5), so every step must accept that its work was
/// already done.
///
/// Spec: a second removal of the same row, after the first deleted
/// everything, succeeds: the terminate and the termination poll both answer
/// not-found, which count as done, and there is nothing left to sweep. So
/// does a removal whose terminate and snapshot delete answer not-found
/// outright.
#[farhelm_testtrace::test]
async fn removal_is_idempotent() {
    let api = FakeApi::default()
        .with_sandbox(
            "sbx-1",
            "farhelm-host-k7q2-dev-m3x9",
            SandboxStatus::Suspended,
        )
        .with_snapshot("snap-a", "sbx-1");
    let (_dir, store, tensorlake) = service(&api).await;
    let host = plant(
        &store,
        "dev",
        "farhelm-host-k7q2-dev-m3x9",
        "rust",
        Some("sbx-1"),
    )
    .await;
    let planted = row(&store, host).await;
    tensorlake
        .delete_sandbox(&planted)
        .await
        .expect("first removal");
    // Tensorlake forgets a terminated sandbox eventually; the retry meets that.
    api.0.lock().unwrap().sandboxes.clear();
    tensorlake
        .delete_sandbox(&planted)
        .await
        .expect("a second removal finds nothing left and succeeds");
    let after_first = api
        .calls()
        .iter()
        .position(|c| c == "delete-snapshot snap-a")
        .unwrap();
    assert_eq!(
        api.calls()[after_first + 1..],
        ["terminate sbx-1", "get sbx-1", "list-snapshots "]
    );

    let api = FakeApi::default()
        .with_sandbox(
            "sbx-1",
            "farhelm-host-k7q2-dev-m3x9",
            SandboxStatus::Running,
        )
        .with_snapshot("snap-a", "sbx-1")
        .failing("terminate", not_found())
        .failing("delete-snapshot", not_found());
    let (_dir, store, tensorlake) = service(&api).await;
    let host = plant(
        &store,
        "dev",
        "farhelm-host-k7q2-dev-m3x9",
        "rust",
        Some("sbx-1"),
    )
    .await;
    // The scripted terminate answers not-found without terminating; the
    // poll must still see an end, so the sandbox is gone by then.
    api.0.lock().unwrap().sandboxes.clear();
    tensorlake
        .delete_sandbox(&row(&store, host).await)
        .await
        .expect("not-found from terminate and delete counts as done");
}

/// Why: the row is written before the sandbox is created (P3), so a create
/// that never answered leaves a row with no recorded id; removing it must
/// still work, and must find the sandbox by name if it did come up.
///
/// Spec: a never-sent row makes no API calls. An outstanding create permits
/// recovery by name, followed by termination and a sweep of the returned ID.
#[farhelm_testtrace::test]
async fn removal_without_a_recorded_id_finds_the_sandbox_by_name() {
    let api = FakeApi::default();
    let (_dir, store, tensorlake) = service(&api).await;
    let host = plant(&store, "dev", "farhelm-host-k7q2-dev-m3x9", "rust", None).await;
    tensorlake
        .delete_sandbox(&row(&store, host).await)
        .await
        .expect("a sandbox that never existed needs nothing deleted");
    assert!(
        api.calls().is_empty(),
        "a never-sent row grants no name lookup"
    );

    let api = FakeApi::default()
        .with_sandbox(
            "sbx-9",
            "farhelm-host-k7q2-dev-m3x9",
            SandboxStatus::Running,
        )
        .with_snapshot("snap-a", "sbx-9");
    let (_dir, store, tensorlake) = service(&api).await;
    let host = plant(&store, "dev", "farhelm-host-k7q2-dev-m3x9", "rust", None).await;
    store
        .set_tensorlake_pending_create(host, Some(super::super::recipes::now_secs()))
        .await
        .unwrap();
    assert!(
        row(&store, host)
            .await
            .tensorlake
            .unwrap()
            .pending_create_at
            .is_some()
    );
    tensorlake
        .delete_sandbox(&row(&store, host).await)
        .await
        .expect("the sandbox found by name is deleted");
    assert_eq!(
        api.calls(),
        [
            "get farhelm-host-k7q2-dev-m3x9",
            "terminate sbx-9",
            "get sbx-9",
            "list-snapshots ",
            "delete-snapshot snap-a",
        ]
    );
}

/// Why: a terminated sandbox's name can stop resolving, so an id a removal
/// recovered by name and then lost would make the retry take "never
/// existed" and forget the host while the snapshots the first attempt did
/// not sweep keep billing (goal criterion 5).
///
/// Spec: a removal with an outstanding create saves the id it found by
/// name before terminating; when its snapshot listing then fails, the row
/// keeps that id, and a retry after the name stops resolving terminates and
/// sweeps by the saved id before issuing its proof.
#[farhelm_testtrace::test]
async fn a_recovered_id_is_saved_before_terminating_so_a_retry_still_sweeps() {
    let api = FakeApi::default()
        .with_sandbox(
            "sbx-9",
            "farhelm-host-k7q2-dev-m3x9",
            SandboxStatus::Running,
        )
        .with_snapshot("snap-a", "sbx-9")
        .failing(
            "list-snapshots",
            TensorlakeError::Status {
                status: 500,
                message: "try later".into(),
            },
        );
    let (_dir, store, tensorlake) = service(&api).await;
    let host = plant(&store, "dev", "farhelm-host-k7q2-dev-m3x9", "rust", None).await;
    store
        .set_tensorlake_pending_create(host, Some(super::super::recipes::now_secs()))
        .await
        .unwrap();
    assert!(
        row(&store, host)
            .await
            .tensorlake
            .unwrap()
            .pending_create_at
            .is_some()
    );
    tensorlake
        .delete_sandbox(&row(&store, host).await)
        .await
        .expect_err("the failed listing fails the first removal");
    assert!(api.calls().contains(&"terminate sbx-9".to_string()));
    assert_eq!(
        row(&store, host)
            .await
            .tensorlake
            .unwrap()
            .sandbox_id
            .as_deref(),
        Some("sbx-9"),
        "the id found by name was saved"
    );

    // Tensorlake forgets the terminated sandbox: its name no longer resolves.
    api.0.lock().unwrap().sandboxes.clear();
    let first_attempt = api.calls().len();
    let proof = tensorlake
        .delete_sandbox(&row(&store, host).await)
        .await
        .expect("the retry finishes the job");
    assert_eq!(
        api.calls()[first_attempt..],
        [
            "terminate sbx-9",
            "get sbx-9",
            "list-snapshots ",
            "delete-snapshot snap-a",
        ],
        "the retry sweeps by the saved id rather than looking the name up"
    );
    assert!(api.snapshot_ids().is_empty());
    store.remove_host_row(host, Some(&proof)).await.unwrap();
}

/// Why: the row is Farhelm's only record of a billing sandbox, so a removal
/// that could not finish must keep it (D6), and say why in words the user
/// can act on.
///
/// Spec: a snapshot delete Tensorlake refuses fails the removal with a 503
/// refusal naming the snapshot; a missing API key fails it with a 409
/// refusal pointing at Settings; in both cases the store still refuses to
/// forget the row without a proof.
#[farhelm_testtrace::test]
async fn a_removal_that_cannot_finish_keeps_the_row() {
    let api = FakeApi::default()
        .with_sandbox(
            "sbx-1",
            "farhelm-host-k7q2-dev-m3x9",
            SandboxStatus::Running,
        )
        .with_snapshot("snap-a", "sbx-1")
        .failing(
            "delete-snapshot",
            TensorlakeError::Conflict {
                message: "snapshot is still being written".into(),
            },
        );
    let (_dir, store, tensorlake) = service(&api).await;
    let host = plant(
        &store,
        "dev",
        "farhelm-host-k7q2-dev-m3x9",
        "rust",
        Some("sbx-1"),
    )
    .await;
    let error = tensorlake
        .delete_sandbox(&row(&store, host).await)
        .await
        .expect_err("a refused snapshot delete fails the removal");
    let refusal = error
        .downcast_ref::<crate::SupervisorError>()
        .expect("a helm refusal");
    assert_eq!(refusal.kind(), ErrorKind::Unavailable);
    assert!(error.to_string().contains("snap-a"), "{error}");
    let refused = store
        .remove_ssh_host(host)
        .await
        .expect_err("no proof, no forgetting");
    assert!(matches!(
        refused.downcast_ref::<HostStoreError>(),
        Some(HostStoreError::SandboxNotDeleted(id)) if *id == host
    ));

    store
        .save_tensorlake_config(SaveSettings {
            api_key: None,
            clear_api_key: true,
            create_without_asking: None,
        })
        .await
        .map_err(|_| "save failed")
        .unwrap();
    let error = tensorlake
        .delete_sandbox(&row(&store, host).await)
        .await
        .expect_err("no key, no removal");
    let refusal = error
        .downcast_ref::<crate::SupervisorError>()
        .expect("a helm refusal");
    assert_eq!(refusal.kind(), ErrorKind::Conflict);
    assert!(
        error.to_string().contains("Settings → Tensorlake"),
        "{error}"
    );
    assert!(
        store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .any(|r| r.id == host)
    );
}

/// Why: a suspend in flight when the sandbox is terminated may still be
/// writing its snapshot; sweeping before termination finishes would miss it
/// and leave it billing.
///
/// Spec: while Tensorlake keeps reporting the sandbox as not yet terminated,
/// removal polls once a second and gives up after 30 seconds with a refusal
/// that says to try again, without listing or deleting any snapshot.
#[farhelm_testtrace::test(start_paused = true)]
async fn removal_waits_for_termination_before_sweeping() {
    let api = FakeApi::default()
        .with_sandbox(
            "sbx-1",
            "farhelm-host-k7q2-dev-m3x9",
            SandboxStatus::Running,
        )
        .with_snapshot("snap-a", "sbx-1");
    api.0.lock().unwrap().terminate_leaves = Some(SandboxStatus::Suspending);
    let (_dir, store, tensorlake) = service(&api).await;
    let host = plant(
        &store,
        "dev",
        "farhelm-host-k7q2-dev-m3x9",
        "rust",
        Some("sbx-1"),
    )
    .await;
    let started = tokio::time::Instant::now();
    let error = tensorlake
        .delete_sandbox(&row(&store, host).await)
        .await
        .expect_err("a termination that never finishes fails the removal");
    assert!(
        error.to_string().contains("has not finished terminating"),
        "{error}"
    );
    assert!(
        started.elapsed() <= TERMINATION_WAIT,
        "{:?}",
        started.elapsed()
    );
    let calls = api.calls();
    assert!(
        calls.iter().filter(|c| *c == "get sbx-1").count() >= 29,
        "{calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("list-snapshots")),
        "{calls:?}"
    );
    assert_eq!(api.snapshot_ids(), ["snap-a"]);
}

/// Why: the connection actor decides Suspended versus dialing from this
/// answer, and must be able to tell "suspended" from "Tensorlake could not
/// answer" (which dials anyway).
///
/// Spec: a sandbox Tensorlake has answers its status (and a failed resume's
/// diagnosis); one it does not have is `Missing`; an API failure and a
/// missing key are `Unavailable` with a reason, never a guess.
#[farhelm_testtrace::test]
async fn the_status_check_tells_answers_from_unavailability() {
    let api = FakeApi::default()
        .with_sandbox("sbx-1", "farhelm-host-a", SandboxStatus::Suspended)
        .with_sandbox("sbx-2", "farhelm-host-b", SandboxStatus::ResumeFailed)
        .failing(
            "get",
            TensorlakeError::Auth {
                status: 403,
                message: String::new(),
            },
        );
    let (_dir, store, tensorlake) = service(&api).await;
    assert!(matches!(
        tensorlake.check_sandbox("farhelm-host-a").await,
        SandboxCheck::Unavailable(reason) if reason.contains("rejected the API key")
    ));
    assert_eq!(
        tensorlake.check_sandbox("farhelm-host-a").await,
        SandboxCheck::Status {
            status: SandboxStatus::Suspended,
            error_details: None,
        }
    );
    assert_eq!(
        tensorlake.check_sandbox("farhelm-host-b").await,
        SandboxCheck::Status {
            status: SandboxStatus::ResumeFailed,
            error_details: Some("the snapshot could not be restored".into()),
        }
    );
    assert_eq!(
        tensorlake.check_sandbox("farhelm-host-gone").await,
        SandboxCheck::Missing
    );
    store
        .save_tensorlake_config(SaveSettings {
            api_key: None,
            clear_api_key: true,
            create_without_asking: None,
        })
        .await
        .map_err(|_| "save failed")
        .unwrap();
    let calls = api.calls().len();
    assert!(matches!(
        tensorlake.check_sandbox("farhelm-host-a").await,
        SandboxCheck::Unavailable(reason) if reason.contains("no Tensorlake API key")
    ));
    assert_eq!(api.calls().len(), calls, "no key, no call");
}

// ---------------------------------------------------------------------------
// The host model in the store (schema 72)
// ---------------------------------------------------------------------------

/// Planned setup and discovery share the convergence writer. Neither may
/// open an unfinished sandbox's dial gate, even without a reported identity;
/// only explicit admission after the copied-state check may record paths.
#[farhelm_testtrace::test]
async fn ordinary_ssh_convergence_requires_tensorlake_install_admission() {
    let dir = tempfile::tempdir().unwrap();
    let store = HelmStore::open(&dir.path().join("helm.db")).await.unwrap();
    let host = plant(
        &store,
        "dev",
        "farhelm-host-test-dev-aaaa",
        "test-recipe",
        None,
    )
    .await;
    let before = store.list_hosts().await.unwrap();
    for identity in [None, Some("copied-identity")] {
        let error = store
            .register_probed_ssh_host(
                &super::test_support::destination("farhelm-host-test-dev-aaaa"),
                Some("/copied/farhelm"),
                Some("/copied/state"),
                identity,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error.downcast_ref::<HostStoreError>(), Some(HostStoreError::TensorlakeSetupRequired(id)) if *id == host)
        );
        assert_eq!(store.list_hosts().await.unwrap(), before);
    }
    assert!(
        store
            .admit_tensorlake_install(host, "/admitted/farhelm", "/admitted/state")
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
    assert_eq!(row.remote_farhelm.as_deref(), Some("/admitted/farhelm"));
    assert_eq!(row.remote_state_dir.as_deref(), Some("/admitted/state"));
    assert!(
        row.host_identity.is_none(),
        "admission never imports a discovered identity"
    );
}

/// Why: an upgraded helm's existing hosts must stay exactly what they were;
/// only a row Farhelm registers as a sandbox may become one.
///
/// Spec: a database rewound to schema 71 without its Tensorlake columns (and
/// with an ssh host in it) reopens at the current version with all eight
/// present and null, and the ssh host still decodes as an ordinary ssh host
/// with no binding. Reopening runs the schema-72 binding migration, the
/// schema-73 pending-create migration and the schema-77 credential-delivery
/// migration and schema-79 prepared-create recovery; all remain null for an
/// ordinary host, which must never gain inferred installation facts.
#[farhelm_testtrace::test]
async fn schema_72_leaves_every_existing_host_ordinary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("helm.db");
    let store = HelmStore::open(&path).await.unwrap();
    let ssh = store.add_ssh_host("user@box", None, None).await.unwrap();
    drop(store);
    let conn = rusqlite::Connection::open(&path).unwrap();
    for column in [
        "tensorlake_sandbox",
        "tensorlake_sandbox_id",
        "tensorlake_recipe",
        "tensorlake_resources",
        "tensorlake_suspended_at",
        "tensorlake_pending_create_at",
        "tensorlake_credential_delivery",
        "tensorlake_prepared_install_json",
    ] {
        conn.execute_batch(&format!("ALTER TABLE hosts DROP COLUMN {column};"))
            .unwrap();
    }
    conn.execute_batch("PRAGMA user_version = 71;").unwrap();
    let columns = |conn: &rusqlite::Connection| -> i64 {
        conn.query_row(
            "SELECT count(*) FROM pragma_table_info('hosts') WHERE name LIKE 'tensorlake_%'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        columns(&conn),
        0,
        "fixture premise: schema 71 has no Tensorlake columns"
    );
    drop(conn);

    let store = HelmStore::open(&path).await.unwrap();
    let hosts = store.list_hosts().await.unwrap();
    let ssh_row = hosts.iter().find(|row| row.id == ssh).unwrap();
    assert_eq!(ssh_row.kind, crate::store::HostKind::Ssh);
    assert_eq!(ssh_row.tensorlake, None);
    drop(store);
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(columns(&conn), 8);
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        crate::store::SCHEMA_VERSION,
        "the schema 72 step runs as part of the whole ladder to the current version"
    );
    let nulls: i64 = conn
        .query_row(
            "SELECT count(*) FROM hosts WHERE tensorlake_sandbox IS NULL \
             AND tensorlake_sandbox_id IS NULL AND tensorlake_recipe IS NULL \
             AND tensorlake_resources IS NULL AND tensorlake_suspended_at IS NULL \
             AND tensorlake_pending_create_at IS NULL \
             AND tensorlake_credential_delivery IS NULL \
             AND tensorlake_prepared_install_json IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(nulls, 2, "the local row and the ssh row");
}

/// Why: the sandbox column is what makes a row a Tensorlake host; the local
/// row can never be one, and the schema holds that even against a hand edit.
///
/// Spec: naming a sandbox on the local row is refused by the CHECK; an ssh
/// row may carry one.
#[farhelm_testtrace::test]
async fn only_an_ssh_row_can_name_a_sandbox() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("helm.db");
    let store = HelmStore::open(&path).await.unwrap();
    store.add_ssh_host("user@box", None, None).await.unwrap();
    drop(store);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let local = conn.execute(
        "UPDATE hosts SET tensorlake_sandbox = 'x', tensorlake_recipe = 'r' WHERE kind = 'local'",
        [],
    );
    assert!(
        matches!(&local, Err(rusqlite::Error::SqliteFailure(error, _)) if error.code == rusqlite::ErrorCode::ConstraintViolation),
        "{local:?}"
    );
    conn.execute(
        "UPDATE hosts SET tensorlake_sandbox = 'x', tensorlake_recipe = 'r' WHERE kind = 'ssh'",
        [],
    )
    .unwrap();
}

/// Why: every reader must derive the same kind from the same columns, or a
/// sandbox row would be retargeted or forgotten as a plain ssh row; and a
/// sandbox row a hand edit broke must fail loudly like any corrupt registry
/// row (P2).
///
/// Spec: a registered Tensorlake host lists as `HostKind::Tensorlake` with
/// its binding (sandbox, recipe, size, the id once recorded, the suspend
/// time once set), the cloud icon and default color, YOLO on and the other
/// permissions off; a sandbox row whose recipe was removed by hand fails the
/// whole registry read.
#[farhelm_testtrace::test]
async fn a_tensorlake_host_decodes_with_its_binding() {
    let api = FakeApi::default();
    let (_dir, store, _tensorlake) = service(&api).await;
    let host = plant(
        &store,
        "gpu dev",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        "gpu-dev",
        None,
    )
    .await;
    let planted = row(&store, host).await;
    assert_eq!(planted.kind, crate::store::HostKind::Tensorlake);
    assert_eq!(planted.alias.as_deref(), Some("gpu dev"));
    assert_eq!(
        planted.destination.as_deref(),
        Some("farhelm-host-k7q2-gpu-dev-m3x9@farhelm-host-k7q2-gpu-dev-m3x9.sandbox.tensorlake.ai")
    );
    assert_eq!(
        planted.tensorlake,
        Some(crate::store::TensorlakeBinding {
            sandbox: "farhelm-host-k7q2-gpu-dev-m3x9".into(),
            sandbox_id: None,
            pending_create_at: None,
            recipe: "gpu-dev".into(),
            resources: Some(farhelm_proto::tensorlake::recipes::RecipeSize {
                cpus: 2,
                memory_gb: 4,
                disk_gb: 20,
            }),
            suspended_at: None,
        })
    );
    assert_eq!(
        planted.icon,
        farhelm_proto::host_appearance::HostIcon::Cloud
    );
    assert_eq!(
        planted.color,
        farhelm_proto::host_appearance::HostColor::Default
    );
    assert!(
        planted.yolo_without_asking,
        "D17: new Tensorlake hosts start YOLO without asking"
    );
    assert!(!planted.commands_without_asking);
    assert!(!planted.links_without_asking);
    assert!(!planted.local_links_without_asking);

    assert!(
        store
            .set_tensorlake_sandbox_id(host, "sbx-7")
            .await
            .unwrap()
    );
    assert!(
        store
            .set_tensorlake_suspended_at(host, Some(1_700_000_000_000))
            .await
            .unwrap()
    );
    let binding = row(&store, host).await.tensorlake.unwrap();
    assert_eq!(binding.sandbox_id.as_deref(), Some("sbx-7"));
    assert_eq!(binding.suspended_at, Some(1_700_000_000_000));
    assert!(store.set_tensorlake_suspended_at(host, None).await.unwrap());
    assert_eq!(
        row(&store, host).await.tensorlake.unwrap().suspended_at,
        None
    );
    let local = store.list_hosts().await.unwrap()[0].id;
    assert!(
        !store
            .set_tensorlake_sandbox_id(local, "sbx-x")
            .await
            .unwrap(),
        "only a Tensorlake row takes a sandbox id"
    );

    store
        .conn()
        .call("test hand edit", move |conn| {
            conn.execute(
                "UPDATE hosts SET tensorlake_recipe = NULL WHERE id = ?1",
                [host],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let error = store
        .list_hosts()
        .await
        .expect_err("a broken binding fails the read");
    assert!(error.to_string().contains("no recipe"), "{error:#}");
}

/// Why: a Tensorlake host must obey every kind decision as a sandbox, not
/// fall through to whatever an ssh row does (P2); this pins the predicate
/// table in one place.
///
/// Spec: each predicate answers per kind as the design's table says.
#[farhelm_testtrace::test]
fn the_kind_predicates_decide_for_sandboxes() {
    use crate::ssh::SshAccessKind;
    use crate::store::HostKind::{Local, Ssh, Tensorlake};
    let table = [
        // kind, reserved local, remote install, panel updates, panel uninstalls,
        // missing local supervisor, remote, destination editable, manual masters,
        // removal deletes machine, ssh access
        (
            Local, true, false, false, false, true, false, false, false, false, None,
        ),
        (
            Ssh,
            false,
            true,
            true,
            true,
            false,
            true,
            true,
            true,
            false,
            Some(SshAccessKind::UserConfig),
        ),
        (
            Tensorlake,
            false,
            true,
            true,
            false,
            false,
            true,
            false,
            false,
            true,
            Some(SshAccessKind::Tensorlake),
        ),
    ];
    for (
        kind,
        local,
        install,
        updates,
        uninstalls,
        missing,
        remote,
        editable,
        masters,
        deletes,
        access,
    ) in table
    {
        assert_eq!(kind.is_reserved_local(), local, "{kind:?}");
        assert_eq!(kind.has_remote_install(), install, "{kind:?}");
        assert_eq!(kind.panel_updates(), updates, "{kind:?}");
        assert_eq!(kind.panel_uninstalls(), uninstalls, "{kind:?}");
        assert_eq!(kind.reports_missing_local_supervisor(), missing, "{kind:?}");
        assert_eq!(kind.is_remote(), remote, "{kind:?}");
        assert_eq!(kind.destination_is_editable(), editable, "{kind:?}");
        assert_eq!(kind.offers_manual_ssh_masters(), masters, "{kind:?}");
        assert_eq!(kind.removal_deletes_machine(), deletes, "{kind:?}");
        assert_eq!(kind.ssh_access(), access, "{kind:?}");
    }
    assert_eq!(
        Tensorlake.column(),
        Ssh.column(),
        "a sandbox is an ssh row on disk"
    );
}

/// Why: the store is where "no path forgets a Tensorlake host while its
/// sandbox bills" is enforced (goal criterion 5), so every edit path that
/// could lose the sandbox must be refused there, whoever calls it.
///
/// Spec: retargeting a Tensorlake host is refused (`DestinationManaged`) and
/// changes nothing; forgetting it without a proof, or with a proof for
/// another host, is refused (`SandboxNotDeleted`); its alias and appearance
/// stay editable; forgetting an ordinary ssh host still needs no proof.
#[farhelm_testtrace::test]
async fn the_store_refuses_to_lose_a_sandbox() {
    let api = FakeApi::default();
    let (_dir, store, _tensorlake) = service(&api).await;
    let host = plant(
        &store,
        "gpu dev",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        "gpu-dev",
        None,
    )
    .await;
    let ssh = store.add_ssh_host("user@box", None, None).await.unwrap();
    let before = row(&store, host).await;

    let retarget = store
        .update_ssh_destination(host, "user@elsewhere")
        .await
        .expect_err("a sandbox's destination is not editable");
    assert!(matches!(
        retarget.downcast_ref::<HostStoreError>(),
        Some(HostStoreError::DestinationManaged)
    ));
    assert_eq!(row(&store, host).await, before);

    for proof in [None, Some(SandboxDeleted::attest(ssh))] {
        let refused = store
            .remove_host_row(host, proof.as_ref())
            .await
            .expect_err("no proof for this host, no forgetting");
        assert!(matches!(
            refused.downcast_ref::<HostStoreError>(),
            Some(HostStoreError::SandboxNotDeleted(id)) if *id == host
        ));
    }
    assert_eq!(row(&store, host).await, before);

    assert!(store.update_alias(host, Some("renamed")).await.unwrap());
    store
        .set_appearance(
            host,
            farhelm_proto::host_appearance::HostIcon::Rocket,
            farhelm_proto::host_appearance::HostColor::Default,
        )
        .await
        .unwrap();
    assert_eq!(
        row(&store, host).await.icon,
        farhelm_proto::host_appearance::HostIcon::Rocket
    );
    store
        .remove_ssh_host(ssh)
        .await
        .expect("an ssh host is merely forgotten");
}

/// Why: a recipe a host was made from must not disappear under it (P9);
/// the recipes change left the count for this change to fill in.
///
/// Spec: deleting a recipe a Tensorlake host uses is refused with the
/// count of such hosts; once the host is removed, the delete succeeds.
#[farhelm_testtrace::test]
async fn a_recipe_in_use_cannot_be_deleted() {
    let api = FakeApi::default();
    let (_dir, store, tensorlake) = service(&api).await;
    crate::tensorlake::recipes::tests_support::create(&store, "gpu-dev").await;
    let first = plant(&store, "one", "farhelm-host-k7q2-one-aaaa", "gpu-dev", None).await;
    let second = plant(&store, "two", "farhelm-host-k7q2-two-bbbb", "gpu-dev", None).await;
    assert!(matches!(
        store
            .delete_tensorlake_recipe("gpu-dev".into())
            .await
            .unwrap(),
        Err(crate::tensorlake::recipes::Refusal::InUse(2))
    ));
    for host in [first, second] {
        let proof = tensorlake
            .delete_sandbox(&row(&store, host).await)
            .await
            .unwrap();
        store.remove_host_row(host, Some(&proof)).await.unwrap();
    }
    assert!(matches!(
        store
            .delete_tensorlake_recipe("gpu-dev".into())
            .await
            .unwrap(),
        Ok(())
    ));
}

// ---------------------------------------------------------------------------
// Removal through the REST route
// ---------------------------------------------------------------------------

/// A helm over `api` with the key saved and one planted Tensorlake host,
/// reconciled so it has an actor like any listed host.
async fn helm_with_sandbox_host(
    api: &FakeApi,
) -> (crate::rest_harness::Harness, crate::store::HostId) {
    sandbox_helm(api, false).await
}

/// Preserve removal tests' API-call isolation while lifecycle tests opt into
/// the actor's status seam. Both use the same registration and saved key.
async fn sandbox_helm(
    api: &FakeApi,
    actor_checks_status: bool,
) -> (crate::rest_harness::Harness, HostId) {
    let mut builder = crate::rest_harness::FleetBuilder::new()
        .await
        .tensorlake_api(Arc::new(api.clone()));
    if actor_checks_status {
        builder = builder.sandboxes(Arc::new(api.clone()));
    }
    let harness = builder.start().await;
    harness
        .store
        .save_tensorlake_config(SaveSettings {
            api_key: Some(KEY.into()),
            clear_api_key: false,
            create_without_asking: None,
        })
        .await
        .map_err(|_| "save failed")
        .unwrap();
    let host = plant(
        &harness.store,
        "gpu dev",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        "gpu-dev",
        Some("sbx-1"),
    )
    .await;
    // Removal exercises an installed host. Initial setup's durable gate
    // stays closed on rows without these coordinates, before any dial.
    harness
        .store
        .admit_tensorlake_install(host, "/fixture/farhelm", "/fixture/state")
        .await
        .unwrap();
    harness.state.manager.sync_registry().await.unwrap();
    (harness, host)
}

/// `DELETE /api/hosts/{id}` through the real protected router.
async fn delete_host(
    harness: &crate::rest_harness::Harness,
    host: crate::store::HostId,
) -> (axum::http::StatusCode, String) {
    use tower::ServiceExt as _;
    let request = axum::http::Request::builder()
        .method("DELETE")
        .uri(format!("/api/hosts/{host}"))
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = harness.router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

/// Exercise the protected lifecycle routes rather than bypassing admission,
/// task ownership or the provisioning lock in a service-only test.
async fn lifecycle_host(
    harness: &crate::rest_harness::Harness,
    host: HostId,
    verb: &str,
) -> (axum::http::StatusCode, String) {
    use tower::ServiceExt as _;
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/hosts/{host}/{verb}"))
        .header("host", "127.0.0.1:7433")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = harness.router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

/// Suspend must withdraw routing without losing the registered machine. Resume
/// must preserve the host/session identity while earning a fresh connection.
/// Repeating either verb is safe and does not issue a second vendor mutation.
#[farhelm_testtrace::test]
async fn lifecycle_suspend_resume_preserves_the_host_and_is_idempotent() {
    let api = FakeApi::default().with_sandbox(
        "sbx-1",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        SandboxStatus::Running,
    );
    let (harness, host) = sandbox_helm(&api, true).await;
    harness.await_state(host, HostState::is_connected).await;
    let before = harness.state.manager.status(host).unwrap().incarnation;
    // A prior Farhelm suspension may have been resumed through Tensorlake's
    // own controls. A new Suspend must replace that stale wording timestamp.
    harness
        .store
        .set_tensorlake_suspended_at(host, Some(1))
        .await
        .unwrap();
    let mut suspended_at = None;
    for _ in 0..2 {
        let (status, body) = lifecycle_host(&harness, host, "suspend").await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        let at = row(&harness.store, host)
            .await
            .tensorlake
            .unwrap()
            .suspended_at
            .unwrap();
        assert!(at > 1, "a fresh suspension replaces stale metadata");
        if let Some(previous) = suspended_at {
            assert_eq!(at, previous, "repeated Suspend keeps its original time");
        }
        suspended_at = Some(at);
    }
    harness
        .await_state(host, |state| matches!(state, HostState::Suspended { .. }))
        .await;
    let suspended = harness.state.manager.status(host).unwrap();
    assert!(matches!(suspended.state, HostState::Suspended { .. }));
    assert!(suspended.client.is_none());
    assert!(
        row(&harness.store, host)
            .await
            .tensorlake
            .unwrap()
            .suspended_at
            .is_some()
    );
    assert_eq!(
        api.calls()
            .iter()
            .filter(|call| call.starts_with("suspend "))
            .count(),
        1
    );
    let mut resumed_incarnation = None;
    for _ in 0..2 {
        let (status, body) = lifecycle_host(&harness, host, "resume").await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        harness.await_state(host, HostState::is_connected).await;
        let current = harness.state.manager.status(host).unwrap().incarnation;
        if let Some(previous) = resumed_incarnation {
            assert_eq!(
                current, previous,
                "repeated Resume must not detach connected viewers"
            );
        }
        resumed_incarnation = Some(current);
    }
    harness.await_state(host, HostState::is_connected).await;
    assert_ne!(
        harness.state.manager.status(host).unwrap().incarnation,
        before
    );
    assert!(
        row(&harness.store, host)
            .await
            .tensorlake
            .unwrap()
            .suspended_at
            .is_none()
    );
    assert_eq!(
        api.calls()
            .iter()
            .filter(|call| call.starts_with("resume "))
            .count(),
        1
    );
}

/// External suspension can precede the actor's detection. A real Resume
/// must invalidate that old Connected token; only a repeated Resume once
/// running and recovered may preserve a connected viewer's token.
#[farhelm_testtrace::test]
async fn lifecycle_resume_replaces_a_pre_suspension_connected_actor() {
    let api = FakeApi::default().with_sandbox(
        "sbx-1",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        SandboxStatus::Running,
    );
    let (harness, host) = sandbox_helm(&api, true).await;
    harness.await_state(host, HostState::is_connected).await;
    let before = harness.state.manager.status(host).unwrap().incarnation;
    api.0.lock().unwrap().sandboxes[0].2 = SandboxStatus::Suspended;
    assert!(
        harness
            .state
            .manager
            .status(host)
            .unwrap()
            .state
            .is_connected(),
        "premise: the actor has not detected the external suspension"
    );
    let (status, body) = lifecycle_host(&harness, host, "resume").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert!(
        !harness
            .state
            .manager
            .status(host)
            .unwrap()
            .state
            .is_connected()
            || harness.state.manager.status(host).unwrap().incarnation != before,
        "the pre-suspension connection must be withdrawn before success"
    );
    harness.await_state(host, HostState::is_connected).await;
    let recovered = harness.state.manager.status(host).unwrap().incarnation;
    assert_ne!(recovered, before);
    let (status, body) = lifecycle_host(&harness, host, "resume").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(
        harness.state.manager.status(host).unwrap().incarnation,
        recovered
    );
    assert_eq!(
        api.calls()
            .iter()
            .filter(|call| call.starts_with("resume "))
            .count(),
        1
    );
}

/// A failed suspend may not strand a hold, and a busy lifecycle request may
/// not race removal or an update. Neither refusal forgets the registered host.
#[farhelm_testtrace::test]
async fn lifecycle_refusal_releases_the_hold_and_busy_hosts_do_not_mutate() {
    let api = FakeApi::default()
        .with_sandbox(
            "sbx-1",
            "farhelm-host-k7q2-gpu-dev-m3x9",
            SandboxStatus::Running,
        )
        .failing(
            "suspend",
            TensorlakeError::Transport("injected failure".into()),
        );
    let (harness, host) = sandbox_helm(&api, true).await;
    harness.await_state(host, HostState::is_connected).await;
    let (status, body) = lifecycle_host(&harness, host, "suspend").await;
    assert_eq!(
        status,
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "{body}"
    );
    harness.await_state(host, HostState::is_connected).await;
    let lock = harness.state.manager.try_host_provision_lock(host).unwrap();
    let calls = api.calls().len();
    let (status, body) = lifecycle_host(&harness, host, "suspend").await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert_eq!(api.calls().len(), calls);
    drop(lock);
    let local = harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.kind.is_reserved_local())
        .unwrap()
        .id;
    let (status, body) = lifecycle_host(&harness, local, "resume").await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
}

/// Resume is an explicit recovery action even when a transport bug retired
/// the actor. Releasing a hold alone cannot wake a dead task; the fresh retry
/// must revive it before the host can carry sessions again.
#[farhelm_testtrace::test]
async fn lifecycle_resume_revives_a_retired_actor() {
    let api = FakeApi::default().with_sandbox(
        "sbx-1",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        SandboxStatus::Running,
    );
    let (harness, host) = sandbox_helm(&api, true).await;
    harness.await_state(host, HostState::is_connected).await;
    harness
        .fleet
        .edit(host, |script| script.panic_on_dial = true);
    harness.fleet.kill_connection(host);
    harness
        .await_state(host, |state| matches!(state, HostState::Retired { .. }))
        .await;
    api.0.lock().unwrap().sandboxes[0].2 = SandboxStatus::Suspended;
    harness
        .fleet
        .edit(host, |script| script.panic_on_dial = false);
    let (status, body) = lifecycle_host(&harness, host, "resume").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    harness.await_state(host, HostState::is_connected).await;
}

/// A concurrent Retry cannot reopen a held transport, and Remove cannot
/// terminate a sandbox while Suspend owns its operation lock. Observe both
/// while the vendor call is deliberately unsettled, before releasing it.
/// After confirmation an active Retry may briefly publish Connecting while
/// rechecking the sandbox; inspect attribution once that probe settles.
#[farhelm_testtrace::test]
async fn lifecycle_suspend_holds_routing_and_excludes_removal() {
    let api = FakeApi::default().with_sandbox(
        "sbx-1",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        SandboxStatus::Running,
    );
    let gate = Arc::new(Gate::default());
    api.0.lock().unwrap().suspend_gate = Some(gate.clone());
    let (harness, host) = sandbox_helm(&api, true).await;
    harness.await_state(host, HostState::is_connected).await;
    let harness = Arc::new(harness);
    let suspended = tokio::spawn({
        let harness = harness.clone();
        async move { lifecycle_host(&harness, host, "suspend").await }
    });
    tokio::time::timeout(Duration::from_secs(10), gate.entered.notified())
        .await
        .unwrap();
    let held = harness.state.manager.status(host).unwrap();
    assert!(held.client.is_none());
    let HostState::TensorlakeChanging {
        direction,
        requested_at,
    } = held.state
    else {
        panic!(
            "unconfirmed suspension must publish its intent: {:?}",
            held.state
        );
    };
    assert_eq!(direction, LifecycleDirection::Suspending);
    assert!(requested_at > 0);
    assert_eq!(api.0.lock().unwrap().sandboxes[0].2, SandboxStatus::Running);
    assert!(
        row(&harness.store, host)
            .await
            .tensorlake
            .unwrap()
            .suspended_at
            .is_none(),
        "no completion time exists before the vendor confirms"
    );
    let dials = harness.fleet.dials(host);
    harness.state.manager.retry_now(host).await.unwrap();
    let (status, body) = delete_host(&harness, host).await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert_eq!(harness.fleet.dials(host), dials);
    gate.release.notify_one();
    let (status, body) = suspended.await.unwrap();
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(
        api.0.lock().unwrap().sandboxes[0].2,
        SandboxStatus::Suspended
    );
    harness
        .await_state(host, |state| matches!(state, HostState::Suspended { .. }))
        .await;
    let view = crate::hosts::host_views(&harness.state)
        .await
        .unwrap()
        .into_iter()
        .find(|view| view.id == host)
        .unwrap();
    let crate::hosts::HostStateView::Suspended {
        suspended_by_farhelm_at,
        ..
    } = view.state
    else {
        panic!(
            "the settled suspension must retain its confirmation: {:?}",
            view.state
        );
    };
    assert!(suspended_by_farhelm_at.is_some_and(|at| at >= requested_at));
}

/// A disconnected Resume shows intent while Tensorlake is still suspended.
/// It must not publish another Suspended state between confirmation and the
/// fresh connection, and its request cannot open a transport while held.
#[farhelm_testtrace::test]
async fn lifecycle_resume_holds_its_direction_until_the_api_finishes() {
    let api = FakeApi::default().with_sandbox(
        "sbx-1",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        SandboxStatus::Suspended,
    );
    let gate = Arc::new(Gate::default());
    api.0.lock().unwrap().resume_gate = Some(gate.clone());
    let (harness, host) = sandbox_helm(&api, true).await;
    harness
        .await_state(host, |state| matches!(state, HostState::Suspended { .. }))
        .await;
    let harness = Arc::new(harness);
    let resumed = tokio::spawn({
        let harness = harness.clone();
        async move { lifecycle_host(&harness, host, "resume").await }
    });
    tokio::time::timeout(Duration::from_secs(10), gate.entered.notified())
        .await
        .unwrap();
    let held = harness.state.manager.status(host).unwrap();
    assert!(matches!(
        held.state,
        HostState::TensorlakeChanging {
            direction: LifecycleDirection::Resuming,
            requested_at: 1..,
        }
    ));
    assert!(held.client.is_none());
    assert_eq!(
        api.0.lock().unwrap().sandboxes[0].2,
        SandboxStatus::Suspended
    );
    let dials = harness.fleet.dials(host);
    harness.state.manager.retry_now(host).await.unwrap();
    assert_eq!(harness.fleet.dials(host), dials);
    gate.release.notify_one();
    let (status, body) = resumed.await.unwrap();
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert!(!matches!(
        harness.state.manager.state(host),
        Some(HostState::Suspended { .. })
    ));
    harness.await_state(host, HostState::is_connected).await;
}

/// Neither a vendor refusal nor an expired lifecycle budget may strand a
/// connection hold. Observe the unsettled operation before causing either
/// outcome, then prove a running sandbox can obtain a fresh connection.
#[farhelm_testtrace::test(start_paused = true)]
async fn lifecycle_failures_and_timeouts_release_each_direction() {
    for resume in [false, true] {
        for timed_out in [false, true] {
            let verb = if resume { "resume" } else { "suspend" };
            let initial = if resume {
                SandboxStatus::Suspended
            } else {
                SandboxStatus::Running
            };
            let api =
                FakeApi::default().with_sandbox("sbx-1", "farhelm-host-k7q2-gpu-dev-m3x9", initial);
            let gate = Arc::new(Gate::default());
            {
                let mut state = api.0.lock().unwrap();
                if resume {
                    state.resume_gate = Some(gate.clone());
                } else {
                    state.suspend_gate = Some(gate.clone());
                }
                state.failures.push((
                    verb,
                    TensorlakeError::Transport("injected lifecycle failure".into()),
                ));
            }
            let (harness, host) = sandbox_helm(&api, true).await;
            if resume {
                harness
                    .await_state(host, |state| matches!(state, HostState::Suspended { .. }))
                    .await;
            } else {
                harness.await_state(host, HostState::is_connected).await;
            }
            let harness = Arc::new(harness);
            let request = tokio::spawn({
                let harness = harness.clone();
                async move { lifecycle_host(&harness, host, verb).await }
            });
            tokio::time::timeout(Duration::from_secs(10), gate.entered.notified())
                .await
                .unwrap();
            assert!(matches!(
                harness.state.manager.state(host),
                Some(HostState::TensorlakeChanging { direction, .. })
                    if direction == if resume { LifecycleDirection::Resuming } else { LifecycleDirection::Suspending }
            ));
            assert_eq!(api.0.lock().unwrap().sandboxes[0].2, initial);
            if timed_out {
                tokio::time::advance(Duration::from_secs(if resume { 120 } else { 60 })).await;
            } else {
                gate.release.notify_one();
            }
            let (status, body) = request.await.unwrap();
            assert_eq!(
                status,
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "{body}"
            );
            assert!(
                body.contains(if timed_out {
                    "within"
                } else {
                    "injected lifecycle failure"
                }),
                "{body}"
            );
            assert!(
                row(&harness.store, host)
                    .await
                    .tensorlake
                    .unwrap()
                    .suspended_at
                    .is_none()
            );
            // A refused Resume leaves the sandbox suspended. Simulate it
            // running externally before asking the actor to prove recovery;
            // a leftover hold would still prevent this retry from dialing.
            api.0.lock().unwrap().sandboxes[0].2 = SandboxStatus::Running;
            harness.state.manager.retry_now(host).await.unwrap();
            harness.await_state(host, HostState::is_connected).await;
        }
    }
}

/// Suspend and Delete must drop actual actor-owned tunnel leases before
/// reaching their vendor calls. Resume acquires a new lease from the edited
/// recipe. A stalled acquisition must be cancelled by a hold too: otherwise
/// it can start forwards after the lifecycle operation retired its connection.
/// The marked fake replaces listener execution only; REST admission, manager
/// parking, lease ownership and recipe loading are production paths.
#[farhelm_testtrace::test]
async fn lifecycle_holds_end_tunnels_and_resume_reads_the_recipe_again() {
    use crate::tensorlake::e2e::E2eTensorlake;
    use farhelm_proto::tensorlake::{
        TunnelState,
        recipes::{RecipeBase, RecipeSave, RecipeSize, Tunnel},
    };
    let api = FakeApi::default().with_sandbox(
        "sbx-1",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        SandboxStatus::Running,
    );
    let (harness, host) = sandbox_helm(&api, true).await;
    harness.await_state(host, HostState::is_connected).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("fake");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("ENABLED"), "farhelm-e2e-tensorlake-v1\n").unwrap();
    let fake = Arc::new(E2eTensorlake::new(root, dir.path()).unwrap());
    let service = super::super::Tensorlake(Arc::new(super::super::Inner::new(
        harness.store.clone(),
        fake.clone(),
        fake.clone(),
        None,
        Some(fake),
        crate::provisioning::credential_delivery::CredentialDelivery::detached(
            harness.store.clone(),
        ),
        Arc::new(crate::tensorlake::prepared::UnavailablePreparer),
    )));
    let mut recipe = RecipeSave {
        size: RecipeSize {
            cpus: 2,
            memory_gb: 4,
            disk_gb: 20,
        },
        packages: vec![],
        base: RecipeBase::Image {
            image: "tensorlake/ubuntu-systemd".into(),
        },
        claude: None,
        codex: None,
        github_token_credential: None,
        claude_login_credential: None,
        codex_login_credential: None,
        tunnels: vec![Tunnel {
            sandbox_port: 8080,
            host: "127.0.0.1".into(),
            port: 8080,
        }],
    };
    harness
        .store
        .create_tensorlake_recipe("gpu-dev".into(), recipe.clone())
        .await
        .unwrap()
        .unwrap();
    api.0.lock().unwrap().tunnels = Some(service.clone());
    let before = harness.state.manager.status(host).unwrap().incarnation;
    harness.state.manager.retry_now(host).await.unwrap();
    harness.await_refreshed_after(host, before).await;
    // Connected publication follows lease acquisition, so a receiver exists
    // here. Up establishes the executing fake's readiness before teardown.
    let mut first = service.0.tunnels.subscribe(host).unwrap();
    tokio::time::timeout(
        Duration::from_secs(10),
        first.wait_for(|view| view.state == TunnelState::Up),
    )
    .await
    .unwrap()
    .unwrap();
    let suspend_gate = Arc::new(Gate::default());
    api.0.lock().unwrap().suspend_gate = Some(suspend_gate.clone());
    let harness = Arc::new(harness);
    let suspended = tokio::spawn({
        let harness = harness.clone();
        async move { lifecycle_host(&harness, host, "suspend").await }
    });
    tokio::time::timeout(Duration::from_secs(10), suspend_gate.entered.notified())
        .await
        .unwrap();
    assert_eq!(
        api.0.lock().unwrap().sandboxes[0].2,
        SandboxStatus::Running,
        "premise: Suspend has not mutated the vendor yet"
    );
    assert!(
        service.tunnel_view(host).is_none(),
        "lease must drop before Suspend reaches the API"
    );
    tokio::time::timeout(Duration::from_secs(10), first.changed())
        .await
        .unwrap()
        .expect_err("the old tunnel worker must end");
    recipe.tunnels[0].port = 9090;
    harness
        .store
        .update_tensorlake_recipe("gpu-dev".into(), recipe)
        .await
        .unwrap()
        .unwrap();
    suspend_gate.release.notify_one();
    let (status, body) = suspended.await.unwrap();
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert!(service.tunnel_view(host).is_none());
    let (status, body) = lifecycle_host(&harness, host, "resume").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    harness.await_state(host, HostState::is_connected).await;
    let mut resumed = service.0.tunnels.subscribe(host).unwrap();
    tokio::time::timeout(
        Duration::from_secs(10),
        resumed.wait_for(|view| view.state == TunnelState::Up),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        resumed.borrow().tunnels[0].port,
        9090,
        "Resume reads the current recipe"
    );

    // Hold while the next connected phase is still waiting for its recipe.
    // Releasing this gate afterwards must not resurrect the cancelled work.
    let connected_gate = Arc::new(Gate::default());
    api.0.lock().unwrap().connected_gate = Some(connected_gate.clone());
    harness.state.manager.retry_now(host).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), connected_gate.entered.notified())
        .await
        .unwrap();
    let suspended = tokio::spawn({
        let harness = harness.clone();
        async move { lifecycle_host(&harness, host, "suspend").await }
    });
    tokio::time::timeout(Duration::from_secs(10), suspend_gate.entered.notified())
        .await
        .expect("a hold must cancel resource acquisition before its API call");
    connected_gate.release.notify_one();
    api.0.lock().unwrap().connected_gate = None;
    assert!(service.tunnel_view(host).is_none());
    suspend_gate.release.notify_one();
    let (status, body) = suspended.await.unwrap();
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let (status, body) = lifecycle_host(&harness, host, "resume").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    harness.await_state(host, HostState::is_connected).await;
    let mut deleting = service.0.tunnels.subscribe(host).unwrap();
    tokio::time::timeout(
        Duration::from_secs(10),
        deleting.wait_for(|view| view.state == TunnelState::Up),
    )
    .await
    .unwrap()
    .unwrap();
    let terminate_gate = Arc::new(Gate::default());
    api.0.lock().unwrap().terminate_gate = Some(terminate_gate.clone());
    let deleted = tokio::spawn({
        let harness = harness.clone();
        async move { delete_host(&harness, host).await }
    });
    tokio::time::timeout(Duration::from_secs(10), terminate_gate.entered.notified())
        .await
        .unwrap();
    assert_eq!(
        api.0.lock().unwrap().sandboxes[0].2,
        SandboxStatus::Running,
        "premise: Delete has not terminated the vendor sandbox yet"
    );
    assert!(
        service.tunnel_view(host).is_none(),
        "lease must drop before Delete reaches the API"
    );
    terminate_gate.release.notify_one();
    let (status, body) = deleted.await.unwrap();
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert!(harness.state.manager.status(host).is_none());
}

/// Why: `DELETE /api/hosts/{id}` is the GUI's removal, and for a Tensorlake
/// host it must delete the sandbox before the host leaves the list, and keep
/// the host when it cannot (D6, goal criterion 5).
///
/// Spec: a removal Tensorlake refuses partway answers 503 with the reason
/// and leaves the host listed with its actor; removing again finishes the
/// job (terminate, sweep), answers 200 and the host is gone from the
/// registry and the fleet.
#[farhelm_testtrace::test]
async fn the_rest_removal_deletes_the_sandbox_before_forgetting_the_host() {
    let api = FakeApi::default()
        .with_sandbox(
            "sbx-1",
            "farhelm-host-k7q2-gpu-dev-m3x9",
            SandboxStatus::Running,
        )
        .with_snapshot("snap-a", "sbx-1")
        .failing(
            "list-snapshots",
            TensorlakeError::Status {
                status: 500,
                message: "try later".into(),
            },
        );
    let (harness, host) = helm_with_sandbox_host(&api).await;
    let (status, body) = delete_host(&harness, host).await;
    assert_eq!(
        status,
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "{body}"
    );
    assert!(body.contains("try later"), "{body}");
    assert!(
        harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .any(|r| r.id == host)
    );
    assert!(
        harness.state.manager.status(host).is_some(),
        "the actor stays"
    );

    let (status, body) = delete_host(&harness, host).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert!(
        harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .all(|r| r.id != host)
    );
    assert!(
        harness.state.manager.status(host).is_none(),
        "the actor stopped"
    );
    assert!(api.snapshot_ids().is_empty());
    assert!(api.calls().contains(&"terminate sbx-1".to_string()));
}

/// Why: closing a sandbox's shared connections only helps if the host's own
/// actor cannot open new ones while the removal waits on Tensorlake, and a
/// host being deleted must not take new work (review P2-2, design C §7.1).
///
/// Spec: while a removal's terminate call is pending, the host is listed as
/// not connected with no routable client, and neither a retry nor time
/// passing produces a dial; once terminate answers, the removal finishes and
/// the host is gone without having been dialed again.
#[farhelm_testtrace::test]
async fn no_dial_or_client_while_the_sandbox_is_being_terminated() {
    let api = FakeApi::default().with_sandbox(
        "sbx-1",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        SandboxStatus::Running,
    );
    let gate = Arc::new(Gate::default());
    api.0.lock().unwrap().terminate_gate = Some(Arc::clone(&gate));
    let (harness, host) = helm_with_sandbox_host(&api).await;
    harness
        .state
        .manager
        .wait_for_state(host, |state| {
            matches!(state, crate::manager::HostState::Connected { .. })
        })
        .await
        .expect("premise: the host's actor connects first");
    let harness = Arc::new(harness);
    let removal = tokio::spawn({
        let harness = Arc::clone(&harness);
        async move { delete_host(&harness, host).await }
    });
    gate.entered.notified().await;
    let dials = harness.fleet.dials(host);
    let held = harness.state.manager.status(host).expect("still listed");
    assert!(held.client.is_none(), "no routable client mid-removal");
    assert!(!held.state.is_connected(), "{:?}", held.state);

    harness.state.manager.retry_now(host).await.unwrap();
    // sleep-ok: an observation window longer than the harness's retry ladder, in which no dial may happen
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(
        harness.fleet.dials(host),
        dials,
        "no dial while terminating"
    );
    assert!(harness.state.manager.status(host).unwrap().client.is_none());

    gate.release.notify_one();
    let (status, body) = removal.await.unwrap();
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert!(harness.state.manager.status(host).is_none());
    assert_eq!(harness.fleet.dials(host), dials);
}

/// A Retry of a retired host must inherit removal's hold before starting a
/// replacement. Otherwise its still-running sandbox accepts a new connection
/// after removal has already closed the shared masters.
#[farhelm_testtrace::test]
async fn retry_preserves_a_retired_hosts_removal_hold() {
    removal_hold_survives_replacement(true, false).await;
}

/// Registry reconciliation can replace a dead actor after an unrelated host
/// edit. It must preserve the same exclusion as Retry throughout termination,
/// with no dial or routable client even though the sandbox is still Running.
#[farhelm_testtrace::test]
async fn registry_sync_preserves_a_retired_hosts_removal_hold() {
    removal_hold_survives_replacement(false, false).await;
}

/// Failed removal must release the replacement's hold, rather than only the
/// dead actor's: a Running sandbox must reconnect and become routable again
/// while its retained host row lets the user retry removal.
#[farhelm_testtrace::test]
async fn failed_removal_releases_the_replacement_actors_hold() {
    removal_hold_survives_replacement(true, true).await;
}

/// Exercise replacement through the real removal route with termination held
/// before it changes sandbox state. Retirement is observed before removal;
/// the dial count and routability distinguish a carried hold from a new actor
/// that merely has not connected yet. A refused termination leaves the sandbox
/// Running so reconnection after release is observable too.
async fn removal_hold_survives_replacement(retry: bool, fail_removal: bool) {
    let api = FakeApi::default().with_sandbox(
        "sbx-1",
        "farhelm-host-k7q2-gpu-dev-m3x9",
        SandboxStatus::Running,
    );
    if fail_removal {
        api.0.lock().unwrap().failures.push((
            "terminate",
            TensorlakeError::Status {
                status: 500,
                message: "try later".into(),
            },
        ));
    }
    let gate = Arc::new(Gate::default());
    api.0.lock().unwrap().terminate_gate = Some(Arc::clone(&gate));
    let (harness, host) = helm_with_sandbox_host(&api).await;
    harness
        .await_state(host, |state| state.is_connected())
        .await;
    harness
        .fleet
        .edit(host, |script| script.panic_on_dial = true);
    harness.fleet.kill_connection(host);
    harness
        .await_state(host, |state| state.phase() == "retired")
        .await;
    assert!(harness.state.manager.status(host).unwrap().client.is_none());
    harness
        .fleet
        .edit(host, |script| script.panic_on_dial = false);

    let harness = Arc::new(harness);
    let removal = tokio::spawn({
        let harness = Arc::clone(&harness);
        async move { delete_host(&harness, host).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), gate.entered.notified())
        .await
        .expect("removal reaches termination after holding the retired host");
    assert_eq!(
        api.0.lock().unwrap().sandboxes[0].2,
        SandboxStatus::Running,
        "premise: termination has not stopped the sandbox"
    );
    let dials = harness.fleet.dials(host);
    let held = harness.state.manager.status(host).unwrap().state;
    assert!(!held.is_connected());
    if retry {
        harness.state.manager.retry_now(host).await.unwrap();
    } else {
        harness.state.manager.sync_registry().await.unwrap();
    }
    // sleep-ok: let an erroneously unheld replacement dial over an interval longer than the harness's retry ladder
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let status = harness
        .state
        .manager
        .status(host)
        .expect("held row stays listed");
    assert_eq!(
        harness.fleet.dials(host),
        dials,
        "replacement cannot dial during removal"
    );
    assert!(
        status.client.is_none(),
        "replacement cannot route during removal"
    );
    assert_eq!(
        status.state, held,
        "replacement preserves removal's presentation"
    );
    assert_eq!(api.0.lock().unwrap().sandboxes[0].2, SandboxStatus::Running);

    gate.release.notify_one();
    let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(10), removal)
        .await
        .expect("removal completes after termination is released")
        .unwrap();
    if fail_removal {
        assert_eq!(
            status,
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "{body}"
        );
        assert!(body.contains("try later"), "{body}");
        harness
            .await_state(host, |state| state.is_connected())
            .await;
        assert!(
            harness.state.manager.status(host).unwrap().client.is_some(),
            "the failed removal releases the current actor for routing"
        );
        assert_eq!(harness.fleet.dials(host), dials + 1);
        assert!(
            harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == host)
        );
    } else {
        assert_eq!(status, axum::http::StatusCode::OK, "{body}");
        assert!(harness.state.manager.status(host).is_none());
        assert_eq!(
            harness.fleet.dials(host),
            dials,
            "success stops the held replacement"
        );
    }
}
