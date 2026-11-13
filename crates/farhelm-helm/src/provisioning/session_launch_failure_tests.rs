//! The normal launch entry point owns cleanup through preparation, attachment
//! and first session creation. This fixture selects existing injectable seams
//! before actors start, and never changes process environment or runs a shell.

use super::*;
use crate::environment::podman::{
    Podman,
    failure_tests::{BACKEND_PHASES, COMMAND_PHASES, Commands, Progress},
};
use crate::environment::podman_probe::PodmanPlaces;
use crate::environment::{EnvironmentProvider, PreparationProgress};
use crate::rest_harness::{FleetBuilder, Harness};
use crate::tensorlake::recipes::StoredRecipe;

/// One real provider, registry, route cache and installer serve the whole
/// fixture. The existing backend models remote install actions; Commands
/// models only Podman's command results, not provider orchestration.
pub(crate) struct Fixture {
    pub(crate) commands: Arc<Commands>,
    pub(crate) provider: Arc<Podman>,
    pub(crate) place: HostId,
    pub(crate) store: HelmStore,
    harness: Harness,
    service: Arc<ProvisioningService>,
    recipe: StoredRecipe,
    backend_root: PathBuf,
    _root: tempfile::TempDir,
}

/// Match the actual account observed by the Podman passwd fake. Different
/// defaults would fail logind/home validation before an intended later fault.
fn behavior() -> serde_json::Value {
    serde_json::json!({"home":"/home/ubuntu","user_unit_dir":"/home/ubuntu/.config/systemd/user",
        "message":"injected backend boundary"})
}

impl Fixture {
    /// A stopped initial harness lends its scripted transport and owned store.
    /// The replacement manager is constructed once with both actual providers;
    /// no actor sees a partially assembled registry or another place cache.
    pub(crate) async fn new() -> Self {
        let mut harness = FleetBuilder::new().await.start().await;
        harness.manager.shutdown();
        let root = tempfile::tempdir().unwrap();
        let store = harness.store.clone();
        let place = store
            .add_ssh_host("place.example", None, None)
            .await
            .unwrap();
        crate::tensorlake::recipes::tests_support::create(&store, "dev").await;
        let recipe = store.tensorlake_recipe_row("dev").await.unwrap().unwrap().0;
        let tag = crate::tensorlake::config::helm_tag(&store.registry_namespace().await.unwrap());
        let commands = Commands::new(store.clone(), place, tag);
        let places = PodmanPlaces::with_commands(commands.clone());
        places.fresh(&store, place).await.unwrap();
        let backend_root = root.path().join("backend");
        std::fs::create_dir(&backend_root).unwrap();
        std::fs::write(
            backend_root.join("ENABLED"),
            "farhelm-e2e-provisioning-v1\n",
        )
        .unwrap();
        std::fs::write(
            backend_root.join("config.json"),
            serde_json::json!({"default":behavior()}).to_string(),
        )
        .unwrap();
        let backend: Arc<dyn ProvisioningBackend> = Arc::new(
            super::super::e2e::E2eProvisioningBackend::new(backend_root.clone(), root.path())
                .unwrap(),
        );
        let payloads: Arc<dyn PayloadSource> =
            Arc::new(super::super::e2e::E2ePayloads(backend_root.join("ENABLED")));
        let delivery =
            CredentialDelivery::with_places(store.clone(), backend.clone(), places.clone());
        let sources = ProductionProvisioning {
            backend: backend.clone(),
            preparation_backend: backend,
            payloads,
            delivery: delivery.clone(),
        };
        let installer = sources.environment_preparer(root.path());
        let tensorlake =
            crate::tensorlake::Tensorlake::detached_with_delivery(store.clone(), delivery.clone());
        let provider = Arc::new(Podman::new(
            store.clone(),
            places,
            delivery,
            tensorlake.builds().clone(),
            installer,
        ));
        let cadence = crate::manager::Cadence {
            connect_backoff: vec![Duration::from_millis(10), Duration::from_millis(20)],
            reprobe: Duration::from_secs(3600),
            refresh: Duration::from_secs(3600),
            ..Default::default()
        };
        let manager = ConnectionManager::start_with_providers(
            store.clone(),
            harness.fleet.clone(),
            cadence,
            crate::environment::Providers::new(Arc::new(tensorlake.clone()), provider.clone()),
        )
        .await
        .unwrap();
        let service = ProvisioningService::production(
            store.clone(),
            manager.clone(),
            root.path().into(),
            sources,
        )
        .unwrap();
        let mut state = crate::AppState::with_provisioning(
            manager.clone(),
            store.clone(),
            service.clone(),
            harness.state.mode,
        );
        state.tensorlake = tensorlake;
        state.podman = Some(provider.clone());
        harness.manager = manager;
        harness.state = Arc::new(state);
        tokio::time::timeout(
            Duration::from_secs(10),
            harness
                .manager
                .wait_for_state(place, |state| state.is_connected()),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "ordinary place actor did not connect: {:?}",
                harness.manager.state(place)
            )
        })
        .expect("ordinary place actor remains registered");
        Self {
            commands,
            provider,
            place,
            store,
            harness,
            service,
            recipe,
            backend_root,
            _root: root,
        }
    }

    /// The public provider selection produces the private artifact. Tests
    /// cannot bypass source exclusion or final installer verification.
    pub(crate) async fn prepare(
        &self,
        progress: &dyn PreparationProgress,
    ) -> anyhow::Result<crate::environment::PreparedArtifact> {
        tokio::time::timeout(
            Duration::from_secs(10),
            self.provider.ensure_prepared(
                "dev",
                &self.recipe,
                farhelm_proto::Place::PodmanHost(self.place),
                self.harness.manager.events().clone(),
                progress,
            ),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "provider did not settle; phases {:?}; backend {:?}; containers {:?}",
                self.commands.phases(),
                self.backend_events(),
                self.commands.container_ids()
            )
        })
    }

    /// Resource ownership is read from the real recipe writer, not inferred
    /// from a command reply or from the fake's own in-memory container list.
    pub(crate) async fn recipe_state(&self) -> crate::environment::recipes::PodmanRecipeState {
        self.store
            .tensorlake_recipe_row("dev")
            .await
            .unwrap()
            .unwrap()
            .1
            .podman
            .get(&self.place)
            .cloned()
            .unwrap_or_default()
    }

    /// The deterministic canonical helper ID selects only one backend target:
    /// bootstrap=1, software=2, preparer=3, first session=4.
    pub(crate) fn fail_backend(&self, container: u64, event: &str) {
        let mut target = behavior();
        if event == "inspect" {
            target["inspect"] = "error".into();
        } else {
            target["fail_action"] = event.into();
        }
        let mut targets = serde_json::Map::new();
        targets.insert(format!("podman:{container:064x}"), target);
        let config = serde_json::json!({"default":behavior(),"targets":targets});
        std::fs::write(self.backend_root.join("config.json"), config.to_string()).unwrap();
    }

    /// Events come from the existing backend itself, independently of the
    /// Podman model's named phases, so reaching the installer is observable.
    pub(crate) fn backend_events(&self) -> Vec<String> {
        std::fs::read_to_string(self.backend_root.join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| {
                serde_json::from_str::<serde_json::Value>(line).unwrap()["event"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    }

    /// Repeated logind/installer names are not interchangeable. Assert the
    /// selected canonical helper reached the exact injected backend boundary.
    pub(crate) fn backend_reached(&self, container: u64, event: &str) -> bool {
        let expected = format!("podman:{container:064x}");
        std::fs::read_to_string(self.backend_root.join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .any(|entry| {
                entry["event"].as_str() == Some(event)
                    && entry["target"].as_str() == Some(expected.as_str())
            })
    }

    /// Retry through ordinary global deletion admission and both actual
    /// providers; no test erases ownership directly to manufacture cleanup.
    pub(crate) async fn clean_recipe(&self) -> anyhow::Result<()> {
        let guard = self
            .harness
            .state
            .recipe_builds()
            .begin_delete("dev")
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        self.harness
            .manager
            .providers()
            .clean_recipe("dev", &guard)
            .await
    }

    /// Keep the actual accepted request fields and trusted target assignment in
    /// start_session_environment. No alternate launch helper is under test.
    async fn start(
        &self,
    ) -> (
        super::super::http::RunAccepted,
        tokio::sync::oneshot::Receiver<anyhow::Result<farhelm_proto::SessionInfo>>,
    ) {
        let request = serde_json::from_value(serde_json::json!({
            "cwd":"~","command":{"command":"true","yolo":false}
        }))
        .unwrap();
        self.service
            .clone()
            .start_session_environment(
                self.harness.state.clone(),
                "dev".into(),
                farhelm_proto::Place::PodmanHost(self.place),
                request,
                None,
            )
            .await
            .unwrap()
    }

    /// Bound the owned task result while its supervisor peer is still alive.
    /// A timeout reports the actual phase, resource and actor premises rather
    /// than hiding a broken fixture behind a generic elapsed error.
    async fn finish(
        &self,
        host: HostId,
        result: tokio::sync::oneshot::Receiver<anyhow::Result<farhelm_proto::SessionInfo>>,
    ) -> anyhow::Result<farhelm_proto::SessionInfo> {
        match tokio::time::timeout(Duration::from_secs(10), result).await {
            Ok(result) => result.expect("the owned task must publish one result"),
            Err(_) => panic!(
                "owned launch did not finish; host {:?}; phases {:?}; backend {:?}; containers {:?}",
                self.harness.manager.state(host),
                self.commands.phases(),
                self.backend_events(),
                self.commands.container_ids()
            ),
        }
    }

    /// Wait for the run-send oracle before installing a spliced live peer.
    /// The row is already durable, while create remains blocked by our gate.
    async fn at_session_run(&self) -> HostId {
        let mut phases = self.commands.gate_reached();
        tokio::time::timeout(
            Duration::from_secs(10),
            phases.wait_for(|phases| phases.iter().any(|phase| phase == "session-run")),
        )
        .await
        .unwrap_or_else(|_| {
            panic!(
                "cold launch never reached session run; phases {:?}; backend {:?}; containers {:?}",
                self.commands.phases(),
                self.backend_events(),
                self.commands.container_ids()
            )
        })
        .unwrap();
        self.store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.is_session_owned())
            .unwrap()
            .id
    }

    /// Spliced supervisors handle real CreateSession frames. The harness owns
    /// heartbeat/list traffic; this peer owns exactly the initial create reply.
    fn peer(&self, host: HostId, refused: bool) -> tokio::task::JoinHandle<usize> {
        use farhelm_proto::ControlMsg;
        use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
        let (client, peer) = tokio::io::duplex(64 * 1024);
        self.harness.fleet.edit(host, |script| {
            *script = crate::rest_harness::HostScript {
                identity: Some("new-container-install".into()),
                peer: Some(client),
                ..Default::default()
            };
        });
        tokio::spawn(async move {
            let (read, write) = tokio::io::split(peer);
            let mut reader = FrameReader::new(read);
            let mut writer = FrameWriter::new(write);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let frame = reader
                .read_frame()
                .await
                .unwrap()
                .expect("the real launch sent CreateSession");
            let ControlMsg::CreateSession { req_id, .. } = parse_control(&frame).unwrap() else {
                panic!("unexpected first supervisor request");
            };
            let reply = if refused {
                ControlMsg::Error {
                    req_id,
                    kind: farhelm_proto::ErrorKind::InvalidRequest,
                    create_unaccepted: true,
                    message: "injected first session refusal".into(),
                }
            } else {
                ControlMsg::SessionCreated {
                    req_id,
                    session: crate::rest_harness::session("initial", 100),
                }
            };
            writer.write_control(&reply).await.unwrap();
            1
        })
    }
}

/// A full cold successful launch establishes every fixture prerequisite:
/// prepared publication, actual connection verdict, one supervisor create and
/// cached session ownership. A fresh clone must observe its boot-started user
/// manager before reading proxies, while leaving ordinary logind setup to the
/// image builder. The same normal entry point is used below.
#[farhelm_testtrace::test]
async fn cold_container_launch_success_establishes_real_task_and_session_premise() {
    let f = Fixture::new().await;
    f.commands.gate_session();
    let (accepted, result) = f.start().await;
    let host = f.at_session_run().await;
    assert_eq!(host, accepted.host_id);
    let peer = f.peer(host, false);
    f.commands.release_session();
    let session = f.finish(host, result).await.unwrap();
    assert!(session.id.ends_with(":initial"));
    assert_eq!(peer.await.unwrap(), 1);
    assert_eq!(f.commands.session_ids().len(), 1);
    assert_eq!(f.store.cached_sessions(host).await.unwrap().len(), 1);
    assert!(f.recipe_state().await.prepared.is_some());
    assert!(
        f.backend_events()
            .iter()
            .any(|event| event == "finish-snapshot-install")
    );
    let phases = f.commands.phases();
    let ready = phases
        .iter()
        .position(|phase| phase == "session-user-ready")
        .expect("the clone's user-manager readiness was observed");
    let proxies = phases
        .iter()
        .position(|phase| phase == "session-proxy-read")
        .expect("the clone's proxy environment was read");
    assert!(
        ready < proxies,
        "user-manager readiness must precede its proxy read"
    );
    assert!(
        !f.backend_reached(4, "prepare-sandbox"),
        "the prepared clone must boot its user manager without setup repairing it"
    );
    f.commands.assert_ownership_accounted().await;
}

/// A completed Podman refusal with confirmed absence settles the accepted
/// launch's cleanup immediately. The durable pending row must not outlive the
/// failure as if run's reply had been lost: that would make Delete wait for
/// the 15-minute uncertainty window despite a settled command outcome.
/// The normal finish helper bounds completion without advancing the clock;
/// row removal, not an elapsed-time comparison, proves cleanup succeeded.
#[farhelm_testtrace::test]
async fn normal_launch_removes_a_definitely_refused_absent_container_without_waiting() {
    use crate::environment::podman::failure_tests::AbsentRun;

    let f = Fixture::new().await;
    f.commands.gate_session();
    f.commands.absent_run("session-run", AbsentRun::Refused);
    let (accepted, result) = f.start().await;
    let host = f.at_session_run().await;
    assert_eq!(host, accepted.host_id);
    let row = f
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .expect("premise: accepted launch owns a durable row before run returns");
    let binding = row.podman.as_ref().expect("premise: a Podman launch");
    assert!(binding.pending_create_at.is_some());
    assert!(binding.container_id.is_none());
    assert!(f.recipe_state().await.prepared.is_some());
    assert!(f.commands.session_ids().is_empty());

    f.commands.release_session();
    let error = f.finish(host, result).await.unwrap_err();
    assert!(error.to_string().starts_with("Not launched."), "{error:#}");
    assert_eq!(f.commands.failed().as_deref(), Some("session-run"));
    assert!(
        f.commands.container_ids().is_empty(),
        "the refused run and completed preparation leave no container"
    );
    assert!(
        !f.store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .any(|row| row.id == host),
        "ordinary launch cleanup must remove the settled pending row immediately"
    );
    {
        let memory = f.service.memory.lock().await;
        // Successful row cleanup forgets its host-local progress as well.
        // The accepted task's Not launched result above carries the failure;
        // retaining a stale run here would describe a host that no longer exists.
        assert!(
            !memory.runs.contains_key(&host),
            "confirmed cleanup must forget the removed host's progress"
        );
        assert!(!memory.busy.contains(&host));
    }
    f.commands.assert_ownership_accounted().await;
}

/// Failures from provider preparation through session creation are owned by
/// the same launch task. A half-created helper/session must be removed by its
/// actual provider cleanup, then the fresh row may disappear. A clone whose
/// user manager is unavailable must fail before proxy access or attachment.
#[farhelm_testtrace::test]
async fn normal_launch_rolls_back_prepare_create_proxy_attach_and_session_failures() {
    for &phase in COMMAND_PHASES.iter().chain(
        [
            "session-run",
            "session-user-ready",
            "session-proxy-read",
            "session-proxy-write",
            "attach",
            "session-create",
        ]
        .iter(),
    ) {
        let f = Fixture::new().await;
        if phase == "attach" {
            f.fail_backend(4, "attach-supervisor");
        } else if phase != "session-create" {
            f.commands.fail(phase);
        }
        let needs_peer = phase == "session-create";
        if needs_peer {
            f.commands.gate_session();
        }
        let (accepted, result) = f.start().await;
        let peer = if needs_peer {
            let host = f.at_session_run().await;
            let peer = f.peer(host, true);
            f.commands.release_session();
            Some(peer)
        } else {
            None
        };
        let error = f.finish(accepted.host_id, result).await.unwrap_err();
        assert!(
            error.to_string().starts_with("Not launched."),
            "{phase}: {error:#}"
        );
        if let Some(peer) = peer {
            assert_eq!(peer.await.unwrap(), 1);
        }
        if phase == "attach" {
            assert!(f.backend_reached(4, "attach-supervisor"));
        } else if phase != "session-create" {
            assert_eq!(f.commands.failed().as_deref(), Some(phase));
        }
        if phase == "session-user-ready" {
            assert!(
                !f.commands
                    .phases()
                    .iter()
                    .any(|phase| phase == "session-proxy-read"),
                "a refused readiness gate must prevent proxy access"
            );
            assert!(
                !f.backend_reached(4, "attach-supervisor"),
                "a refused readiness gate must prevent attachment and session creation"
            );
        }
        assert!(
            f.commands.container_ids().is_empty(),
            "{phase}: cleanup left a container"
        );
        assert!(
            !f.store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == accepted.host_id),
            "{phase}: fresh row retained after confirmed absence"
        );
        assert!(
            !f.service
                .memory
                .lock()
                .await
                .busy
                .contains(&accepted.host_id)
        );
        f.commands.assert_ownership_accounted().await;
    }
}

/// Installer, login-manager and initial proxy activation failures must pass
/// through the normal accepted launch. A failed proxy install or supervisor
/// restart cannot admit the session container or attach its supervisor.
/// Confirmed cleanup permits the fresh pending row to be removed.
#[farhelm_testtrace::test]
async fn normal_launch_rolls_back_every_login_and_common_installer_boundary() {
    for &(container, event) in BACKEND_PHASES
        .iter()
        .chain([(4, "write-proxy"), (4, "restart-supervisor")].iter())
    {
        let f = Fixture::new().await;
        f.fail_backend(container, event);
        let (accepted, result) = f.start().await;
        let error = f.finish(accepted.host_id, result).await.unwrap_err();
        assert!(
            error.to_string().starts_with("Not launched."),
            "{container}/{event}: {error:#}"
        );
        assert!(
            f.backend_reached(container, event),
            "{container}/{event}: the requested boundary never ran"
        );
        if container == 4 {
            assert!(
                !f.backend_reached(container, "attach-supervisor"),
                "initial proxy activation must finish before attachment"
            );
        }
        assert!(
            f.commands.container_ids().is_empty(),
            "{container}/{event}: a helper survived confirmed cleanup"
        );
        assert!(
            !f.store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == accepted.host_id)
        );
        assert!(
            !f.service
                .memory
                .lock()
                .await
                .busy
                .contains(&accepted.host_id)
        );
        f.commands.assert_ownership_accounted().await;
    }
}

/// Client loss cannot cancel an accepted task. An unanswered absent session
/// run retains its pending row, and failed rm retains its canonical resource;
/// neither may be hidden by deleting the registry row or reporting success.
#[farhelm_testtrace::test]
async fn normal_launch_preserves_ambiguous_send_or_failed_cleanup_after_waiter_loss() {
    for ambiguous in [true, false] {
        let f = Fixture::new().await;
        if ambiguous {
            f.commands.lost_run(false);
        } else {
            f.commands.fail("session-proxy-write");
            f.commands.fail_cleanup();
        }
        let (accepted, result) = f.start().await;
        drop(result);
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if f.service.memory.lock().await.runs.get(&accepted.host_id).is_some_and(|run| run.status == super::super::http::RunStatus::Failed) { break; }
                tokio::task::yield_now().await;
            }
        }).await.unwrap_or_else(|_| panic!("owned failure never settled after waiter loss; host {:?}; phases {:?}; containers {:?}",
            f.harness.manager.state(accepted.host_id), f.commands.phases(), f.commands.container_ids()));
        let row = f
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == accepted.host_id)
            .expect("unconfirmed ownership retained");
        let binding = row.podman.as_ref().unwrap();
        if ambiguous {
            assert!(binding.pending_create_at.is_some() && binding.container_id.is_none());
            assert!(f.commands.session_ids().is_empty());
        } else {
            assert!(binding.container_id.is_some());
            assert!(
                f.commands
                    .session_ids()
                    .contains(binding.container_id.as_ref().unwrap())
            );
            let response = crate::environment::podman::list_leftovers(axum::extract::State(
                f.harness.state.clone(),
            ))
            .await;
            assert!(response.status().is_success());
            let body = axum::body::to_bytes(response.into_body(), 1 << 20)
                .await
                .unwrap();
            let entries: Vec<farhelm_proto::environment::ContainerLeftoverView> =
                serde_json::from_slice(&body).unwrap();
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.host == Some(accepted.host_id)
                        && Some(entry.id.as_str()) == binding.container_id.as_deref()),
                "retained incomplete row must be discoverable for confirmed Delete"
            );
        }
        assert!(
            !f.service
                .memory
                .lock()
                .await
                .busy
                .contains(&accepted.host_id)
        );
        f.commands.assert_ownership_accounted().await;
    }
}

/// A lost run reply with a visible owned container can still succeed. Once
/// canonical identity is observed, an explicit create recovery must keep that
/// same resource and must never issue a second run or any destructive cleanup.
#[farhelm_testtrace::test]
async fn lost_session_reply_recovers_canonical_identity_without_a_second_run() {
    let f = Fixture::new().await;
    let artifact = f.prepare(&Progress::default()).await.unwrap();
    let host = crate::environment::ProviderHandle::register_session(
        &f.store,
        "farhelm-sess-recovery".into(),
        "dev".into(),
        farhelm_proto::Place::PodmanHost(f.place),
        f.recipe.size,
    )
    .await
    .unwrap();
    let binding = f
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .unwrap()
        .environment_binding()
        .unwrap();
    f.commands.lost_run(true);
    f.provider
        .create(&binding, Some(artifact), &Progress::default())
        .await
        .unwrap();
    let row = f
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .unwrap();
    let first = row.podman.as_ref().unwrap().container_id.clone().unwrap();
    let runs = f.commands.run_count();
    let removed = f.commands.removed_containers();
    f.provider
        .create(
            &row.environment_binding().unwrap(),
            None,
            &Progress::default(),
        )
        .await
        .unwrap();
    assert_eq!(f.commands.run_count(), runs);
    assert_eq!(f.commands.removed_containers(), removed);
    assert_eq!(f.commands.session_ids(), vec![first]);
}

/// A payload staging failure belongs to common preparation, before any
/// supervisor attach. Its helper still needs confirmed provider cleanup.
#[farhelm_testtrace::test]
async fn missing_preparation_payload_refuses_publication_and_cleans_helper() {
    let f = Fixture::new().await;
    std::fs::remove_file(f.backend_root.join("ENABLED")).unwrap();
    let (accepted, result) = f.start().await;
    assert!(f.finish(accepted.host_id, result).await.is_err());
    assert!(f.backend_events().iter().any(|event| event == "inspect"));
    assert!(
        !f.backend_events()
            .iter()
            .any(|event| event == "upload-farhelm")
    );
    assert!(f.commands.container_ids().is_empty());
    assert!(f.recipe_state().await.prepared.is_none());
    assert!(
        !f.store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .any(|row| row.id == accepted.host_id)
    );
}
