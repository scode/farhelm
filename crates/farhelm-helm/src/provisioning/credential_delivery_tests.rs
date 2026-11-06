//! Delivery contracts over an owned filesystem and the existing provisioning
//! fake. Counters include reads and home recovery, so an unchanged reconnect
//! proves absence of remote work rather than merely absence of a final write.

use super::*;
use crate::provisioning::tests::FakeBackend;
use farhelm_proto::credentials::{CreateCredential, CredentialKind, SaveCredential};
use farhelm_proto::tensorlake::recipes::EndpointSave;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::task::{Context, Waker};

const FIRST: &str = "fictional-first-endpoint-token";
const SECOND: &str = "fictional-replacement-endpoint-token";

/// Own both sides of delivery until every file and durable stamp is inspected.
struct Fixture {
    _root: tempfile::TempDir,
    home: PathBuf,
    store: HelmStore,
    row: HostRow,
    backend: Arc<FakeBackend>,
    delivery: Arc<CredentialDelivery>,
    events: FleetEvents,
}

impl Fixture {
    /// Registration uses the real recipe/reference store; no supervisor or
    /// vendor service is needed to exercise serialized endpoint convergence.
    async fn new(codex: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let store = HelmStore::open(&root.path().join("helm.db")).await.unwrap();
        Self::with_store(root, store, codex).await
    }

    /// Actor cases share the manager's store rather than planting a parallel
    /// database that the connection hook could never actually consult.
    async fn with_store(root: tempfile::TempDir, store: HelmStore, codex: bool) -> Self {
        let home = root.path().join("home");
        tokio::fs::create_dir(&home).await.unwrap();
        store
            .create_credential(CreateCredential {
                name: "work".into(),
                kind: CredentialKind::EndpointToken,
                secret: FIRST.into(),
            })
            .await
            .unwrap();
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.claude = Some(endpoint("https://first.example/v1", "work"));
        recipe.codex = codex.then(|| endpoint("https://codex.example/v1", "work"));
        store
            .create_tensorlake_recipe("recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
        let id = crate::tensorlake::hosts::test_support::plant(
            &store,
            "fixture host",
            "sandbox",
            "recipe",
            Some("sandbox-id"),
        )
        .await;
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == id)
            .unwrap();
        let backend = FakeBackend::absent(home.clone());
        backend.stateful.store(true, Ordering::SeqCst);
        let delivery = CredentialDelivery::new(store.clone(), backend.clone());
        Self {
            _root: root,
            home,
            store,
            row,
            backend,
            delivery,
            events: FleetEvents::new(),
        }
    }

    /// A connection lease is a fixture premise, not a consequence of connect:
    /// the actor owns it before the hook starts reading selected credentials.
    async fn connected(&self) -> ConnectionLease {
        let lease = self.delivery.enter(&self.row);
        self.delivery.connect(&self.row, &self.events).await;
        assert_eq!(self.delivery.warning(self.row.id), None);
        assert!(self.claude().await.contains(FIRST));
        lease
    }

    /// Commit before push, matching the real replacement handler's contract.
    async fn replace(&self, name: &str, value: &str) {
        self.store
            .save_credential(
                name.into(),
                SaveCredential {
                    secret: Some(value.into()),
                },
            )
            .await
            .unwrap();
    }

    /// Inspect only the fixture-owned destination; no operator configuration
    /// is involved in the byte-level delivery assertions.
    async fn claude(&self) -> String {
        tokio::fs::read_to_string(self.home.join(".claude/settings.json"))
            .await
            .unwrap()
    }

    /// Every transport operation, including reads, participates in this oracle.
    fn operations(&self) -> Vec<String> {
        self.backend.operations.lock().unwrap().clone()
    }

    /// Read expected catalog identity independently of the delivery record.
    /// Name/revision alone cannot distinguish a recreated credential.
    async fn stamp(&self, name: &str, revision: u64) -> (String, u64, String) {
        let name = name.to_owned();
        self.store
            .conn()
            .call("fixture catalog identity", move |conn| {
                let incarnation = conn.query_row(
                    "SELECT incarnation FROM credentials WHERE name = ?1",
                    [&name],
                    |row| row.get(0),
                )?;
                Ok((name, revision, incarnation))
            })
            .await
            .unwrap()
    }
}

/// Pending recipe edits do not update host stamps. Reusing an unreferenced
/// name at revision1 and restoring its endpoint URLs must still deliver the
/// newly created credential to both agents on their next connection.
#[farhelm_testtrace::test]
async fn recreated_endpoint_names_do_not_match_a_previous_delivery() {
    let f = Fixture::new(true).await;
    let _lease = f.connected().await;
    let first = f.stamp("work", 1).await;
    let empty = crate::tensorlake::recipes::tests_support::save();
    f.store
        .update_tensorlake_recipe("recipe".into(), empty)
        .await
        .unwrap()
        .unwrap();
    f.store.delete_credential("work".into()).await.unwrap();
    let saved = f
        .store
        .create_credential(CreateCredential {
            name: "work".into(),
            kind: CredentialKind::EndpointToken,
            secret: SECOND.into(),
        })
        .await
        .unwrap();
    assert_eq!(saved.revision, 1);
    let next = f.stamp("work", 1).await;
    assert_ne!(first.2, next.2);
    let mut recipe = crate::tensorlake::recipes::tests_support::save();
    recipe.claude = Some(endpoint("https://first.example/v1", "work"));
    recipe.codex = Some(endpoint("https://codex.example/v1", "work"));
    f.store
        .update_tensorlake_recipe("recipe".into(), recipe)
        .await
        .unwrap()
        .unwrap();
    assert!(
        f.claude().await.contains(FIRST),
        "pending recipe edits have not reconnected"
    );
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert!(f.claude().await.contains(SECOND));
    assert!(
        std::fs::read_to_string(f.home.join(".codex/config.toml"))
            .unwrap()
            .contains(SECOND)
    );
    let record = f.delivery.read_record(&f.row).await.unwrap();
    for stamp in [record.claude, record.codex] {
        let Some(EndpointStamp::Endpoint { credential, .. }) = stamp else {
            panic!("endpoint stamp missing")
        };
        assert_eq!(credential, Some(next.clone()));
    }
    let before = f.operations();
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(f.operations(), before);
}

/// Distinct URLs and names let the edit-timing test distinguish a binding
/// snapshot from accidentally reloading the newest recipe during a push.
fn endpoint(url: &str, name: &str) -> EndpointSave {
    EndpointSave {
        base_url: url.into(),
        token_credential: Some(name.into()),
    }
}

/// Poll once to establish that the operation actually reached its held wait.
/// Its eventual await supplies a real waker; this observation schedules no work.
fn assert_pending(future: Pin<&mut impl Future>) {
    assert!(
        future
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
}

/// Restarting the delivery object models a helm restart: unchanged persisted
/// stamps must avoid even a home probe while preserving unrelated host settings.
#[farhelm_testtrace::test]
async fn unchanged_reconnect_uses_durable_stamps_without_remote_work() {
    let f = Fixture::new(false).await;
    let lease = f.connected().await;
    let path = f.home.join(".claude/settings.json");
    let mut settings: serde_json::Value = serde_json::from_str(&f.claude().await).unwrap();
    settings["unrelated"] = serde_json::json!(true);
    tokio::fs::write(&path, serde_json::to_vec(&settings).unwrap())
        .await
        .unwrap();
    drop(lease);
    let before = f.operations();
    let restarted = CredentialDelivery::new(f.store.clone(), f.backend.clone());
    let _lease = restarted.enter(&f.row);
    restarted.connect(&f.row, &f.events).await;
    assert_eq!(f.operations(), before);
    assert_eq!(restarted.warning(f.row.id), None);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&f.claude().await).unwrap(),
        settings
    );
    let record = restarted.read_record(&f.row).await.unwrap();
    assert_eq!(record.home, Some(f.home.clone()));
    assert_eq!(record.codex, Some(EndpointStamp::Absent));
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

use std::os::unix::fs::PermissionsExt;

/// Credential replacement uses the active connection's URL/name selection;
/// editing a recipe cannot silently activate its pending changes on that host.
#[farhelm_testtrace::test]
async fn replacement_defers_recipe_edits_until_the_next_connection() {
    let f = Fixture::new(false).await;
    let lease = f.connected().await;
    f.store
        .create_credential(CreateCredential {
            name: "next".into(),
            kind: CredentialKind::EndpointToken,
            secret: "fictional-next-token".into(),
        })
        .await
        .unwrap();
    let mut changed = crate::tensorlake::recipes::tests_support::save();
    changed.claude = Some(endpoint("https://next.example/v1", "next"));
    f.store
        .update_tensorlake_recipe("recipe".into(), changed)
        .await
        .unwrap()
        .unwrap();
    f.replace("work", SECOND).await;
    assert!(f.delivery.push("work", &f.events).await.is_empty());
    let bytes = f.claude().await;
    assert!(bytes.contains(SECOND) && bytes.contains("https://first.example/v1"));
    assert!(!bytes.contains("next.example"));
    let before = f.operations();
    f.replace("next", "fictional-newer-next-token").await;
    assert!(f.delivery.push("next", &f.events).await.is_empty());
    assert_eq!(f.operations(), before);
    drop(lease);
    let _lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    let bytes = f.claude().await;
    assert!(
        bytes.contains("https://next.example/v1") && bytes.contains("fictional-newer-next-token")
    );
    assert!(!bytes.contains(SECOND));
}

/// A saved replacement cannot wait indefinitely behind another delivery. The
/// deadline includes the mutex wait, and the committed value survives failure.
#[farhelm_testtrace::test]
async fn replacement_deadline_includes_waiting_for_the_host_lock() {
    let f = Fixture::new(false).await;
    let _lease = f.connected().await;
    f.replace("work", SECOND).await;
    let before = f.operations();
    let entry = f.delivery.entry(&f.row);
    let _held = entry.serial.clone().lock_owned().await;
    tokio::time::pause();
    let mut push = Box::pin(f.delivery.push("work", &f.events));
    assert_pending(push.as_mut());
    tokio::time::advance(COMMAND_TIMEOUT + Duration::from_millis(1)).await;
    let failures = push.await;
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].host, "fixture host");
    assert_eq!(failures[0].reason, TIMED_OUT);
    assert_eq!(f.operations(), before);
    assert!(f.claude().await.contains(FIRST));
    assert_eq!(f.store.credentials().await.unwrap()[0].revision, 2);
}

/// A hook that has not selected credentials owns its own failure report. An
/// early replacement must not supersede that warning and then silently discard
/// its outcome because no active recipe binding ever became available.
#[farhelm_testtrace::test]
async fn an_early_replacement_does_not_hide_an_unready_connections_timeout() {
    let f = Fixture::new(false).await;
    let _lease = f.delivery.enter(&f.row);
    let entry = f.delivery.entry(&f.row);
    let _held = entry.serial.clone().lock_owned().await;
    let mut connect = Box::pin(f.delivery.connect(&f.row, &f.events));
    assert_pending(connect.as_mut());
    assert!(entry.live.lock().unwrap().binding.is_none());
    f.replace("work", SECOND).await;
    tokio::time::pause();
    let mut push = Box::pin(f.delivery.push("work", &f.events));
    let first_poll = push.as_mut().poll(&mut Context::from_waker(Waker::noop()));
    tokio::time::advance(COMMAND_TIMEOUT + Duration::from_millis(1)).await;
    connect.await;
    let failures = match first_poll {
        std::task::Poll::Ready(failures) => failures,
        std::task::Poll::Pending => push.await,
    };
    assert!(failures.is_empty());
    assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(TIMED_OUT));
    assert!(f.operations().is_empty());
    assert_eq!(f.store.credentials().await.unwrap()[0].revision, 2);
}

/// Suspension cancels a replacement already inside transport work. Opening the
/// gate afterwards must not resume it and overwrite the host's previous file.
#[farhelm_testtrace::test]
async fn last_connection_lease_cancels_an_in_progress_push() {
    let f = Fixture::new(false).await;
    let lease = f.connected().await;
    f.replace("work", SECOND).await;
    f.backend.block_first.store(true, Ordering::SeqCst);
    let mut push = Box::pin(f.delivery.push("work", &f.events));
    tokio::select! {
        _ = f.backend.entered.notified() => {},
        result = &mut push => panic!("push finished before its transport gate: {} failures", result.len()),
    }
    let stopped_at = f.operations();
    drop(lease);
    assert!(push.await.is_empty());
    f.backend.release.notify_one();
    assert_eq!(f.operations(), stopped_at);
    assert!(f.claude().await.contains(FIRST));
    assert_eq!(
        f.delivery.read_record(&f.row).await.unwrap().claude,
        Some(EndpointStamp::Endpoint {
            base_url: "https://first.example/v1".into(),
            credential: Some(f.stamp("work", 1).await),
        })
    );
}

/// An older connection ending must not cancel a still-live newer incarnation;
/// cancellation belongs to the last resource bundle, not whichever drops first.
#[farhelm_testtrace::test]
async fn an_old_lease_cannot_cancel_a_second_live_connection() {
    let f = Fixture::new(false).await;
    let old = f.connected().await;
    let _new = f.delivery.enter(&f.row);
    drop(old);
    f.replace("work", SECOND).await;
    assert!(f.delivery.push("work", &f.events).await.is_empty());
    assert!(f.claude().await.contains(SECOND));
}

/// Claude's completed file is recorded before Codex is attempted. A malformed
/// later document cannot cause a retry to reread or replace the successful one.
#[farhelm_testtrace::test]
async fn a_later_slot_failure_preserves_the_completed_slot_stamp() {
    let f = Fixture::new(true).await;
    tokio::fs::create_dir(f.home.join(".codex")).await.unwrap();
    let codex = f.home.join(".codex/config.toml");
    tokio::fs::write(&codex, b"[invalid fictional-private-marker")
        .await
        .unwrap();
    let _lease = f.delivery.enter(&f.row);
    f.delivery.connect(&f.row, &f.events).await;
    assert!(f.claude().await.contains(FIRST));
    let record = f.delivery.read_record(&f.row).await.unwrap();
    assert!(record.claude.is_some() && record.codex.is_none());
    let warning = f.delivery.warning(f.row.id).unwrap();
    assert!(!warning.contains("fictional-private-marker"));
    assert_eq!(warning, "~/.codex/config.toml is not valid TOML");
    let before = f.operations().len();
    tokio::fs::write(&codex, b"unrelated = true\n")
        .await
        .unwrap();
    f.delivery.connect(&f.row, &f.events).await;
    assert_eq!(
        &f.operations()[before..],
        ["read-agent-settings", "create-directories", "write-unit"]
    );
    assert_eq!(f.delivery.warning(f.row.id), None);
    assert!(
        tokio::fs::read_to_string(codex)
            .await
            .unwrap()
            .contains("unrelated = true")
    );
    assert!(
        f.delivery
            .read_record(&f.row)
            .await
            .unwrap()
            .codex
            .is_some()
    );
}

/// Each backend failure is mapped at the delivery boundary; transport context
/// and stderr must not reach the warning or saved-with-failure acknowledgement.
#[farhelm_testtrace::test]
async fn backend_failures_leave_saved_values_and_safe_warnings() {
    for (operation, reason) in [
        ("read-home", "could not read the sandbox's home directory"),
        ("read-agent-settings", "could not read the agent settings"),
        (
            "create-directories",
            "could not prepare the agent settings directory",
        ),
        ("write-unit", "could not install the agent settings"),
    ] {
        let f = Fixture::new(false).await;
        let _lease = f.delivery.enter(&f.row);
        *f.backend.fail.lock().unwrap() = Some(operation.into());
        f.delivery.connect(&f.row, &f.events).await;
        assert_eq!(f.delivery.warning(f.row.id).as_deref(), Some(reason));
        f.replace("work", SECOND).await;
        let failures = f.delivery.push("work", &f.events).await;
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].reason, reason);
        assert!(
            !serde_json::to_string(&failures)
                .unwrap()
                .contains("host said no")
        );
        assert_eq!(f.store.credentials().await.unwrap()[0].revision, 2);
        assert!(!f.home.join(".claude/settings.json").exists());
        *f.backend.fail.lock().unwrap() = None;
        assert!(f.delivery.push("work", &f.events).await.is_empty());
        assert_eq!(f.delivery.warning(f.row.id), None);
        assert!(f.claude().await.contains(SECOND));
    }
}

/// Cancelling an await cannot release the host lock while its blocking SQLite
/// write is queued. A later delivery must observe that write before proceeding.
#[farhelm_testtrace::test]
async fn a_cancelled_record_write_keeps_serialization_until_sqlite_finishes() {
    let f = Fixture::new(false).await;
    let store = f.store.clone();
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, held) = std::sync::mpsc::channel();
    // Dropping release also wakes recv if an assertion unwinds this fixture;
    // runtime teardown never depends on a forgotten blocking-thread gate.
    let blocker = tokio::spawn(async move {
        store
            .conn()
            .call("hold the fixture database", move |_| {
                let _ = entered.send(());
                let _ = held.recv();
                Ok(())
            })
            .await
            .unwrap();
    });
    ready.await.unwrap();
    let entry = f.delivery.entry(&f.row);
    let guard = entry.serial.clone().lock_owned().await;
    let wanted = Record {
        home: Some(f.home.clone()),
        ..Record::default()
    };
    let mut write = Box::pin(f.delivery.write_record(&f.row, &wanted, guard));
    assert_pending(write.as_mut());
    drop(write);
    assert!(entry.serial.clone().try_lock_owned().is_err());
    let before = f.operations();
    let mut next = Box::pin(f.delivery.connect(&f.row, &f.events));
    assert_pending(next.as_mut());
    assert_eq!(f.operations(), before);
    release.send(()).unwrap();
    blocker.await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), next)
        .await
        .unwrap();
    let record = f.delivery.read_record(&f.row).await.unwrap();
    assert_eq!(record.home, Some(f.home.clone()));
    assert!(record.claude.is_some());
    assert!(
        !f.operations()
            .iter()
            .any(|operation| operation == "read-home")
    );
    assert!(f.claude().await.contains(FIRST));
}

use crate::manager::{ConnectedResources, HostState, SandboxCheck, SandboxStatusSource};
use crate::rest_harness::{FleetBuilder, Harness};
use crate::tensorlake::Tensorlake;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use farhelm_proto::credentials::SavedCredential;
use std::sync::atomic::AtomicBool;
use tower::ServiceExt as _;

/// Hold the real hook's resources after acquisition but before the actor can
/// publish them. Late fixture assembly happens before any sandbox row exists.
#[derive(Default)]
struct HookGate {
    service: Mutex<Option<Tensorlake>>,
    hold: AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

#[async_trait::async_trait]
impl SandboxStatusSource for HookGate {
    async fn check(&self, _sandbox: &str) -> SandboxCheck {
        SandboxCheck::Unavailable("fixture asks the supervisor directly".into())
    }

    async fn connected(&self, row: &HostRow, events: Arc<FleetEvents>) -> ConnectedResources {
        if row.tensorlake.is_none() {
            return ConnectedResources::default();
        }
        let service = self.service.lock().unwrap().clone().unwrap();
        let resources = SandboxStatusSource::connected(&service, row, events).await;
        if self.hold.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        resources
    }
}

/// Use the ordinary protected router and real actor with the same injected
/// delivery backend. The planted sandbox is scripted before registry sync.
async fn actor_fixture(hold_hook: bool, hold_backend: bool) -> (Harness, Fixture, Arc<HookGate>) {
    let gate = Arc::new(HookGate::default());
    gate.hold.store(hold_hook, Ordering::SeqCst);
    let mut harness = FleetBuilder::new()
        .await
        .sandboxes(gate.clone())
        .start()
        .await;
    let root = tempfile::tempdir().unwrap();
    let mut f = Fixture::with_store(root, harness.store.clone(), false).await;
    f.backend.block_first.store(hold_backend, Ordering::SeqCst);
    let service = crate::provisioning::tests::service(&harness, f.backend.clone(), f._root.path());
    f.delivery = service.credential_delivery();
    harness.state = Arc::new(crate::AppState::with_provisioning(
        harness.manager.clone(),
        harness.store.clone(),
        service,
        crate::ServingMode::Standalone,
    ));
    *gate.service.lock().unwrap() = Some(harness.state.tensorlake.clone());
    harness
        .store
        .admit_tensorlake_install(f.row.id, "/fixture/farhelm", "/fixture/state")
        .await
        .unwrap();
    harness.fleet.edit(f.row.id, |_| {});
    harness.manager.sync_registry().await.unwrap();
    (harness, f, gate)
}

/// Exercise authentication, response decoding and host projection together;
/// no fixture-only route bypasses the save handler or connection actor.
fn request(method: &str, path: &str, body: String) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:7433")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

/// Replacement responses may carry metadata and fixed failure reasons only.
async fn replace_route(harness: &Harness, token: &str) -> SavedCredential {
    let response = harness
        .router()
        .oneshot(request(
            "PUT",
            "/api/credentials/work",
            serde_json::to_string(&SaveCredential {
                secret: Some(token.into()),
            })
            .unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(!text.contains(token) && !text.contains(FIRST));
    serde_json::from_slice(&bytes).unwrap()
}

/// A save includes a connection whose real hook has read revision one but has
/// not published Connected yet. Failed subsequent pushes remain successful
/// saves, with a routable host and a matching sanitized host warning.
#[farhelm_testtrace::test]
async fn route_replacement_covers_the_hook_publication_gap_and_reports_failures() {
    let (harness, f, gate) = actor_fixture(true, false).await;
    tokio::time::timeout(Duration::from_secs(5), gate.entered.notified())
        .await
        .unwrap();
    assert!(f.claude().await.contains(FIRST));
    assert!(!matches!(
        harness.manager.state(f.row.id),
        Some(HostState::Connected { .. })
    ));
    assert!(harness.manager.status(f.row.id).unwrap().client.is_none());
    let ack = replace_route(&harness, SECOND).await;
    assert_eq!(ack.credential.revision, 2);
    assert!(ack.delivery_failures.is_empty());
    assert!(f.claude().await.contains(SECOND));
    assert!(harness.manager.status(f.row.id).unwrap().client.is_none());
    gate.release.notify_one();
    harness
        .await_state(f.row.id, |state| {
            matches!(state, HostState::Connected { .. })
        })
        .await;
    assert!(harness.manager.status(f.row.id).unwrap().client.is_some());
    *f.backend.fail.lock().unwrap() = Some("read-agent-settings".into());
    let ack = replace_route(&harness, "fictional-third-token").await;
    assert_eq!(ack.credential.revision, 3);
    assert_eq!(ack.delivery_failures.len(), 1);
    assert_eq!(
        ack.delivery_failures[0].reason,
        "could not read the agent settings"
    );
    assert_eq!(f.store.credentials().await.unwrap()[0].revision, 3);
    assert!(harness.manager.status(f.row.id).unwrap().client.is_some());
    let response = harness
        .router()
        .oneshot(request("GET", "/api/hosts", String::new()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let hosts: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let host = hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|host| host["id"].as_i64() == Some(f.row.id))
        .unwrap();
    assert_eq!(
        host["tensorlake"]["credential_warning"],
        "could not read the agent settings"
    );
    assert!(
        !std::str::from_utf8(&bytes)
            .unwrap()
            .contains("host said no")
    );
    *f.backend.fail.lock().unwrap() = None;
    assert!(
        replace_route(&harness, "fictional-fourth-token")
            .await
            .delivery_failures
            .is_empty()
    );
    assert_eq!(f.delivery.warning(f.row.id), None);

    // An accepted replacement owns its push even if the HTTP caller vanishes.
    f.backend.block_first.store(true, Ordering::SeqCst);
    let mut saving = Box::pin(
        harness.router().oneshot(request(
            "PUT",
            "/api/credentials/work",
            serde_json::to_string(&SaveCredential {
                secret: Some("fictional-disconnected-caller-token".into()),
            })
            .unwrap(),
        )),
    );
    tokio::select! {
        _ = f.backend.entered.notified() => {},
        result = &mut saving => panic!("save returned before its held push: {:?}", result.map(|response| response.status())),
    }
    drop(saving);
    f.backend.release.notify_one();
    let entry = f.delivery.entry(&f.row);
    let _settled = tokio::time::timeout(Duration::from_secs(5), entry.serial.clone().lock_owned())
        .await
        .unwrap();
    assert!(
        f.claude()
            .await
            .contains("fictional-disconnected-caller-token")
    );
    assert_eq!(f.store.credentials().await.unwrap()[0].revision, 5);
}

/// Parking an actual connection actor cancels its acquisition before replying.
/// Releasing the old backend gate cannot restart file delivery after the hold.
#[farhelm_testtrace::test]
async fn actor_hold_cancels_the_real_hook_before_acknowledging_parking() {
    let (harness, f, _gate) = actor_fixture(false, true).await;
    tokio::time::timeout(Duration::from_secs(5), f.backend.entered.notified())
        .await
        .unwrap();
    assert_eq!(*f.delivery.entry(&f.row).active.borrow(), 1);
    assert!(harness.manager.status(f.row.id).unwrap().client.is_none());
    let shown = harness.manager.state(f.row.id).unwrap();
    assert!(harness.manager.hold_connection(f.row.id, shown).await);
    assert_eq!(*f.delivery.entry(&f.row).active.borrow(), 0);
    let before = f.operations();
    f.backend.release.notify_one();
    assert_eq!(f.operations(), before);
    assert!(!f.home.join(".claude/settings.json").exists());
    harness.manager.release_hold(f.row.id, true);
    harness
        .await_state(f.row.id, |state| {
            matches!(state, HostState::Connected { .. })
        })
        .await;
    assert!(f.claude().await.contains(FIRST));
    assert_eq!(*f.delivery.entry(&f.row).active.borrow(), 1);
    let before = f.operations();
    harness.fleet.kill_connection(f.row.id);
    harness
        .await_state(f.row.id, |state| {
            !matches!(state, HostState::Connected { .. })
        })
        .await;
    harness
        .await_state(f.row.id, |state| {
            matches!(state, HostState::Connected { .. })
        })
        .await;
    assert_eq!(f.operations(), before);
}
