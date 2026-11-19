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
        let mut state =
            crate::AppState::with_provisioning(manager.clone(), store.clone(), service.clone());
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

    /// The helm's state over this fixture's manager, provider and store, for
    /// tests that call a route's handler the way the router does.
    pub(crate) fn state(&self) -> Arc<crate::AppState> {
        self.harness.state.clone()
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

/// A cold container launch's progress says SPEC.md's container words
/// ("Container sessions"): **Preparing <recipe> on <host>…**, **Creating a
/// <recipe> container on <host>…** and **Starting Farhelm in the container…**,
/// with the place host named as Hosts shows it (its alias, not its
/// destination), under the step ids every provider shares. Why: the
/// launcher's pill shows these names as they are, and the shared launch used
/// to show provider-neutral ones and a "Prepare snapshot" step for a
/// container. Specified on the run's listed steps while its create is held at
/// the session run, after preparation has been needed.
#[farhelm_testtrace::test]
async fn cold_container_launch_progress_names_its_steps_in_container_words() {
    let f = Fixture::new().await;
    assert!(
        f.store
            .update_alias(f.place, Some("build-box"))
            .await
            .unwrap(),
        "premise: the place host's alias differs from its destination"
    );
    f.commands.gate_session();
    let (_, result) = f.start().await;
    let host = f.at_session_run().await;
    let runs = f.service.runs().await.runs;
    let run = runs.iter().find(|run| run.host_id == host).unwrap();
    let names: Vec<_> = run
        .steps
        .iter()
        .map(|step| (step.step.as_str(), step.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("build-snapshot", "Select the recipe source"),
            ("prepare-snapshot", "Preparing dev on build-box…"),
            ("create-sandbox", "Creating a dev container on build-box…"),
            (
                "configure-agents",
                "Configure credentials and agent endpoints"
            ),
            ("attach-supervisor", "Starting Farhelm in the container…"),
            ("create-session", "Launch the session"),
        ]
    );
    let peer = f.peer(host, false);
    f.commands.release_session();
    f.finish(host, result).await.unwrap();
    assert_eq!(peer.await.unwrap(), 1);
}

/// `POST /api/environment-sessions` at a container host: a 202 naming the
/// launch's host and run while its create is still held, then a refused first
/// session's "Not launched." reason on that run in the pushed host progress
/// (`ProvisioningService::runs`), still listed after cleanup removed the host.
/// Why: the Tensorlake tests cover the route at the account; this is the other
/// place, and the failure path a client can learn only from that kept run.
#[farhelm_testtrace::test]
async fn the_environment_route_answers_202_and_a_refused_launch_ends_its_run() {
    use super::super::http::RunStatus;
    let f = Fixture::new().await;
    f.commands.gate_session();
    let request = serde_json::from_value(serde_json::json!({
        "recipe": "dev", "place": {"kind": "podman_host", "host": f.place},
        "request": {"cwd": "~", "command": {"command": "true", "yolo": false}},
    }))
    .unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(10),
        crate::sessions::create_environment_session(
            axum::extract::State(f.state()),
            axum::Json(request),
        ),
    )
    .await
    .expect("admission answers while the launch's create is held");
    assert_eq!(response.status(), axum::http::StatusCode::ACCEPTED);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap(),
    )
    .unwrap();
    let run = body["run_id"].as_str().expect("the 202 names its run");
    let host = f.at_session_run().await;
    assert_eq!(body["host_id"], host, "the 202 names the launch's host");
    let peer = f.peer(host, true);
    f.commands.release_session();
    let outcome = f
        .service
        .wait_for_run_end(host, run, Duration::from_secs(10))
        .await
        .unwrap_or_else(|| panic!("the launch's run never ended: {:?}", f.commands.phases()));
    assert_eq!(peer.await.unwrap(), 1);
    assert_eq!(outcome.status, RunStatus::Failed, "{outcome:?}");
    let reason = outcome.message.as_deref().unwrap_or_default();
    assert!(
        reason.starts_with("Not launched.") && reason.contains("injected first session refusal"),
        "{reason}"
    );
    assert!(
        !f.store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .any(|row| row.id == host),
        "the failure is published once cleanup has removed the host"
    );
    f.commands.assert_ownership_accounted().await;
}

/// A launch task that panics still ends its run, failed with the words the
/// streamed reply once used for a task that ended without a result and marked
/// `outcome_unknown`, and lets go of the host's busy marker and of the hold
/// that showed the host as being set up. Why: the run is the accepted environment
/// create's only outcome, and without this a client waiting on it would wait
/// for good. The panic is injected before the launch's first step, the one
/// way a test can make that task panic.
#[farhelm_testtrace::test]
async fn a_launch_task_that_panics_ends_its_run_without_a_result() {
    use super::super::http::RunStatus;
    let f = Fixture::new().await;
    f.service
        .panic_session_launch
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let (accepted, result) = f.start().await;
    let outcome = f
        .service
        .wait_for_run_end(accepted.host_id, &accepted.run_id, Duration::from_secs(10))
        .await
        .expect("the panicked launch's run ends");
    assert_eq!(outcome.status, RunStatus::Failed, "{outcome:?}");
    assert_eq!(
        outcome.message.as_deref(),
        Some(super::tensorlake_add::LAUNCH_ENDED_WITHOUT_A_RESULT),
        "{outcome:?}"
    );
    assert!(outcome.outcome_unknown, "{outcome:?}");
    // The hold's state stays published until the actor's next publication
    // replaces it, so wait for that publication rather than read once.
    let setting_up = |state: &HostState| {
        matches!(state, HostState::Unreachable { last_error, .. }
            if last_error == "Farhelm is setting up this environment")
    };
    let state = tokio::time::timeout(
        Duration::from_secs(10),
        f.harness
            .manager
            .wait_for_state(accepted.host_id, |state| !setting_up(state)),
    )
    .await
    .unwrap_or_else(|_| {
        panic!(
            "the host still shows the setting-up hold: {:?}",
            f.harness.manager.state(accepted.host_id)
        )
    });
    // A wait that ended because the host's actor went away would also
    // pass the timeout; the hold must have been replaced by a real
    // publication.
    assert!(
        state.is_some(),
        "the host's state wait ended without a state"
    );
    assert!(
        !f.service
            .memory
            .lock()
            .await
            .busy
            .contains(&accepted.host_id)
    );
    assert!(
        result.await.is_err(),
        "the task left no result for an in-process waiter"
    );
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
        // The accepted task's Not launched result above carries the failure,
        // and so does the run's kept copy the host progress lists
        // (`ended_launches`); a run left here would describe a host that no
        // longer exists.
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
/// For that same empty container, the cleanup route distinguishes a running
/// first launch from a failed one by reading the helm's provisioning state.
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
            let list = || async {
                let response =
                    crate::environment::podman::list_leftovers(axum::extract::State(f.state()))
                        .await;
                assert!(response.status().is_success());
                let body = axum::body::to_bytes(response.into_body(), 1 << 20)
                    .await
                    .unwrap();
                serde_json::from_slice::<Vec<farhelm_proto::environment::ContainerLeftoverView>>(
                    &body,
                )
                .unwrap()
            };
            assert!(
                list()
                    .await
                    .iter()
                    .any(|entry| entry.host == Some(accepted.host_id)
                        && Some(entry.id.as_str()) == binding.container_id.as_deref()),
                "retained incomplete row must be discoverable for confirmed Delete"
            );
            // Hold everything the route sees constant apart from the phase
            // of the same Add run, as it would be before the first session.
            let mut run = f
                .service
                .runs()
                .await
                .runs
                .into_iter()
                .find(|run| run.host_id == accepted.host_id)
                .unwrap();
            run.status = RunStatus::Running;
            f.service.insert_run_for_test(run).await;
            assert!(
                list()
                    .await
                    .iter()
                    .all(|entry| entry.host != Some(accepted.host_id)),
                "a running first launch must keep its container out of the cleanup list"
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

/// Removing a container host deletes its container first, and each Podman
/// command may wait 30 seconds on the place host, so the GUI's ordered channel
/// needs the reply within the host reply bound while the user's confirmed
/// deletion still finishes. Specified: for a registered session container
/// whose `podman rm` is held, with the clock paused, `DELETE /api/hosts/{id}`
/// answers 504 at `HOST_REPLY_BOUND` (to the timer's millisecond) saying the
/// removal carries on, with the row still registered and the container still
/// there. Released, the removal deletes the container and forgets the row on
/// its own.
#[farhelm_testtrace::test]
async fn a_container_host_removal_answers_at_the_reply_bound_and_finishes_after() {
    const CONTAINER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let f = Fixture::new().await;
    let host = f
        .store
        .register_session_container(crate::store::NewPodmanHost {
            name: "farhelm-sess-removal".into(),
            recipe: "dev".into(),
            place_host: f.place,
            user: "ubuntu".into(),
            home: "/home/ubuntu".into(),
            uid: 1000,
        })
        .await
        .unwrap();
    assert!(
        f.store
            .set_podman_container_id(host, CONTAINER)
            .await
            .unwrap()
    );
    f.commands.plant_session(CONTAINER, "farhelm-sess-removal");
    f.commands.gate_removal();
    let registered = |f: &Fixture| {
        let store = f.store.clone();
        async move {
            store
                .list_hosts()
                .await
                .unwrap()
                .iter()
                .any(|row| row.id == host)
        }
    };

    tokio::time::pause();
    let started = tokio::time::Instant::now();
    // Bounded from outside too, so a removal that ignored its bound fails here with a message rather than waiting
    // for the held `rm` forever and hanging the test.
    let response = crate::rest_harness::within_reply_bound(crate::hosts::remove_host(
        axum::extract::State(f.state()),
        axum::extract::Path(host),
    ))
    .await;
    let waited = started.elapsed();
    crate::rest_harness::assert_answered_at_reply_bound(waited, "the removal");
    assert!(
        f.commands.removal_sent(),
        "premise: the removal reached the held host"
    );
    assert_eq!(response.status(), axum::http::StatusCode::GATEWAY_TIMEOUT);
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&body).contains("the removal carries on"));
    assert!(
        registered(&f).await,
        "the row stays while the container is held"
    );
    assert!(f.commands.container_ids().contains(&CONTAINER.to_owned()));

    f.commands.release_removal();
    // The clock runs again for the wait below: this loop never idles, so on a paused clock its timeout could never
    // fire and a removal that never finished would spin forever.
    tokio::time::resume();
    tokio::time::timeout(Duration::from_secs(10), async {
        while registered(&f).await {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the removal finished after its reply");
    assert_eq!(f.commands.removed_containers(), vec![CONTAINER.to_owned()]);
}
