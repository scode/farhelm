//! Exercise the actual add orchestrator with filesystem-controlled API and
//! backend fakes. No shell or live sandbox is used; ownership and step state
//! remain the production implementations.

use super::*;
use crate::rest_harness::{FleetBuilder, Harness};
use crate::tensorlake::e2e::{E2eTensorlake, FakeModel};
use crate::tensorlake::prepared::{PreparedInstallFacts, PreparedSnapshot};
use farhelm_proto::tensorlake::{
    SaveSettings,
    recipes::{RecipeBase, RecipeSave},
};

/// Own all fake paths alongside the helm fixture. Supplying these explicitly
/// keeps concurrent tests isolated without changing process environment.
struct Fixture {
    harness: Harness,
    root: tempfile::TempDir,
    service: Arc<ProvisioningService>,
    tensorlake: Tensorlake,
}

/// Assemble a real provisioning service over the repository's two controlled
/// fakes. Ubuntu fixtures build on add; image fixtures start with a recorded
/// built snapshot so later setup tests still exercise preparation without an
/// unrelated package/agent build. Preparation uses the service's real installer
/// over the same explicit backend and payload fakes as ordinary host setup.
async fn fixture(build_on_add: bool) -> Fixture {
    let harness = FleetBuilder::new().await.start().await;
    let root = tempfile::tempdir().unwrap();
    let api_dir = root.path().join("api");
    let backend_dir = root.path().join("backend");
    for (path, marker) in [
        (&api_dir, "farhelm-e2e-tensorlake-v1\n"),
        (&backend_dir, "farhelm-e2e-provisioning-v1\n"),
    ] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("ENABLED"), marker).unwrap();
        std::fs::write(path.join("config.json"), "{}").unwrap();
    }
    let api = Arc::new(E2eTensorlake::new(api_dir, root.path()).unwrap());
    let mut model = FakeModel::default();
    let existing_snapshot = (!build_on_add).then(|| model.plant_snapshot("earlierbuilder"));
    std::fs::write(
        root.path().join("api/state.json"),
        serde_json::to_vec(&model).unwrap(),
    )
    .unwrap();
    let tensorlake = Tensorlake::production(harness.store.clone(), root.path())
        .unwrap()
        .with_api(api.clone())
        .with_shell(api);
    harness
        .store
        .save_tensorlake_config(SaveSettings {
            api_key: Some("fixture-key".into()),
            ..Default::default()
        })
        .await
        .ok()
        .unwrap();
    let mut recipe: RecipeSave = crate::tensorlake::recipes::tests_support::save();
    if !build_on_add {
        recipe.base = RecipeBase::Image {
            image: "example/systemd".into(),
        };
    }
    assert!(
        harness
            .store
            .create_tensorlake_recipe("test-recipe".into(), recipe)
            .await
            .unwrap()
            .is_ok()
    );
    if let Some(id) = existing_snapshot {
        harness
            .store
            .update_tensorlake_recipe_state("test-recipe", move |spec, state| {
                state.snapshot = Some(crate::tensorlake::build::BuiltSnapshot {
                    id,
                    base: spec.base.clone(),
                    disk_gb: spec.size.disk_gb,
                    packages: spec.packages.clone(),
                    ..Default::default()
                });
            })
            .await
            .unwrap()
            .unwrap();
    }
    let service = ProvisioningService::injected(
        harness.store.clone(),
        Arc::clone(&harness.manager),
        Arc::new(E2eProvisioningBackend::new(backend_dir.clone(), root.path()).unwrap()),
        Arc::new(E2ePayloads(backend_dir.join("ENABLED"))),
        PlanLayout::production(root.path().to_path_buf()),
        "/fixture/farhelm".into(),
    );
    let tensorlake = tensorlake.with_preparer(service.snapshot_preparer());
    Fixture {
        harness,
        root,
        service,
        tensorlake,
    }
}

impl Fixture {
    /// Put the same injected backend and account behind the real user router.
    /// The ordinary Add tests keep their service-only composition; recipe launch
    /// additionally needs the resulting session and cleanup to use this account.
    async fn bind_session_launch(&mut self) {
        self.tensorlake.ensure_host_key().await.unwrap();
        let mut state = crate::AppState::with_provisioning(
            Arc::clone(&self.harness.manager),
            self.harness.store.clone(),
            Arc::clone(&self.service),
            self.harness.state.mode,
        );
        state.tensorlake = self.tensorlake.clone();
        self.harness.state = Arc::new(state);
    }

    /// Exercise the new protected endpoint, with ordinary command fields and
    /// no installed-host selector. A bounded wait for headers is independent
    /// from reading the intentionally unfinished result body.
    async fn session_launch(&self, confirmed: bool) -> axum::response::Response {
        let body = serde_json::json!({
            "recipe": "test-recipe", "confirmed": confirmed,
            "request": { "cwd": "~", "command": { "command": "true", "yolo": false } },
        });
        self.session_launch_request(body).await
    }

    /// Keep malformed launch cases on the real route as well: a service-only
    /// rejection would not establish that HTTP admission precedes billing.
    async fn session_launch_request(&self, body: serde_json::Value) -> axum::response::Response {
        use tower::ServiceExt as _;
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/api/sandbox-sessions")
            .header("host", "127.0.0.1:7433")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap();
        tokio::time::timeout(
            Duration::from_secs(15),
            self.harness.router().oneshot(request),
        )
        .await
        .expect("admission headers arrive before the held setup finishes")
        .unwrap()
    }

    /// A probe has no network side effects; its opaque id is the sole
    /// authority the subsequent POST consumes.
    async fn probe(&self, target: ProbeDestination) -> ProbeResponse {
        self.service
            .probe_tensorlake(
                ProbeRequest {
                    target,
                    remote_farhelm: None,
                    remote_state_dir: None,
                    new_entry: None,
                },
                self.tensorlake.clone(),
            )
            .await
            .unwrap()
    }

    /// Start the ordinary initial Add through its one-use offer, so tests retain
    /// registration and confirmation semantics alongside the controlled backend.
    async fn add(&self) -> RunAccepted {
        let ProbeResponse::TensorlakeProvisionable { probe_id, .. } = self
            .probe(ProbeDestination::Tensorlake {
                name: "Test host".into(),
                recipe: "test-recipe".into(),
            })
            .await
        else {
            panic!("expected a creation plan")
        };
        self.service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap()
    }

    /// The run's final state, never the idle view before its admission.
    async fn finish(&self, host: HostId) -> ProvisioningView {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let view = self.service.view(host).await.unwrap();
                if view.run_id.is_some() && view.status != RunStatus::Running {
                    return view;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("add run finishes")
    }

    /// Read what the API fake durably owns, rather than inferring billed
    /// resources from a successful setup reply alone.
    fn model(&self) -> FakeModel {
        serde_json::from_slice(&std::fs::read(self.root.path().join("api/state.json")).unwrap())
            .unwrap()
    }

    /// Record a legacy host directly, before the prepared Add path existed.
    /// Its sandbox is already billed but its install coordinates are absent,
    /// which preserves the ordinary copied-state and rerun admission fence.
    async fn legacy_host(&self) -> HostId {
        let sandbox = "farhelm-host-test-legacy";
        let recipe = self
            .harness
            .store
            .tensorlake_recipe_row("test-recipe")
            .await
            .unwrap()
            .unwrap()
            .0;
        let host = self
            .harness
            .store
            .register_tensorlake_host(NewTensorlakeHost {
                destination: crate::tensorlake::ssh::destination(sandbox),
                entry: NewHostEntry::new(Some("Test host")).unwrap(),
                sandbox: sandbox.into(),
                recipe: "test-recipe".into(),
                resources: recipe.size,
            })
            .await
            .unwrap();
        let mut model = self.model();
        let created = model
            .create(&CreateSandbox {
                name: sandbox.into(),
                source: SandboxSource::Image("legacy/systemd".into()),
                resources: Resources {
                    cpus: 2.0,
                    memory_mb: 4096,
                    disk_mb: 20480,
                },
                timeout_secs: 0,
                auto_resume: false,
                max_pending_secs: 900,
            })
            .unwrap();
        std::fs::write(
            self.root.path().join("api/state.json"),
            serde_json::to_vec(&model).unwrap(),
        )
        .unwrap();
        assert!(
            self.harness
                .store
                .set_tensorlake_sandbox_id(host, &created.sandbox_id)
                .await
                .unwrap()
        );
        assert!(
            self.harness
                .store
                .tensorlake_prepared_install(host)
                .await
                .unwrap()
                .is_none()
        );
        self.harness.manager.sync_registry().await.unwrap();
        host
    }

    /// A controlled existing-host run must use the ordinary installer even
    /// when its recipe later acquires a reusable prepared snapshot.
    async fn rerun(&self, host: HostId) -> RunAccepted {
        let ProbeResponse::TensorlakeProvisionable { probe_id, .. } =
            self.probe(ProbeDestination::TensorlakeHost { host }).await
        else {
            panic!("expected rerun plan")
        };
        self.service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap()
    }

    /// Plant a complete cache only when a test needs its failure injection to
    /// reach host creation rather than the preceding preparer creation.
    async fn seed_prepared(&self) -> PreparedInstallFacts {
        let facts = PreparedInstallFacts {
            home: "/home/cached-user".into(),
            farhelm_path: "/home/cached-user/.local/lib/farhelm/farhelm".into(),
            state_dir: "/home/cached-user/.local/state/farhelm".into(),
            farhelm_version: farhelm_proto::BUILD_VERSION.into(),
        };
        let mut model = self.model();
        let id = model.plant_snapshot("earlierpreparer");
        std::fs::write(
            self.root.path().join("api/state.json"),
            serde_json::to_vec(&model).unwrap(),
        )
        .unwrap();
        let retained = facts.clone();
        self.harness
            .store
            .update_tensorlake_recipe_state("test-recipe", move |_, state| {
                let built = state
                    .snapshot
                    .as_ref()
                    .expect("cache requires a built source");
                state.prepared = Some(PreparedSnapshot {
                    id,
                    prepared_at: crate::tensorlake::recipes::now_secs(),
                    farhelm_version: retained.farhelm_version.clone(),
                    source_snapshot: built.id.clone(),
                    disk_gb: built.disk_gb,
                    facts: retained,
                });
            })
            .await
            .unwrap()
            .unwrap();
        facts
    }

    /// Targeted JSON events distinguish preparer installation from credential
    /// delivery on a host; substring order across both targets cannot do that.
    fn backend_events(&self) -> Vec<serde_json::Value> {
        std::fs::read_to_string(self.root.path().join("backend/events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    /// Publish gate changes atomically while the backend is rereading them.
    /// A partially written JSON file would fail the run for the wrong reason.
    fn backend_config(&self, config: serde_json::Value) {
        let next = self.root.path().join("backend/config.next");
        std::fs::write(&next, serde_json::to_vec(&config).unwrap()).unwrap();
        std::fs::rename(next, self.root.path().join("backend/config.json")).unwrap();
    }

    /// Observe the deliberately held preparation boundary, rather than merely
    /// the presence of a planned step before execution reaches it.
    async fn wait_for_backend(&self, event: &str) {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if self
                    .backend_events()
                    .iter()
                    .any(|entry| entry["event"] == event)
                {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("backend never reached {event}: {:?}", self.backend_events()));
    }

    /// Reassemble from the same durable database and fake files with no retained
    /// service memory. Restart recovery must use the host's saved create facts.
    async fn restart(self) -> Self {
        let Self {
            harness,
            root,
            service,
            tensorlake,
        } = self;
        drop(service);
        drop(tensorlake);
        let harness = harness.restart_with(|_| {}).await;
        let api = Arc::new(E2eTensorlake::new(root.path().join("api"), root.path()).unwrap());
        let backend_dir = root.path().join("backend");
        let service = ProvisioningService::injected(
            harness.store.clone(),
            Arc::clone(&harness.manager),
            Arc::new(E2eProvisioningBackend::new(backend_dir.clone(), root.path()).unwrap()),
            Arc::new(E2ePayloads(backend_dir.join("ENABLED"))),
            PlanLayout::production(root.path().to_path_buf()),
            "/fixture/farhelm".into(),
        );
        let tensorlake = Tensorlake::production(harness.store.clone(), root.path())
            .unwrap()
            .with_api(api.clone())
            .with_shell(api)
            .with_preparer(service.snapshot_preparer());
        Self {
            harness,
            root,
            service,
            tensorlake,
        }
    }

    /// Every credential operation belongs to this host, while initial install
    /// operations belong to its preparer. This is a workflow assertion, not a
    /// claim about files inherited from a user-supplied base.
    async fn assert_host_delivery(&self, host: HostId, required: &str) {
        let row = self
            .harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        let target = format!("ssh:{}", row.destination.unwrap());
        let events = self.backend_events();
        let delivery = events
            .iter()
            .filter(|event| {
                matches!(
                    event["event"].as_str(),
                    Some(
                        "read-agent-settings"
                            | "wrote-claude-settings"
                            | "remove-github-token"
                            | "configure-github-helper"
                    )
                )
            })
            .collect::<Vec<_>>();
        assert!(
            delivery.iter().any(|event| event["event"] == required),
            "missing {required}: {events:?}"
        );
        assert!(
            delivery.iter().all(|event| event["target"] == target),
            "preparer must receive no credentials: {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|event| event["target"] == target && event["event"] == "enable-supervisor"),
            "the host must boot the prepared supervisor: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|event| event["target"] != target && event["event"] == "enable-supervisor"),
            "premise: the preparer performed installation"
        );
        let attach = events
            .iter()
            .position(|event| event["target"] == target && event["event"] == "attach-supervisor")
            .unwrap();
        let delivered = events
            .iter()
            .rposition(|event| event["target"] == target && event["event"] == required)
            .unwrap();
        assert!(
            delivered < attach,
            "delivery finishes before the connection becomes usable: {events:?}"
        );
    }
}

/// Recipe launch must refuse before billing when its software has not been
/// built. The same fixture's ordinary Add path may build, so this distinguishes
/// the new launch contract from merely reusing Add without a preflight.
#[farhelm_testtrace::test]
async fn recipe_session_preflight_never_builds_a_missing_software_snapshot() {
    let mut f = fixture(true).await;
    f.bind_session_launch().await;
    let invalid = f.session_launch_request(serde_json::json!({
        "recipe": "test-recipe", "confirmed": true,
        "request": { "cwd": "~", "prompt": "not accepted by a command", "command": { "command": "true", "yolo": false } },
    })).await;
    assert_eq!(invalid.status(), axum::http::StatusCode::BAD_REQUEST);
    let reason = axum::body::to_bytes(invalid.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(
        String::from_utf8_lossy(&reason).contains("initial prompt"),
        "{reason:?}"
    );
    assert!(f.model().sandboxes.is_empty());
    assert!(f.backend_events().is_empty());
    for (confirmed, expected) in [(false, "confirm creation"), (true, "no built snapshot")] {
        let response = f.session_launch(confirmed).await;
        assert!(!response.status().is_success());
        assert_ne!(
            response.status(),
            axum::http::StatusCode::UNAUTHORIZED,
            "a user refusal must not sign the browser out"
        );
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&bytes).contains(expected),
            "{bytes:?}"
        );
        assert!(
            f.model().sandboxes.is_empty(),
            "no billed build or session sandbox"
        );
        assert_eq!(f.harness.store.list_hosts().await.unwrap().len(), 1);
        assert!(f.backend_events().is_empty());
    }
}

/// Drive the owned setup task through a live scripted supervisor, rather than
/// calling the session helper directly. Success retains its resource and clears
/// admission; a post-attach refusal uses the same host teardown as explicit Delete.
#[farhelm_testtrace::test]
async fn recipe_session_task_publishes_success_and_cleans_a_connected_create_failure() {
    use farhelm_proto::ControlMsg;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    for fails in [false, true] {
        let mut f = fixture(false).await;
        f.seed_prepared().await;
        f.bind_session_launch().await;
        f.backend_config(serde_json::json!({"default": {"hold_actions": true}}));
        let response = f.session_launch(true).await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let host: HostId = response.headers()["x-farhelm-sandbox-host"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        f.wait_for_backend("wait-tensorlake-ssh").await;
        assert!(
            f.model().live_sandboxes().len() == 1,
            "premise: one fresh resource waits for setup"
        );
        let (client_side, peer) = tokio::io::duplex(64 * 1024);
        let (done, mut finished) = tokio::sync::oneshot::channel();
        let responder = tokio::spawn(async move {
            let (read, write) = tokio::io::split(peer);
            let mut reader = FrameReader::new(read);
            let mut writer = FrameWriter::new(write);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let mut creates = 0;
            loop {
                let frame = tokio::select! {
                    _ = &mut finished => break,
                    frame = reader.read_frame() => match frame.unwrap() { Some(frame) => frame, None => break },
                };
                let ControlMsg::CreateSession { req_id, .. } = parse_control(&frame).unwrap()
                else {
                    panic!("unexpected request after the setup connection attached");
                };
                creates += 1;
                let reply = if fails {
                    ControlMsg::Error {
                        req_id,
                        kind: farhelm_proto::ErrorKind::InvalidRequest,
                        create_unaccepted: true,
                        message: "connected create refused by fixture".into(),
                    }
                } else {
                    ControlMsg::SessionCreated {
                        req_id,
                        session: crate::rest_harness::session("owned-child", 100),
                    }
                };
                writer.write_control(&reply).await.unwrap();
            }
            creates
        });
        f.harness.fleet.edit(host, |script| {
            *script = crate::rest_harness::HostScript {
                identity: Some("fresh-task-install".into()),
                peer: Some(client_side),
                ..crate::rest_harness::HostScript::default()
            }
        });
        f.backend_config(serde_json::json!({}));
        let bytes = tokio::time::timeout(
            Duration::from_secs(15),
            axum::body::to_bytes(response.into_body(), 1 << 20),
        )
        .await
        .expect("owned launch finishes while its supervisor fixture is alive")
        .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(!f.service.memory.lock().await.busy.contains(&host));
        if fails {
            let error = body["error"].as_str().unwrap();
            assert!(
                error.starts_with("Not launched.")
                    && error.contains("connected create refused by fixture"),
                "{error}"
            );
            assert!(f.model().live_sandboxes().is_empty());
            assert!(
                !f.harness
                    .store
                    .list_hosts()
                    .await
                    .unwrap()
                    .iter()
                    .any(|row| row.id == host)
            );
        } else {
            assert!(
                body["session"]["id"]
                    .as_str()
                    .unwrap_or_else(|| panic!("expected a created session, got {body}"))
                    .ends_with(":owned-child")
            );
            assert_eq!(
                f.service.view(host).await.unwrap().status,
                RunStatus::Completed
            );
            assert_eq!(
                f.harness.store.cached_sessions(host).await.unwrap().len(),
                1
            );
            assert_eq!(f.model().live_sandboxes().len(), 1);
        }
        let _ = done.send(());
        assert_eq!(
            responder.await.unwrap(),
            1,
            "one create dispatch per owned task"
        );
    }
}

/// Injected attachment can complete while the real host actor is Connecting.
/// The task must wait for its actual verdict, preserve a settled refusal's
/// routing message, and clean up its fresh resource without a supervisor create.
#[farhelm_testtrace::test]
async fn recipe_session_task_reports_settled_connection_failures_before_cleanup() {
    for (reachable, protocol, phase, detail) in [
        (
            false,
            farhelm_proto::PROTOCOL_VERSION,
            "unreachable-reprobing",
            "scripted host",
        ),
        (
            true,
            farhelm_proto::PROTOCOL_VERSION + 1,
            "version-skew",
            "speaks protocol",
        ),
    ] {
        let mut f = fixture(false).await;
        f.seed_prepared().await;
        f.bind_session_launch().await;
        f.backend_config(serde_json::json!({"default": {"hold_actions": true}}));
        let response = f.session_launch(true).await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let host: HostId = response.headers()["x-farhelm-sandbox-host"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        f.wait_for_backend("wait-tensorlake-ssh").await;
        assert_eq!(f.model().live_sandboxes().len(), 1);
        assert!(
            f.harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == host && row.is_session_owned()),
            "premise: this launch owns the resource still waiting for setup"
        );
        f.harness.fleet.edit(host, |script| {
            *script = crate::rest_harness::HostScript {
                reachable,
                protocol,
                ..crate::rest_harness::HostScript::default()
            };
        });
        f.backend_config(serde_json::json!({}));
        let bytes = tokio::time::timeout(
            Duration::from_secs(15),
            axum::body::to_bytes(response.into_body(), 1 << 20),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "launch never reported {phase}; host state: {:?}, live resources: {}",
                f.harness.manager.state(host),
                f.model().live_sandboxes().len()
            )
        })
        .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let error = body["error"].as_str().unwrap_or_else(|| panic!("{body}"));
        assert!(
            error.starts_with("Not launched.")
                && error.contains(&format!("is {phase},"))
                && error.contains(detail),
            "{error}"
        );
        assert!(!f.service.memory.lock().await.busy.contains(&host));
        assert!(f.model().live_sandboxes().is_empty());
        assert!(
            !f.harness
                .store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == host)
        );
    }
}

/// Admission must publish progress before setup finishes. Closing the body at
/// that boundary loses only the reply: the owned launch still attempts cleanup.
/// A failed cleanup instead keeps the named resource and an explicit remedy.
#[farhelm_testtrace::test]
async fn recipe_session_failure_cleanup_outlives_its_client_and_retains_failed_cleanup() {
    for cleanup_fails in [false, true] {
        let mut f = fixture(false).await;
        f.seed_prepared().await;
        f.bind_session_launch().await;
        f.backend_config(serde_json::json!({"default": {"hold_actions": true, "fail_action": "wait-tensorlake-ssh"}}));
        if cleanup_fails {
            std::fs::write(
                f.root.path().join("api/config.json"),
                serde_json::json!({
                    "fail": {"terminate_sandbox": {"status": 503, "message": "keep this resource"}}
                })
                .to_string(),
            )
            .unwrap();
        }
        let response = f.session_launch(true).await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let host: HostId = response.headers()["x-farhelm-sandbox-host"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        let run = response.headers()["x-farhelm-sandbox-run"]
            .to_str()
            .unwrap();
        assert!(uuid::Uuid::parse_str(run).is_ok());
        f.wait_for_backend("wait-tensorlake-ssh").await;
        let rows = f.harness.store.list_hosts().await.unwrap();
        let row = rows.iter().find(|row| row.id == host).unwrap();
        assert!(row.is_session_owned());
        let sandbox = row.tensorlake.as_ref().unwrap().sandbox.clone();
        assert!(sandbox.starts_with("farhelm-sess-"));
        let model = f.model();
        let created = model
            .sandboxes
            .iter()
            .find(|entry| entry.name.as_deref() == Some(&sandbox))
            .unwrap();
        assert_eq!(
            created.status, "running",
            "premise: cleanup has a real created resource to remove"
        );
        assert_eq!(created.timeout_secs, 1800);
        assert!(!created.auto_resume);
        assert!(
            f.harness
                .store
                .cached_sessions(host)
                .await
                .unwrap()
                .is_empty(),
            "no placeholder session"
        );
        let body = if cleanup_fails {
            Some(response.into_body())
        } else {
            drop(response);
            None
        };
        f.backend_config(serde_json::json!({"default": {"fail_action": "wait-tensorlake-ssh"}}));
        if cleanup_fails {
            let bytes = tokio::time::timeout(
                Duration::from_secs(15),
                axum::body::to_bytes(body.unwrap(), 1 << 20),
            )
            .await
            .unwrap()
            .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let error = body["error"].as_str().unwrap();
            assert!(
                error.contains(&sandbox) && error.contains("Hosts"),
                "{error}"
            );
            assert!(
                f.harness
                    .store
                    .list_hosts()
                    .await
                    .unwrap()
                    .iter()
                    .any(|row| row.id == host)
            );
            assert_eq!(
                f.service.view(host).await.unwrap().status,
                RunStatus::Failed
            );
        } else {
            tokio::time::timeout(Duration::from_secs(15), async {
                while f
                    .harness
                    .store
                    .list_hosts()
                    .await
                    .unwrap()
                    .iter()
                    .any(|row| row.id == host)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("owned cleanup finishes after client departure");
            assert!(f.model().live_sandboxes().is_empty());
        }
    }
}

/// Endpoint delivery shares setup's injected backend. A credential failure
/// remains a warning while connection completes, so an image
/// with unreadable agent settings can still become a usable Farhelm host.
#[farhelm_testtrace::test]
async fn credential_delivery_warning_does_not_abort_tensorlake_setup() {
    use farhelm_proto::credentials::{CreateCredential, CredentialKind};
    let f = fixture(false).await;
    f.harness
        .store
        .create_credential(CreateCredential {
            name: "endpoint".into(),
            kind: CredentialKind::EndpointToken,
            secret: "fictional-setup-token".into(),
        })
        .await
        .unwrap();
    let mut recipe = crate::tensorlake::recipes::tests_support::save();
    recipe.base = RecipeBase::Image {
        image: "example/systemd".into(),
    };
    recipe.claude = Some(farhelm_proto::tensorlake::recipes::EndpointSave {
        base_url: "https://endpoint.example/v1".into(),
        token_credential: Some("endpoint".into()),
    });
    f.harness
        .store
        .update_tensorlake_recipe("test-recipe".into(), recipe)
        .await
        .unwrap()
        .unwrap();
    std::fs::write(f.root.path().join("backend/config.json"),
        r#"{"default":{"fail_action":"read-agent-settings","message":"fictional-private-diagnostic"}}"#).unwrap();
    let accepted = f.add().await;
    let view = f.finish(accepted.host_id).await;
    assert_eq!(view.status, RunStatus::Completed);
    let configure = view
        .steps
        .iter()
        .find(|step| step.step == "configure-agents")
        .unwrap();
    assert_eq!(configure.status, StepStatus::Completed);
    assert_eq!(
        configure.message.as_deref(),
        Some("could not read the agent settings")
    );
    assert_eq!(
        f.service
            .credential_delivery()
            .warning(accepted.host_id)
            .as_deref(),
        Some("could not read the agent settings")
    );
    f.assert_host_delivery(accepted.host_id, "read-agent-settings")
        .await;
    assert!(
        !serde_json::to_string(&view)
            .unwrap()
            .contains("fictional-private-diagnostic")
    );
    let host = accepted.host_id;
    let record: String = f
        .harness
        .store
        .conn()
        .call("inspect fixture delivery home", move |conn| {
            Ok(conn.query_row(
                "SELECT tensorlake_credential_delivery FROM hosts WHERE id = ?1",
                [host],
                |row| row.get(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&record).unwrap()["home"],
        "/home/farhelm-e2e"
    );
}

/// Codex modes need setup's credential step even without any API endpoint.
/// Subscription seeds before the host connection becomes usable; None still
/// reports a failed settings cleanup as a warning while connection continues.
#[farhelm_testtrace::test]
async fn codex_subscription_and_none_enter_setup_without_an_endpoint() {
    use farhelm_proto::credentials::{CreateCredential, CredentialKind};
    for subscription in [true, false] {
        let f = fixture(false).await;
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.base = RecipeBase::Image {
            image: "example/systemd".into(),
        };
        if subscription {
            f.harness
                .store
                .create_credential(CreateCredential {
                    name: "codex".into(),
                    kind: CredentialKind::CodexLogin,
                    secret: r#"{"opaque":"fictional-setup-codex-secret"}"#.into(),
                })
                .await
                .unwrap();
            recipe.codex_login_credential = Some("codex".into());
        } else {
            std::fs::write(f.root.path().join("backend/config.json"), r#"{"default":{"fail_action":"read-agent-settings","message":"fictional-private-diagnostic"}}"#).unwrap();
        }
        f.harness
            .store
            .update_tensorlake_recipe("test-recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
        let accepted = f.add().await;
        let view = f.finish(accepted.host_id).await;
        assert_eq!(view.status, RunStatus::Completed);
        let step = view
            .steps
            .iter()
            .find(|step| step.step == "configure-agents")
            .expect("every Codex mode has a configuration step");
        assert_eq!(step.status, StepStatus::Completed);
        assert_eq!(step.message.is_none(), subscription);
        if !subscription {
            assert_eq!(
                step.message.as_deref(),
                Some("could not read the agent settings")
            );
        }
        let events = std::fs::read_to_string(f.root.path().join("backend/events.jsonl")).unwrap();
        f.assert_host_delivery(accepted.host_id, "read-agent-settings")
            .await;
        assert!(
            !events.contains("fictional-setup-codex-secret")
                && !events.contains("fictional-private-diagnostic")
        );
        let host = accepted.host_id;
        let json: String = f
            .harness
            .store
            .conn()
            .call("inspect fixture Codex stamp", move |conn| {
                Ok(conn.query_row(
                    "SELECT tensorlake_credential_delivery FROM hosts WHERE id = ?1",
                    [host],
                    |row| row.get(0),
                )?)
            })
            .await
            .unwrap();
        let record: serde_json::Value = serde_json::from_str(&json).unwrap();
        if subscription {
            assert_eq!(record["codex"], "Cleared");
            let auth = record["codex_auth"].as_array().unwrap();
            assert_eq!(auth.len(), 3);
            assert_eq!(auth[0], "codex");
            assert_eq!(auth[1], 1);
            uuid::Uuid::parse_str(auth[2].as_str().unwrap()).unwrap();
        } else {
            assert!(record["codex"].is_null() && record["codex_auth"].is_null());
        }
    }
}

/// Claude subscription setup must finish its private settings write before
/// making the host connection usable, even without API endpoints. A
/// nonsecret durable stamp witnesses completion without logging the token.
#[farhelm_testtrace::test]
async fn claude_subscription_setup_delivers_only_to_the_host_before_attach() {
    use farhelm_proto::credentials::{CreateCredential, CredentialKind};
    let f = fixture(false).await;
    let mut recipe = crate::tensorlake::recipes::tests_support::save();
    recipe.base = RecipeBase::Image {
        image: "example/systemd".into(),
    };
    f.harness
        .store
        .create_credential(CreateCredential {
            name: "claude".into(),
            kind: CredentialKind::ClaudeLogin,
            secret: "sk-ant-oat01-fictional-setup-claude".into(),
        })
        .await
        .unwrap();
    recipe.claude_login_credential = Some("claude".into());
    f.harness
        .store
        .update_tensorlake_recipe("test-recipe".into(), recipe)
        .await
        .unwrap()
        .unwrap();
    let accepted = f.add().await;
    let view = f.finish(accepted.host_id).await;
    assert_eq!(view.status, RunStatus::Completed);
    let step = view
        .steps
        .iter()
        .find(|step| step.step == "configure-agents")
        .unwrap();
    assert_eq!(step.status, StepStatus::Completed);
    assert!(step.message.is_none());
    let events = std::fs::read_to_string(f.root.path().join("backend/events.jsonl")).unwrap();
    f.assert_host_delivery(accepted.host_id, "wrote-claude-settings")
        .await;
    assert!(!events.contains("sk-ant-oat01-fictional-setup-claude"));
    let host = accepted.host_id;
    let json: String = f
        .harness
        .store
        .conn()
        .call("inspect fixture Claude stamp", move |conn| {
            Ok(conn.query_row(
                "SELECT tensorlake_credential_delivery FROM hosts WHERE id = ?1",
                [host],
                |row| row.get(0),
            )?)
        })
        .await
        .unwrap();
    let record: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(record["claude"]["Subscription"]["name"], "claude");
    assert_eq!(record["claude"]["Subscription"]["revision"], 1);
    assert!(!json.contains("sk-ant-oat01-fictional-setup-claude"));
}

/// GitHub-only recipes must enter setup's credential step without an agent
/// endpoint to trigger it. Hosts without gh still finish connecting after the
/// token upload, keeping the unsuccessful delivery slot retryable.
#[farhelm_testtrace::test]
async fn github_only_setup_delivers_only_to_the_host_before_attach() {
    use farhelm_proto::credentials::{CreateCredential, CredentialKind};
    for github_cli in [true, false] {
        let f = fixture(false).await;
        f.harness
            .store
            .create_credential(CreateCredential {
                name: "github".into(),
                kind: CredentialKind::GithubToken,
                secret: "fictional-setup-github-token".into(),
            })
            .await
            .unwrap();
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.base = RecipeBase::Image {
            image: "example/systemd".into(),
        };
        recipe.github_token_credential = Some("github".into());
        f.harness
            .store
            .update_tensorlake_recipe("test-recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
        std::fs::write(
            f.root.path().join("backend/config.json"),
            serde_json::to_vec(&serde_json::json!({"default": {"github_cli": github_cli}}))
                .unwrap(),
        )
        .unwrap();
        let accepted = f.add().await;
        let view = f.finish(accepted.host_id).await;
        assert_eq!(view.status, RunStatus::Completed);
        let step = view
            .steps
            .iter()
            .find(|step| step.step == "configure-agents")
            .expect("GitHub alone creates the credential step");
        assert_eq!(step.status, StepStatus::Completed);
        let warning = f.service.credential_delivery().warning(accepted.host_id);
        assert_eq!(warning.is_none(), github_cli);
        if !github_cli {
            assert!(warning.unwrap().contains("private clones need gh"));
        }
        f.assert_host_delivery(accepted.host_id, "configure-github-helper")
            .await;
        let host = accepted.host_id;
        let record: String = f
            .harness
            .store
            .conn()
            .call("inspect GitHub setup stamp", move |conn| {
                Ok(conn.query_row(
                    "SELECT tensorlake_credential_delivery FROM hosts WHERE id = ?1",
                    [host],
                    |row| row.get(0),
                )?)
            })
            .await
            .unwrap();
        assert!(!record.contains("fictional-setup-github-token"));
        let record: serde_json::Value = serde_json::from_str(&record).unwrap();
        assert_eq!(!record["github"].is_null(), github_cli);
        if github_cli {
            assert_eq!(record["github"]["Token"]["name"], "github");
            assert_eq!(record["github"]["Token"]["revision"], 1);
        }
    }
}

/// A missing key and malformed outer request fail before API work or host
/// registration. This is the boundary that keeps Continue free of billing.
#[farhelm_testtrace::test]
async fn tensorlake_probe_is_free_and_validates_outer_fields() {
    let f = fixture(false).await;
    let ProbeResponse::TensorlakeProvisionable {
        sandbox,
        confirmation,
        ..
    } = f
        .probe(ProbeDestination::Tensorlake {
            name: "Workspace".into(),
            recipe: "test-recipe".into(),
        })
        .await
    else {
        panic!("expected plan")
    };
    assert!(sandbox.starts_with("farhelm-host-"));
    assert!(confirmation.contains("bills while running"));
    assert!(!f.root.path().join("api/events.jsonl").exists());
    assert_eq!(f.harness.store.list_hosts().await.unwrap().len(), 1);
    for request in [
        ProbeRequest {
            target: ProbeDestination::Tensorlake {
                name: "".into(),
                recipe: "test-recipe".into(),
            },
            remote_farhelm: None,
            remote_state_dir: None,
            new_entry: None,
        },
        ProbeRequest {
            target: ProbeDestination::Tensorlake {
                name: "x".into(),
                recipe: "test-recipe".into(),
            },
            remote_farhelm: Some("/x".into()),
            remote_state_dir: None,
            new_entry: None,
        },
    ] {
        assert!(
            f.service
                .probe_tensorlake(request, f.tensorlake.clone())
                .await
                .is_err()
        );
    }
    f.harness
        .store
        .save_tensorlake_config(SaveSettings {
            clear_api_key: true,
            ..Default::default()
        })
        .await
        .ok()
        .unwrap();
    let error = f
        .service
        .probe_tensorlake(
            ProbeRequest {
                target: ProbeDestination::Tensorlake {
                    name: "x".into(),
                    recipe: "test-recipe".into(),
                },
                remote_farhelm: None,
                remote_state_dir: None,
                new_entry: None,
            },
            f.tensorlake.clone(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("API key"));
}

/// A generated name is not proof that this helm created the sandbox. A
/// collision must fail before any host preparation or credential work, and
/// deleting the failed entry must reveal the untouched sandbox as a leftover.
#[farhelm_testtrace::test]
async fn an_unrequested_name_collision_refuses_setup_and_delete_leaves_it_alone() {
    let f = fixture(false).await;
    let ProbeResponse::TensorlakeProvisionable {
        probe_id, sandbox, ..
    } = f
        .probe(ProbeDestination::Tensorlake {
            name: "Test host".into(),
            recipe: "test-recipe".into(),
        })
        .await
    else {
        panic!("expected a creation plan")
    };
    let mut foreign = f.model();
    let created = foreign
        .create(&CreateSandbox {
            name: sandbox.clone(),
            source: SandboxSource::Image("example/systemd".into()),
            resources: Resources {
                cpus: 2.0,
                memory_mb: 4096,
                disk_mb: 20480,
            },
            timeout_secs: 0,
            auto_resume: false,
            max_pending_secs: 900,
        })
        .unwrap();
    foreign.plant_snapshot(&created.sandbox_id);
    std::fs::write(
        f.root.path().join("api/state.json"),
        serde_json::to_vec(&foreign).unwrap(),
    )
    .unwrap();
    assert_eq!(foreign.get(&sandbox).unwrap().id, created.sandbox_id);
    let backend_events = f.root.path().join("backend/events.jsonl");
    assert!(!backend_events.exists(), "premise: no backend work ran");

    let accepted = f
        .service
        .start_add(ProvisionRequest { probe_id })
        .await
        .unwrap();
    let failed = f.finish(accepted.host_id).await;
    assert_eq!(failed.status, RunStatus::Failed);
    let message = failed.message.as_deref().unwrap();
    assert!(
        message.contains(&format!("a sandbox named {sandbox} already exists")),
        "{message}"
    );
    assert!(message.contains("Delete this host entry"), "{message}");
    assert!(
        !backend_events.exists(),
        "refusal precedes preparation and delivery"
    );
    let row = f
        .harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == accepted.host_id)
        .unwrap();
    let binding = row.tensorlake.as_ref().unwrap();
    assert!(binding.sandbox_id.is_none() && binding.pending_create_at.is_none());
    assert!(row.remote_farhelm.is_none() && row.remote_state_dir.is_none());
    assert_eq!(
        f.model(),
        foreign,
        "setup leaves foreign resources unchanged"
    );
    assert!(
        !f.tensorlake
            .orphan_sandboxes()
            .await
            .unwrap()
            .iter()
            .any(|leftover| leftover.id == created.sandbox_id)
    );

    let api_events = f.root.path().join("api/events.jsonl");
    let before_delete = std::fs::read(&api_events).unwrap();
    let proof = f.tensorlake.delete_sandbox(&row).await.unwrap();
    assert_eq!(
        std::fs::read(&api_events).unwrap(),
        before_delete,
        "a never-requested row must not even look the name up during deletion"
    );
    f.harness
        .store
        .remove_host_row(row.id, Some(&proof))
        .await
        .unwrap();
    assert_eq!(
        f.model(),
        foreign,
        "deletion leaves the sandbox and snapshots intact"
    );
    assert!(
        f.tensorlake
            .orphan_sandboxes()
            .await
            .unwrap()
            .iter()
            .any(|leftover| leftover.id == created.sandbox_id)
    );
}

/// Every sandbox a session owns is created with the 30-minute idle timeout
/// (SPEC.md), so a forgotten one stops billing for compute. A session-owned
/// row can outlive its launch without a sandbox (the helm stopped before the
/// create, or cleanup failed), and Hosts still offers the ordinary setup retry
/// for it. That retry carries no session, so the timeout must follow the row's
/// ownership rather than the launch; this pins it, alongside the ordinary
/// host's zero timeout in the test below.
#[farhelm_testtrace::test]
async fn setup_retry_on_a_leftover_session_owned_row_keeps_the_idle_timeout() {
    let f = fixture(true).await;
    let sandbox = "farhelm-sess-test-leftover";
    let size = f
        .harness
        .store
        .tensorlake_recipe_row("test-recipe")
        .await
        .unwrap()
        .unwrap()
        .0
        .size;
    let host = f
        .harness
        .store
        .register_session_sandbox(NewTensorlakeHost {
            destination: crate::tensorlake::ssh::destination(sandbox),
            entry: NewHostEntry::new(Some("Leftover sandbox")).unwrap(),
            sandbox: sandbox.into(),
            recipe: "test-recipe".into(),
            resources: size,
        })
        .await
        .unwrap();
    f.harness.manager.sync_registry().await.unwrap();
    assert!(
        f.model()
            .sandboxes
            .iter()
            .all(|entry| entry.name.as_deref() != Some(sandbox)),
        "premise: the leftover row has no sandbox yet"
    );

    let accepted = f.rerun(host).await;
    let view = f.finish(accepted.host_id).await;
    assert_eq!(view.status, RunStatus::Completed, "{:?}", view.message);
    let model = f.model();
    let created = model
        .sandboxes
        .iter()
        .find(|entry| entry.name.as_deref() == Some(sandbox))
        .expect("the retry created the row's sandbox");
    assert_eq!(created.timeout_secs, 1800);
    assert!(!created.auto_resume);
}

/// Initial Add builds and prepares once, then boots a host without reinstalling.
/// Rerun retains ordinary installation on that same sandbox; otherwise recovery
/// would silently depend on the recipe's current prepared cache.
#[farhelm_testtrace::test]
async fn managed_add_uses_preparation_and_rerun_installs_on_the_same_sandbox() {
    let f = fixture(true).await;
    let accepted = f.add().await;
    assert!(
        f.harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .any(|row| row.id == accepted.host_id)
    );
    let view = f.finish(accepted.host_id).await;
    assert_eq!(view.status, RunStatus::Completed, "{:?}", view.message);
    assert_eq!(view.steps[0].step, "build-snapshot");
    assert!(view.steps.iter().any(|s| s.step == "attach-supervisor"));
    assert!(!view.steps.iter().any(|s| s.step == "set-up-farhelm"));
    let row = f
        .harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == accepted.host_id)
        .unwrap();
    assert!(row.yolo_without_asking);
    assert!(
        !row.commands_without_asking
            && !row.links_without_asking
            && !row.local_links_without_asking
    );
    let model = f.model();
    assert_eq!(model.live_sandboxes().len(), 1);
    assert_eq!(
        model.sandboxes.len(),
        3,
        "one builder, one preparer and one host"
    );
    let sandbox = model.live_sandboxes().pop().unwrap();
    let created = model.sandboxes.iter().find(|s| s.id == sandbox.id).unwrap();
    assert_eq!(created.timeout_secs, 0);
    assert!(!created.auto_resume);
    let host_target = format!("ssh:{}", row.destination.as_deref().unwrap());
    let events = f.backend_events();
    assert!(
        !events
            .iter()
            .any(|event| event["target"] == host_target && event["event"] == "install-farhelm")
    );
    let before_rerun = events.len();
    let ProbeResponse::TensorlakeProvisionable { probe_id, .. } = f
        .probe(ProbeDestination::TensorlakeHost { host: row.id })
        .await
    else {
        panic!("expected rerun plan")
    };
    f.service
        .start_add(ProvisionRequest {
            probe_id: probe_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(f.finish(row.id).await.status, RunStatus::Completed);
    assert!(
        f.service
            .start_add(ProvisionRequest { probe_id })
            .await
            .is_err(),
        "one-use confirmation"
    );
    assert_eq!(
        f.model().sandboxes.len(),
        3,
        "rerun creates neither builder, preparer nor host"
    );
    assert!(
        f.backend_events()[before_rerun..]
            .iter()
            .any(|event| event["target"] == host_target && event["event"] == "install-farhelm"),
        "ordinary rerun still installs Farhelm"
    );
}

/// A held first preparation keeps its durable explanation visible while step
/// messages change. A second Add reuses the exact cache and performs no further
/// preparation or installation, so the optimization is observable at both the
/// confirmation and execution boundaries.
#[farhelm_testtrace::test]
async fn lazy_preparation_explains_its_wait_and_fresh_cache_skips_installation() {
    let f = fixture(false).await;
    f.backend_config(serde_json::json!({"default": {"hold_actions": true}}));
    let ProbeResponse::TensorlakeProvisionable {
        probe_id,
        confirmation,
        steps,
        ..
    } = f
        .probe(ProbeDestination::Tensorlake {
            name: "First host".into(),
            recipe: "test-recipe".into(),
        })
        .await
    else {
        panic!("first Add plan")
    };
    let explanation =
        "Only the first time, and then at most once every 24 hours or after Farhelm updates.";
    let prepare = steps
        .steps
        .iter()
        .find(|step| step.id == "prepare-snapshot")
        .unwrap();
    assert_eq!(prepare.explanation.as_deref(), Some(explanation));
    assert!(confirmation.contains(explanation));
    let first = f
        .service
        .start_add(ProvisionRequest { probe_id })
        .await
        .unwrap();
    f.wait_for_backend("prepare-sandbox").await;
    let waiting = f.service.view(first.host_id).await.unwrap();
    assert_eq!(waiting.status, RunStatus::Running);
    let prepare = waiting
        .steps
        .iter()
        .find(|step| step.step == "prepare-snapshot")
        .unwrap();
    assert_eq!(prepare.status, StepStatus::Running);
    assert_eq!(prepare.explanation.as_deref(), Some(explanation));
    assert!(
        f.model().sandboxes.iter().all(|sandbox| !sandbox
            .name
            .as_deref()
            .is_some_and(|name| name.starts_with("farhelm-host-"))),
        "the preparation gate precedes host creation"
    );
    f.backend_config(serde_json::json!({}));
    assert_eq!(f.finish(first.host_id).await.status, RunStatus::Completed);
    let state = f
        .harness
        .store
        .tensorlake_recipe_row("test-recipe")
        .await
        .unwrap()
        .unwrap()
        .1;
    let prepared = state.prepared.unwrap();
    let before_model = f.model();
    let before_events = f.backend_events();
    assert_eq!(
        before_model.sandboxes.len(),
        2,
        "built source was seeded; only preparer and first host are new"
    );
    assert_eq!(
        before_events
            .iter()
            .filter(|event| event["event"] == "finish-snapshot-install")
            .count(),
        1
    );
    let ProbeResponse::TensorlakeProvisionable {
        probe_id,
        confirmation,
        steps,
        ..
    } = f
        .probe(ProbeDestination::Tensorlake {
            name: "Second host".into(),
            recipe: "test-recipe".into(),
        })
        .await
    else {
        panic!("fresh Add plan")
    };
    assert!(!steps.steps.iter().any(|step| step.id == "prepare-snapshot"));
    assert!(!confirmation.contains("Prepare snapshot"));
    let second = f
        .service
        .start_add(ProvisionRequest { probe_id })
        .await
        .unwrap();
    let completed = f.finish(second.host_id).await;
    assert_eq!(
        completed.status,
        RunStatus::Completed,
        "{:?}",
        completed.message
    );
    assert!(
        !completed
            .steps
            .iter()
            .any(|step| step.step == "prepare-snapshot")
    );
    let after = f.model();
    let added = after
        .sandboxes
        .iter()
        .filter(|sandbox| {
            !before_model
                .sandboxes
                .iter()
                .any(|old| old.id == sandbox.id)
        })
        .collect::<Vec<_>>();
    assert_eq!(added.len(), 1, "fresh cache creates only the second host");
    assert_eq!(added[0].snapshot_id.as_deref(), Some(prepared.id.as_str()));
    let later = &f.backend_events()[before_events.len()..];
    assert!(
        !later.iter().any(|event| matches!(
            event["event"].as_str(),
            Some(
                "prepare-sandbox"
                    | "inspect"
                    | "upload-farhelm"
                    | "install-farhelm"
                    | "enable-supervisor"
                    | "finish-snapshot-install"
            )
        )),
        "fresh Add must not reinstall: {later:?}"
    );
}

/// The API can report Running before SSH accepts a channel. A prepared clone
/// must stay unadmitted and receive no credential operations until that separate
/// transport boundary succeeds, without running setup on the clone itself.
#[farhelm_testtrace::test]
async fn prepared_clone_waits_for_ssh_before_admission_and_delivery() {
    let f = fixture(false).await;
    f.seed_prepared().await;
    f.backend_config(serde_json::json!({"default": {"hold_actions": true}}));
    let host = f.add().await.host_id;
    f.wait_for_backend("wait-tensorlake-ssh").await;
    let row = f
        .harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .unwrap();
    assert!(
        row.tensorlake.as_ref().unwrap().sandbox_id.is_some(),
        "the API already created the clone"
    );
    assert!(row.remote_farhelm.is_none() && row.remote_state_dir.is_none());
    assert_eq!(f.harness.fleet.dials(host), 0);
    assert!(
        f.backend_events()
            .iter()
            .all(|event| event["event"] == "wait-tensorlake-ssh"),
        "no installation or credential work starts while SSH is held"
    );
    f.backend_config(serde_json::json!({}));
    assert_eq!(f.finish(host).await.status, RunStatus::Completed);
    let events = f.backend_events();
    let ready = events
        .iter()
        .position(|event| event["event"] == "wait-tensorlake-ssh")
        .unwrap();
    let delivery = events
        .iter()
        .position(|event| event["event"] == "read-agent-settings")
        .unwrap();
    assert!(ready < delivery);
    assert!(
        !events
            .iter()
            .any(|event| event["event"] == "prepare-sandbox")
    );
}

/// The checkpoint-safety hook is an actual success boundary for the engine.
/// Refusing copied state or refusing final cleanup must stop before snapshot
/// creation and before allocating a host. The hook fake proves propagation;
/// filesystem removal and fresh clone identities have separate real proofs.
#[farhelm_testtrace::test]
async fn preparation_refusal_never_checkpoints_or_creates_a_host_clone() {
    for action in ["check-supervisor-state", "finish-snapshot-install"] {
        let f = fixture(false).await;
        f.backend_config(serde_json::json!({"default": {"fail_action": action, "message": "retained supervisor state"}}));
        let accepted = f.add().await;
        let failed = f.finish(accepted.host_id).await;
        assert_eq!(failed.status, RunStatus::Failed);
        assert!(
            failed
                .steps
                .iter()
                .any(|step| step.step == "prepare-snapshot" && step.status == StepStatus::Failed)
        );
        assert!(
            f.backend_events()
                .iter()
                .any(|event| event["event"] == action),
            "premise: refusal came from the intended hook"
        );
        let model = f.model();
        assert_eq!(model.sandboxes.len(), 1, "only the preparer was created");
        assert!(
            model.sandboxes[0]
                .name
                .as_deref()
                .is_some_and(|name| name.starts_with("farhelm-prepare-"))
        );
        assert_eq!(model.sandboxes[0].status, "terminated");
        let events = std::fs::read_to_string(f.root.path().join("api/events.jsonl")).unwrap();
        assert!(
            !events
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .any(|event| event["op"] == "create_snapshot"),
            "refusal must prevent checkpoint: {events}"
        );
        let row = f
            .harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == accepted.host_id)
            .unwrap();
        let binding = row.tensorlake.unwrap();
        assert!(binding.sandbox_id.is_none() && binding.pending_create_at.is_none());
        assert!(
            row.remote_farhelm.is_none()
                && row.remote_state_dir.is_none()
                && row.host_identity.is_none()
        );
        assert_eq!(f.harness.fleet.dials(row.id), 0);
        assert!(
            !f.backend_events().iter().any(|event| matches!(
                event["event"].as_str(),
                Some(
                    "read-agent-settings"
                        | "wrote-claude-settings"
                        | "configure-github-helper"
                        | "remove-github-token"
                )
            )),
            "preparer receives no credential delivery"
        );
    }
}

/// An uncertain create may become visible after a helm restart and after a
/// different prepared snapshot replaces its source. Its own saved facts must
/// survive both an outstanding request and an observed ID, without opening
/// dialing early or reinstalling from the later recipe's metadata.
#[farhelm_testtrace::test]
async fn prepared_create_recovers_its_exact_install_facts_after_restart() {
    for recorded_id in [false, true] {
        let f = fixture(false).await;
        let selected = f.seed_prepared().await;
        let original = f
            .harness
            .store
            .tensorlake_recipe_row("test-recipe")
            .await
            .unwrap()
            .unwrap()
            .1
            .prepared
            .unwrap();
        std::fs::write(
            f.root.path().join("api/config.json"),
            r#"{"fail":{"create_sandbox":{"status":500,"message":"reply lost"}}}"#,
        )
        .unwrap();
        let host = f.add().await.host_id;
        assert_eq!(f.finish(host).await.status, RunStatus::Failed);
        let row = f
            .harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert!(row.tensorlake.as_ref().unwrap().pending_create_at.is_some());
        assert!(row.remote_farhelm.is_none() && row.remote_state_dir.is_none());
        assert_eq!(
            f.harness
                .store
                .tensorlake_prepared_install(host)
                .await
                .unwrap(),
            Some(selected.clone())
        );
        let mut model = f.model();
        let created = model
            .create(&CreateSandbox {
                name: row.tensorlake.as_ref().unwrap().sandbox.clone(),
                source: SandboxSource::Snapshot(original.id),
                resources: Resources {
                    cpus: 2.0,
                    memory_mb: 4096,
                    disk_mb: 20480,
                },
                timeout_secs: 0,
                auto_resume: false,
                max_pending_secs: 900,
            })
            .unwrap();
        let latest_snapshot = model.plant_snapshot("laterpreparer");
        std::fs::write(
            f.root.path().join("api/state.json"),
            serde_json::to_vec(&model).unwrap(),
        )
        .unwrap();
        if recorded_id {
            assert!(
                f.harness
                    .store
                    .set_tensorlake_sandbox_id(host, &created.sandbox_id)
                    .await
                    .unwrap()
            );
        }
        f.harness
            .store
            .update_tensorlake_recipe_state("test-recipe", move |_, state| {
                let current = state.prepared.as_mut().unwrap();
                current.id = latest_snapshot;
                current.farhelm_version = "a-different-helm-build".into();
                current.facts.home = "/home/later-user".into();
                current.facts.farhelm_path = "/later/farhelm".into();
                current.facts.state_dir = "/later/state".into();
            })
            .await
            .unwrap()
            .unwrap();
        std::fs::write(f.root.path().join("api/config.json"), "{}").unwrap();
        let f = f.restart().await;
        assert_eq!(
            f.harness
                .store
                .tensorlake_prepared_install(host)
                .await
                .unwrap(),
            Some(selected.clone())
        );
        assert_eq!(
            f.harness.fleet.dials(host),
            0,
            "saved facts remain unadmitted across restart"
        );
        let ProbeResponse::TensorlakeProvisionable {
            probe_id, steps, ..
        } = f.probe(ProbeDestination::TensorlakeHost { host }).await
        else {
            panic!("prepared recovery offer")
        };
        assert!(
            !steps.steps.iter().any(|step| step.id == "prepare-snapshot"),
            "recovering the chosen clone must not promise renewal of the now-stale recipe cache"
        );
        assert!(!steps.steps.iter().any(|step| step.id == "set-up-farhelm"));
        f.service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let completed = f.finish(host).await;
        assert_eq!(
            completed.status,
            RunStatus::Completed,
            "{:?}",
            completed.message
        );
        let row = f
            .harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert_eq!(
            row.remote_farhelm.as_deref(),
            selected.farhelm_path.to_str()
        );
        assert_eq!(row.remote_state_dir.as_deref(), selected.state_dir.to_str());
        assert_eq!(
            row.tensorlake.unwrap().sandbox_id.as_deref(),
            Some(created.sandbox_id.as_str())
        );
        assert!(
            f.harness
                .store
                .tensorlake_prepared_install(host)
                .await
                .unwrap()
                .is_none(),
            "admission consumes the transient facts"
        );
        let stamp: String = f
            .harness
            .store
            .conn()
            .call("inspect recovered credential home", move |conn| {
                Ok(conn.query_row(
                    "SELECT tensorlake_credential_delivery FROM hosts WHERE id = ?1",
                    [host],
                    |row| row.get(0),
                )?)
            })
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&stamp).unwrap()["home"],
            selected.home.to_str().unwrap()
        );
        assert_eq!(
            f.model().sandboxes.len(),
            1,
            "recovery adopts the one exact clone"
        );
        assert!(
            !f.backend_events().iter().any(|event| matches!(
                event["event"].as_str(),
                Some("prepare-sandbox" | "inspect" | "install-farhelm" | "enable-supervisor")
            )),
            "recovery must not reinstall or inspect a booted prepared clone"
        );
    }
}

/// Migrated image and snapshot recipes have no built snapshot, but their old
/// hosts already own a sandbox. Rerun must skip the builder by durable ID and
/// by name while the pending-create marker survives a lost ID write,
/// regardless of that empty recipe build state.
#[farhelm_testtrace::test]
async fn rerun_of_an_existing_sandbox_skips_building_even_without_a_recipe_snapshot() {
    for lost_id in [false, true] {
        let f = fixture(false).await;
        let host = f.legacy_host().await;
        let ProbeResponse::TensorlakeProvisionable {
            probe_id, steps, ..
        } = f.probe(ProbeDestination::TensorlakeHost { host }).await
        else {
            panic!("ordinary recovery offer")
        };
        assert!(
            steps.steps.iter().any(|step| step.id == "set-up-farhelm"),
            "a legacy unadmitted sandbox still authorizes ordinary installation"
        );
        assert!(!steps.steps.iter().any(|step| step.id == "prepare-snapshot"));
        f.service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        assert_eq!(f.finish(host).await.status, RunStatus::Completed);
        let before = f.model();
        assert_eq!(before.live_sandboxes().len(), 1);
        let sandbox = before.live_sandboxes()[0].id.clone();
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.packages.clear();
        recipe.base = if lost_id {
            RecipeBase::Snapshot {
                snapshot_id: "oldusersnapshot".into(),
            }
        } else {
            RecipeBase::Image {
                image: "old/base".into(),
            }
        };
        f.harness
            .store
            .update_tensorlake_recipe("test-recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
        f.harness
            .store
            .update_tensorlake_recipe_state("test-recipe", |_, state| *state = Default::default())
            .await
            .unwrap()
            .unwrap();
        if lost_id {
            f.harness
                .store
                .conn()
                .call("forget fixture sandbox id", move |conn| {
                    conn.execute(
                        "UPDATE hosts SET tensorlake_sandbox_id = NULL, tensorlake_pending_create_at = ?2 WHERE id = ?1",
                        rusqlite::params![host, crate::tensorlake::recipes::now_secs()],
                    )?;
                    Ok(())
                })
                .await
                .unwrap();
        }
        assert!(
            f.harness
                .store
                .tensorlake_recipe_row("test-recipe")
                .await
                .unwrap()
                .unwrap()
                .1
                .snapshot
                .is_none()
        );
        let events_before_probe = std::fs::read(f.root.path().join("api/events.jsonl")).unwrap();
        let ProbeResponse::TensorlakeProvisionable {
            probe_id,
            steps,
            confirmation,
            ..
        } = f.probe(ProbeDestination::TensorlakeHost { host }).await
        else {
            panic!("rerun plan")
        };
        assert_eq!(
            std::fs::read(f.root.path().join("api/events.jsonl")).unwrap(),
            events_before_probe,
            "rerun probing must remain local"
        );
        if lost_id {
            assert!(
                steps.steps[0].name.contains("only if"),
                "a lookup by name may still need a settled create: {confirmation}"
            );
        } else {
            assert_eq!(
                steps.steps[0].name,
                "Use the host's recorded sandbox (no snapshot build)"
            );
            assert!(
                !confirmation.contains("Build the snapshot"),
                "{confirmation}"
            );
            assert!(steps.steps[1].name.starts_with("Use the recorded sandbox"));
        }
        f.service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let view = f.finish(host).await;
        assert_eq!(view.status, RunStatus::Completed, "{:?}", view.message);
        assert_eq!(view.steps[0].step, "build-snapshot");
        assert_eq!(view.steps[0].status, StepStatus::Skipped);
        let after = f.model();
        assert_eq!(after.sandboxes.len(), before.sandboxes.len());
        assert_eq!(after.live_sandboxes()[0].id, sandbox);
        assert!(
            f.harness
                .store
                .tensorlake_recipe_row("test-recipe")
                .await
                .unwrap()
                .unwrap()
                .1
                .snapshot
                .is_none()
        );
    }
}

/// A 404 is not permission to build while a host create may still settle, and
/// a recorded sandbox is never replaced even when it is gone. Empty migrated
/// recipe state must not turn either rerun into a billed builder request.
#[farhelm_testtrace::test]
async fn rerun_does_not_build_while_a_sandbox_is_unsettled_or_recorded_gone() {
    for recorded in [false, true] {
        let f = fixture(false).await;
        let host = f.legacy_host().await;
        f.rerun(host).await;
        assert_eq!(f.finish(host).await.status, RunStatus::Completed);
        let mut model = f.model();
        model.sandboxes.clear();
        std::fs::write(
            f.root.path().join("api/state.json"),
            serde_json::to_vec(&model).unwrap(),
        )
        .unwrap();
        f.harness
            .store
            .update_tensorlake_recipe_state("test-recipe", |_, state| *state = Default::default())
            .await
            .unwrap()
            .unwrap();
        if !recorded {
            f.harness.store.conn().call("plant unsettled fixture create", move |conn| {
                conn.execute("UPDATE hosts SET tensorlake_sandbox_id = NULL, tensorlake_pending_create_at = ?2 WHERE id = ?1", rusqlite::params![host, crate::tensorlake::recipes::now_secs()])?;
                Ok(())
            }).await.unwrap();
        }
        let row = f
            .harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        let binding = row.tensorlake.unwrap();
        assert_eq!(binding.sandbox_id.is_some(), recorded);
        assert_eq!(binding.pending_create_at.is_some(), !recorded);
        assert!(
            f.model().live_sandboxes().is_empty(),
            "premise: sandbox lookup is 404"
        );
        assert!(
            f.harness
                .store
                .tensorlake_recipe_row("test-recipe")
                .await
                .unwrap()
                .unwrap()
                .1
                .snapshot
                .is_none(),
            "premise: a migrated recipe has no snapshot to reuse"
        );
        let before = std::fs::read_to_string(f.root.path().join("api/events.jsonl")).unwrap();
        let creates_before = before
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|event| event["op"] == "create_sandbox")
            .count();
        assert_eq!(
            creates_before, 0,
            "premise: the legacy sandbox was planted without a Farhelm create"
        );
        let ProbeResponse::TensorlakeProvisionable { probe_id, .. } =
            f.probe(ProbeDestination::TensorlakeHost { host }).await
        else {
            panic!("rerun plan")
        };
        f.service
            .start_add(ProvisionRequest { probe_id })
            .await
            .unwrap();
        let view = f.finish(host).await;
        assert_eq!(view.status, RunStatus::Failed);
        assert_eq!(view.steps[0].status, StepStatus::Skipped);
        let skip = view.steps[0].message.as_deref().unwrap();
        if recorded {
            assert!(skip.contains("recorded sandbox"), "{skip}");
            assert!(!skip.contains("already exists"), "{skip}");
        } else {
            assert!(skip.contains("may still be starting"), "{skip}");
            assert!(
                view.message
                    .as_deref()
                    .unwrap()
                    .contains("may still be starting")
            );
        }
        assert_eq!(view.steps[1].step, "create-sandbox");
        assert_eq!(view.steps[1].status, StepStatus::Failed);
        let after = std::fs::read_to_string(f.root.path().join("api/events.jsonl")).unwrap();
        let creates_after = after
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|event| event["op"] == "create_sandbox")
            .count();
        assert_eq!(
            creates_after, creates_before,
            "neither a host nor a builder create may be sent"
        );
        assert!(f.model().sandboxes.is_empty());
        let state = f
            .harness
            .store
            .tensorlake_recipe_row("test-recipe")
            .await
            .unwrap()
            .unwrap()
            .1;
        assert!(
            state.snapshot.is_none()
                && state.builder_sandbox_id.is_none()
                && state.build_started_at.is_none()
                && state.prepared.is_none()
                && state.preparer_sandbox_id.is_none()
                && state.prepare_started_at.is_none()
                && state.prepare_pending_create_at.is_none()
        );
    }
}

/// New hosts from a migrated image or snapshot definition need the same built
/// snapshot as Ubuntu recipes. The base starts the builder; only its result
/// starts the host, so agents and package layers cannot be bypassed.
#[farhelm_testtrace::test]
async fn new_hosts_from_image_and_snapshot_bases_build_before_create() {
    for snapshot_base in [false, true] {
        let f = fixture(false).await;
        let mut model = f.model();
        let users_snapshot = model.plant_snapshot("usersbuilder");
        std::fs::write(
            f.root.path().join("api/state.json"),
            serde_json::to_vec(&model).unwrap(),
        )
        .unwrap();
        let mut recipe = crate::tensorlake::recipes::tests_support::save();
        recipe.base = if snapshot_base {
            RecipeBase::Snapshot {
                snapshot_id: users_snapshot.clone(),
            }
        } else {
            RecipeBase::Image {
                image: "team/base".into(),
            }
        };
        f.harness
            .store
            .update_tensorlake_recipe("test-recipe".into(), recipe)
            .await
            .unwrap()
            .unwrap();
        f.harness
            .store
            .update_tensorlake_recipe_state("test-recipe", |_, state| *state = Default::default())
            .await
            .unwrap()
            .unwrap();
        let host = f.add().await.host_id;
        let view = f.finish(host).await;
        assert_eq!(view.status, RunStatus::Completed, "{:?}", view.message);
        assert_eq!(view.steps[0].status, StepStatus::Completed);
        let state = f
            .harness
            .store
            .tensorlake_recipe_row("test-recipe")
            .await
            .unwrap()
            .unwrap()
            .1;
        let built = state.snapshot.unwrap();
        let prepared = state.prepared.unwrap();
        assert_eq!(prepared.source_snapshot, built.id);
        let model = f.model();
        assert_eq!(model.sandboxes.len(), 3, "builder, preparer, then host");
        let builder = &model.sandboxes[0];
        assert_eq!(builder.status, "terminated");
        if snapshot_base {
            assert_eq!(
                builder.snapshot_id.as_deref(),
                Some(users_snapshot.as_str())
            );
        } else {
            assert_eq!(builder.image.as_deref(), Some("team/base"));
        }
        assert_eq!(
            model.sandboxes[1].snapshot_id.as_deref(),
            Some(built.id.as_str())
        );
        assert!(
            model.sandboxes[1]
                .name
                .as_deref()
                .is_some_and(|name| name.starts_with("farhelm-prepare-"))
        );
        assert_eq!(model.sandboxes[1].status, "terminated");
        assert_eq!(
            model.sandboxes[2].snapshot_id.as_deref(),
            Some(prepared.id.as_str())
        );
    }
}

/// Failure after create must preserve the row and sandbox id, so Delete
/// sandbox can clean up and Rerun can continue without rebuilding its snapshot.
#[farhelm_testtrace::test]
async fn failed_prepare_keeps_ownership_and_rerun_continues_without_recreate() {
    let f = fixture(false).await;
    let host = f.legacy_host().await;
    std::fs::write(
        f.root.path().join("backend/config.json"),
        r#"{"default":{"fail_action":"prepare-sandbox","message":"image needs systemd"}}"#,
    )
    .unwrap();
    let accepted = f.rerun(host).await;
    let failed = f.finish(accepted.host_id).await;
    assert_eq!(failed.status, RunStatus::Failed);
    assert!(
        failed
            .steps
            .iter()
            .any(|s| s.step == "prepare-sandbox" && s.status == StepStatus::Failed)
    );
    assert_eq!(f.model().sandboxes.len(), 1);
    let row = f
        .harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == accepted.host_id)
        .unwrap();
    assert!(row.tensorlake.as_ref().unwrap().sandbox_id.is_some());
    // Stand in for a create whose sandbox exists but whose ID write was
    // lost while the earlier pending marker survived. That outstanding
    // request, rather than the name alone, authorizes recovery on rerun.
    let host = row.id;
    f.harness
        .store
        .conn()
        .call("clear fixture sandbox id", move |conn| {
            conn.execute(
                "UPDATE hosts SET tensorlake_sandbox_id = NULL, tensorlake_pending_create_at = ?2 WHERE id = ?1",
                rusqlite::params![host, crate::tensorlake::recipes::now_secs()],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    std::fs::write(f.root.path().join("backend/config.json"), "{}").unwrap();
    let ProbeResponse::TensorlakeProvisionable { probe_id, .. } = f
        .probe(ProbeDestination::TensorlakeHost { host: row.id })
        .await
    else {
        panic!("expected rerun")
    };
    f.service
        .start_add(ProvisionRequest { probe_id })
        .await
        .unwrap();
    assert_eq!(f.finish(row.id).await.status, RunStatus::Completed);
    assert_eq!(f.model().sandboxes.len(), 1);
    let deleted = f.tensorlake.delete_sandbox(&row).await.unwrap();
    f.harness
        .store
        .remove_host_row(row.id, Some(&deleted))
        .await
        .unwrap();
    assert!(f.model().live_sandboxes().is_empty());
}

/// Copied supervisor state is a refusal with its own explanation, before
/// installation paths are adopted. It must stay retryable without creating
/// a second sandbox or silently accepting the copied identity.
#[farhelm_testtrace::test]
async fn copied_supervisor_state_refuses_before_installation() {
    let f = fixture(false).await;
    let host = f.legacy_host().await;
    std::fs::write(f.root.path().join("backend/config.json"), r#"{"default":{"fail_action":"check-supervisor-state","message":"image contains Farhelm supervisor state"}}"#).unwrap();
    let accepted = f.rerun(host).await;
    let failed = f.finish(accepted.host_id).await;
    assert_eq!(failed.status, RunStatus::Failed);
    assert!(failed.message.unwrap().contains("supervisor state"));
    let row = f
        .harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == accepted.host_id)
        .unwrap();
    assert!(row.remote_state_dir.is_none());
    assert!(row.host_identity.is_none());
    assert_eq!(f.model().live_sandboxes().len(), 1);
    // Naming the same sandbox through the generic SSH probe must not adopt
    // a copied supervisor and thereby write the coordinates that open dialing.
    let refused = f
        .service
        .probe(ProbeRequest {
            target: ProbeDestination::Ssh {
                destination: row.destination.clone().unwrap(),
            },
            remote_farhelm: None,
            remote_state_dir: None,
            new_entry: None,
        })
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("initial Tensorlake setup"));
    assert!(
        f.service
            .plan_update(row.id)
            .await
            .unwrap_err()
            .to_string()
            .contains("initial Tensorlake setup")
    );
    f.harness.fleet.edit(row.id, |script| {
        script.reachable = true;
        script.identity = Some("copied-supervisor".into());
    });
    // Deliberately drop the in-memory hold, as a restart would. The durable
    // coordinates gate must still refuse before any transport/hello runs.
    f.harness.manager.release_hold(row.id, true);
    f.harness.await_state(row.id, |state| matches!(state, HostState::Unreachable { last_error, .. } if last_error.contains("supervisor state check"))).await;
    assert_eq!(f.harness.fleet.dials(row.id), 0);
    let Fixture {
        harness,
        service,
        tensorlake,
        root: _root,
    } = f;
    drop(service);
    drop(tensorlake);
    let harness = harness.restart_with(|_| {}).await;
    harness.await_state(row.id, |state| matches!(state, HostState::Unreachable { last_error, .. } if last_error.contains("supervisor state check"))).await;
    assert_eq!(
        harness.fleet.dials(row.id),
        0,
        "restart must not dial the copied supervisor"
    );
    let restarted = harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == row.id)
        .unwrap();
    assert!(restarted.host_identity.is_none());
    // Positive control: the very same peer answers with the copied identity
    // once this test explicitly admits it. A dead peer cannot make the
    // preceding zero-dial observations pass by accident.
    harness
        .store
        .admit_tensorlake_install(row.id, "/fixture/farhelm", "/fixture/state")
        .await
        .unwrap();
    harness.manager.sync_registry().await.unwrap();
    harness.manager.retry_now(row.id).await.unwrap();
    harness
        .await_refreshed_as(row.id, "copied-supervisor", 0)
        .await;
}

/// A create's failed reply and immediate 404s do not settle ownership. Delete
/// must retain the row across a restart, then adopt and terminate the sandbox
/// when that same outstanding request finally becomes visible by name.
#[farhelm_testtrace::test]
async fn unresolved_create_blocks_delete_across_restart_until_sandbox_appears() {
    let f = fixture(false).await;
    f.seed_prepared().await;
    std::fs::write(
        f.root.path().join("api/config.json"),
        r#"{"fail":{"create_sandbox":{"status":500,"message":"reply lost"}}}"#,
    )
    .unwrap();
    let accepted = f.add().await;
    assert_eq!(f.finish(accepted.host_id).await.status, RunStatus::Failed);
    let row = f
        .harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == accepted.host_id)
        .unwrap();
    let binding = row.tensorlake.as_ref().unwrap();
    assert!(binding.sandbox_id.is_none());
    let sent = binding
        .pending_create_at
        .expect("failed create is durably outstanding");
    assert!(
        f.model().live_sandboxes().is_empty(),
        "premise: lookups still answer 404"
    );
    let error = f.tensorlake.delete_sandbox(&row).await.unwrap_err();
    assert!(error.to_string().contains("may still be starting"));
    assert!(
        f.harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .any(|r| r.id == row.id)
    );
    let Fixture {
        harness,
        service,
        tensorlake,
        root,
    } = f;
    drop(service);
    drop(tensorlake);
    let harness = harness.restart_with(|_| {}).await;
    let row = harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == row.id)
        .unwrap();
    assert_eq!(
        row.tensorlake.as_ref().unwrap().pending_create_at,
        Some(sent)
    );
    let api = Arc::new(E2eTensorlake::new(root.path().join("api"), root.path()).unwrap());
    let tensorlake = Tensorlake::production(harness.store.clone(), root.path())
        .unwrap()
        .with_api(api);
    assert!(
        tensorlake
            .delete_sandbox(&row)
            .await
            .unwrap_err()
            .to_string()
            .contains("may still be starting")
    );
    std::fs::write(root.path().join("api/config.json"), "{}").unwrap();
    // Complete the outstanding request in Tensorlake's own model without
    // sending another create through Farhelm.
    let mut model: FakeModel =
        serde_json::from_slice(&std::fs::read(root.path().join("api/state.json")).unwrap())
            .unwrap();
    let created = model
        .create(&CreateSandbox {
            name: row.tensorlake.as_ref().unwrap().sandbox.clone(),
            source: SandboxSource::Image("example/systemd".into()),
            resources: Resources {
                cpus: 2.0,
                memory_mb: 4096,
                disk_mb: 20480,
            },
            timeout_secs: 0,
            auto_resume: false,
            max_pending_secs: 900,
        })
        .unwrap();
    std::fs::write(
        root.path().join("api/state.json"),
        serde_json::to_vec(&model).unwrap(),
    )
    .unwrap();
    let proof = tensorlake.delete_sandbox(&row).await.unwrap();
    let recorded = harness
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.id == row.id)
        .unwrap();
    assert_eq!(
        recorded.tensorlake.as_ref().unwrap().sandbox_id.as_deref(),
        Some(created.sandbox_id.as_str())
    );
    assert!(
        recorded
            .tensorlake
            .as_ref()
            .unwrap()
            .pending_create_at
            .is_none()
    );
    harness
        .store
        .remove_host_row(row.id, Some(&proof))
        .await
        .unwrap();
    let model: FakeModel =
        serde_json::from_slice(&std::fs::read(root.path().join("api/state.json")).unwrap())
            .unwrap();
    assert!(model.live_sandboxes().is_empty());
}

/// A missing name becomes conclusive only after the shared settle window.
/// Definite API refusals clear the marker immediately and need no such wait.
#[farhelm_testtrace::test]
async fn settled_absence_and_definite_refusal_allow_delete() {
    for status in [500, 401] {
        let f = fixture(false).await;
        f.seed_prepared().await;
        std::fs::write(
            f.root.path().join("api/config.json"),
            format!(r#"{{"fail":{{"create_sandbox":{{"status":{status}}}}}}}"#),
        )
        .unwrap();
        let accepted = f.add().await;
        assert_eq!(f.finish(accepted.host_id).await.status, RunStatus::Failed);
        let row = f
            .harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.id == accepted.host_id)
            .unwrap();
        assert_eq!(
            row.tensorlake.as_ref().unwrap().pending_create_at.is_some(),
            status == 500
        );
        if status == 500 {
            let settled = crate::tensorlake::recipes::now_secs()
                - crate::tensorlake::build::CREATE_SETTLE_SECS;
            f.harness
                .store
                .set_tensorlake_pending_create(row.id, Some(settled))
                .await
                .unwrap();
        }
        let row = f
            .harness
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.id == accepted.host_id)
            .unwrap();
        let proof = f.tensorlake.delete_sandbox(&row).await.unwrap();
        f.harness
            .store
            .remove_host_row(row.id, Some(&proof))
            .await
            .unwrap();
    }
}

/// Names are ownership tags, not arbitrary display text. Sanitizing Unicode
/// and punctuation must not collide with the browser harness's reserved tag.
#[test]
fn names_fit_tensorlake_grammar() {
    for name in ["", "---", "😀", "GPU Dev / 1", &"a".repeat(300)] {
        let result = sandbox_name(crate::tensorlake::orphans::HOST_PREFIX, "k7q2", name);
        assert!(result.starts_with("farhelm-host-k7q2-"));
        assert!(result.len() <= 63);
        assert!(
            result
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        );
        assert!(!result.ends_with('-'));
        assert!(!result.starts_with("farhelm-sbx-"));
    }
}
