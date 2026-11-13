//! Real local and explicitly selected SSH Podman environments, through the
//! public helm and ordinary installer.
//!
//! The pinned Ubuntu base must already be cached, and FARHELM_HELM_PAYLOAD_DIR
//! must hold same-revision release-shaped Linux payloads. The production recipe
//! build still uses the network for apt and the official agent installers. The
//! managed checkout also clones the public repository from GitHub. The
//! launched agent is a stand-in. This suite spends no vendor turns. An explicit
//! FARHELM_TEST_SKIP_PODMAN=1 is the only runtime skip and prints missing coverage.
//! SSH acceptance is ignored by default and selected explicitly on an owned host.

use crate::harness::*;
use anyhow::{Context as _, bail, ensure};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Output, Stdio};
use tokio::io::AsyncReadExt as _;
use tokio::net::TcpListener;
use tokio::task::JoinSet;

/// Match the production managed-base pin. Checking before launch prevents a
/// routine test selection from pulling an image into shared rootless storage.
const UBUNTU: &str = "docker.io/library/ubuntu@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852";
const OUTPUT_CAP: u64 = 64 * 1024;
const COMMAND_BUDGET: Duration = Duration::from_secs(120);
/// A small public repository keeps managed-checkout verification independent
/// of private credentials. Its remote, HEAD and inside path are asserted live.
const PUBLIC_REPO: &str = "octocat/Hello-World";
/// Credential delivery writes real agent settings, without issuing an auth
/// request to an actual endpoint or consuming a vendor turn.
const ENDPOINT: &str = "https://podman-e2e.invalid/v1";
/// Network bootstrap owns most of this allowance. Cleanup gets its own bound
/// so a failed scenario still has time to end the resources it created.
const SCENARIO_BUDGET: Duration = Duration::from_secs(30 * 60);
const CLEANUP_BUDGET: Duration = Duration::from_secs(5 * 60);

/// A Claude-shaped fixture using the real injected callback and record format
/// from farhelm-fixtures. Python is already used for process stand-ins in this
/// suite and is installed by the recipe. No vendor executable is launched.
/// Each start publishes a fresh process nonce. Resume keeps the conversation.
/// The heartbeat's cadence is fixture workload, not a readiness delay: every
/// Rust wait observes the identity file, progress, engine state or public API.
const AGENT: &str = r#"
import datetime, http.server, json, os, pathlib, subprocess, sys, threading, time, uuid
args = sys.argv[1:]
conversation = args[args.index('--resume') + 1] if '--resume' in args else str(uuid.uuid4())
settings = json.loads(args[args.index('--settings') + 1])
hook = settings['hooks']['SessionStart'][0]['hooks'][0]['command']
cwd = os.getcwd()
project = ''.join('-' if c in '/._' else c for c in cwd)
record = pathlib.Path.home() / '.claude' / 'projects' / project / (conversation + '.jsonl')
record.parent.mkdir(parents=True, exist_ok=True)
with record.open('a') as stream:
    stream.write(json.dumps({'type':'user', 'sessionId':conversation, 'cwd':cwd,
        'timestamp':datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z')}) + '\n')
result = subprocess.run(['sh', '-c', hook], input=json.dumps({'session_id':conversation,
    'hook_event_name':'SessionStart', 'source':'startup'}), text=True,
    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=45)
if result.returncode:
    raise RuntimeError('injected hook failed: ' + result.stderr[-4096:])
state = pathlib.Path.home() / '.farhelm-podman-e2e'
state.mkdir(exist_ok=True)
identity = {'nonce':str(uuid.uuid4()), 'pid':os.getpid(), 'conversation':conversation,
    'start':pathlib.Path('/proc/self/stat').read_text().rsplit(')',1)[1].split()[19], 'cwd':cwd}
# A loopback-only service in the agent's own process exercises the ordinary
# TCP relay, and the process nonce ties the response to this exact container.
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != '/podman-e2e':
            self.send_error(404)
            return
        body = ('PODMAN-TUNNEL ' + identity['nonce']).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Connection', 'close')
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *args):
        pass
server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
identity['port'] = server.server_address[1]
threading.Thread(target=server.serve_forever, daemon=True).start()
with (state / 'launches.jsonl').open('a') as stream:
    stream.write(json.dumps(identity) + '\n')
print('PODMAN-E2E READY ' + conversation, flush=True)
counter = 0
while True:
    identity['counter'] = counter
    temporary = state / 'current.tmp'
    temporary.write_text(json.dumps(identity))
    temporary.replace(state / 'current.json')
    counter += 1
    time.sleep(0.1)
"#;

/// Independent engine observations and the ordinary stdio peer share the same
/// credential scrub. This changes only each owned child's startup environment.
fn control_command(program: &str, args: &[&str]) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (name, _) in std::env::vars_os() {
        if name
            .to_str()
            .is_some_and(|name| name.starts_with("FARHELM_"))
        {
            command.env_remove(name);
        }
    }
    for name in ["TMUX", "TMUX_PANE", "TMUX_TMPDIR"] {
        command.env_remove(name);
    }
    command
}

/// Every command belongs to this test and has bounded capture and lifetime.
/// Dropping a timed-out child kills the Podman client. Cleanup independently
/// checks the preselected owned names because a mutator's outcome may be lost.
async fn engine(args: &[&str]) -> anyhow::Result<Output> {
    let mut child = control_command("podman", args)
        .spawn()
        .context("start the test's Podman client")?;
    let mut stdout = child.stdout.take().unwrap().take(OUTPUT_CAP + 1);
    let mut stderr = child.stderr.take().unwrap().take(OUTPUT_CAP + 1);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let (status, _, _) = tokio::time::timeout(COMMAND_BUDGET, async {
        tokio::try_join!(
            child.wait(),
            stdout.read_to_end(&mut out),
            stderr.read_to_end(&mut err)
        )
    })
    .await
    .context("Podman client exceeded its test deadline")??;
    ensure!(
        out.len() <= OUTPUT_CAP as usize && err.len() <= OUTPUT_CAP as usize,
        "Podman exceeded bounded test capture"
    );
    Ok(Output {
        status,
        stdout: out,
        stderr: err,
    })
}

/// Keep command failures visible without treating a storage error as absence.
async fn require_engine(args: &[&str]) -> anyhow::Result<String> {
    let output = engine(args).await?;
    ensure!(
        output.status.success(),
        "podman {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).context("Podman returned non-UTF-8 data")
}

/// Podman's exists exit 1 alone means absent. Every other failure is unknown.
async fn exists(kind: &str, id: &str) -> anyhow::Result<bool> {
    let output = engine(&[kind, "exists", id]).await?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!(
            "could not confirm {kind} {id}: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

/// Fail before any resource is created when a required payload or cached base
/// is missing. The payload directory is an intentional recorder input, never a
/// mutation of the test process's environment or a release download fallback.
async fn prerequisites() -> PathBuf {
    let payload = PathBuf::from(std::env::var_os("FARHELM_HELM_PAYLOAD_DIR")
        .expect("real Podman tests require FARHELM_HELM_PAYLOAD_DIR with same-revision Linux release payloads"));
    assert!(
        payload.is_absolute() && payload.is_dir(),
        "the staged payload directory must be absolute and present"
    );
    let target = match std::env::consts::ARCH {
        "x86_64" => "x86_64-unknown-linux-musl",
        "aarch64" => "aarch64-unknown-linux-musl",
        arch => panic!("no real Podman payload fixture for {arch}"),
    };
    // The farhelm archive is the only payload provisioning pushes to a Linux
    // host, and so the only one `scripts/build-podman-test-payloads.sh` stages
    // (provisioning no longer pushes a static tmux).
    let archive = format!("farhelm-{target}.tar.gz");
    assert!(
        payload.join(&archive).is_file(),
        "missing staged payload {archive}"
    );
    assert!(
        exists("image", UBUNTU)
            .await
            .expect("rootless Podman must be usable"),
        "cache the pinned Ubuntu image before running real Podman tests; this test never pulls its prerequisite"
    );
    payload
}

/// Read a file through the ordinary supervisor's credit-paced download API.
/// Every path and body belongs to this fixture. The cap also keeps assertion
/// diagnostics bounded if the wrong file is returned.
async fn read_inside_file(client: &Arc<SupervisorClient>, session: &str, path: &str) -> Vec<u8> {
    tokio::time::timeout(COMMAND_BUDGET, async {
        let mut download = client.begin_download(session, path).await.unwrap();
        let mut bytes = Vec::new();
        while let Some(chunk) = download.recv().await.unwrap() {
            assert!(
                bytes.len() + chunk.len() <= OUTPUT_CAP as usize,
                "inside file exceeded capture cap"
            );
            bytes.extend_from_slice(&chunk);
        }
        bytes
    })
    .await
    .expect("inside file download stalled")
}

/// One random recipe owns every provider-created object, including failed
/// builders. Keep the helm alive through assertions, then reap it before fallback
/// cleanup so its automatic-start actor cannot race the test's resource removal.
struct Fixture {
    local: SupervisorProcess,
    /// The SSH variant registers this isolated supervisor at localhost. Engine
    /// evidence still observes the same account's owned objects independently.
    ssh_place: Option<SupervisorProcess>,
    ensure_hosts: Option<PathBuf>,
    helm: Option<HelmProcess>,
    http: reqwest::Client,
    payload: PathBuf,
    recipe: String,
    hold: String,
}

impl Fixture {
    /// Allocate ordinary owned supervisors and the optional SSH registration.
    /// The scenario's panic boundary is installed before the helm can create
    /// any Podman resource.
    async fn new(payload: PathBuf, via_ssh: bool) -> Self {
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let local = supervisor_process().await;
        wait_for_supervisor_ready(local.state.path()).await;
        let ssh_place = if via_ssh {
            let remote = supervisor_process().await;
            wait_for_supervisor_ready(remote.state.path()).await;
            Some(remote)
        } else {
            None
        };
        let ensure_hosts = if let Some(remote) = &ssh_place {
            let path = local.state.path().join("ensure-hosts.json5");
            tokio::fs::write(
                &path,
                json!({"hosts":[{
                    "ssh":"localhost", "remote_farhelm":farhelm_bin(),
                    "remote_state_dir":remote.state.path().to_string_lossy()
                }]})
                .to_string(),
            )
            .await
            .unwrap();
            Some(path)
        } else {
            None
        };
        Self {
            local,
            ssh_place,
            ensure_hosts,
            helm: None,
            http: reqwest::Client::new(),
            payload,
            recipe: format!("e2e-{}", &nonce[..24]),
            hold: format!("farhelm-e2e-hold-{nonce}"),
        }
    }

    /// Restart only the owned helm. Its local and optional SSH-place supervisor,
    /// and the in-container supervisor, stay running so reconnection tests
    /// persistence rather than rebuilding the fixture.
    async fn start_helm(&mut self) {
        if let Some(helm) = self.helm.take() {
            helm.stop_and_reap().await;
        }
        let payload = self.payload.to_str().expect("payload directory is UTF-8");
        let helm = helm_process_with_args(
            self.local.state.path(),
            self.ensure_hosts.as_deref(),
            &["--payload-dir", payload, "--backstop-refresh-secs", "1"],
        )
        .await;
        self.http = authenticated_client(self.local.state.path(), &helm.base).await;
        self.helm = Some(helm);
    }

    /// Keep each request on the current owned helm after its port changes.
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.helm.as_ref().unwrap().base)
    }

    /// Transitional HTTP refusals are observations, not a polling failure.
    /// The shared wait helper retains the last bounded body on timeout.
    async fn observe(&self, path: &str) -> anyhow::Result<Value> {
        let mut response = self.http.get(self.url(path)).send().await?;
        let status = response.status();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                body.len() + chunk.len() <= OUTPUT_CAP as usize,
                "HTTP test observation exceeded its bound"
            );
            body.extend_from_slice(&chunk);
        }
        if status.is_success() {
            Ok(serde_json::from_slice(&body)?)
        } else {
            Ok(json!({"http_status":status.as_u16(), "body":String::from_utf8_lossy(&body)}))
        }
    }

    /// A connected host is the public API's usable-peer boundary. Container
    /// state alone does not prove the inside supervisor completed its hello.
    async fn connected(&self, host: i64) -> Value {
        let hosts = wait_for_listing_with(
            || self.observe("/api/hosts"),
            120,
            "the environment's real inside supervisor is connected",
            |value: &Value| {
                value["hosts"].as_array().is_some_and(|rows| {
                    rows.iter()
                        .any(|row| row["id"] == host && row["state"]["phase"] == "connected")
                })
            },
        )
        .await;
        hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == host)
            .unwrap()
            .clone()
    }

    /// Capture only this owned container's identity and lifecycle evidence.
    async fn inspect(&self, container: &str) -> Value {
        let body = require_engine(&["container", "inspect", container])
            .await
            .unwrap();
        serde_json::from_str::<Value>(&body).unwrap()[0].clone()
    }

    /// The heartbeat is read through real exec into the container. Its fresh
    /// nonce distinguishes starts. Callers separately validate /proc only after
    /// their nonce/progress predicate: Stop or reboot may leave an older file.
    async fn agent(&self, container: &str) -> anyhow::Result<Value> {
        let output = engine(&[
            "exec",
            "--user",
            "ubuntu",
            container,
            "cat",
            "/home/ubuntu/.farhelm-podman-e2e/current.json",
        ])
        .await?;
        Ok(if output.status.success() {
            serde_json::from_slice(&output.stdout)?
        } else {
            json!({"probe_exit":output.status.code(), "probe_stderr":String::from_utf8_lossy(&output.stderr)})
        })
    }

    /// Check PID plus start time, not PID alone, before relying on the fixture.
    async fn assert_agent_alive(&self, container: &str, agent: &Value) {
        let path = format!("/proc/{}/stat", agent["pid"].as_u64().unwrap());
        let stat = require_engine(&["exec", container, "cat", &path])
            .await
            .unwrap();
        let start = stat
            .rsplit_once(')')
            .unwrap()
            .1
            .split_whitespace()
            .nth(19)
            .unwrap();
        assert_eq!(
            start,
            agent["start"].as_str().unwrap(),
            "the fixture peer was replaced"
        );
    }

    /// Await actual progress from the same process, rather than a readiness delay.
    async fn advancing_agent(&self, container: &str, before: &Value) -> Value {
        let after = wait_for_listing_with(
            || self.agent(container),
            120,
            "the same agent advances after Resume/reconnection",
            |value: &Value| {
                value["nonce"] == before["nonce"]
                    && value["counter"].as_u64() > before["counter"].as_u64()
            },
        )
        .await;
        assert_eq!(after["pid"], before["pid"]);
        assert_eq!(after["start"], before["start"]);
        self.assert_agent_alive(container, &after).await;
        after
    }

    /// Event history supplies a positive count of actual starts for this full
    /// canonical container ID. Cache polls or a Running state cannot prove once.
    async fn starts(&self, container: &str) -> usize {
        let filter = format!("container={container}");
        require_engine(&[
            "events",
            "--stream=false",
            "--filter",
            &filter,
            "--filter",
            "event=start",
            "--format",
            "{{.Status}}",
        ])
        .await
        .unwrap()
        .lines()
        .filter(|line| *line == "start")
        .count()
    }

    /// Keep a non-session container on the prepared image. Recipe deletion must
    /// respect that user rather than force-removing its image or its container.
    async fn hold_image(&self, image: &str) -> String {
        let label = format!("farhelm.e2e-run={}", self.recipe);
        let id = require_engine(&[
            "create",
            "--pull=never",
            "--name",
            &self.hold,
            "--label",
            &label,
            "--entrypoint",
            "/bin/true",
            image,
        ])
        .await
        .unwrap()
        .trim()
        .to_owned();
        assert!(exists("container", &id).await.unwrap());
        assert_eq!(self.inspect(&id).await["Image"], image);
        id
    }

    /// Exercise the ordinary inside-supervisor protocol, including the TCP
    /// relay used by desktop localhost tunnels. Standalone helm intentionally has
    /// no native opener, so this owns one loopback listener and sends its accepted
    /// socket through SupervisorClient::relay_tcp. The SSH variant carries this
    /// exact protocol over ssh localhost followed by podman exec as well.
    ///
    /// Returns the kept session the in-container agent created
    /// ([`Self::agent_requests_stay_inside`]), which stays in the container
    /// until the scenario's closing Deletes.
    async fn ordinary_features(
        &self,
        container: &str,
        session: &SessionInfo,
        host: &Value,
        agent: &Value,
        token: &str,
    ) -> farhelm_proto::SessionRef {
        let reference = session.id.parse::<farhelm_proto::SessionRef>().unwrap();
        let bare = reference.bare_id();
        let program = host["remote_farhelm"].as_str().unwrap();
        let state = host["remote_state_dir"].as_str().unwrap();
        assert!(std::path::Path::new(program).is_absolute());
        let argv = [
            "exec",
            "--interactive",
            "--user",
            "ubuntu",
            "--workdir",
            "/home/ubuntu",
            "--env",
            "HOME=/home/ubuntu",
            "--env",
            "USER=ubuntu",
            "--env",
            "LOGNAME=ubuntu",
            "--env",
            "XDG_RUNTIME_DIR=/run/user/1000",
            container,
            program,
            "internal",
            "stdio",
            "--state-dir",
            state,
            "--stdin-idle-limit-secs",
            "120",
        ];
        let mut command = if self.ssh_place.is_some() {
            let script = std::iter::once("podman")
                .chain(argv.iter().copied())
                .map(farhelm_proto::text::shell_quote)
                .collect::<Vec<_>>()
                .join(" ");
            control_command(
                "ssh",
                &[
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "StrictHostKeyChecking=yes",
                    "-o",
                    "ConnectTimeout=10",
                    "localhost",
                    &script,
                ],
            )
        } else {
            control_command("podman", &argv)
        };
        // Kill-on-drop covers a panic during any feature assertion. The normal
        // path also reaps this specific client before Suspend parks the helm's
        // own exec connection. The helper has the ordinary environment idle cap.
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let reader = child.stdout.take().unwrap();
        let writer = child.stdin.take().unwrap();
        let client = tokio::time::timeout(COMMAND_BUDGET, SupervisorClient::start(reader, writer))
            .await
            .expect("inside protocol hello timed out")
            .unwrap();
        assert_eq!(
            client.peer_hello().host_identity.as_deref(),
            Some(host["identity"].as_str().unwrap()),
            "the ordinary client must reach this environment's supervisor"
        );
        self.assert_agent_alive(container, agent).await;

        // The agent's real terminal must attach as a grid, with its own ready
        // output read through this attachment rather than a process-side probe.
        let (channel, mut seen, mut stream) = client.attach_live(bare, 100, 30).await.unwrap();
        wait_for(&mut stream, &mut seen, "PODMAN-E2E READY", 30).await;
        client.detach(channel).await;
        drop(stream);

        // Relative paths resolve in the managed checkout. Absolute file links
        // also reach files elsewhere in this same container.
        let readme = client.stat_file(bare, "README").await.unwrap();
        assert_eq!(readme.status, farhelm_proto::DownloadFileStatus::Ready);
        assert_eq!(readme.path, format!("{}/README", session.cwd));
        let actual = read_inside_file(&client, bare, "README").await;
        assert!(!actual.is_empty());
        let independent =
            require_engine(&["exec", "--user", "ubuntu", container, "cat", &readme.path])
                .await
                .unwrap();
        assert_eq!(actual.as_slice(), independent.as_bytes());
        let settings = read_inside_file(&client, bare, "/home/ubuntu/.claude/settings.json").await;
        let settings: Value = serde_json::from_slice(&settings).unwrap();
        assert_eq!(settings["env"]["ANTHROPIC_BASE_URL"], ENDPOINT);
        assert!(
            settings["env"]["ANTHROPIC_AUTH_TOKEN"] == token,
            "the selected fictional endpoint token must reach ordinary Claude settings"
        );

        let bytes: Vec<u8> = (0..8192).map(|index| (index % 251) as u8).collect();
        let mut upload = client
            .begin_upload(bare, "podman-e2e.bin", bytes.len() as u64)
            .await
            .unwrap();
        upload.send_upload_chunk(&bytes).await.unwrap();
        let uploaded = upload.commit().await.unwrap();
        assert!(uploaded.starts_with("/home/ubuntu/"));
        let uploaded_stat = client.stat_file(bare, &uploaded).await.unwrap();
        assert_eq!(
            uploaded_stat.status,
            farhelm_proto::DownloadFileStatus::Ready
        );
        assert_eq!(uploaded_stat.size, Some(bytes.len() as u64));
        assert_eq!(read_inside_file(&client, bare, &uploaded).await, bytes);

        // Open and type through a real terminal tab. The file is the execution
        // oracle: seeing the echoed command in the grid would not prove it ran.
        let tab = client.open_tab(bare).await.unwrap();
        let (tab_channel, _replay, tab_stream) = client
            .attach_terminal_live(
                bare,
                100,
                30,
                TerminalSelector::Tab { id: tab.id.clone() },
                "podman-e2e-tab",
            )
            .await
            .unwrap();
        let marker = format!("{}/.podman-e2e-tab", session.cwd);
        let line = format!(
            "printf %s tab-roundtrip > {}\r",
            farhelm_proto::text::shell_quote(&marker)
        );
        client.send_keys(tab_channel, line.into_bytes()).await;
        wait_for_listing_with(
            || client.stat_file(bare, &marker),
            30,
            "the inside tab executed its typed command",
            |info: &farhelm_proto::DownloadFileInfo| {
                info.status == farhelm_proto::DownloadFileStatus::Ready && info.size == Some(13)
            },
        )
        .await;
        assert_eq!(
            read_inside_file(&client, bare, &marker).await,
            b"tab-roundtrip"
        );
        let kept = self
            .agent_requests_stay_inside(&client, &reference, host, tab_channel)
            .await;
        client.detach(tab_channel).await;
        drop(tab_stream);
        client.close_tab(bare, &tab.id).await.unwrap();
        let listed = client.list_sessions().await.unwrap();
        assert!(
            listed
                .sessions
                .iter()
                .find(|row| row.id == bare)
                .unwrap()
                .tabs
                .is_empty()
        );

        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let local = listener.local_addr().unwrap();
        let port = u16::try_from(agent["port"].as_u64().unwrap()).unwrap();
        assert_ne!(port, 0);
        let target = bare.to_owned();
        let relay = Arc::clone(&client);
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            relay.relay_tcp(&target, None, port, socket).await.unwrap();
        });
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        let response = http
            .get(format!("http://{local}/podman-e2e"))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        let expected = format!("PODMAN-TUNNEL {}", agent["nonce"].as_str().unwrap());
        assert_eq!(response.content_length(), Some(expected.len() as u64));
        assert_eq!(
            response.bytes().await.unwrap().as_ref(),
            expected.as_bytes(),
            "the localhost tunnel must reach the service in this exact inside agent"
        );
        tokio::time::timeout(Duration::from_secs(30), async {
            while let Some(result) = tasks.join_next().await {
                result.expect("owned TCP relay task failed");
            }
        })
        .await
        .expect("ordinary TCP relay did not close");

        drop(client);
        child.start_kill().unwrap();
        tokio::time::timeout(COMMAND_BUDGET, child.wait())
            .await
            .expect("owned inside protocol client did not exit")
            .unwrap();
        kept
    }

    /// An agent in a real container session acts only within its container
    /// (SPEC.md, "Agents and environments"), driven the way an agent drives
    /// it: `farhelm agent` typed into a terminal tab of the session, whose
    /// environment carries the session's credential, against the real helm.
    ///
    /// The container's host is set to run commands without asking first, so
    /// nothing waits for a card nobody answers here; that setting is the
    /// ordinary one, which the helm unit tests check is honored. Files the
    /// shell writes are the oracle, as for the tab roundtrip above: the hosts
    /// listing shows only this container, a create with no `--host` makes a
    /// second session in it whose command really runs (it writes a marker the
    /// test waits for before anything deletes it), a create naming the real
    /// ordinary host outside is refused with the one scope sentence and puts
    /// nothing there, and deleting that second session removes only it while
    /// this session stays listed.
    ///
    /// The agent also creates a third, kept session the same way and returns
    /// its reference. The user's environment Delete at the end of the
    /// scenario needs a second session in the container to show a Delete
    /// that keeps the container, and the user cannot put one there: the helm
    /// refuses ordinary creates on a disposable host, so an agent inside is
    /// the only way besides Replace.
    async fn agent_requests_stay_inside(
        &self,
        client: &Arc<SupervisorClient>,
        asking: &farhelm_proto::SessionRef,
        host: &Value,
        tab_channel: u32,
    ) -> farhelm_proto::SessionRef {
        let bare = asking.bare_id();
        let environment = host["id"].as_i64().unwrap();
        let (status, body) = post(
            &self.http,
            &self.url(&format!("/api/hosts/{environment}/commands-without-asking")),
            json!({"commands_without_asking": true}),
        )
        .await;
        assert!(
            status.is_success(),
            "let the container's agents act: {body}"
        );
        let program = farhelm_proto::text::shell_quote(host["remote_farhelm"].as_str().unwrap());
        // A real host outside the container, so the refusal is the scope rule
        // and not an unknown name: the connected ordinary host the place is on.
        let hosts_now = self.observe("/api/hosts").await.unwrap();
        let outside = hosts_now["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["is_session_owned"] == false && row["state"]["phase"] == "connected")
            .expect("premise: a connected host outside the container")
            .clone();
        assert_ne!(outside["id"], host["id"]);
        let outside_sessions = || async {
            self.observe("/api/sessions").await.unwrap()["sessions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["host"] == outside["id"])
                .count()
        };
        let outside_before = outside_sessions().await;
        // First the creates, whose sessions prove they ran by writing a
        // marker and staying alive; the test sees that before anything
        // deletes them. The kept one outlives this method (see above); its
        // sleep may end or be killed by the later container stop, which
        // leaves it an ended session that still counts as one.
        let line = format!(
            "P={program}; \"$P\" agent hosts > ~/.e2e-hosts 2>&1; \
             \"$P\" agent create --cwd /home/ubuntu \
             --command 'touch ~/.e2e-sibling-alive; exec sleep 600' --no-yolo --title spawned \
             > ~/.e2e-created 2> ~/.e2e-created.err; \
             \"$P\" agent create --cwd /home/ubuntu \
             --command 'touch ~/.e2e-kept-alive; exec sleep 600' --no-yolo --title kept \
             > ~/.e2e-kept 2> ~/.e2e-kept.err; echo done > ~/.e2e-create-done\r"
        );
        client.send_keys(tab_channel, line.into_bytes()).await;
        wait_for_listing_with(
            || client.stat_file(bare, "/home/ubuntu/.e2e-create-done"),
            120,
            "the create typed in the container's tab finished",
            |info: &farhelm_proto::DownloadFileInfo| {
                info.status == farhelm_proto::DownloadFileStatus::Ready
            },
        )
        .await;
        for (marker, what) in [
            (
                "/home/ubuntu/.e2e-sibling-alive",
                "the created session's command ran in the container",
            ),
            (
                "/home/ubuntu/.e2e-kept-alive",
                "the kept session's command ran in the container",
            ),
        ] {
            wait_for_listing_with(
                || client.stat_file(bare, marker),
                60,
                what,
                |info: &farhelm_proto::DownloadFileInfo| {
                    info.status == farhelm_proto::DownloadFileStatus::Ready
                },
            )
            .await;
        }
        let line = format!(
            "\"$P\" agent create --host {outside_name} --cwd /tmp --command true --no-yolo \
             > /dev/null 2> ~/.e2e-refused; echo $? > ~/.e2e-refused.status; \
             \"$P\" agent delete --session \"$(cat ~/.e2e-created)\" --stop-if-running \
             > ~/.e2e-deleted 2>&1; echo $? > ~/.e2e-deleted.status; echo done > ~/.e2e-agent-done\r",
            outside_name = farhelm_proto::text::shell_quote(outside["name"].as_str().unwrap()),
        );
        client.send_keys(tab_channel, line.into_bytes()).await;
        wait_for_listing_with(
            || client.stat_file(bare, "/home/ubuntu/.e2e-agent-done"),
            120,
            "the refused create and the delete typed in the container's tab finished",
            |info: &farhelm_proto::DownloadFileInfo| {
                info.status == farhelm_proto::DownloadFileStatus::Ready
            },
        )
        .await;
        let text = |bytes: Vec<u8>| String::from_utf8(bytes).unwrap();
        let hosts = text(read_inside_file(client, bare, "/home/ubuntu/.e2e-hosts").await);
        let own_name = host["name"].as_str().unwrap();
        assert!(
            hosts.contains(own_name),
            "the container lists itself: {hosts}"
        );
        assert!(
            !hosts.contains("this machine"),
            "the container lists no other host: {hosts}"
        );
        let created = text(read_inside_file(client, bare, "/home/ubuntu/.e2e-created").await);
        let created_err =
            text(read_inside_file(client, bare, "/home/ubuntu/.e2e-created.err").await);
        let created = created
            .trim()
            .parse::<farhelm_proto::SessionRef>()
            .unwrap_or_else(|error| panic!("a created session id ({error}): {created_err}"));
        assert_eq!(
            created.entry_uuid(),
            asking.entry_uuid(),
            "the new session is in this container, the asking session's own host entry"
        );
        let refused = text(read_inside_file(client, bare, "/home/ubuntu/.e2e-refused").await);
        let refused_status =
            text(read_inside_file(client, bare, "/home/ubuntu/.e2e-refused.status").await);
        assert_ne!(refused_status.trim(), "0", "another host is refused");
        assert!(
            refused.contains("can only act within their own sandbox or container"),
            "{refused}"
        );
        assert_eq!(
            outside_sessions().await,
            outside_before,
            "the refused create put no session on the outside host"
        );
        let deleted = text(read_inside_file(client, bare, "/home/ubuntu/.e2e-deleted").await);
        let deleted_status =
            text(read_inside_file(client, bare, "/home/ubuntu/.e2e-deleted.status").await);
        assert_eq!(
            deleted_status.trim(),
            "0",
            "the sibling is deleted: {deleted}"
        );
        let listed = client.list_sessions().await.unwrap();
        assert!(
            listed
                .sessions
                .iter()
                .all(|row| row.id != created.bare_id()),
            "deleting the sibling removed it"
        );
        assert!(
            listed.sessions.iter().any(|row| row.id == bare),
            "the asking session carries on in the same container"
        );
        let kept = text(read_inside_file(client, bare, "/home/ubuntu/.e2e-kept").await);
        let kept_err = text(read_inside_file(client, bare, "/home/ubuntu/.e2e-kept.err").await);
        let kept = kept
            .trim()
            .parse::<farhelm_proto::SessionRef>()
            .unwrap_or_else(|error| panic!("a kept session id ({error}): {kept_err}"));
        assert_eq!(
            kept.entry_uuid(),
            asking.entry_uuid(),
            "the kept session is in this container too"
        );
        assert!(
            listed.sessions.iter().any(|row| row.id == kept.bare_id()),
            "deleting the first sibling left the kept one"
        );
        kept
    }

    /// Exercise lifecycle through the public API, using direct Podman only for
    /// independent evidence and the external-stop incident the product recovers.
    async fn exercise(&mut self) {
        self.start_helm().await;
        let place_kind = if self.ssh_place.is_some() {
            "ssh"
        } else {
            "local"
        };
        let hosts = wait_for_listing_with(
            || self.observe("/api/hosts"),
            120,
            "the selected real container place is connected",
            |value: &Value| {
                value["hosts"].as_array().is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row["kind"] == place_kind && row["state"]["phase"] == "connected"
                    })
                })
            },
        )
        .await;
        let place = hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["kind"] == place_kind)
            .unwrap()["id"]
            .as_i64()
            .unwrap();
        let probe_path = format!("/api/hosts/{place}/podman-probe");
        let (status, body) = post(&self.http, &self.url(&probe_path), json!({})).await;
        assert!(status.is_success(), "probe admission: {body}");
        let probe = wait_for_listing_with(
            || self.observe(&probe_path),
            120,
            "the real rootless Podman probe completed",
            |value: &Value| value["checked_at"].is_number() && value["refreshing"] == false,
        )
        .await;
        assert_eq!(
            probe["usable"], true,
            "real Podman prerequisite refused: {probe}"
        );
        assert!(
            probe["reason"].is_null(),
            "real Podman prerequisite refused: {probe}"
        );
        // New environment hosts inherit this test helm's target-relative root.
        // Save it through Settings before the public launch resolves its preview.
        let checkout_root = format!("~/.farhelm-podman-e2e/{}/checkouts", self.recipe);
        for (path, body) in [
            ("/api/checkout-root", json!({"root":checkout_root})),
            ("/api/checkout-spare", json!({"enabled":false})),
        ] {
            let response = self
                .http
                .put(self.url(path))
                .json(&body)
                .send()
                .await
                .unwrap();
            assert!(
                response.status().is_success(),
                "save fixture checkout configuration"
            );
        }
        assert_eq!(
            self.observe("/api/checkout-root").await.unwrap()["root"],
            checkout_root
        );
        // A fictional endpoint token exercises the ordinary catalog-to-host
        // delivery path. The stand-in never calls that endpoint or a vendor.
        let token = format!("fictional-{}", self.recipe);
        let (status, body) = post(
            &self.http,
            &self.url("/api/credentials"),
            json!({"name":self.recipe,"kind":"endpoint_token","secret":token}),
        )
        .await;
        assert!(
            status.is_success(),
            "create fictional endpoint credential: {body}"
        );
        // Metadata-only coverage starts with a successful listing of this
        // fixture's credential, whose name and kind establish the premise.
        let credentials = self.observe("/api/credentials").await.unwrap();
        let rows = credentials
            .as_array()
            .expect("credential catalog is an array");
        assert!(
            rows.iter().any(|credential| {
                credential["name"] == self.recipe && credential["kind"] == "endpoint_token"
            }),
            "the created fixture credential must appear in the catalog"
        );
        assert!(!credentials.to_string().contains(&token));
        let (status, body) = post(
            &self.http,
            &self.url("/api/tensorlake/recipes"),
            json!({
                "name":self.recipe, "recipe": {"base":{"kind":"tensorlake_ubuntu"},
                    "size":{"cpus":1,"memory_gb":1,"disk_gb":10}, "packages":["python3"],
                    "claude":{"base_url":ENDPOINT,"token_credential":self.recipe},
                    "codex":null,"claude_login_credential":null,
                    "codex_login_credential":null,"github_token_credential":null,"tunnels":[]}
            }),
        )
        .await;
        assert!(status.is_success(), "create recipe: {body}");
        let command = format!(
            "python3 -u -c {} {{farhelm_args}}",
            farhelm_proto::text::shell_quote(AGENT)
        );
        let resume = format!(
            "python3 -u -c {} --resume {{conversation}} {{farhelm_args}}",
            farhelm_proto::text::shell_quote(AGENT)
        );
        let response = self
            .http
            .post(self.url("/api/environment-sessions"))
            .json(&json!({
                "recipe":self.recipe,"place":{"kind":"podman_host","host":place},
                "request":{"cwd":"","title":"real Podman lifecycle",
                    "github_checkout":{"repo":PUBLIC_REPO}, "command":{
                    "command":command,"yolo":false,"agent":"claude","resume":resume}}
            }))
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "environment admission: {}",
            response.status()
        );
        let host = response.headers()["x-farhelm-environment-host"]
            .to_str()
            .unwrap()
            .parse::<i64>()
            .unwrap();
        let reply: Value = response.json().await.unwrap();
        assert!(
            reply["error"].is_null(),
            "real environment launch failed: {reply}"
        );
        let session: SessionInfo = serde_json::from_value(reply["session"].clone()).unwrap();
        assert!(session.id.parse::<farhelm_proto::SessionRef>().is_ok());
        let host_view = self.connected(host).await;
        assert_eq!(host_view["kind"], "podman");
        assert_eq!(host_view["is_session_owned"], true);
        let name = host_view["environment"]["name"].as_str().unwrap();
        let inspected = self.inspect(name).await;
        let container = inspected["Id"].as_str().unwrap().to_owned();
        let image = inspected["Image"].as_str().unwrap().to_owned();
        assert_eq!(inspected["Config"]["Labels"]["farhelm.recipe"], self.recipe);
        assert_eq!(inspected["State"]["Running"], true);
        let inside_socket = format!(
            "{}/supervisor.sock",
            host_view["remote_state_dir"].as_str().unwrap()
        );
        require_engine(&[
            "exec",
            "--user",
            "ubuntu",
            &container,
            "test",
            "-S",
            &inside_socket,
        ])
        .await
        .unwrap();
        let initial = wait_for_listing_with(
            || self.agent(&container),
            120,
            "the stand-in publishes its in-container process identity",
            |value: &Value| value["nonce"].is_string(),
        )
        .await;
        self.assert_agent_alive(&container, &initial).await;
        assert_eq!(initial["cwd"], session.cwd);
        let expected_root = format!(
            "/home/ubuntu/.farhelm-podman-e2e/{}/checkouts/",
            self.recipe
        );
        assert!(
            session.cwd.starts_with(&expected_root),
            "managed checkout must be inside its configured container root"
        );
        assert_eq!(
            session.github_repo,
            Some(farhelm_proto::parse_github_repo(PUBLIC_REPO).unwrap())
        );
        assert!(
            session.working_copy.is_some(),
            "the public launch must own a managed checkout"
        );
        let origin = require_engine(&[
            "exec",
            "--user",
            "ubuntu",
            &container,
            "git",
            "-C",
            &session.cwd,
            "remote",
            "get-url",
            "origin",
        ])
        .await
        .unwrap();
        // Git stores the canonical clone URL. The repository parser accepts
        // owner/repo identifiers, so compare the live remote to its URL form.
        assert_eq!(
            origin.trim(),
            farhelm_proto::parse_github_repo(PUBLIC_REPO)
                .unwrap()
                .clone_url()
        );
        let head = require_engine(&[
            "exec",
            "--user",
            "ubuntu",
            &container,
            "git",
            "-C",
            &session.cwd,
            "rev-parse",
            "--verify",
            "HEAD",
        ])
        .await
        .unwrap();
        assert!(
            [40, 64].contains(&head.trim().len())
                && head.trim().bytes().all(|byte| byte.is_ascii_hexdigit()),
            "the inside checkout must have an actual Git HEAD"
        );
        let kept = self
            .ordinary_features(&container, &session, &host_view, &initial, &token)
            .await;
        let session_path = format!("/api/sessions/{}", session.id);
        wait_for_listing_with(
            || self.observe(&session_path),
            120,
            "the genuine hook made the conversation resumable",
            |value: &Value| value["restart_offer"] == "resume",
        )
        .await;

        let (status, body) = post(
            &self.http,
            &self.url(&format!("/api/hosts/{host}/suspend")),
            json!({}),
        )
        .await;
        assert!(status.is_success(), "Suspend: {body}");
        assert_eq!(self.inspect(&container).await["State"]["Paused"], true);
        // SPEC.md, "Container sessions": while the container is paused the
        // helm cannot ask its Farhelm, so a session Delete decides nothing
        // and says so with the marker that makes the page offer the warned
        // whole-container Delete instead. Nothing may be deleted or resumed.
        let (status, unasked, body) = self
            .delete_session(&session.id, "delete_environment_session=true")
            .await;
        assert_eq!(
            status,
            reqwest::StatusCode::CONFLICT,
            "a paused container's session Delete is unasked: {body}"
        );
        assert_eq!(
            unasked.as_deref(),
            Some(farhelm_proto::http::ENVIRONMENT_UNASKED),
            "only the helm's marker offers the warned Delete: {body}"
        );
        assert_eq!(
            self.inspect(&container).await["State"]["Paused"],
            true,
            "an unasked Delete never resumes the container"
        );
        assert!(
            self.listed(&session.id).await,
            "an unasked Delete deleted nothing"
        );
        let (status, body) = post(
            &self.http,
            &self.url(&format!("/api/hosts/{host}/resume")),
            json!({}),
        )
        .await;
        assert!(status.is_success(), "Resume: {body}");
        self.connected(host).await;
        assert_eq!(self.inspect(&container).await["State"]["Paused"], false);
        let after_resume = self.agent(&container).await.unwrap();
        assert_eq!(
            after_resume["nonce"], initial["nonce"],
            "pause/Resume keeps the same process"
        );
        self.advancing_agent(&container, &after_resume).await;

        let (status, body) = post(
            &self.http,
            &self.url(&format!("{session_path}/stop")),
            json!({}),
        )
        .await;
        assert!(status.is_success(), "Stop: {body}");
        let stopped_path = format!("/proc/{}/stat", initial["pid"].as_u64().unwrap());
        wait_for_listing_with(
            || async {
                let stopped = engine(&["exec", &container, "test", "-e", &stopped_path]).await?;
                ensure!(
                    matches!(stopped.status.code(), Some(0 | 1)),
                    "Stop's process probe failed: {}",
                    String::from_utf8_lossy(&stopped.stderr)
                );
                Ok(stopped.status.code())
            },
            120,
            "Stop leaves the owned agent process absent",
            |code: &Option<i32>| *code == Some(1),
        )
        .await;
        assert_eq!(self.inspect(&container).await["State"]["Running"], true);
        let (status, body) = post(
            &self.http,
            &self.url(&format!("{session_path}/restart")),
            json!({}),
        )
        .await;
        assert!(status.is_success(), "Restart after Stop: {body}");
        let restarted = self.new_agent(&container, &initial).await;
        assert_eq!(restarted["conversation"], initial["conversation"]);
        let starts = self.starts(&container).await;
        assert!(
            starts >= 1,
            "premise: the engine records this container's initial start"
        );

        self.start_helm().await;
        self.connected(host).await;
        let after_reconnect = self.agent(&container).await.unwrap();
        assert_eq!(
            after_reconnect["nonce"], restarted["nonce"],
            "a helm restart keeps the same agent"
        );
        self.advancing_agent(&container, &after_reconnect).await;
        assert_eq!(
            self.starts(&container).await,
            starts,
            "a helm restart must reconnect without starting the container"
        );
        let before_stop = self.inspect(&container).await["State"]["StartedAt"].clone();
        require_engine(&["stop", "--time", "30", &container])
            .await
            .unwrap();
        wait_for_listing_with(
            || async { Ok(self.inspect(&container).await) },
            120,
            "external stop is followed by an actual new container start",
            |value: &Value| {
                value["State"]["Running"] == true && value["State"]["StartedAt"] != before_stop
            },
        )
        .await;
        self.connected(host).await;
        // Stopping this container leaves the host's kernel boot unchanged.
        // Ordinary reconciliation reports the ended agent as Exited. A real
        // host reboot instead interrupts sessions last known live, while
        // retaining any exit the supervisor already witnessed.
        let ended = wait_for_listing_with(
            || self.observe(&session_path),
            120,
            "the reconnected supervisor offers conversation Restart for the ended agent",
            |value: &Value| {
                value["status"]["state"] == "exited" && value["restart_offer"] == "resume"
            },
        )
        .await;
        assert_eq!(ended["id"], session.id);
        assert_eq!(
            self.starts(&container).await,
            starts + 1,
            "one automatic start per stopped episode"
        );
        let (status, body) = post(
            &self.http,
            &self.url(&format!("{session_path}/restart")),
            json!({}),
        )
        .await;
        assert!(status.is_success(), "Restart after external stop: {body}");
        let resumed = self.new_agent(&container, &restarted).await;
        assert_eq!(resumed["conversation"], initial["conversation"]);
        assert_eq!(
            self.starts(&container).await,
            starts + 1,
            "session Restart cannot start another container"
        );
        let launches = require_engine(&[
            "exec",
            "--user",
            "ubuntu",
            &container,
            "cat",
            "/home/ubuntu/.farhelm-podman-e2e/launches.jsonl",
        ])
        .await
        .unwrap();
        let launches: Vec<Value> = launches
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            launches.len(),
            3,
            "only the initial launch and two explicit Restarts start an agent"
        );
        assert!(
            launches
                .iter()
                .all(|launch| launch["conversation"] == initial["conversation"])
        );

        let held = self.hold_image(&image).await;
        self.delete_sessions_until_sealed(host, &container, &session.id, &kept.to_string())
            .await;
        let recipe_path = format!("/api/tensorlake/recipes/{}", self.recipe);
        let response = self
            .http
            .delete(self.url(&recipe_path))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let refusal = response.text().await.unwrap();
        assert!(
            !status.is_success() && refusal.contains("in use"),
            "an in-use prepared image must refuse recipe cleanup with its cause: {status}: {refusal}"
        );
        assert!(exists("image", &image).await.unwrap());
        assert!(
            exists("container", &held).await.unwrap(),
            "nonforcing cleanup must leave the unrelated owned container"
        );
        require_engine(&["rm", &held]).await.unwrap();
        assert!(!exists("container", &held).await.unwrap());
        let response = self
            .http
            .delete(self.url(&recipe_path))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body = response.text().await.unwrap();
        assert!(
            status.is_success(),
            "retry recipe cleanup after releasing its image: {body}"
        );
        assert!(self.owned_ids("image").await.unwrap().is_empty());
        assert!(self.owned_ids("container").await.unwrap().is_empty());
        assert!(
            exists("image", UBUNTU).await.unwrap(),
            "the prerequisite base is never owned or removed by this test"
        );
    }

    /// A new launch must replace the old nonce and prove its peer is alive.
    async fn new_agent(&self, container: &str, old: &Value) -> Value {
        let agent = wait_for_listing_with(
            || self.agent(container),
            120,
            "Restart publishes a different live agent",
            |value: &Value| value["nonce"].is_string() && value["nonce"] != old["nonce"],
        )
        .await;
        self.assert_agent_alive(container, &agent).await;
        agent
    }

    /// Send the user's `DELETE /api/sessions/{reference}?{query}` and return
    /// its status, the helm's "unasked" marker if any, and the bounded body.
    /// The marker is returned apart from the text because it, not any prose,
    /// is what tells a page that nothing was decided.
    async fn delete_session(
        &self,
        reference: &str,
        query: &str,
    ) -> (reqwest::StatusCode, Option<String>, String) {
        let response = self
            .http
            .delete(self.url(&format!("/api/sessions/{reference}?{query}")))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let unasked = response
            .headers()
            .get(farhelm_proto::http::ENVIRONMENT_UNASKED_HEADER)
            .map(|value| value.to_str().unwrap().to_owned());
        let body = response.text().await.unwrap();
        assert!(
            body.len() <= OUTPUT_CAP as usize,
            "Delete answer exceeded its bound"
        );
        (status, unasked, body)
    }

    /// Whether the helm's merged session listing names `reference`. The
    /// listing keeps a disconnected host's cached sessions, so this also
    /// answers for a paused container.
    async fn listed(&self, reference: &str) -> bool {
        self.observe("/api/sessions").await.unwrap()["sessions"]
            .as_array()
            .expect("the session listing answered")
            .iter()
            .any(|row| row["id"] == reference)
    }

    /// The user's environment Delete, through the helm's REST route, of the
    /// container's two sessions in turn (SPEC.md, "Sandbox sessions" and
    /// "Container sessions"): the container's own Farhelm decides which
    /// Delete is the last, so the first keeps the container and reports the
    /// count it made, and the second seals it and the helm destroys it before
    /// answering.
    ///
    /// The count must be exactly the one session left; a helm that counted
    /// its own list, or a supervisor that sealed with a session remaining,
    /// fails here. The final checks are independent of the helm's answer:
    /// Podman's own view of the container and the hosts listing.
    async fn delete_sessions_until_sealed(
        &self,
        host: i64,
        container: &str,
        last: &str,
        first: &str,
    ) {
        self.connected(host).await;
        assert!(
            self.listed(first).await && self.listed(last).await,
            "premise: both sessions are in the container before either Delete"
        );
        let (status, _, body) = self
            .delete_session(first, "delete_environment_session=true")
            .await;
        assert!(
            status.is_success(),
            "the first session's environment Delete: {status}: {body}"
        );
        let reply: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            reply["environment_remaining"], 1,
            "the container's Farhelm reports the one session left: {reply}"
        );
        assert!(
            exists("container", container).await.unwrap(),
            "a Delete that leaves a session keeps the container"
        );
        assert_eq!(self.inspect(container).await["State"]["Running"], true);
        self.connected(host).await;
        assert!(
            !self.listed(first).await,
            "the deleted session left the listing"
        );
        assert!(self.listed(last).await, "the other session is still listed");

        let (status, _, body) = self
            .delete_session(last, "delete_environment_session=true")
            .await;
        assert!(
            status.is_success(),
            "the last session's environment Delete: {status}: {body}"
        );
        let reply: Value = serde_json::from_str(&body).unwrap();
        assert!(
            reply.get("environment_remaining").is_none(),
            "the last session's Delete reports no remaining sessions: {reply}"
        );
        assert!(
            !exists("container", container).await.unwrap(),
            "the last session's Delete sealed and destroyed the container before answering"
        );
        assert!(
            self.observe("/api/hosts").await.unwrap()["hosts"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["id"] != host),
            "the destroyed container's host is forgotten"
        );
        assert!(
            self.observe("/api/sessions").await.unwrap()["sessions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["host"] != host),
            "no session of the destroyed container is listed"
        );
    }

    /// Query only the unguessable recipe label allocated by this fixture.
    /// Canonical IDs are deduplicated because an image may have several tags.
    async fn owned_ids(&self, kind: &str) -> anyhow::Result<Vec<String>> {
        let filter = format!("label=farhelm.recipe={}", self.recipe);
        let args = if kind == "container" {
            vec![
                "container",
                "ls",
                "--all",
                "--no-trunc",
                "--filter",
                &filter,
                "--format",
                "{{.ID}}",
            ]
        } else {
            vec![
                "image",
                "ls",
                "--no-trunc",
                "--filter",
                &filter,
                "--format",
                "{{.ID}}",
            ]
        };
        let mut ids: Vec<String> = require_engine(&args)
            .await?
            .lines()
            .map(str::to_owned)
            .collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }

    /// Run on success, panic and scenario timeout. Stop the actor first, then
    /// confirm every owned object absent. Images are never forcibly removed.
    /// Four passes cover the managed-base/software/prepared dependency chain.
    async fn cleanup(&mut self) -> anyhow::Result<()> {
        if let Some(helm) = self.helm.take() {
            helm.stop_and_reap().await;
        }
        if exists("container", &self.hold).await? {
            let held = self.inspect(&self.hold).await;
            ensure!(
                held["Config"]["Labels"]["farhelm.e2e-run"] == self.recipe,
                "the hold name belongs to another run"
            );
            let id = held["Id"]
                .as_str()
                .context("held container has no canonical identity")?;
            require_engine(&["rm", "-f", id]).await?;
            ensure!(
                !exists("container", id).await?,
                "hold container removal is unconfirmed"
            );
        }
        for id in self.owned_ids("container").await? {
            let owned = self.inspect(&id).await;
            ensure!(
                owned["Config"]["Labels"]["farhelm.recipe"] == self.recipe,
                "container ownership changed"
            );
            require_engine(&["rm", "-f", &id]).await?;
            ensure!(
                !exists("container", &id).await?,
                "owned container removal is unconfirmed"
            );
        }
        for _ in 0..4 {
            for id in self.owned_ids("image").await? {
                let value: Value =
                    serde_json::from_str(&require_engine(&["image", "inspect", &id]).await?)?;
                ensure!(
                    value[0]["Config"]["Labels"]["farhelm.recipe"] == self.recipe,
                    "image ownership changed"
                );
                // A child image may still refer to its parent on this pass.
                // Keep it for the next pass. The final query decides success.
                let _ = engine(&["image", "rm", &id]).await?;
            }
            if self.owned_ids("image").await?.is_empty() {
                break;
            }
        }
        ensure!(
            self.owned_ids("container").await?.is_empty(),
            "owned containers remain after cleanup"
        );
        ensure!(
            self.owned_ids("image").await?.is_empty(),
            "owned images remain after nonforcing cleanup"
        );
        Ok(())
    }
}

/// Each selected place gets one recipe build and the same bounded cleanup on
/// success, assertion failure and timeout. The ignored SSH acceptance uses only
/// a preconfigured localhost SSH endpoint and never creates a cloud machine.
async fn run_managed_acceptance(via_ssh: bool) {
    if std::env::var("FARHELM_TEST_SKIP_PODMAN").as_deref() == Ok("1") {
        eprintln!("SKIPPED real Podman environment coverage: FARHELM_TEST_SKIP_PODMAN=1");
        return;
    }
    if via_ssh {
        assert!(
            crate::host_connection::self_ssh_available().await,
            "real SSH Podman acceptance requires strict passwordless ssh localhost. Configure the owned substrate first"
        );
    }
    let payload = prerequisites().await;
    let mut fixture = Fixture::new(payload, via_ssh).await;
    // End the scenario's mutable borrow before cleanup. Catch at each poll so
    // a failed assertion follows the same cleanup path as a timeout, without
    // moving the fixture into a spawned task or adding a dependency for this.
    let result = {
        let mut scenario =
            std::pin::pin!(tokio::time::timeout(SCENARIO_BUDGET, fixture.exercise()));
        std::future::poll_fn(|context| {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                std::future::Future::poll(scenario.as_mut(), context)
            })) {
                Ok(Poll::Pending) => Poll::Pending,
                Ok(Poll::Ready(output)) => Poll::Ready(Ok(output)),
                Err(panic) => Poll::Ready(Err(panic)),
            }
        })
        .await
    };
    let cleanup = tokio::time::timeout(CLEANUP_BUDGET, fixture.cleanup())
        .await
        .context("owned Podman cleanup exceeded its five-minute bound")
        .and_then(|result| result);
    if let Err(error) = cleanup {
        let outcome = match &result {
            Ok(Ok(())) => "passed",
            Ok(Err(_)) => "timed out",
            Err(_) => "panicked",
        };
        panic!(
            "owned Podman cleanup failed for recipe {} and hold {}: {error:#}; scenario {outcome}",
            fixture.recipe, fixture.hold
        );
    }
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => panic!("real Podman lifecycle scenario exceeded its bound: {error}"),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// The real local place covers a public managed checkout, credential delivery,
/// ordinary tabs/files/transfers/TCP relay and the full owned lifecycle. Its
/// stand-in preserves the exact conversation across both explicit Restarts.
/// It ends with the user's environment Deletes: an unasked one while paused,
/// one that keeps the container for the agent-created session, and the last
/// session's, which seals and destroys the container.
#[farhelm_testtrace::test]
async fn real_local_environment_lifecycle_and_owned_image_cleanup() {
    run_managed_acceptance(false).await;
}

/// Run explicitly inside the one authorized owned SSH container-host machine.
/// The existing sandbox shard setup supplies strict passwordless localhost SSH.
/// This variant registers an isolated supervisor there, uses the public SSH place
/// and forwards the ordinary feature protocol through SSH plus podman exec.
/// Keeping it ignored avoids a second expensive build in every local suite and
/// makes missing SSH a failed prerequisite when real acceptance is requested.
#[farhelm_testtrace::test]
#[ignore = "real SSH acceptance requires an explicitly selected owned localhost SSH substrate"]
async fn real_ssh_environment_managed_checkout_and_ordinary_features() {
    run_managed_acceptance(true).await;
}
