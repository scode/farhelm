//! Failure-boundary tests execute the real provider over bounded fake commands.
//! No process environment, container engine or user manager is changed.

use super::*;
use crate::environment::podman_probe::PlaceCommands;
use std::sync::Mutex;

const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// The command model distinguishes an absent resource from an unanswered
/// command and records actual mutators, so a refusal cannot pass merely by
/// returning the same error text as a destructive implementation.
#[derive(Default)]
struct Model {
    exists: bool,
    status: String,
    helm: String,
    role: String,
    name: String,
    using_image: bool,
    image_exists: bool,
    lost_run: bool,
    helper_oracle: bool,
    commands: Vec<Vec<String>>,
}

/// The runner reads durable state at the send boundary, before returning its
/// scripted outcome. That oracle detects a name recorded after the send.
struct Commands {
    model: Mutex<Model>,
    store: HelmStore,
    place: HostId,
}

#[async_trait::async_trait]
impl PlaceCommands for Commands {
    async fn run(&self, _: &BaseRoute, argv: &[String]) -> anyhow::Result<String> {
        Ok(match argv[0].as_str() {
            "uname" if argv[1] == "-s" => "Linux".into(),
            "uname" => "x86_64".into(),
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
        assert!(!bound.is_zero(), "every real provider command is bounded");
        let (helper_oracle, lost_run) = {
            let mut model = self.model.lock().unwrap();
            model.commands.push(argv.to_vec());
            (model.helper_oracle, model.lost_run)
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
                anyhow::bail!("scripted lost run reply");
            }
            return Ok(output(0, ID));
        }
        assert_eq!(
            argv[0], "/usr/bin/podman",
            "private absolute probe path is the only executable"
        );
        let mut model = self.model.lock().unwrap();
        let args = &argv[1..];
        Ok(match (args[0].as_str(), args.get(1).map(String::as_str)) {
            ("container", Some("exists")) => output(if model.exists { 0 } else { 1 }, ""),
            ("container", Some("inspect")) => output(0, &serde_json::json!([{
                "Id": ID, "Name": model.name, "State": {"Status": model.status},
                "Config": {"Labels":{"farhelm.helm":model.helm,"farhelm.role":model.role,"farhelm.recipe":"dev"}}
            }]).to_string()),
            ("rm", _) => { model.exists = false; output(0, ID) },
            ("image", Some("exists")) => output(if model.image_exists { 0 } else { 1 }, ""),
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
    });
    let places = PodmanPlaces::with_commands(commands.clone());
    places.fresh(&store, place).await.unwrap();
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
    (directory, provider, commands, place)
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
    let (expected, _, _) = provider.place(place).await.unwrap();
    assert!(
        provider
            .helper("dev", &expected, "build", "user/base", false)
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
    let row = provider
        .0
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .unwrap();
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
                Some(PreparedArtifact::Podman(PreparedContainer { image, guard })),
                &Progress
            )
            .await
            .is_err()
    );
    let row = provider
        .0
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .unwrap();
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

/// Status preserves stopped separately from paused, and explicit deletion
/// removes a paused canonical resource before minting a row-specific proof.
/// Repeating removal after absence performs no second rm.
#[farhelm_testtrace::test]
async fn status_and_deletion_preserve_stopped_paused_and_confirmed_absence() {
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
    let row = provider
        .0
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .unwrap();
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
/// Once unused, cleanup verifies labels and uses plain image rm, never force.
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

/// An absent tag cannot settle a disconnected commit that may still publish.
/// Once the image appears, cleanup must pin its canonical ID and confirm its
/// removal before releasing the same retained ownership record.
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
    commands.model.lock().unwrap().image_exists = true;
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
    let listed = provider.leftovers().await.unwrap();
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

/// Catalog deletion closes the same registry used by lazy preparation and
/// Rebuild. Cancellation waits for an admitted command boundary instead of
/// aborting a task while its provider mutation may still be running.
#[farhelm_testtrace::test]
async fn shared_admission_joins_lazy_work_and_deletion_waits_for_its_boundary() {
    let (_directory, provider, _commands, place) = fixture().await;
    let builds = provider.0.builds.clone();
    let (entered, at_boundary) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let first = builds
        .start_podman("dev", place, false, move |_, cancel| async move {
            entered.send(cancel.clone()).unwrap();
            released.await.unwrap();
            check(&cancel)?;
            panic!("deletion must cancel before another command begins")
        })
        .unwrap();
    let cancelled = at_boundary.await.unwrap();
    let joined = builds
        .start_podman("dev", place, false, |_, _| async {
            panic!("joined work must not start another task")
        })
        .unwrap();
    assert!(
        first.same_channel(&joined),
        "both launches observe the admitted job"
    );
    assert!(
        builds
            .start_podman("dev", place, true, |_, _| async {
                panic!("Rebuild cannot join a lazy task")
            })
            .is_err()
    );
    let guard = Arc::new(builds.begin_delete("dev").unwrap());
    assert!(builds.lock_podman_use("dev", place).await.is_err());
    assert!(
        builds
            .start_podman("dev", place, false, |_, _| async {
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
    assert!(!builds.podman_job_active("dev", place));
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
/// earlier probe, including replacement at the same destination, before run.
#[farhelm_testtrace::test]
async fn first_helper_write_refuses_removed_retargeted_or_replaced_probe_place() {
    for boundary in ["removed", "retargeted", "replaced"] {
        let (_directory, provider, commands, place) = fixture().await;
        let (expected, _, _) = provider.place(place).await.unwrap();
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
            _ => unreachable!(),
        }
        assert!(
            provider
                .helper("dev", &expected, "build", "user/base", false)
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
                .any(|argv| argv[0] == "systemd-run"),
            "{boundary}"
        );
    }
}
