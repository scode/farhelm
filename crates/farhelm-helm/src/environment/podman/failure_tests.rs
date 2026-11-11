//! Cold launches exercise the real provider and common installer over owned
//! command/backend fakes. Faults describe command boundaries, not alternate
//! implementations of preparation, creation, admission or cleanup.

use super::*;
use crate::environment::podman_probe::PlaceCommands;
use crate::provisioning::session_launch_failure_tests::Fixture;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::{Notify, watch};

/// Include shell metacharacters and a newline so a passing privacy test also
/// proves that exact inheritance was not replaced with lossy sanitization.
const PROXY_SECRET: &str = "https://u:private-proxy-marker a'$x\\\"@proxy.example/\nline";

/// These all leave the model absent, but only an own completed refusal can
/// settle that fact immediately. The other outcomes may still publish later.
#[derive(Clone, Copy, Debug)]
pub(crate) enum AbsentRun {
    Refused,
    SshDisconnected,
    Signalled,
    LostReply,
    InvalidReply,
    AbsenceUnanswered,
}

/// The same phase set is exercised directly and through the normal accepted
/// launch task, which additionally owns row rollback and the session result.
pub(crate) const COMMAND_PHASES: &[&str] = &[
    "pull-base",
    "bootstrap-run",
    "bootstrap",
    "bootstrap-stop",
    "bootstrap-commit",
    "software-run",
    "software-login",
    "readiness",
    "software-proxy-read",
    "software-proxy-directory",
    "software-proxy-write",
    "packages",
    "claude",
    "codex",
    "software-finalize",
    "software-stop",
    "software-commit",
    "prepare-run",
    "prepare-login",
    "prepare-proxy-read",
    "prepare-proxy-directory",
    "prepare-proxy-write",
    "prepare-stop",
    "prepare-commit",
];
/// Target identity separates software logind from preparer logind and proves
/// an injected installer failure did not merely hit an earlier same-name call.
pub(crate) const BACKEND_PHASES: &[(u64, &str)] = &[
    (2, "prepare-sandbox"),
    (2, "write-proxy"),
    (3, "prepare-sandbox"),
    (3, "write-proxy"),
    (3, "inspect"),
    (3, "check-supervisor-state"),
    (3, "create-directories"),
    (3, "upload-farhelm"),
    (3, "install-farhelm"),
    (3, "write-unit"),
    (3, "daemon-reload"),
    (3, "enable-supervisor"),
    (3, "finish-snapshot-install"),
];

/// Each container has its own canonical identity. Reusing one fixture ID
/// would hide a cleanup that accidentally removes a prior or foreign resource.
#[derive(Clone)]
struct Container {
    id: String,
    name: String,
    role: String,
    stage: String,
    image: String,
    status: String,
}

/// Committed images inherit the helper's ownership labels. Pulled bases have
/// no helm ownership and must survive every cleanup and retry.
#[derive(Clone)]
struct Image {
    id: String,
    artifact: String,
    owned: bool,
}

/// Command inputs and fault controls are independent of resource state. Cleanup
/// refusal arms only after the requested command failure, so a session fault
/// cannot accidentally fail an earlier successful helper's removal.
#[derive(Default)]
struct State {
    containers: HashMap<String, Container>,
    images: HashMap<String, Image>,
    image_tags: HashMap<String, String>,
    next_container: u64,
    next_image: u64,
    fail: Option<String>,
    failed: Option<String>,
    lost_absent_run: bool,
    lost_created_run: bool,
    cleanup_fails: bool,
    session_gate: bool,
    lost_commit: Option<String>,
    invalid_commit_reply: bool,
    absent_run: Option<(String, AbsentRun)>,
    fail_absence: bool,
    proxy: Option<BTreeMap<String, String>>,
    calls: Vec<Vec<String>>,
    inputs: Vec<(Vec<String>, Vec<u8>)>,
    removed_containers: Vec<String>,
    removed_images: Vec<String>,
}

/// Observe the named phase before the fake replies, and gate only a session
/// run when a test needs to install its supervisor peer after row registration.
/// No sleep or guessed actor timing establishes that boundary.
pub(crate) struct Commands {
    state: Mutex<State>,
    phases: watch::Sender<Vec<String>>,
    session_release: Notify,
    helm: String,
    store: HelmStore,
    place: HostId,
}

impl Commands {
    /// Bind the same real store/place as the selected provisioning backend.
    pub(crate) fn new(store: HelmStore, place: HostId, helm: String) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::default(),
            phases: watch::Sender::new(vec![]),
            session_release: Notify::new(),
            helm,
            store,
            place,
        })
    }

    /// Fault only once, so recovery and explicit cleanup exercise their real
    /// commands after the original failed operation has ended.
    pub(crate) fn fail(&self, phase: &str) {
        self.state.lock().unwrap().fail = Some(phase.into());
    }
    /// Select an absent outcome at one actual run boundary. Other phases keep
    /// their successful behavior, including the initial pre-send absence read.
    pub(crate) fn absent_run(&self, phase: &str, outcome: AbsentRun) {
        self.state.lock().unwrap().absent_run = Some((phase.into(), outcome));
    }
    /// Delay only session run; preparation still establishes its full premise.
    pub(crate) fn gate_session(&self) {
        self.state.lock().unwrap().session_gate = true;
    }
    /// Notify retains a permit when setup has not yet reached the gate await.
    pub(crate) fn release_session(&self) {
        self.session_release.notify_one();
    }
    /// Refuse removal only after the selected phase actually failed.
    pub(crate) fn fail_cleanup(&self) {
        self.state.lock().unwrap().cleanup_fails = true;
    }
    /// Model both unanswered outcomes, keeping visible and absent creation
    /// distinct so recovery cannot use one as proof of the other.
    pub(crate) fn lost_run(&self, created: bool) {
        let mut state = self.state.lock().unwrap();
        state.lost_created_run = created;
        state.lost_absent_run = !created;
    }
    // Observation snapshots never retain the model mutex across an await.
    pub(crate) fn phases(&self) -> Vec<String> {
        self.phases.borrow().clone()
    }
    pub(crate) fn failed(&self) -> Option<String> {
        self.state.lock().unwrap().failed.clone()
    }
    pub(crate) fn container_ids(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .containers
            .keys()
            .cloned()
            .collect()
    }
    pub(crate) fn session_ids(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .containers
            .values()
            .filter(|item| item.role == "session")
            .map(|item| item.id.clone())
            .collect()
    }
    pub(crate) fn removed_containers(&self) -> Vec<String> {
        self.state.lock().unwrap().removed_containers.clone()
    }
    pub(crate) fn run_count(&self) -> usize {
        self.state
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|argv| argv.iter().any(|arg| arg == "run"))
            .count()
    }
    pub(crate) fn gate_reached(&self) -> watch::Receiver<Vec<String>> {
        self.phases.subscribe()
    }

    /// Successful prerequisites and named command visits are distinct from
    /// mutation results. Matrix tests assert both the visit and its effect.
    fn hit(&self, phase: &str) -> bool {
        self.phases.send_modify(|phases| phases.push(phase.into()));
        let mut state = self.state.lock().unwrap();
        if state.fail.as_deref() == Some(phase) {
            state.fail = None;
            state.failed = Some(phase.into());
            true
        } else {
            false
        }
    }

    fn container(&self, target: &str) -> Option<Container> {
        self.state
            .lock()
            .unwrap()
            .containers
            .values()
            .find(|item| item.id == target || item.name == target)
            .cloned()
    }

    /// Tags can remain the only durable authority after a commit reply is
    /// lost. Inspect/exists resolve them without forgetting canonical identity.
    fn image(&self, target: &str) -> Option<Image> {
        let state = self.state.lock().unwrap();
        state
            .images
            .get(target)
            .or_else(|| state.images.get(&format!("sha256:{target}")))
            .or_else(|| {
                state
                    .image_tags
                    .get(target)
                    .and_then(|id| state.images.get(id))
            })
            .cloned()
    }

    /// All model-owned images must remain in durable provider state until a
    /// confirmed removal. A base is deliberately excluded from this invariant.
    pub(crate) async fn assert_ownership_accounted(&self) {
        let (_, state) = self
            .store
            .tensorlake_recipe_row("dev")
            .await
            .unwrap()
            .unwrap();
        let saved = state.podman.get(&self.place).cloned().unwrap_or_default();
        let owned: Vec<_> = self
            .state
            .lock()
            .unwrap()
            .images
            .values()
            .filter(|image| image.owned)
            .map(|image| image.id.clone())
            .collect();
        for id in owned {
            assert!(
                saved
                    .image
                    .as_ref()
                    .is_some_and(|image| same_image(&image.id, &id))
                    || saved
                        .prepared
                        .as_ref()
                        .is_some_and(|image| same_image(&image.id, &id))
                    || saved
                        .retired_images
                        .iter()
                        .any(|recorded| self.image(recorded).is_some_and(|image| image.id == id)),
                "unrecorded owned image {id}"
            );
        }
        let model = self.state.lock().unwrap();
        assert!(
            !model.removed_images.iter().any(|image| image == UBUNTU),
            "user-owned registry base must survive"
        );
    }
}

/// Only the prerequisite probe uses the text convenience call. Every engine
/// command goes through bounded capture, retaining nonzero status separately.
#[async_trait::async_trait]
impl PlaceCommands for Commands {
    async fn run(&self, _: &BaseRoute, argv: &[String]) -> anyhow::Result<String> {
        Ok(match argv[0].as_str() {
            "uname" if argv[1] == "-s" => "Linux".into(),
            "uname" => "x86_64".into(),
            "sh" => "/usr/bin/podman".into(),
            "/usr/bin/podman" => serde_json::json!({"version":{"Version":"4.9.3"},
                "host":{"security":{"rootless":true},"cgroupVersion":"v2","cgroupManager":"systemd"}}).to_string(),
            "busctl" => {
                let proxy = self.state.lock().unwrap().proxy.clone().unwrap_or_else(|| BTreeMap::from([
                    ("HTTP_PROXY".into(), String::new()), ("https_proxy".into(), PROXY_SECRET.into()),
                ]));
                serde_json::json!({"type":"as","data":proxy.into_iter().map(|(name,value)|format!("{name}={value}")).collect::<Vec<_>>()}).to_string()
            },
            "systemd-run" | "systemctl" => String::new(),
            other => panic!("unexpected probe {other}: {argv:?}"),
        })
    }

    async fn run_bounded(
        &self,
        _: &BaseRoute,
        argv: &[String],
        bound: Duration,
    ) -> anyhow::Result<CommandResult> {
        self.execute(argv, None, bound).await
    }

    async fn run_with_input(
        &self,
        _: &BaseRoute,
        argv: &[String],
        input: &[u8],
        bound: Duration,
    ) -> anyhow::Result<CommandResult> {
        self.execute(argv, Some(input), bound).await
    }
}

impl Commands {
    /// Input and argv remain separate even in the model. Phase recognition
    /// reads private input; recorded argv is exactly what production lowers.
    async fn execute(
        &self,
        argv: &[String],
        input: Option<&[u8]>,
        bound: Duration,
    ) -> anyhow::Result<CommandResult> {
        assert!(bound > Duration::ZERO && bound <= IMAGE_WORK);
        self.state.lock().unwrap().calls.push(argv.to_vec());
        if let Some(input) = input {
            self.state
                .lock()
                .unwrap()
                .inputs
                .push((argv.to_vec(), input.to_vec()));
        }
        let args = if argv[0] == "systemd-run" {
            assert!(
                argv.iter().any(|arg| arg == "--scope")
                    && argv.iter().any(|arg| arg == "--collect")
            );
            let program = argv
                .iter()
                .position(|arg| arg == "/usr/bin/podman")
                .unwrap();
            &argv[program + 1..]
        } else {
            assert_eq!(argv[0], "/usr/bin/podman", "current private absolute path");
            &argv[1..]
        };
        let last = args.last().unwrap().as_str();
        match (args[0].as_str(), args.get(1).map(String::as_str)) {
            ("container", Some("exists")) => {
                if self.state.lock().unwrap().fail_absence {
                    return Ok(failure("absence-unanswered"));
                }
                Ok(output(
                    if self.container(last).is_some() { 0 } else { 1 },
                    "",
                ))
            }
            ("container", Some("inspect")) => {
                let item = self.container(last).expect("inspect only after existence");
                Ok(output(0, &serde_json::json!([{"Id":item.id,"Name":item.name,"State":{"Status":item.status},
                    "Config":{"Labels":{"farhelm.helm":self.helm,"farhelm.role":item.role,"farhelm.recipe":"dev"}}}]).to_string()))
            }
            ("image", Some("exists")) => {
                Ok(output(if self.image(last).is_some() { 0 } else { 1 }, ""))
            }
            ("image", Some("inspect")) => {
                let image = self.image(last).expect("inspect an existing image");
                let labels = if image.owned {
                    serde_json::json!({"farhelm.helm":self.helm,"farhelm.recipe":"dev","farhelm.artifact":image.artifact})
                } else {
                    serde_json::json!({})
                };
                Ok(output(
                    0,
                    &serde_json::json!([{"Id":image.id,"Config":{"Labels":labels}}]).to_string(),
                ))
            }
            ("pull", _) => {
                if self.hit("pull-base") {
                    return Ok(failure("pull-base"));
                }
                self.state.lock().unwrap().images.insert(
                    last.into(),
                    Image {
                        id: last.into(),
                        owned: false,
                        artifact: String::new(),
                    },
                );
                Ok(output(0, ""))
            }
            ("run", _) => {
                let value =
                    |flag: &str| args[args.iter().position(|arg| arg == flag).unwrap() + 1].clone();
                assert!(args.iter().any(|arg| arg == "--http-proxy=false"));
                assert!(args.iter().any(|arg| arg == "--systemd=always"));
                let name = value("--name");
                let role = args
                    .iter()
                    .find_map(|arg| arg.strip_prefix("farhelm.role="))
                    .unwrap()
                    .to_owned();
                let stage = if role == "session" {
                    "session"
                } else if role == "prepare" {
                    "prepare"
                } else if value("--entrypoint") == "/bin/sh" {
                    "bootstrap"
                } else {
                    "software"
                };
                // The name must already belong to the actual row/recipe before
                // any reply can make it look as if run succeeded.
                let rows = self.store.list_hosts().await?;
                let (_, saved) = self.store.tensorlake_recipe_row("dev").await?.unwrap();
                if role == "session" {
                    let binding = rows
                        .iter()
                        .find_map(|row| row.podman.as_ref().filter(|binding| binding.name == name))
                        .expect("row before run");
                    assert!(binding.pending_create_at.is_some() && binding.login.is_some());
                } else {
                    let saved = &saved.podman[&self.place];
                    let recorded = if role == "prepare" {
                        &saved.preparer_container_id
                    } else {
                        &saved.builder_container_id
                    };
                    assert_eq!(
                        recorded.as_deref(),
                        Some(name.as_str()),
                        "helper name before send"
                    );
                }
                let phase = format!("{stage}-run");
                let failed = self.hit(&phase);
                let (lost_absent, lost_created, gated) = {
                    let state = self.state.lock().unwrap();
                    (
                        stage == "session" && state.lost_absent_run,
                        stage == "session" && state.lost_created_run,
                        stage == "session" && state.session_gate,
                    )
                };
                if gated {
                    self.session_release.notified().await;
                }
                let absent = {
                    let mut state = self.state.lock().unwrap();
                    if state
                        .absent_run
                        .as_ref()
                        .is_some_and(|(selected, _)| selected == &phase)
                    {
                        let (_, outcome) = state.absent_run.take().unwrap();
                        state.failed = Some(phase.clone());
                        Some(outcome)
                    } else {
                        None
                    }
                };
                if let Some(outcome) = absent {
                    return match outcome {
                        AbsentRun::Refused => Ok(failure(&phase)),
                        AbsentRun::SshDisconnected => Ok(output(255, "")),
                        AbsentRun::Signalled => Ok(CommandResult {
                            code: None,
                            signal: Some(15),
                            stdout: vec![],
                            stderr: String::new(),
                        }),
                        AbsentRun::LostReply => {
                            anyhow::bail!("injected failed capture with an unknown outcome")
                        }
                        AbsentRun::InvalidReply => Ok(output(0, "not-a-container-id")),
                        AbsentRun::AbsenceUnanswered => {
                            self.state.lock().unwrap().fail_absence = true;
                            Ok(failure(&phase))
                        }
                    };
                }
                if lost_absent {
                    anyhow::bail!("injected unanswered run before observation");
                }
                let image_offset = args.iter().position(|arg| arg == "--").unwrap() + 1;
                let id = {
                    let mut state = self.state.lock().unwrap();
                    state.next_container += 1;
                    let id = format!("{:064x}", state.next_container);
                    state.containers.insert(
                        id.clone(),
                        Container {
                            id: id.clone(),
                            name,
                            role,
                            stage: stage.into(),
                            image: args[image_offset].clone(),
                            status: if stage == "session" && failed {
                                "exited".into()
                            } else {
                                "running".into()
                            },
                        },
                    );
                    id
                };
                if lost_created {
                    anyhow::bail!("injected lost run reply after container creation");
                }
                if failed {
                    return Ok(failure(&phase));
                }
                Ok(output(0, &id))
            }
            ("exec", _) => {
                let offset = args.iter().position(|arg| arg == "--").unwrap();
                let item = self
                    .container(&args[offset + 1])
                    .expect("exec requires the created container");
                assert_eq!(item.status, "running");
                assert_eq!(
                    last, PRIVATE_SCRIPT,
                    "the script must arrive over stdin, not argv"
                );
                let script = std::str::from_utf8(input.expect("private script input")).unwrap();
                let phase = if script.contains("farhelm-base.log") {
                    "bootstrap".to_owned()
                } else if script.contains("getent passwd --") {
                    format!("{}-login", item.stage)
                } else if script.contains("/proc/1/comm") {
                    "readiness".into()
                } else if script.contains("systemctl --user is-system-running") {
                    format!("{}-user-ready", item.stage)
                } else if script.contains("systemctl --user show-environment") {
                    format!("{}-proxy-read", item.stage)
                } else if script.contains("systemctl --user unset-environment") {
                    format!("{}-proxy-write", item.stage)
                } else if script.contains("mkdir -p --") && script.contains("environment.d") {
                    format!("{}-proxy-directory", item.stage)
                } else if script.contains("farhelm-build-packages.log") {
                    "packages".into()
                } else if script.contains(crate::tensorlake::build_script::CLAUDE_INSTALLER) {
                    "claude".into()
                } else if script.contains(crate::tensorlake::build_script::CODEX_INSTALLER) {
                    "codex".into()
                } else if script.contains("/etc/machine-id") {
                    "software-finalize".into()
                } else {
                    panic!("unrecognized provider script in {}: {script}", item.stage)
                };
                if self.hit(&phase) {
                    return Ok(failure(&phase));
                }
                Ok(output(
                    0,
                    if phase.ends_with("-login") {
                        "ubuntu:x:1000:1000:Ubuntu:/home/ubuntu:/bin/bash\n"
                    } else if phase.ends_with("-proxy-read") {
                        ""
                    } else if phase == "claude" {
                        "claude fixture\n"
                    } else if phase == "codex" {
                        "codex fixture\n"
                    } else {
                        ""
                    },
                ))
            }
            ("stop", _) => {
                assert!(
                    args.windows(2)
                        .any(|pair| pair[0] == "--time" && pair[1] == "-1"),
                    "checkpoint stop cannot force kill"
                );
                let item = self.container(last).expect("stop helper");
                let phase = format!("{}-stop", item.stage);
                if self.hit(&phase) {
                    return Ok(failure(&phase));
                }
                self.state
                    .lock()
                    .unwrap()
                    .containers
                    .get_mut(&item.id)
                    .unwrap()
                    .status = "exited".into();
                Ok(output(0, &item.id))
            }
            ("commit", _) => {
                let offset = args.iter().position(|arg| arg == "--").unwrap();
                let item = self.container(&args[offset + 1]).expect("commit helper");
                assert_eq!(item.status, "exited", "no live capture");
                let phase = format!("{}-commit", item.stage);
                let tag = args[offset + 2].clone();
                let (_, saved) = self.store.tensorlake_recipe_row("dev").await?.unwrap();
                assert!(
                    saved.podman[&self.place].retired_images.contains(&tag),
                    "the random owned commit tag must be durable before commit can create an image"
                );
                if self.hit(&phase) {
                    return Ok(failure(&phase));
                }
                let artifact = args
                    .iter()
                    .find_map(|arg| arg.strip_prefix("LABEL farhelm.artifact="))
                    .unwrap()
                    .to_owned();
                let mut state = self.state.lock().unwrap();
                state.next_image += 1;
                let id = format!("sha256:{:064x}", state.next_image + 100);
                state.images.insert(
                    id.clone(),
                    Image {
                        id: id.clone(),
                        artifact,
                        owned: true,
                    },
                );
                state.image_tags.insert(tag, id.clone());
                if state.lost_commit.as_deref() == Some(phase.as_str()) {
                    state.lost_commit = None;
                    if state.invalid_commit_reply {
                        return Ok(output(0, "not-a-canonical-image"));
                    }
                    anyhow::bail!("injected lost commit reply after image creation");
                }
                Ok(output(0, &id))
            }
            ("rm", _) => {
                let refusing = {
                    let state = self.state.lock().unwrap();
                    state.cleanup_fails && state.failed.is_some()
                };
                if refusing {
                    self.hit("cleanup-refused");
                    return Ok(failure("cleanup-refused"));
                }
                let item = self
                    .container(last)
                    .expect("remove only observed canonical resource");
                assert_eq!(
                    last, item.id,
                    "name must be pinned to canonical ID before deletion"
                );
                let mut state = self.state.lock().unwrap();
                state.containers.remove(&item.id);
                state.removed_containers.push(item.id);
                Ok(output(0, ""))
            }
            ("ps", _) if args.iter().any(|arg| arg == "--format=json") => {
                let ids: Vec<_> = self
                    .state
                    .lock()
                    .unwrap()
                    .containers
                    .keys()
                    .map(|id| serde_json::json!({"Id":id}))
                    .collect();
                Ok(output(0, &serde_json::to_string(&ids).unwrap()))
            }
            ("ps", _) => {
                let ancestor = args
                    .iter()
                    .find_map(|arg| arg.strip_prefix("ancestor="))
                    .unwrap();
                let ancestor = self
                    .image(ancestor)
                    .map(|image| image.id)
                    .unwrap_or_else(|| ancestor.into());
                let users: Vec<_> = self
                    .state
                    .lock()
                    .unwrap()
                    .containers
                    .values()
                    .filter(|item| same_image(&item.image, &ancestor))
                    .map(|item| item.id.clone())
                    .collect();
                Ok(output(0, &users.join("\n")))
            }
            ("image", Some("rm")) => {
                assert!(!args.iter().any(|arg| arg == "--force" || arg == "-f"));
                let image = self.image(last).expect("remove existing image");
                let mut state = self.state.lock().unwrap();
                assert!(image.owned, "never remove user base");
                assert!(
                    !state
                        .containers
                        .values()
                        .any(|item| same_image(&item.image, &image.id)),
                    "no image user can be discarded"
                );
                state.images.remove(&image.id);
                state.image_tags.retain(|_, id| id != &image.id);
                state.removed_images.push(image.id);
                Ok(output(0, ""))
            }
            other => panic!("unexpected Podman operation {other:?}: {args:?}"),
        }
    }
}

/// Retain truthful status and bytes; exists=1 is not a generic command error.
fn output(code: i32, text: &str) -> CommandResult {
    CommandResult {
        code: Some(code),
        signal: None,
        stdout: text.as_bytes().to_vec(),
        stderr: String::new(),
    }
}
/// A named nonzero reply leaves cleanup observable through later commands.
fn failure(phase: &str) -> CommandResult {
    CommandResult {
        code: Some(125),
        signal: None,
        stdout: vec![],
        stderr: format!("injected {phase}"),
    }
}

/// Both full digest spellings are accepted by Podman's image operations.
fn same_image(left: &str, right: &str) -> bool {
    left.strip_prefix("sha256:").unwrap_or(left) == right.strip_prefix("sha256:").unwrap_or(right)
}

/// Reveal actual work and retain its named progress. A successful cold pass is
/// a fixture premise for failure cases; no fabricated artifact enters create.
#[derive(Default)]
pub(crate) struct Progress {
    needed: std::sync::atomic::AtomicUsize,
    lines: Mutex<Vec<String>>,
}
#[async_trait::async_trait]
impl PreparationProgress for Progress {
    async fn needed(&self) {
        self.needed
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    async fn step(&self, line: String) {
        self.lines.lock().unwrap().push(line);
    }
}

/// Establish that this model traverses every stage before using it as a
/// failure oracle. The real common installer must execute and finish, and a
/// second ensure must reuse its actual published image without new helpers.
#[farhelm_testtrace::test]
async fn cold_provider_success_establishes_build_install_and_reuse_premise() {
    let f = Fixture::new().await;
    let progress = Progress::default();
    let artifact = f.prepare(&progress).await.unwrap();
    assert!(progress.needed.load(std::sync::atomic::Ordering::SeqCst) > 0);
    assert!(
        !progress.lines.lock().unwrap().is_empty(),
        "cold work must report actual named progress"
    );
    for &phase in COMMAND_PHASES {
        assert!(
            f.commands.phases().iter().any(|seen| seen == phase),
            "missing success phase {phase}"
        );
    }
    let backend = f.backend_events();
    for &(container, event) in BACKEND_PHASES {
        assert!(
            f.backend_reached(container, event),
            "common installer never reached {container}/{event}: {backend:?}"
        );
    }
    assert!(
        !backend.iter().any(|event| event == "attach-supervisor"),
        "preparation cannot attach a host"
    );
    assert!(
        f.commands.container_ids().is_empty(),
        "all successful helpers ended"
    );
    f.commands.assert_ownership_accounted().await;
    drop(artifact);
    let runs = f.commands.run_count();
    let reused = f.prepare(&Progress::default()).await.unwrap();
    assert_eq!(f.commands.run_count(), runs, "fresh reuse cannot rebuild");
    drop(reused);
}

/// Every named mutator can fail after a helper exists. Real failure cleanup
/// must remove those helpers and retain all committed layers in durable state.
/// Run failures deliberately leave a created helper, proving recovery by name.
#[farhelm_testtrace::test]
async fn cold_provider_command_failures_cleanup_each_reachable_phase() {
    for &phase in COMMAND_PHASES {
        let f = Fixture::new().await;
        f.commands.fail(phase);
        let result = f.prepare(&Progress::default()).await;
        assert!(
            result.is_err(),
            "injected phase {phase} unexpectedly succeeded"
        );
        assert_eq!(
            f.commands.failed().as_deref(),
            Some(phase),
            "the requested boundary must be reached"
        );
        assert!(
            f.commands.container_ids().is_empty(),
            "{phase}: helper cleanup left a resource"
        );
        if phase != "pull-base" {
            assert!(
                !f.commands.removed_containers().is_empty(),
                "{phase}: an observed helper must actually be removed"
            );
        }
        let state = f.recipe_state().await;
        assert!(
            state.builder_container_id.is_none()
                && state.preparer_container_id.is_none()
                && state.build_started_at.is_none()
                && state.prepare_started_at.is_none(),
            "{phase}: {state:?}"
        );
        assert!(
            state.last_failure.is_some(),
            "{phase}: failure context must remain"
        );
        assert!(
            state.retired_images.iter().all(|id| image_id(id).is_ok()),
            "{phase}: a definite command refusal cannot leave an unresolved absent commit tag"
        );
        f.commands.assert_ownership_accounted().await;
    }
}

/// Logind and ordinary installer actions run through the selected existing
/// backend. Their failure must propagate before prepared publication, then use
/// the same Podman helper cleanup as script failures.
#[farhelm_testtrace::test]
async fn cold_provider_backend_and_common_installer_failures_cleanup_before_publication() {
    for &(container, event) in BACKEND_PHASES {
        let f = Fixture::new().await;
        f.fail_backend(container, event);
        let result = f.prepare(&Progress::default()).await;
        assert!(
            result.is_err(),
            "backend {container}/{event} unexpectedly succeeded"
        );
        assert!(
            f.backend_reached(container, event),
            "the intended canonical helper/backend boundary never ran"
        );
        assert!(f.commands.container_ids().is_empty(), "{container}/{event}");
        assert!(
            f.recipe_state().await.prepared.is_none(),
            "no failed installer can certify a checkpoint"
        );
        f.commands.assert_ownership_accounted().await;
    }
}

/// Failed cleanup is an explicit retained owner, never a fake success. A later
/// real guarded cleanup retries that canonical helper and removes ownership
/// only after both the helper and owned images are confirmed absent.
#[farhelm_testtrace::test]
async fn cleanup_failure_keeps_canonical_helper_and_explicit_recipe_retry_finishes() {
    let f = Fixture::new().await;
    f.commands.fail("packages");
    f.commands.fail_cleanup();
    assert!(f.prepare(&Progress::default()).await.is_err());
    let saved = f.recipe_state().await;
    let helper = saved
        .builder_container_id
        .clone()
        .expect("unconfirmed helper retained");
    assert_eq!(helper.len(), 64, "canonical observation survives failed rm");
    assert!(f.commands.container_ids().contains(&helper));
    assert!(saved.last_failure.as_deref().unwrap().contains("cleanup"));
    f.commands.state.lock().unwrap().cleanup_fails = false;
    f.clean_recipe().await.unwrap();
    assert!(f.commands.container_ids().is_empty());
    assert!(!f.recipe_state().await.needs_cleanup());
}

/// Commit can create an owned layer before its reply is lost or unusable.
/// A pre-send durable tag must keep that layer reachable after helper cleanup,
/// and ordinary guarded recipe cleanup must finish it without deleting a base.
#[farhelm_testtrace::test]
async fn lost_or_invalid_commit_reply_retains_tag_ownership_until_confirmed_cleanup() {
    for phase in ["bootstrap-commit", "software-commit", "prepare-commit"] {
        for invalid_reply in [false, true] {
            let f = Fixture::new().await;
            {
                let mut state = f.commands.state.lock().unwrap();
                state.lost_commit = Some(phase.into());
                state.invalid_commit_reply = invalid_reply;
            }
            assert!(
                f.prepare(&Progress::default()).await.is_err(),
                "{phase}/{invalid_reply}"
            );
            assert!(f.commands.phases().iter().any(|seen| seen == phase));
            assert!(
                f.commands.container_ids().is_empty(),
                "helper cleanup must still finish"
            );
            f.commands.assert_ownership_accounted().await;
            let unremoved = f
                .commands
                .state
                .lock()
                .unwrap()
                .images
                .values()
                .any(|image| image.owned);
            assert!(
                !unremoved || f.recipe_state().await.needs_cleanup(),
                "a remaining committed layer must retain ownership; confirmed cleanup may already have removed it"
            );
            f.clean_recipe().await.unwrap();
            assert!(!f.recipe_state().await.needs_cleanup());
            assert!(
                !f.commands
                    .state
                    .lock()
                    .unwrap()
                    .images
                    .values()
                    .any(|image| image.owned)
            );
            assert!(
                f.commands.image(UBUNTU).is_some(),
                "registry base survives every failure retry"
            );
        }
    }
}

/// A real successful cold build must keep proxies out of every command layer,
/// including the SSH wrapper and the bootstrap. The command model receives the
/// exact private input separately, so redacting only a display cannot pass.
#[farhelm_testtrace::test]
async fn cold_build_sends_exact_proxy_values_only_as_private_input() {
    let f = Fixture::new().await;
    let artifact = f.prepare(&Progress::default()).await.unwrap();
    assert!(f.recipe_state().await.prepared.is_some());
    assert!(
        f.commands.container_ids().is_empty(),
        "success cleans all helpers"
    );
    drop(artifact);
    let directory = tempfile::tempdir().unwrap();
    let state = f.commands.state.lock().unwrap();
    assert!(
        !state.inputs.is_empty(),
        "the real provider must use the input seam"
    );
    let values = BTreeMap::from([
        ("HTTP_PROXY".into(), String::new()),
        ("https_proxy".into(), PROXY_SECRET.into()),
    ]);
    let exports = build::proxy_exports(&values);
    let routes = [
        BaseRoute::Local,
        BaseRoute::Ssh {
            destination: "place.example".into(),
            access: crate::ssh::SshAccessKind::UserConfig,
        },
    ];
    for argv in &state.calls {
        for route in &routes {
            let process = crate::environment::podman_probe::lowered_command_for_test(
                directory.path(),
                route,
                argv,
            )
            .unwrap();
            assert!(
                !process
                    .program
                    .to_string_lossy()
                    .contains("private-proxy-marker")
            );
            assert!(
                process
                    .args
                    .iter()
                    .all(|arg| !arg.contains("private-proxy-marker")),
                "a lowerer leaked a proxy value"
            );
        }
    }
    let mut bootstrap = false;
    let mut imported = false;
    for (argv, input) in &state.inputs {
        assert_eq!(argv.last().map(String::as_str), Some(PRIVATE_SCRIPT));
        let input = std::str::from_utf8(input).unwrap();
        assert!(
            input.starts_with(&exports),
            "exports must preserve the exact observed values"
        );
        assert!(!input.contains("systemctl --user set-environment "));
        if input.contains("farhelm-base.log") {
            bootstrap = true;
            assert_eq!(
                input.matches("private-proxy-marker").count(),
                1,
                "bootstrap must inherit once"
            );
        }
        if let Some(import) = input
            .lines()
            .find(|line| line.starts_with("systemctl --user import-environment "))
        {
            imported = true;
            assert_eq!(
                import,
                format!(
                    "systemctl --user import-environment {} {} || exit 1",
                    crate::ssh::shell_quote("HTTP_PROXY"),
                    crate::ssh::shell_quote("https_proxy")
                )
            );
        }
    }
    assert!(
        bootstrap && imported,
        "both bootstrap and the running manager need inheritance"
    );
}

/// Actual prepared artifacts establish the ordinary installer premise before
/// testing create settlement. Delete is immediate only after an own completed
/// refusal and confirmed absence; all ambiguous outcomes retain the safeguard.
#[farhelm_testtrace::test]
async fn absent_session_create_settles_only_a_completed_confirmed_refusal() {
    for outcome in [
        AbsentRun::Refused,
        AbsentRun::SshDisconnected,
        AbsentRun::Signalled,
        AbsentRun::LostReply,
        AbsentRun::InvalidReply,
        AbsentRun::AbsenceUnanswered,
    ] {
        let f = Fixture::new().await;
        let artifact = f.prepare(&Progress::default()).await.unwrap();
        assert!(f.recipe_state().await.prepared.is_some());
        let host = f
            .store
            .register_pending_session_container(
                "farhelm-sess-refusal".into(),
                "dev".into(),
                f.place,
            )
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
        assert!(row.podman.as_ref().unwrap().pending_create_at.is_none());
        f.commands.absent_run("session-run", outcome);
        assert!(
            f.provider
                .create(
                    &row.environment_binding().unwrap(),
                    Some(artifact),
                    &Progress::default()
                )
                .await
                .is_err()
        );
        assert_eq!(
            f.commands.failed().as_deref(),
            Some("session-run"),
            "the real run boundary must be reached"
        );
        assert!(
            f.commands.session_ids().is_empty(),
            "all outcomes deliberately leave Podman absent"
        );
        let row = f
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        let settled = matches!(outcome, AbsentRun::Refused);
        assert_eq!(
            row.podman.as_ref().unwrap().pending_create_at.is_none(),
            settled,
            "{outcome:?}"
        );
        let deletion = f
            .provider
            .destroy(&row.environment_binding().unwrap())
            .await;
        assert_eq!(
            deletion.is_ok(),
            settled,
            "{outcome:?}: immediate Delete must follow settlement, not absence alone"
        );
    }
}

/// Every helper role uses the same outcome distinction as session creation.
/// A refused absent run must permit an immediate real rebuild, while a lost
/// outcome still blocks it even though the scripted engine currently has no
/// such helper. Time is never advanced to make the assertion pass.
#[farhelm_testtrace::test]
async fn absent_helper_create_allows_immediate_rebuild_only_after_definite_refusal() {
    for phase in ["bootstrap-run", "software-run", "prepare-run"] {
        for outcome in [
            AbsentRun::Refused,
            AbsentRun::SshDisconnected,
            AbsentRun::Signalled,
            AbsentRun::LostReply,
            AbsentRun::InvalidReply,
            AbsentRun::AbsenceUnanswered,
        ] {
            let f = Fixture::new().await;
            f.commands.absent_run(phase, outcome);
            assert!(
                f.prepare(&Progress::default()).await.is_err(),
                "{phase}/{outcome:?}"
            );
            assert_eq!(f.commands.failed().as_deref(), Some(phase));
            assert!(
                f.commands.container_ids().is_empty(),
                "the configured outcome never creates a helper"
            );
            let saved = f.recipe_state().await;
            let settled = matches!(outcome, AbsentRun::Refused);
            let pending = if phase == "prepare-run" {
                &saved.preparer_container_id
            } else {
                &saved.builder_container_id
            };
            assert_eq!(pending.is_none(), settled, "{phase}/{outcome:?}: {saved:?}");
            // Restore the observer, not the command outcome. An absent lookup
            // after capture loss must still not authorize another run.
            f.commands.state.lock().unwrap().fail_absence = false;
            let runs = f.commands.run_count();
            let retry = f.prepare(&Progress::default()).await;
            assert_eq!(retry.is_ok(), settled, "{phase}/{outcome:?}");
            if settled {
                assert!(
                    f.commands.run_count() > runs,
                    "an actual rebuild must have run"
                );
                assert!(f.recipe_state().await.prepared.is_some());
                assert!(f.commands.container_ids().is_empty());
            } else {
                assert_eq!(
                    f.commands.run_count(),
                    runs,
                    "an unresolved create cannot send a second run"
                );
            }
            drop(retry);
        }
    }
}

/// A new provider process can observe changed place proxies while reusing the
/// same prepared image. Reuse the existing command/backend assembly and source
/// registry, replacing only the probe cache before the create under test.
async fn provider_with_proxy(f: &Fixture, proxy: BTreeMap<String, String>) -> Podman {
    f.commands.state.lock().unwrap().proxy = Some(proxy.clone());
    let places = PodmanPlaces::with_commands(f.commands.clone());
    places.fresh(&f.store, f.place).await.unwrap();
    let provider = Podman::new(
        f.store.clone(),
        places,
        f.provider.0.delivery.clone(),
        f.provider.0.builds.clone(),
        f.provider.0.preparer.clone(),
    );
    assert_eq!(
        provider.place(f.place).await.unwrap().2.proxy,
        proxy,
        "the test must observe the changed place environment"
    );
    provider
}

/// Initial proxy setup belongs to admission, not to run's reply or canonical
/// identity. Lost replies and known-ID unadmitted retries must refresh the
/// supervisor before admission; admitted retries must retain their original
/// snapshot even after a newer probe removes every proxy.
#[farhelm_testtrace::test]
async fn creation_proxy_restart_follows_admission_across_fresh_and_recovered_creates() {
    for lost_reply in [false, true] {
        let f = Fixture::new().await;
        let artifact = f.prepare(&Progress::default()).await.unwrap();
        let image = f.recipe_state().await.prepared.unwrap().id;
        let launch_proxy = BTreeMap::from([
            (
                "http_proxy".into(),
                "https://launch-proxy-marker@proxy.example/".into(),
            ),
            ("HTTPS_PROXY".into(), String::new()),
        ]);
        let provider = provider_with_proxy(&f, launch_proxy.clone()).await;
        assert_eq!(
            f.recipe_state().await.prepared.unwrap().id,
            image,
            "a rebuilt image must not hide the proxy mismatch"
        );
        let host = f
            .store
            .register_pending_session_container("farhelm-sess-proxy".into(), "dev".into(), f.place)
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
        let events_before = f.backend_events().len();
        if lost_reply {
            f.commands.lost_run(true);
        }
        let created = provider
            .create(
                &row.environment_binding().unwrap(),
                Some(artifact),
                &Progress::default(),
            )
            .await
            .unwrap();
        assert_eq!(created.created, !lost_reply);
        let events = f.backend_events();
        assert_eq!(
            &events[events_before..],
            &["write-proxy", "restart-supervisor"],
            "private file installation must precede the ordinary supervisor restart"
        );
        let row = f
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert!(
            row.remote_farhelm.is_none() && row.remote_state_dir.is_none(),
            "create has not admitted its install"
        );
        let id = row.podman.as_ref().unwrap().container_id.clone().unwrap();
        let facts = f
            .store
            .podman_prepared_install(host)
            .await
            .unwrap()
            .unwrap();
        let runs = f.commands.run_count();
        let events_before = events.len();
        let recovered = provider
            .create(
                &row.environment_binding().unwrap(),
                None,
                &Progress::default(),
            )
            .await
            .unwrap();
        assert!(!recovered.created);
        assert_eq!(
            f.commands.run_count(),
            runs,
            "a known-ID recovery must not create another container"
        );
        assert_eq!(
            &f.backend_events()[events_before..],
            &["write-proxy", "restart-supervisor"],
            "known-ID unadmitted recovery still needs proxy setup and restart"
        );
        let row = f
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert!(
            f.store
                .admit_podman_install(
                    crate::store::PodmanInstallGuard::of(&row).unwrap(),
                    facts.farhelm_path.to_str().unwrap(),
                    facts.state_dir.to_str().unwrap(),
                )
                .await
                .unwrap()
        );
        let row = f
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert!(
            row.remote_farhelm.is_some() && row.remote_state_dir.is_some(),
            "the existing durable admission must be present"
        );
        let provider = provider_with_proxy(&f, BTreeMap::new()).await;
        let inputs_before = f.commands.state.lock().unwrap().inputs.len();
        let events_before = f.backend_events().len();
        let recovered = provider
            .create(
                &row.environment_binding().unwrap(),
                None,
                &Progress::default(),
            )
            .await
            .unwrap();
        assert!(!recovered.created);
        assert_eq!(
            f.backend_events().len(),
            events_before,
            "an admitted create cannot rewrite the file or restart the supervisor"
        );
        assert_eq!(
            f.commands.state.lock().unwrap().inputs.len(),
            inputs_before,
            "an admitted create cannot read readiness or import a new environment"
        );
        let row = f
            .store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        assert_eq!(
            row.podman.unwrap().container_id.as_deref(),
            Some(id.as_str())
        );
        assert_eq!(f.commands.run_count(), runs);
    }
}

/// A supervisor restart failure is still initial setup failure. The provider
/// must return it while both admission coordinates remain absent, retaining
/// the selected installation for the ordinary owner to clean up or retry.
#[farhelm_testtrace::test]
async fn failed_initial_proxy_restart_leaves_installation_unadmitted() {
    let f = Fixture::new().await;
    let artifact = f.prepare(&Progress::default()).await.unwrap();
    let host = f
        .store
        .register_pending_session_container(
            "farhelm-sess-proxy-failed".into(),
            "dev".into(),
            f.place,
        )
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
    assert!(row.remote_farhelm.is_none() && row.remote_state_dir.is_none());
    f.fail_backend(4, "restart-supervisor");
    let result = f
        .provider
        .create(
            &row.environment_binding().unwrap(),
            Some(artifact),
            &Progress::default(),
        )
        .await;
    assert!(result.is_err());
    assert!(f.backend_reached(4, "write-proxy") && f.backend_reached(4, "restart-supervisor"));
    let row = f
        .store
        .list_hosts()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == host)
        .unwrap();
    assert!(row.remote_farhelm.is_none() && row.remote_state_dir.is_none());
    assert!(row.podman.as_ref().unwrap().container_id.is_some());
    assert!(
        f.store
            .podman_prepared_install(host)
            .await
            .unwrap()
            .is_some()
    );
    assert!(!f.backend_reached(4, "attach-supervisor"));
}

/// A partial admission is neither a fresh clone nor an admitted environment.
/// Refuse it without importing proxies or restarting possible user work. This
/// fixture deliberately plants each otherwise-invalid coordinate pair.
#[farhelm_testtrace::test]
async fn partial_install_coordinates_refuse_proxy_recovery_without_mutating_the_container() {
    for farhelm_only in [false, true] {
        let f = Fixture::new().await;
        let artifact = f.prepare(&Progress::default()).await.unwrap();
        let host = f
            .store
            .register_pending_session_container(
                "farhelm-sess-partial".into(),
                "dev".into(),
                f.place,
            )
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
        f.provider
            .create(
                &row.environment_binding().unwrap(),
                Some(artifact),
                &Progress::default(),
            )
            .await
            .unwrap();
        f.store
            .conn()
            .call("plant partial installation coordinates", move |conn| {
                conn.execute(
                    "UPDATE hosts SET remote_farhelm = ?1, remote_state_dir = ?2 WHERE id = ?3",
                    rusqlite::params![
                        farhelm_only.then_some("/fixture/farhelm"),
                        (!farhelm_only).then_some("/fixture/state"),
                        host,
                    ],
                )?;
                Ok(())
            })
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
        assert_ne!(row.remote_farhelm.is_some(), row.remote_state_dir.is_some());
        let inputs_before = f.commands.state.lock().unwrap().inputs.len();
        let events_before = f.backend_events().len();
        assert!(
            f.provider
                .create(
                    &row.environment_binding().unwrap(),
                    None,
                    &Progress::default()
                )
                .await
                .is_err()
        );
        assert_eq!(f.backend_events().len(), events_before);
        assert_eq!(f.commands.state.lock().unwrap().inputs.len(), inputs_before);
    }
}
