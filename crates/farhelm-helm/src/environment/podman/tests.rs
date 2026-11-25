//! Failure-boundary tests execute the real provider over bounded fake commands.
//! No process environment, container engine or user manager is changed.

use super::*;
use crate::environment::podman_probe::{
    CONTAINER_PLACE_UNVERIFIED, PlaceCommands, TEST_PLACE_IDENTITY, parse_place_identity,
    simulate_verified_place,
};
use std::sync::Mutex;

const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MOVED_ENDPOINT: &str = "fe001122aabbccddeeff001122aabbcc\n1001\n";

/// The valid initial probe identity, distinct from the image login account.
fn original_endpoint() -> PodmanPlaceIdentity {
    parse_place_identity(TEST_PLACE_IDENTITY).unwrap()
}

/// The command model distinguishes an absent resource from an unanswered
/// command and records actual mutators, so a refusal cannot pass merely by
/// returning the same error text as a destructive implementation.
#[derive(Default)]
struct Model {
    /// Change the host behind the same registry destination. The cached
    /// probe is deliberately free to retain its old observation.
    place_identity: Option<&'static str>,
    /// Move the alias after the initial lookup, when the create shell starts.
    move_on_scoped_run: bool,
    /// The first probe still sees the original endpoint before the alias moves.
    move_after_identity_read: bool,
    exists: bool,
    status: String,
    helm: String,
    role: String,
    name: String,
    using_image: bool,
    image_exists: bool,
    lost_run: bool,
    helper_oracle: bool,
    /// The session's selected image facts must already be durable at run.
    pending_session: Option<(HostId, PreparedImage)>,
    /// The Podman subcommand (`ps`, `rm`) whose reply waits for
    /// [`Commands::release`], standing in for a container host that has
    /// stopped answering. It is recorded before the wait, so a test can tell
    /// a command that reached the host from one never sent.
    hold: Option<&'static str>,
    /// How long every `podman ps` takes to answer, on the test's clock: a slow
    /// but healthy container host, which a paused clock lets a test measure.
    slow_listing: Option<Duration>,
    commands: Vec<Vec<String>>,
}

/// The runner reads durable state at the send boundary, before returning its
/// scripted outcome. That oracle detects a name recorded after the send.
struct Commands {
    model: Mutex<Model>,
    store: HelmStore,
    place: HostId,
    /// Lets the held command (`Model::hold`) answer. A `Notify` keeps the
    /// permit if the release comes before the command reaches its wait.
    release: tokio::sync::Notify,
}

impl Commands {
    /// Read the registry at the requested boundary, including the pending
    /// binding before Podman has supplied a canonical container ID.
    async fn session_row(&self, host: HostId) -> crate::store::HostRow {
        let rows = self.store.list_hosts().await.unwrap();
        rows.into_iter()
            .find(|row| row.id == host)
            .expect("the session must still have its durable host row")
    }
}

#[async_trait::async_trait]
impl PlaceCommands for Commands {
    async fn run(&self, _: &BaseRoute, argv: &[String]) -> anyhow::Result<String> {
        use crate::environment::podman_probe::PLACE_IDENTITY_READ;

        Ok(match argv[0].as_str() {
            "uname" if argv[1] == "-s" => "Linux".into(),
            "uname" => "x86_64".into(),
            "sh" if argv[2] == PLACE_IDENTITY_READ => {
                let mut model = self.model.lock().unwrap();
                let checked = model.place_identity.unwrap_or(TEST_PLACE_IDENTITY);
                if std::mem::take(&mut model.move_after_identity_read) {
                    model.place_identity = Some(MOVED_ENDPOINT);
                }
                checked.into()
            }
            "sh" => "/usr/bin/podman".into(),
            "/usr/bin/podman" => serde_json::json!({"version":{"Version":"4.9.3"},
                "host":{"security":{"rootless":true},"cgroupVersion":"v2","cgroupManager":"systemd"}}).to_string(),
            "busctl" => r#"{"type":"as","data":[]}"#.into(),
            "systemd-run" | "systemctl" => String::new(),
            other => panic!("unexpected probe program {other}"),
        })
    }

    async fn run_bounded(
        &self,
        _: &BaseRoute,
        argv: &[String],
        bound: Duration,
    ) -> anyhow::Result<CommandResult> {
        {
            let mut model = self.model.lock().unwrap();
            if model.move_on_scoped_run && argv.iter().any(|arg| arg == "systemd-run") {
                model.move_on_scoped_run = false;
                model.place_identity = Some(MOVED_ENDPOINT);
            }
        }
        simulate_verified_place(argv, self.endpoint_identity(), |argv| {
            self.engine(argv, bound)
        })
        .await
    }
}

impl Commands {
    /// The command reads the current endpoint independently of its probe.
    fn endpoint_identity(&self) -> &'static str {
        self.model
            .lock()
            .unwrap()
            .place_identity
            .unwrap_or(TEST_PLACE_IDENTITY)
    }

    /// Only commands that pass verification reach the modeled engine.
    async fn engine(&self, argv: &[String], bound: Duration) -> anyhow::Result<CommandResult> {
        assert!(!bound.is_zero(), "every real provider command is bounded");
        let (helper_oracle, lost_run, pending_session) = {
            let mut model = self.model.lock().unwrap();
            model.commands.push(argv.to_vec());
            (
                model.helper_oracle,
                model.lost_run,
                model.pending_session.clone(),
            )
        };
        if argv[0] == "systemd-run" {
            assert!(argv.iter().any(|arg| arg == "/usr/bin/podman"));
            assert!(argv.iter().any(|arg| arg == "--scope"));
            assert!(argv.iter().any(|arg| arg == "--collect"));
            if helper_oracle {
                let offset = argv
                    .iter()
                    .position(|arg| arg == "--name")
                    .expect("helper run");
                let (_, state) = self.store.tensorlake_recipe_row("dev").await?.unwrap();
                let state = &state.podman[&self.place];
                assert_eq!(
                    state.builder_container_id.as_deref(),
                    Some(argv[offset + 1].as_str()),
                    "the durable name must precede the first run send"
                );
                assert!(
                    state.build_started_at.is_some(),
                    "an interrupted send needs its settle marker"
                );
            }
            if lost_run {
                // A lost reply can require recovery after a crash. Read the
                // session's facts from the store at send, before any reply.
                if let Some((host, image)) = pending_session {
                    let offset = argv.iter().position(|arg| arg == "--name").unwrap();
                    let binding = self.session_row(host).await.podman.unwrap();
                    assert_eq!(binding.name, argv[offset + 1]);
                    assert_eq!(binding.login_user().unwrap(), image.login);
                    assert!(binding.pending_create_at.is_some());
                    assert_eq!(binding.place_identity, Some(original_endpoint()));
                    assert_eq!(
                        self.store.podman_prepared_install(host).await.unwrap(),
                        Some(image.facts)
                    );
                }
                anyhow::bail!("scripted lost run reply");
            }
            return Ok(output(0, ID));
        }
        assert_eq!(
            argv[0], "/usr/bin/podman",
            "private absolute probe path is the only executable"
        );
        if argv[1] == "pull" {
            let (_, state) = self.store.tensorlake_recipe_row("dev").await?.unwrap();
            assert!(state.podman[&self.place].builder_container_id.is_some());
            return Ok(output(125, ""));
        }
        let (held, slow) = {
            let model = self.model.lock().unwrap();
            (model.hold, model.slow_listing)
        };
        if held.is_some() && argv.get(1).map(String::as_str) == held {
            self.release.notified().await;
        }
        if let Some(slow) = slow
            && argv.get(1).is_some_and(|arg| arg == "ps")
        {
            // sleep-ok: the scripted host's answer time is the measured quantity, on the test's paused clock.
            tokio::time::sleep(slow).await;
        }
        let mut model = self.model.lock().unwrap();
        let args = &argv[1..];
        let projected = args
            .get(2..4)
            .is_some_and(|flags| flags == ["--format", CONTAINER_INSPECTION]);
        Ok(match (args[0].as_str(), args.get(1).map(String::as_str)) {
            ("container", Some("exists")) =>
                output(if model.place_identity.is_none() && model.exists { 0 } else { 1 }, ""),
            ("container", Some("inspect")) if projected => output(0, &serde_json::json!({
                "Id": ID, "Name": model.name, "Status": model.status,
                "Labels":{"farhelm.helm":model.helm,"farhelm.role":model.role,"farhelm.recipe":"dev"}
            }).to_string()),
            ("rm", _) => { model.exists = false; output(0, ID) },
            ("image", Some("exists")) =>
                output(if model.place_identity.is_none() && model.image_exists { 0 } else { 1 }, ""),
            ("image", Some("inspect")) => output(0, &serde_json::json!([{"Id":ID,"Config":{"Labels":{
                "farhelm.helm":model.helm,"farhelm.recipe":"dev","farhelm.artifact":"software"
            }}}]).to_string()),
            ("image", Some("rm")) => { model.image_exists = false; output(0, ID) },
            ("ps", _) if args.iter().any(|arg| arg == "--format=json") => {
                let listed = if model.exists {
                    serde_json::json!([{"Id": ID}]).to_string()
                } else {
                    "[]".to_owned()
                };
                output(0, &listed)
            }
            ("ps", _) => output(0, if model.using_image { ID } else { "" }),
            other => panic!("unexpected provider command {other:?}: {args:?}"),
        })
    }
}

/// Fake exits retain nonzero statuses so exists can distinguish absence.
fn output(code: i32, text: &str) -> CommandResult {
    CommandResult {
        code: Some(code),
        signal: None,
        stdout: text.as_bytes().to_vec(),
        stderr: String::new(),
    }
}

/// Installer work is outside these ownership tests. Calling it would mean
/// the tested boundary accidentally started an unrelated preparation engine.
struct NoInstaller;
#[async_trait::async_trait]
impl EnvironmentPreparer for NoInstaller {
    async fn prepare(
        &self,
        _: &RemoteRoute,
        _: std::path::PathBuf,
    ) -> anyhow::Result<crate::environment::recipes::PreparedInstallFacts> {
        panic!("this test must not install a supervisor")
    }
}

/// Creation-only tests consume an already selected artifact, so no preparation
/// may be revealed and no second progress engine is needed.
struct Progress;
#[async_trait::async_trait]
impl PreparationProgress for Progress {
    async fn needed(&self) {
        panic!("creation consumes its selected artifact without preparing another image")
    }
    async fn step(&self, _: String) {}
}

/// A complete catalog and warm route establish all local prerequisites. The
/// real shared registry is retained, so source/deletion admission is exercised.
async fn fixture() -> (tempfile::TempDir, Podman, Arc<Commands>, HostId) {
    let directory = tempfile::tempdir().unwrap();
    let store = HelmStore::open(&directory.path().join("helm.db"))
        .await
        .unwrap();
    let place = store
        .add_ssh_host("place.example", None, None)
        .await
        .unwrap();
    let (provider, commands) = fixture_on(store, place).await;
    provider
        .0
        .places
        .fresh(&provider.0.store, place)
        .await
        .unwrap();
    (directory, provider, commands, place)
}

/// Build the command model without probing, so the caller can observe
/// first contact or warm the route before testing a later boundary.
async fn fixture_on(store: HelmStore, place: HostId) -> (Podman, Arc<Commands>) {
    crate::tensorlake::recipes::tests_support::create(&store, "dev").await;
    let tag = crate::tensorlake::config::helm_tag(&store.registry_namespace().await.unwrap());
    let commands = Arc::new(Commands {
        store: store.clone(),
        place,
        model: Mutex::new(Model {
            helm: tag,
            role: "session".into(),
            status: "running".into(),
            name: "farhelm-sess-test".into(),
            ..Default::default()
        }),
        release: tokio::sync::Notify::new(),
    });
    let places = PodmanPlaces::with_commands(commands.clone());
    let delivery =
        crate::provisioning::credential_delivery::CredentialDelivery::detached(store.clone());
    let tensorlake = crate::tensorlake::Tensorlake::detached(store.clone());
    let provider = Podman::new(
        store,
        places,
        delivery,
        tensorlake.builds().clone(),
        Arc::new(NoInstaller),
    );
    (provider, commands)
}

/// An unanswered helper send retains its pre-send name. Retrying cleanup
/// cannot erase that ownership on a recent absent lookup, or delete a name
/// collision whose labels belong to another helm.
#[farhelm_testtrace::test]
async fn interrupted_helper_send_retains_name_and_revalidates_collision_labels() {
    let (_directory, provider, commands, place) = fixture().await;
    {
        let mut model = commands.model.lock().unwrap();
        model.helper_oracle = true;
        model.lost_run = true;
    }
    let (expected, _, ready) = provider.place(place).await.unwrap();
    let identity = ready.identity.as_ref().unwrap();
    assert!(
        provider
            .helper(
                "dev",
                &expected,
                identity,
                "build",
                HelperBase::Built("user/base")
            )
            .await
            .is_err()
    );
    let saved = provider.state("dev", place).await.unwrap();
    assert!(
        saved
            .builder_container_id
            .as_deref()
            .unwrap()
            .starts_with("farhelm-build-")
    );
    assert!(provider.reconcile_helpers("dev", place).await.is_err());
    {
        let mut model = commands.model.lock().unwrap();
        model.exists = true;
        model.helm = "another-helm".into();
        model.role = "build".into();
        model.name = saved.builder_container_id.clone().unwrap();
    }
    assert!(provider.reconcile_helpers("dev", place).await.is_err());
    assert_eq!(
        provider
            .state("dev", place)
            .await
            .unwrap()
            .builder_container_id,
        saved.builder_container_id
    );
    assert!(
        !commands
            .model
            .lock()
            .unwrap()
            .commands
            .iter()
            .any(|argv| argv.get(1).is_some_and(|arg| arg == "rm"))
    );
}

/// The selected login and install facts become durable before run. A lost
/// reply can be retried without creating a second container or selecting a
/// newer checkpoint; recent absence retains the original pending outcome.
#[farhelm_testtrace::test]
async fn lost_session_create_records_login_and_refuses_an_automatic_second_run() {
    let (_directory, provider, commands, place) = fixture().await;
    commands.model.lock().unwrap().lost_run = true;
    let host = provider
        .0
        .store
        .register_pending_session_container("farhelm-sess-pending".into(), "dev".into(), place)
        .await
        .unwrap();
    let row = commands.session_row(host).await;
    assert!(
        row.podman.as_ref().unwrap().login.is_none(),
        "pending registration invents no account"
    );
    let binding = row.environment_binding().unwrap();
    let facts = crate::environment::recipes::PreparedInstallFacts {
        home: "/home/ubuntu".into(),
        farhelm_path: "/home/ubuntu/.local/lib/farhelm/farhelm".into(),
        state_dir: "/home/ubuntu/.local/state/farhelm".into(),
        farhelm_version: farhelm_proto::BUILD_VERSION.into(),
    };
    let image = PreparedImage {
        id: ID.into(),
        prepared_at: now(),
        farhelm_version: farhelm_proto::BUILD_VERSION.into(),
        source_image: ID.into(),
        facts: facts.clone(),
        login: ContainerUser {
            name: "ubuntu".into(),
            home: "/home/ubuntu".into(),
            uid: 1000,
        },
    };
    commands.model.lock().unwrap().pending_session = Some((host, image.clone()));
    let guard = provider
        .0
        .builds
        .lock_podman_use("dev", place)
        .await
        .unwrap();
    assert!(
        provider
            .create(
                &binding,
                Some(PreparedArtifact::Podman(PreparedContainer {
                    image: image.clone(),
                    guard
                })),
                &Progress
            )
            .await
            .is_err()
    );
    let row = commands.session_row(host).await;
    assert_eq!(row.podman.as_ref().unwrap().login_user().unwrap().uid, 1000);
    assert!(row.podman.as_ref().unwrap().pending_create_at.is_some());
    assert_eq!(
        provider
            .0
            .store
            .podman_prepared_install(host)
            .await
            .unwrap(),
        Some(facts)
    );
    assert!(
        provider
            .create(&row.environment_binding().unwrap(), None, &Progress)
            .await
            .is_err()
    );
    // A retry past the uncertainty window can see absence at the original
    // endpoint. Move the alias only as the scoped create starts.
    assert!(
        provider
            .0
            .store
            .set_podman_pending_create(host, Some(0))
            .await
            .unwrap()
    );
    let row = commands.session_row(host).await;
    assert_eq!(row.podman.as_ref().unwrap().pending_create_at, Some(0));
    commands.model.lock().unwrap().move_on_scoped_run = true;
    let guard = provider
        .0
        .builds
        .lock_podman_use("dev", place)
        .await
        .unwrap();
    let error = provider
        .create(
            &row.environment_binding().unwrap(),
            Some(PreparedArtifact::Podman(PreparedContainer { image, guard })),
            &Progress,
        )
        .await
        .err()
        .expect("an explicit retry must not send the create to the new endpoint");
    assert_eq!(error.to_string(), CONTAINER_PLACE_UNVERIFIED);
    assert_eq!(commands.endpoint_identity(), MOVED_ENDPOINT);
    let pending = commands.session_row(host).await.podman.unwrap();
    assert_eq!(pending.place_identity, Some(original_endpoint()));
    assert!(
        pending.pending_create_at.is_some_and(|at| at > 0),
        "the send intent must have reached the store before the shell refused"
    );
    assert_eq!(
        commands
            .model
            .lock()
            .unwrap()
            .commands
            .iter()
            .filter(|argv| argv[0] == "systemd-run")
            .count(),
        1
    );
}

/// The new endpoint would report absence even though the original container
/// still exists. Status and sealed Delete must keep both the binding and its
/// cached session, then Retry can finish deletion on the restored endpoint.
#[farhelm_testtrace::test]
async fn moved_container_place_keeps_the_session_and_offers_delete_retry() {
    use crate::rest_harness::{FleetBuilder, HostScript, session};

    let (builder, place) = FleetBuilder::new()
        .await
        .ssh("container-place.test", HostScript::default())
        .await;
    let (provider, commands) = fixture_on(builder.store().clone(), place).await;
    provider
        .0
        .places
        .fresh(&provider.0.store, place)
        .await
        .unwrap();
    let harness = builder.podman(Arc::new(provider.clone())).start().await;
    let store = &harness.store;
    let host = store
        .register_pending_session_container("farhelm-sess-test".into(), "dev".into(), place)
        .await
        .unwrap();
    let row = commands.session_row(host).await;
    let facts = crate::environment::recipes::PreparedInstallFacts {
        home: "/home/ubuntu".into(),
        farhelm_path: "/home/ubuntu/farhelm".into(),
        state_dir: "/home/ubuntu/state".into(),
        farhelm_version: farhelm_proto::BUILD_VERSION.into(),
    };
    assert!(
        store
            .set_podman_prepared_create(
                crate::store::PodmanInstallGuard::of(&row).unwrap(),
                now(),
                &facts,
                &ContainerUser {
                    name: "ubuntu".into(),
                    home: facts.home.to_str().unwrap().to_owned(),
                    uid: 1000
                },
                Some(&original_endpoint()),
            )
            .await
            .unwrap()
    );
    assert!(store.set_podman_container_id(host, ID).await.unwrap());
    let row = commands.session_row(host).await;
    assert!(
        store
            .admit_podman_install(
                crate::store::PodmanInstallGuard::of(&row).unwrap(),
                facts.farhelm_path.to_str().unwrap(),
                facts.state_dir.to_str().unwrap(),
            )
            .await
            .unwrap()
    );
    commands.model.lock().unwrap().exists = true;
    harness.fleet.edit(host, |script| {
        script.identity = Some("sandbox-install".into());
        script.sessions = vec![session("keep-me", 1_700_000_000)];
    });
    harness.manager.sync_registry().await.unwrap();
    harness.await_refreshed_as(host, "sandbox-install", 1).await;
    let row = commands.session_row(host).await;
    let binding = row.environment_binding().unwrap();
    assert_eq!(
        provider.status(&binding).await.unwrap(),
        EnvironmentStatus::Running
    );
    assert_eq!(store.cached_sessions(host).await.unwrap().len(), 1);

    commands.model.lock().unwrap().place_identity = Some(MOVED_ENDPOINT);
    assert!(
        provider
            .status(&binding)
            .await
            .unwrap_err()
            .0
            .contains(CONTAINER_PLACE_UNVERIFIED),
        "the new route cannot report the original container missing"
    );
    let whole = crate::hosts::remove_host(
        axum::extract::State(Arc::clone(&harness.state)),
        axum::extract::Path(host),
    )
    .await;
    assert!(!whole.status().is_success());
    let body = axum::body::to_bytes(whole.into_body(), 4096).await.unwrap();
    assert!(
        String::from_utf8_lossy(&body).contains(CONTAINER_PLACE_UNVERIFIED),
        "whole-container Delete must report the route issue and leave the entry available"
    );
    assert!(!harness.manager.sealed_failed(host));
    assert_eq!(
        commands
            .session_row(host)
            .await
            .podman
            .unwrap()
            .container_id
            .as_deref(),
        Some(ID)
    );
    assert_eq!(store.cached_sessions(host).await.unwrap().len(), 1);
    let provisioning = harness.manager.host_provision_lock(host).await;
    let error = crate::sessions::finish_seal_locked(&harness.state, host, provisioning)
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), CONTAINER_PLACE_UNVERIFIED);
    assert!(matches!(
        harness.manager.state(host),
        Some(crate::manager::HostState::Deleting { failure: Some(reason) })
            if reason.contains(CONTAINER_PLACE_UNVERIFIED)
    ));
    assert_eq!(
        commands
            .session_row(host)
            .await
            .podman
            .unwrap()
            .container_id
            .as_deref(),
        Some(ID)
    );
    assert_eq!(store.cached_sessions(host).await.unwrap().len(), 1);
    assert!(
        commands.model.lock().unwrap().exists,
        "the original container is still present"
    );

    commands.model.lock().unwrap().place_identity = None;
    let retry = crate::sessions::retry_environment_delete(
        axum::extract::State(Arc::clone(&harness.state)),
        axum::extract::Path(host),
    )
    .await;
    assert!(
        retry.status().is_success(),
        "Retry failed: {}",
        retry.status()
    );
    assert!(
        store
            .list_hosts()
            .await
            .unwrap()
            .iter()
            .all(|row| row.id != host)
    );
    assert!(!commands.model.lock().unwrap().exists);
}

/// A pre-upgrade binding on a twin remains usable for cleanup. Status
/// preserves stopped separately from paused, and repeating deletion after
/// absence performs no second rm.
#[farhelm_testtrace::test]
async fn status_and_deletion_preserve_stopped_paused_and_confirmed_absence() {
    let (_directory, provider, commands, _) = fixture().await;
    let place = provider
        .0
        .store
        .add_ssh_host("old-twin.example", None, None)
        .await
        .unwrap();
    assert!(
        provider
            .0
            .places
            .fresh(&provider.0.store, place)
            .await
            .err()
            .expect("twin must refuse new work")
            .to_string()
            .contains("same machine and account")
    );
    let host = provider
        .0
        .store
        .register_session_container(crate::store::NewPodmanHost {
            name: "farhelm-sess-test".into(),
            recipe: "dev".into(),
            place_host: place,
            user: "ubuntu".into(),
            home: "/home/ubuntu".into(),
            uid: 1000,
        })
        .await
        .unwrap();
    provider
        .0
        .store
        .set_podman_container_id(host, ID)
        .await
        .unwrap();
    let row = provider
        .0
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .unwrap();
    assert!(row.podman.as_ref().unwrap().place_identity.is_none());
    let binding = row.environment_binding().unwrap();
    {
        let mut model = commands.model.lock().unwrap();
        model.exists = true;
        model.status = "exited".into();
    }
    assert_eq!(
        provider.status(&binding).await.unwrap(),
        EnvironmentStatus::Stopped
    );
    commands.model.lock().unwrap().status = "paused".into();
    assert!(matches!(
        provider.status(&binding).await.unwrap(),
        EnvironmentStatus::Suspended { .. }
    ));
    // No pre-upgrade endpoint evidence exists. The binding keeps its
    // previous absence behavior even when the alias points elsewhere.
    commands.model.lock().unwrap().place_identity = Some(MOVED_ENDPOINT);
    assert!(matches!(
        provider.status(&binding).await.unwrap(),
        EnvironmentStatus::Missing(_)
    ));
    assert!(provider.destroy(&binding).await.is_ok());
    assert!(commands.model.lock().unwrap().exists);
    commands.model.lock().unwrap().place_identity = None;
    let proof = provider.destroy(&binding).await.unwrap();
    provider
        .0
        .store
        .remove_host_row(host, Some(&proof))
        .await
        .unwrap();
    provider.destroy(&binding).await.unwrap();
    assert_eq!(
        commands
            .model
            .lock()
            .unwrap()
            .commands
            .iter()
            .filter(|argv| argv.get(1).is_some_and(|arg| arg == "rm"))
            .count(),
        1
    );
}

/// Even an owned superseded image stays recorded while any container uses it.
/// Once unused, cleanup verifies labels and removes the image without forcing.
#[farhelm_testtrace::test]
async fn retired_images_wait_for_users_and_never_force_user_storage() {
    let (_directory, provider, commands, place) = fixture().await;
    provider
        .edit("dev", place, |state| state.retired_images.push(ID.into()))
        .await
        .unwrap();
    {
        let mut model = commands.model.lock().unwrap();
        model.image_exists = true;
        model.using_image = true;
    }
    provider.retire_images("dev", place).await.unwrap();
    assert_eq!(
        provider.state("dev", place).await.unwrap().retired_images,
        vec![ID.to_owned()]
    );
    commands.model.lock().unwrap().using_image = false;
    provider.retire_images("dev", place).await.unwrap();
    assert!(
        provider
            .state("dev", place)
            .await
            .unwrap()
            .retired_images
            .is_empty()
    );
    let model = commands.model.lock().unwrap();
    let removal = model
        .commands
        .iter()
        .find(|argv| {
            argv.get(1).is_some_and(|arg| arg == "image")
                && argv.get(2).is_some_and(|arg| arg == "rm")
        })
        .unwrap();
    assert!(!removal.iter().any(|arg| arg == "--force" || arg == "-f"));
    assert_eq!(removal.last().unwrap(), ID);
}

/// Retiring an unused owned image must keep its dangling parents. An untagged
/// base selected by ID can lose its last reference when its child is removed.
#[farhelm_testtrace::test]
async fn retiring_owned_images_preserves_dangling_parents() {
    let (_directory, provider, commands, place) = fixture().await;
    provider
        .edit("dev", place, |state| state.retired_images.push(ID.into()))
        .await
        .unwrap();
    commands.model.lock().unwrap().image_exists = true;
    assert_eq!(
        provider.state("dev", place).await.unwrap().retired_images,
        [ID]
    );

    provider.retire_images("dev", place).await.unwrap();

    let model = commands.model.lock().unwrap();
    let removal = model
        .commands
        .iter()
        .find(|argv| argv[1..].starts_with(&["image".into(), "rm".into()]))
        .expect("the retired image must be removed");
    assert_eq!(&removal[1..], ["image", "rm", "--no-prune", "--", ID]);
}

/// An absent tag cannot settle a disconnected commit that may still publish.
/// Once the image appears, cleanup must pin its canonical ID and confirm its
/// removal before releasing the same retained ownership record.
/// Confirmed Delete still needs ownership and usage checks.
#[farhelm_testtrace::test]
async fn unresolved_commit_absence_retains_ownership_until_positive_identity() {
    let (_directory, provider, commands, place) = fixture().await;
    let pending = "localhost/farhelm-pending:commit";
    provider
        .edit("dev", place, move |state| {
            state.retired_images.push(pending.into())
        })
        .await
        .unwrap();
    assert!(
        !commands.model.lock().unwrap().image_exists,
        "premise: the commit has no observed image yet"
    );
    assert!(provider.retire_images("dev", place).await.is_err());
    assert_eq!(
        provider.state("dev", place).await.unwrap().retired_images,
        [pending]
    );
    let guard = provider.0.builds.begin_delete("dev").unwrap();
    let guard = guard.with_missing_capture_consent(true);
    let helm = provider.tag().await.unwrap();
    for (owner, using, retained) in [("foreign", false, pending), (helm.as_str(), true, ID)] {
        {
            let mut model = commands.model.lock().unwrap();
            model.image_exists = true;
            model.helm = owner.into();
            model.using_image = using;
        }
        provider.clean_images("dev", &guard).await.unwrap_err();
        let saved = provider.state("dev", place).await.unwrap();
        assert_eq!(saved.retired_images, [retained]);
        assert!(commands.model.lock().unwrap().image_exists);
    }
    commands.model.lock().unwrap().using_image = false;
    provider.clean_images("dev", &guard).await.unwrap();
    assert!(
        provider
            .state("dev", place)
            .await
            .unwrap()
            .retired_images
            .is_empty()
    );
    let model = commands.model.lock().unwrap();
    let removal = model
        .commands
        .iter()
        .find(|argv| {
            argv.get(1).is_some_and(|arg| arg == "image")
                && argv.get(2).is_some_and(|arg| arg == "rm")
        })
        .unwrap();
    assert_eq!(
        removal.last().unwrap(),
        ID,
        "the random tag is never destructive authority"
    );
    assert!(
        !model.image_exists,
        "the canonical image is confirmed absent"
    );
}

/// A failed build step must still say why it stopped: SPEC.md has the build
/// name the base's missing requirement, and a failed package install is only
/// diagnosable from its last lines. The script runs with the container
/// host's proxies exported, so whatever a tool echoes of them (the whole URL,
/// or just its credentials) must be masked before the text reaches the host's
/// details, while the rest of the message stays readable.
#[test]
fn a_failed_setup_script_shows_its_error_with_proxy_values_masked() {
    let proxies = BTreeMap::from([
        (
            "https_proxy".to_string(),
            "http://builder:hunter22@proxy.example:3128".to_string(),
        ),
        ("no_proxy".to_string(), String::new()),
    ]);
    let missing = super::super::script_failure_output("the base is missing curl\n", &proxies);
    assert_eq!(missing, "the base is missing curl");

    let echoed = super::super::script_failure_output(
        "E: Failed to fetch via http://builder:hunter22@proxy.example:3128\n\
         proxy auth builder:hunter22 refused; password hunter22\n",
        &proxies,
    );
    for secret in ["hunter22", "builder:", "proxy.example:3128"] {
        assert!(!echoed.contains(secret), "{secret:?} leaked: {echoed}");
    }
    assert!(echoed.contains("E: Failed to fetch via"), "{echoed}");
    assert!(echoed.contains("refused; password"), "{echoed}");
}

/// environment.d expansion and shell parsing have different escaping rules.
/// Empty and absent proxies remain distinct, and literal dollars/newlines
/// must survive into the login manager rather than expand in the container.
#[test]
fn proxy_writers_preserve_literal_values_and_omit_absent_names() {
    let values = BTreeMap::from([
        ("HTTP_PROXY".into(), String::new()),
        (
            "https_proxy".into(),
            "https://u:p a'$x\\\"@proxy.example/\nline".into(),
        ),
    ]);
    let file = proxy_file(&values);
    assert_eq!(
        file,
        r#"HTTP_PROXY=""
https_proxy="https://u:p a'$$x\\\"@proxy.example/
line"
"#
    );
    assert!(file.contains("HTTP_PROXY=\"\"\n"));
    assert!(file.contains("$$x"));
    assert!(file.contains("\nline\""));
    assert!(!file.contains("no_proxy="));
    let exports = proxy_exports(&values);
    assert!(exports.starts_with("unset http_proxy https_proxy"));
    assert!(
        exports.contains("'\\''"),
        "single quotes require shell escaping"
    );
    assert!(
        exports.contains("$x"),
        "literal dollar data must not be expanded while writing the script"
    );
}

/// The production input runner and private-script wrapper must deliver exact
/// proxy bytes, preserve absent versus empty, and delete the sensitive script
/// on both successful and failed execution. No container or user manager is
/// needed to exercise that byte/cleanup boundary.
#[farhelm_testtrace::test]
async fn private_script_input_preserves_proxy_bytes_and_cleans_its_file() {
    let directory = tempfile::tempdir().unwrap();
    let secret = "https://u:stdin-private-marker a'$x\\\"@proxy.example/\nline";
    let values = BTreeMap::from([
        ("https_proxy".into(), secret.into()),
        ("HTTP_PROXY".into(), String::new()),
    ]);
    let argv = vec!["sh".into(), "-c".into(), PRIVATE_SCRIPT.into()];
    let description = crate::environment::podman_probe::lowered_command_for_test(
        directory.path(),
        &BaseRoute::Local,
        &argv,
    )
    .await
    .unwrap();
    assert!(
        description
            .args
            .iter()
            .all(|arg| !arg.contains("stdin-private-marker"))
    );
    for status in [0, 9] {
        let script = format!(
            "{}\nprintf '%s\\0' \"$https_proxy\" \"${{HTTP_PROXY+present}}\" \"$HTTP_PROXY\" \"${{no_proxy+present}}\" \"$0\"\nexit {status}\n",
            proxy_exports(&values),
        );
        let output = crate::environment::podman_probe::private_input_for_test(
            directory.path(),
            &argv,
            script.as_bytes(),
        )
        .await
        .unwrap();
        assert_eq!(output.code, Some(status));
        assert_eq!(output.signal, None);
        let fields = output.stdout.split(|byte| *byte == 0).collect::<Vec<_>>();
        assert_eq!(
            fields.len(),
            6,
            "the script must reach its post-export observation"
        );
        assert_eq!(fields[0], secret.as_bytes());
        assert_eq!(fields[1], b"present");
        assert_eq!(fields[2], b"");
        assert_eq!(fields[3], b"");
        let temporary = std::path::Path::new(std::str::from_utf8(fields[4]).unwrap());
        assert!(
            temporary.is_absolute(),
            "the wrapper must have created its private file"
        );
        assert!(
            !temporary.exists(),
            "a completed wrapper must leave no sensitive script, even on failure"
        );
        assert!(output.stderr.is_empty());
    }
}

/// Pause and stop preserve different resources: Resume must continue a paused
/// command and start a stopped machine. Already-running Resume stays a no-op.
#[test]
fn lifecycle_commands_distinguish_frozen_processes_from_stopped_filesystems() {
    assert_eq!(lifecycle_verb("running", false).unwrap(), "pause");
    assert_eq!(lifecycle_verb("paused", true).unwrap(), "unpause");
    assert_eq!(lifecycle_verb("paused", false).unwrap(), "");
    assert_eq!(lifecycle_verb("running", true).unwrap(), "");
    for status in ["exited", "stopped", "created"] {
        assert_eq!(lifecycle_verb(status, true).unwrap(), "start");
        assert!(lifecycle_verb(status, false).is_err());
    }
    assert!(lifecycle_verb("removing", true).is_err());
}

/// A helm interrupted before first session creation still has a real cleanup
/// owner. Listing includes that row; the rowless delete adapter cannot bypass
/// its normal actor hold and confirmed host-removal path.
#[farhelm_testtrace::test]
async fn incomplete_session_rows_are_listed_and_refuse_rowless_cleanup() {
    let (_directory, provider, commands, place) = fixture().await;
    let host = provider
        .0
        .store
        .register_session_container(crate::store::NewPodmanHost {
            name: "farhelm-sess-test".into(),
            recipe: "dev".into(),
            place_host: place,
            user: "ubuntu".into(),
            home: "/home/ubuntu".into(),
            uid: 1000,
        })
        .await
        .unwrap();
    provider
        .0
        .store
        .set_podman_container_id(host, ID)
        .await
        .unwrap();
    commands.model.lock().unwrap().exists = true;
    assert!(
        provider
            .0
            .store
            .cached_sessions(host)
            .await
            .unwrap()
            .is_empty()
    );
    let listed = provider
        .leftovers(std::future::ready(HashSet::new()))
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].host, Some(host));
    assert_eq!(listed[0].id, ID);
    assert!(provider.delete_leftover(place, ID).await.is_err());
    assert!(
        !commands
            .model
            .lock()
            .unwrap()
            .commands
            .iter()
            .any(|argv| argv.get(1).is_some_and(|arg| arg == "rm"))
    );
}

/// Two SSH entries can reach the same Podman storage. The second route must
/// neither offer a recorded container as an orphan nor send `rm` for it, even
/// if the container now has a different name than the one reserved at create.
#[farhelm_testtrace::test]
async fn a_second_ssh_route_cannot_list_or_delete_a_recorded_container_as_an_orphan() {
    let (_directory, provider, commands, place) = fixture().await;
    let store = &provider.0.store;
    let second = store
        .add_ssh_host("second.example", None, None)
        .await
        .unwrap();
    let refusal = provider
        .0
        .places
        .fresh(store, second)
        .await
        .err()
        .expect("a twin refuses new work");
    assert!(refusal.to_string().contains("same machine and account"));
    let host = store
        .register_session_container(crate::store::NewPodmanHost {
            name: "farhelm-sess-original".into(),
            recipe: "dev".into(),
            place_host: place,
            user: "ubuntu".into(),
            home: "/home/ubuntu".into(),
            uid: 1000,
        })
        .await
        .unwrap();
    assert!(store.set_podman_container_id(host, ID).await.unwrap());
    commands.model.lock().unwrap().exists = true;
    for route in [place, second] {
        let owned = provider.list_owned(Place::PodmanHost(route)).await.unwrap();
        assert_eq!(owned.len(), 1);
        assert_eq!(owned[0].id, ID);
        assert_eq!(owned[0].name, "farhelm-sess-test");
    }

    let listed = provider
        .leftovers(std::future::ready(HashSet::new()))
        .await
        .unwrap();
    // The first route still offers it once, as the host's own (row-backed) leftover, so hiding it from the twin does
    // not hide it everywhere.
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].place, Place::PodmanHost(place));
    assert_eq!(listed[0].host, Some(host));
    let deletion = provider.delete_leftover(second, ID).await;
    let sent_rm = commands
        .model
        .lock()
        .unwrap()
        .commands
        .iter()
        .any(|argv| is_podman(argv, "rm"));
    assert!(
        !listed
            .iter()
            .any(|row| row.place == Place::PodmanHost(second)),
        "the second SSH place listed the container, delete returned {deletion:?}, rm sent: {sent_rm}"
    );
    assert!(!sent_rm, "orphan deletion must leave the container intact");
    assert_eq!(
        deletion.unwrap_err().to_string(),
        "this container belongs to a host or recipe on this helm; delete it there"
    );
}

/// Catalog deletion closes the same registry used by lazy preparation and
/// Rebuild. Cancellation waits for an admitted command boundary instead of
/// aborting a task while its provider mutation may still be running.
#[farhelm_testtrace::test]
async fn shared_admission_joins_lazy_work_and_deletion_waits_for_its_boundary() {
    let (_directory, provider, _commands, place) = fixture().await;
    let builds = provider.0.builds.clone();
    let events = Arc::new(crate::feed::FleetEvents::new());
    let (entered, at_boundary) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let first = builds
        .start_podman(
            "dev",
            place,
            false,
            events.clone(),
            move |_, cancel| async move {
                entered.send(cancel.clone()).unwrap();
                released.await.unwrap();
                check(&cancel)?;
                panic!("deletion must cancel before another command begins")
            },
        )
        .unwrap();
    let cancelled = at_boundary.await.unwrap();
    let joined = builds
        .start_podman("dev", place, false, events.clone(), |_, _| async {
            panic!("joined work must not start another task")
        })
        .unwrap();
    assert!(
        first.same_channel(&joined),
        "both launches observe the admitted job"
    );
    assert!(
        builds
            .start_podman("dev", place, true, events.clone(), |_, _| async {
                panic!("Rebuild cannot join a lazy task")
            })
            .is_err()
    );
    let guard = Arc::new(builds.begin_delete("dev").unwrap());
    assert!(builds.lock_podman_use("dev", place).await.is_err());
    assert!(
        builds
            .start_podman("dev", place, false, events.clone(), |_, _| async {
                panic!("deletion excludes new work")
            })
            .is_err()
    );
    let owner = builds.clone();
    let deletion = guard.clone();
    let cleanup = tokio::spawn(async move { owner.cancel_podman("dev", &deletion).await });
    tokio::time::timeout(Duration::from_secs(1), async {
        while !cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancellation admission is visible at the owned command boundary");
    assert!(
        !cleanup.is_finished(),
        "the owned command has not reached its next boundary"
    );
    release.send(()).unwrap();
    cleanup.await.unwrap().unwrap();
    assert!(builds.podman_job("dev", place).is_none());
    drop(guard);
    assert!(
        builds.lock_podman_use("dev", place).await.is_ok(),
        "failed cleanup does not permanently close future explicit retries"
    );
}

/// A place's saved images are ownership even with no session rows. Removing
/// or retargeting that endpoint would make confirmed cleanup impossible.
#[farhelm_testtrace::test]
async fn recipe_artifacts_keep_their_place_reachable_until_cleanup() {
    let (_directory, provider, _commands, place) = fixture().await;
    provider
        .edit("dev", place, |state| state.retired_images.push(ID.into()))
        .await
        .unwrap();
    for attempt in [
        provider.0.store.remove_ssh_host(place).await,
        provider
            .0
            .store
            .update_ssh_destination(place, "replacement.example")
            .await,
    ] {
        let reason = attempt.unwrap_err().to_string();
        assert!(
            reason.contains("dev") && reason.contains("clean up"),
            "{reason}"
        );
    }
    assert_eq!(
        provider
            .0
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == place)
            .unwrap()
            .destination
            .as_deref(),
        Some("place.example")
    );
}

/// Before a first helper exists, a place edit may legitimately win. The
/// ownership transaction must then reject the exact row paired with the
/// earlier probe, including replacement at the same destination or a twin
/// taking an unused claim, before run.
#[farhelm_testtrace::test]
async fn first_helper_write_refuses_removed_retargeted_or_replaced_probe_place() {
    for boundary in ["removed", "retargeted", "replaced", "claim transferred"] {
        let (_directory, provider, commands, place) = fixture().await;
        let (expected, _, ready) = provider.place(place).await.unwrap();
        let identity = ready.identity.as_ref().unwrap();
        assert!(
            provider
                .state("dev", place)
                .await
                .unwrap()
                .builder_container_id
                .is_none(),
            "premise: no ownership yet prevents this edit from winning"
        );
        match boundary {
            "removed" => {
                provider.0.store.remove_ssh_host(place).await.unwrap();
            }
            "retargeted" => {
                provider
                    .0
                    .store
                    .update_ssh_destination(place, "replacement.example")
                    .await
                    .unwrap();
            }
            "replaced" => {
                provider
                    .0
                    .store
                    .conn()
                    .call("replace fixture place identity", move |conn| {
                        conn.execute(
                            "UPDATE hosts SET entry_uuid = ?2 WHERE id = ?1",
                            rusqlite::params![place, uuid::Uuid::new_v4().to_string()],
                        )?;
                        Ok(())
                    })
                    .await
                    .unwrap();
            }
            "claim transferred" => {
                use crate::store::{DialedAs, PlaceClaim};

                let store = &provider.0.store;
                let twin = store
                    .add_ssh_host("twin.example", None, None)
                    .await
                    .unwrap();
                let row = store
                    .list_hosts()
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|row| row.id == twin)
                    .unwrap();
                store
                    .record_first_contact(twin, &DialedAs::of(&row), "accepted-supervisor")
                    .await
                    .unwrap();
                let moved = store
                    .claim_podman_place(
                        &row,
                        identity.clone(),
                        Some("accepted-supervisor".into()),
                        None,
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    moved,
                    Some(PlaceClaim::Owned {
                        displaced: Some(place)
                    })
                );
            }
            _ => unreachable!(),
        }
        assert!(
            provider
                .helper(
                    "dev",
                    &expected,
                    identity,
                    "build",
                    HelperBase::Pull("user/base")
                )
                .await
                .is_err(),
            "{boundary}"
        );
        assert!(
            provider
                .state("dev", place)
                .await
                .unwrap()
                .builder_container_id
                .is_none(),
            "{boundary}"
        );
        assert!(
            !commands
                .model
                .lock()
                .unwrap()
                .commands
                .iter()
                .any(|argv| argv[0] == "systemd-run" || argv[1] == "pull"),
            "{boundary}"
        );
    }
}

/// A rejected base pull has reached Podman under a durable reservation.
/// Releasing the unsent helper keeps a known failure from blocking retries.
#[farhelm_testtrace::test]
async fn failed_base_pull_releases_unsent_helper() {
    let (_directory, provider, commands, place) = fixture().await;
    let (expected, _, ready) = provider.place(place).await.unwrap();
    assert!(
        provider
            .helper(
                "dev",
                &expected,
                ready.identity.as_ref().unwrap(),
                "build",
                HelperBase::Pull("user/base"),
            )
            .await
            .is_err()
    );
    let saved = provider.state("dev", place).await.unwrap();
    assert!(saved.builder_container_id.is_none() && saved.build_started_at.is_none());
    let model = commands.model.lock().unwrap();
    assert!(model.commands.iter().any(|argv| argv[1] == "pull"));
    assert!(!model.commands.iter().any(|argv| argv[0] == "systemd-run"));
}

/// The durable place claim survives an external SSH edit. A build must
/// avoid the other account's store, and cleanup there cannot forget the
/// original account's helpers or images.
#[farhelm_testtrace::test]
async fn moved_place_refuses_build_and_preserves_recipe_artifacts() {
    let (_directory, provider, commands, place) = fixture().await;
    let (expected, _, ready) = provider.place(place).await.unwrap();
    assert_eq!(ready.identity.as_ref(), Ok(&original_endpoint()));
    commands.model.lock().unwrap().place_identity = Some(MOVED_ENDPOINT);
    let error = provider
        .helper(
            "dev",
            &expected,
            ready.identity.as_ref().unwrap(),
            "build",
            HelperBase::Pull("user/base"),
        )
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), CONTAINER_PLACE_UNVERIFIED);
    assert!(
        provider
            .state("dev", place)
            .await
            .unwrap()
            .builder_container_id
            .is_none()
    );

    provider
        .edit("dev", place, |state| {
            state.builder_container_id = Some(ID.into());
            state.retired_images.push(ID.into());
        })
        .await
        .unwrap();
    assert_eq!(
        provider
            .reconcile_helpers("dev", place)
            .await
            .unwrap_err()
            .to_string(),
        CONTAINER_PLACE_UNVERIFIED
    );
    assert_eq!(
        provider
            .retire_images("dev", place)
            .await
            .unwrap_err()
            .to_string(),
        CONTAINER_PLACE_UNVERIFIED
    );
    let saved = provider.state("dev", place).await.unwrap();
    assert_eq!(saved.builder_container_id.as_deref(), Some(ID));
    assert_eq!(saved.retired_images, vec![ID]);
    assert!(commands.model.lock().unwrap().commands.is_empty());
}

/// The cleanup that triggers the first probe after an upgrade must use
/// the claim that probe establishes, even if the alias then moves.
#[farhelm_testtrace::test]
async fn cold_recipe_cleanup_uses_the_place_claim_it_establishes() {
    use crate::rest_harness::{FleetBuilder, HostScript};

    let (builder, place) = FleetBuilder::new()
        .await
        .ssh("cold-place.test", HostScript::default())
        .await;
    let store = builder.store();
    let (provider, commands) = fixture_on(store.clone(), place).await;
    provider
        .edit("dev", place, |state| state.retired_images.push(ID.into()))
        .await
        .unwrap();
    assert!(store.podman_place_identity(place).await.unwrap().is_none());
    {
        let mut model = commands.model.lock().unwrap();
        model.move_after_identity_read = true;
        model.image_exists = true;
    }

    let error = provider.retire_images("dev", place).await.unwrap_err();
    assert_eq!(error.to_string(), CONTAINER_PLACE_UNVERIFIED);
    assert_eq!(
        store.podman_place_identity(place).await.unwrap(),
        Some(original_endpoint())
    );
    assert_eq!(commands.endpoint_identity(), MOVED_ENDPOINT);
    assert_eq!(
        provider.state("dev", place).await.unwrap().retired_images,
        vec![ID]
    );
    assert!(commands.model.lock().unwrap().commands.is_empty());
}

// ---------------------------------------------------------------------------
// Settings → Containers' routes answer within the host reply bound
// ---------------------------------------------------------------------------

/// Whether `argv` is the Podman subcommand `subcommand` (`ps`, `rm`, or
/// `container` for `container exists` and `container inspect`).
fn is_podman(argv: &[String], subcommand: &str) -> bool {
    argv.first()
        .is_some_and(|program| program == "/usr/bin/podman")
        && argv.get(1).is_some_and(|arg| arg == subcommand)
}

/// The body of a reply, as text.
async fn body_text(response: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The GUI's channel admits one command at a time and resets the window when
/// one stays unanswered for 30 seconds, so the leftover listing must give up on
/// a container host that stopped answering instead of waiting out Podman's
/// own 30-second command bound. Specified: with the host's `podman ps` held
/// and the clock paused, the listing answers 504 at `HOST_REPLY_BOUND` (to the
/// timer's millisecond) with the shared "did not answer" words, and the held
/// `ps` was really sent, so the reply is the bound's and not an earlier
/// refusal.
#[farhelm_testtrace::test]
async fn a_leftover_listing_answers_at_the_reply_bound_when_its_host_stalls() {
    let (_directory, provider, commands, _place) = fixture().await;
    commands.model.lock().unwrap().hold = Some("ps");
    // Paused, the clock moves only when every task waits on a timer; the
    // store's work on the blocking pool holds it still until it is done.
    tokio::time::pause();
    let started = tokio::time::Instant::now();
    let response = crate::rest_harness::within_reply_bound(listing(
        Some(provider),
        std::future::ready(HashSet::new()),
    ))
    .await;
    let waited = started.elapsed();
    crate::rest_harness::assert_answered_at_reply_bound(waited, "the listing");
    assert!(
        commands
            .model
            .lock()
            .unwrap()
            .commands
            .iter()
            .any(|argv| is_podman(argv, "ps")),
        "premise: the listing reached the held host"
    );
    assert_eq!(response.status(), axum::http::StatusCode::GATEWAY_TIMEOUT);
    assert!(
        body_text(response)
            .await
            .contains("did not answer within 20 seconds")
    );
}

/// A leftover's Delete must answer within the bound too, and answering must
/// abandon only the wait: the user confirmed the removal, so it has to finish
/// on its own once the host answers, and the list's next read must find the
/// container gone. Specified: with an owned, unregistered container's
/// `podman rm` held and the clock paused, the Delete answers 504 at
/// `HOST_REPLY_BOUND` saying the cleanup carries on, while nothing has been
/// removed and the feed has not moved. Released, the removal runs `rm`, then
/// confirms the container's absence, and only then bumps the feed, which is
/// what makes the GUI's pushed state and a later read catch up.
#[farhelm_testtrace::test]
async fn a_leftover_delete_answers_at_the_reply_bound_and_its_removal_finishes_after() {
    let (_directory, provider, commands, place) = fixture().await;
    {
        let mut model = commands.model.lock().unwrap();
        model.exists = true;
        model.hold = Some("rm");
    }
    let events = Arc::new(FleetEvents::new());
    let before = events.revision();
    tokio::time::pause();
    let started = tokio::time::Instant::now();
    let response = crate::rest_harness::within_reply_bound(deleting(
        Some(provider),
        Arc::clone(&events),
        DeleteLeftover {
            place: Place::PodmanHost(place),
            id: ID.into(),
        },
    ))
    .await;
    let waited = started.elapsed();
    crate::rest_harness::assert_answered_at_reply_bound(waited, "the delete");
    assert_eq!(response.status(), axum::http::StatusCode::GATEWAY_TIMEOUT);
    assert!(body_text(response).await.contains("the cleanup carries on"));
    {
        let model = commands.model.lock().unwrap();
        assert!(
            model.commands.iter().any(|argv| is_podman(argv, "rm")),
            "premise: the removal reached the held host"
        );
        assert!(model.exists, "nothing is removed while the host is held");
    }
    assert_eq!(events.revision(), before, "no bump before the removal ends");

    let mut bumped = events.subscribe();
    commands.release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), bumped.changed())
        .await
        .expect("the removal finished and bumped the feed after its reply")
        .unwrap();
    let model = commands.model.lock().unwrap();
    assert!(!model.exists, "the container is gone");
    let removed = model
        .commands
        .iter()
        .position(|argv| is_podman(argv, "rm"))
        .unwrap();
    assert!(
        model.commands[removed + 1..]
            .iter()
            .any(|argv| is_podman(argv, "container") && argv[2] == "exists"),
        "absence was confirmed after the removal"
    );
}

/// The listing is bounded as a whole, so asking the container hosts one after another would make the bound cover the
/// sum of their answer times: a fleet of slow but healthy hosts would get a 504 on every read, and "Containers not on
/// this helm" could never show. Specified: with two container hosts whose `podman ps` each takes 15 seconds (together
/// longer than `HOST_REPLY_BOUND`) and the clock paused, the listing answers 200 after 15 seconds, the slowest host's
/// time, listing the owned container on both hosts.
#[farhelm_testtrace::test]
async fn the_listing_asks_its_container_hosts_concurrently() {
    let (_directory, provider, commands, place) = fixture().await;
    let second = provider
        .0
        .store
        .add_ssh_host("second.example", None, None)
        .await
        .unwrap();
    provider
        .0
        .places
        .runtime(&provider.0.store, second)
        .await
        .unwrap();
    let slow = Duration::from_secs(15);
    {
        let mut model = commands.model.lock().unwrap();
        model.exists = true;
        model.slow_listing = Some(slow);
    }
    assert!(
        slow < crate::sessions::HOST_REPLY_BOUND && slow * 2 > crate::sessions::HOST_REPLY_BOUND,
        "premise: each host answers within the bound, and the two together take longer"
    );
    tokio::time::pause();
    let started = tokio::time::Instant::now();
    let response = crate::rest_harness::within_reply_bound(listing(
        Some(provider),
        std::future::ready(HashSet::new()),
    ))
    .await;
    let waited = started.elapsed();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::OK,
        "answered in {waited:?}"
    );
    assert!(
        waited >= slow && waited < slow + Duration::from_secs(1),
        "the slowest host's time, not the sum: {waited:?}"
    );
    let listed: Vec<farhelm_proto::environment::ContainerLeftoverView> =
        serde_json::from_str(&body_text(response).await).unwrap();
    let mut places: Vec<_> = listed.iter().map(|row| row.place).collect();
    places.sort_by_key(|place| place.container_host());
    assert_eq!(
        places,
        [Place::PodmanHost(place), Place::PodmanHost(second)]
    );
}
