//! Fresh GitHub checkout acceptance through the shipped supervisor and helm.
//!
//! Unit tests cover allocation, preparation, and R1.3 reconciliation separately.
//! These tests keep the production seams assembled: the checkout configuration
//! CLI writes helm.db, authenticated REST supplies an accepted preview, real Git
//! clones a private bare repository, and the shipped launch shim runs the hook
//! and a named fake agent. Recovery deliberately loses the first HTTP reply;
//! its oracle comes from the supervisor and filesystem, never that response.

use crate::harness::*;
use crate::structured_launches::{FakeHarness, fake_harness, observed_argv_in_state};
use farhelm_proto::{
    AcceptedGithubPreview, GithubRepo, LaunchEffort, LaunchHarness, LaunchPermission,
    LaunchSelection, ProfileExistence, SessionInfo,
};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PRODUCTION_URL: &str = "https://github.com/acme/bar.git";
const REPO_TEXT: &str = "acme/bar";
const SENTINEL_NAME: &str = "checkout-sentinel.txt";
const SENTINEL_CONTENT: &str = "private checkout fixture\n";

/// A private bare remote plus the Git configuration mapping one exact URL.
///
/// A separate seed working tree populates the remote. The child config denies
/// every protocol by default and enables file only after the exact production
/// URL has been rewritten.
struct GitSource {
    _storage: farhelm_teststate::TestDir,
    home: PathBuf,
    config: PathBuf,
    head: String,
}

impl GitSource {
    /// Build and prove the repository premise before any Farhelm process starts.
    fn new(home: &Path) -> Self {
        let storage = farhelm_teststate::tempdir().expect("Git source storage");
        let seed = storage.path().join("seed");
        let bare = storage.path().join("remote.git");
        let config = home.join("checkout-gitconfig");
        let file_url = format!("file://{}", bare.display());
        assert!(
            !file_url.contains(['\n', '"']),
            "fixture URL must be safe in the private Git config: {file_url:?}"
        );
        std::fs::write(
            &config,
            format!(
                "[protocol]\n\tallow = never\n[protocol \"file\"]\n\tallow = always\n\
                 [url \"{file_url}\"]\n\tinsteadOf = {PRODUCTION_URL}\n"
            ),
        )
        .expect("write private Git config");

        run_command(
            git_command(home, &config, ["init", "-q", path_text(&seed)]),
            "initialize seed",
        );
        run_command(
            git_command(
                home,
                &config,
                [
                    "-C",
                    path_text(&seed),
                    "config",
                    "user.name",
                    "Farhelm Test",
                ],
            ),
            "configure seed user name",
        );
        run_command(
            git_command(
                home,
                &config,
                [
                    "-C",
                    path_text(&seed),
                    "config",
                    "user.email",
                    "farhelm-test@example.invalid",
                ],
            ),
            "configure seed user email",
        );
        std::fs::write(seed.join(SENTINEL_NAME), SENTINEL_CONTENT)
            .expect("write committed sentinel");
        run_command(
            git_command(
                home,
                &config,
                ["-C", path_text(&seed), "add", SENTINEL_NAME],
            ),
            "stage sentinel",
        );
        run_command(
            git_command(
                home,
                &config,
                ["-C", path_text(&seed), "commit", "-q", "-m", "fixture"],
            ),
            "commit sentinel",
        );
        let head = command_stdout(
            git_command(home, &config, ["-C", path_text(&seed), "rev-parse", "HEAD"]),
            "read seed HEAD",
        );
        run_command(
            git_command(
                home,
                &config,
                ["clone", "-q", "--bare", path_text(&seed), path_text(&bare)],
            ),
            "create bare source",
        );
        assert_eq!(
            command_stdout(
                git_command(
                    home,
                    &config,
                    ["--git-dir", path_text(&bare), "rev-parse", "HEAD"],
                ),
                "read bare HEAD",
            ),
            head,
            "the bare source must carry the seed commit"
        );
        assert_eq!(
            command_stdout(
                git_command(
                    home,
                    &config,
                    [
                        "--git-dir",
                        path_text(&bare),
                        "show",
                        &format!("HEAD:{SENTINEL_NAME}"),
                    ],
                ),
                "read bare sentinel",
            ) + "\n",
            SENTINEL_CONTENT,
            "the clone source must expose the committed sentinel"
        );

        let probe = git_command(home, &config, ["ls-remote", PRODUCTION_URL, "HEAD"]);
        let resolved = command_stdout(probe, "resolve the exact production URL");
        assert_eq!(
            resolved,
            format!("{head}\tHEAD"),
            "the production URL must resolve to the private committed source"
        );

        Self {
            _storage: storage,
            home: home.to_path_buf(),
            config,
            head,
        }
    }

    /// Prove one product-created checkout came from this exact private source.
    fn assert_clone(&self, cwd: &str) {
        assert_eq!(
            command_stdout(
                git_command(&self.home, &self.config, ["-C", cwd, "rev-parse", "HEAD"],),
                "read cloned HEAD",
            ),
            self.head,
            "the checkout must resolve to the private source commit"
        );
        assert_eq!(
            command_stdout(
                git_command(
                    &self.home,
                    &self.config,
                    ["-C", cwd, "config", "--get", "remote.origin.url"],
                ),
                "read cloned origin URL",
            ),
            PRODUCTION_URL,
            "the checkout must retain the exact production URL as its origin"
        );
    }
}

/// One live connection claim copied from the authenticated host view.
#[derive(Clone, Debug)]
struct HostClaim {
    id: Value,
    incarnation: u64,
    identity: String,
}

/// The owned real stack and every filesystem input its children may read.
///
/// Field order keeps the helm ahead of the supervisor and both processes
/// ahead of their private homes, Git source, roots, and marker directory.
struct CheckoutStack {
    helm: Option<HelmProcess>,
    supervisor: SupervisorProcess,
    _fake: FakeHarness,
    git: GitSource,
    root_a: farhelm_teststate::TestDir,
    root_b: farhelm_teststate::TestDir,
    markers: farhelm_teststate::TestDir,
    secret: String,
    client: reqwest::Client,
    claim: HostClaim,
    _slot: tokio::sync::SemaphorePermit<'static>,
}

impl CheckoutStack {
    /// Start the shipped processes with child-local Git and fake-agent inputs.
    async fn start() -> Self {
        let slot = SLOTS.acquire().await.expect("semaphore is never closed");
        let fake = fake_harness();
        let home = PathBuf::from(fake.login_home_with_fake_path());
        let git = GitSource::new(&home);
        let root_a = farhelm_teststate::tempdir().expect("checkout root A");
        let root_b = farhelm_teststate::tempdir().expect("checkout root B");
        let markers = farhelm_teststate::tempdir().expect("checkout markers");
        let inherited_git_environment = std::env::vars_os()
            .filter_map(|(key, _)| key.to_string_lossy().starts_with("GIT_").then_some(key));
        let supervisor = supervisor_process_with_env_removed(
            [
                ("HOME", home.into_os_string()),
                ("SHELL", fake.bash_shell()),
                ("GIT_CONFIG_GLOBAL", git.config.as_os_str().to_os_string()),
                ("GIT_CONFIG_NOSYSTEM", OsString::from("1")),
                ("GIT_TERMINAL_PROMPT", OsString::from("0")),
                ("GIT_ALLOW_PROTOCOL", OsString::from("file")),
            ],
            inherited_git_environment,
        )
        .await;
        wait_for_supervisor_ready(supervisor.state.path()).await;
        let helm = helm_process(supervisor.state.path(), None).await;
        let secret = device_secret(supervisor.state.path(), &helm.base).await;
        let client = client_with_secret(&secret);
        let claim = await_local_claim(&client, &helm.base).await;
        let mut stack = Self {
            helm: Some(helm),
            supervisor,
            _fake: fake,
            git,
            root_a,
            root_b,
            markers,
            secret,
            client,
            claim,
            _slot: slot,
        };
        stack.configure_a().await;
        stack
    }

    fn helm(&self) -> &HelmProcess {
        self.helm.as_ref().expect("the owned helm is running")
    }

    fn hook_a(&self) -> PathBuf {
        self.markers.path().join("hook-a.log")
    }

    fn hook_b(&self) -> PathBuf {
        self.markers.path().join("hook-b.log")
    }

    /// Establish the settings the original requests will accept through the
    /// shipped CLI, then read them back before any preview depends on them.
    async fn configure_a(&mut self) {
        set_checkout_config(
            self.supervisor.state.path(),
            self.root_a.path(),
            &self.hook_a(),
        )
        .await;
        let shown = checkout_config(self.supervisor.state.path(), [OsString::from("show")]).await;
        assert!(
            shown.contains(&format!("root: {}", self.root_a.path().display()))
                && shown.contains(self.hook_a().to_string_lossy().as_ref()),
            "checkout config must report root A and hook A: {shown}"
        );
    }

    /// Replace both mutable checkout inputs and prove the old hook is gone.
    /// Recovery must continue using A even though new requests now resolve B.
    async fn configure_b(&mut self) {
        set_checkout_config(
            self.supervisor.state.path(),
            self.root_b.path(),
            &self.hook_b(),
        )
        .await;
        let shown = checkout_config(self.supervisor.state.path(), [OsString::from("show")]).await;
        assert!(
            shown.contains(&format!("root: {}", self.root_b.path().display()))
                && shown.contains(self.hook_b().to_string_lossy().as_ref())
                && !shown.contains(self.hook_a().to_string_lossy().as_ref()),
            "checkout config must report only root B and hook B: {shown}"
        );
    }

    /// Reboot only the helm, then obtain a fresh browser credential and claim.
    async fn restart_helm(&mut self) {
        self.helm
            .take()
            .expect("the old helm is running")
            .stop_and_reap()
            .await;
        let helm = helm_process(self.supervisor.state.path(), None).await;
        let secret = device_secret(self.supervisor.state.path(), &helm.base).await;
        let client = client_with_secret(&secret);
        let claim = await_local_claim(&client, &helm.base).await;
        assert_eq!(
            claim.identity, self.claim.identity,
            "a helm restart must reconnect to the same supervisor installation"
        );
        self.helm = Some(helm);
        self.secret = secret;
        self.client = client;
        self.claim = claim;
    }
}

/// The four launch modes whose accepted snapshots must survive recovery.
#[derive(Clone, Debug)]
enum Selector {
    Raw,
    Structured(LaunchSelection),
    ProfileId,
    ProfileName,
}

/// One original request and the authoritative outcome observed without its reply.
struct AcceptedCase {
    label: &'static str,
    selector: Selector,
    generation: u32,
    body: Vec<u8>,
    preview: AcceptedGithubPreview,
    session: SessionInfo,
}

impl AcceptedCase {
    /// Keep the original request snapshot beside the server-side result it must replay.
    fn new(
        label: &'static str,
        selector: Selector,
        generation: u32,
        body: Vec<u8>,
        preview: AcceptedGithubPreview,
        session: SessionInfo,
    ) -> Self {
        Self {
            label,
            selector,
            generation,
            body,
            preview,
            session,
        }
    }
}

/// Require a fixture command to succeed when its output has no later use.
fn run_command(command: std::process::Command, what: &str) {
    let _ = command_output(command, what);
}

/// Run a fixture command and retain both streams in any premise failure.
fn command_output(mut command: std::process::Command, what: &str) -> std::process::Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{what}: {error}"));
    assert!(
        output.status.success(),
        "{what} failed ({}): stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Return one successful fixture command's UTF-8 stdout without its final newline.
fn command_stdout(command: std::process::Command, what: &str) -> String {
    String::from_utf8(command_output(command, what).stdout)
        .unwrap_or_else(|error| panic!("{what} stdout is not UTF-8: {error}"))
        .trim_end()
        .to_string()
}

/// Build a Git child whose only configuration comes from fixture-owned state.
fn git_command<const N: usize>(
    home: &Path,
    config: &Path,
    args: [&str; N],
) -> std::process::Command {
    let mut command = std::process::Command::new("git");
    command.env_clear();
    command
        .env(
            "PATH",
            std::env::var_os("PATH").expect("the test runner supplies PATH"),
        )
        .env("HOME", home)
        .env("GIT_CONFIG_GLOBAL", config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ALLOW_PROTOCOL", "file");
    command.args(args);
    command
}

/// Keep fixture path conversion failures at the boundary that introduced them.
fn path_text(path: &Path) -> &str {
    path.to_str().expect("fixture paths are UTF-8")
}

/// Run the actual checkout-config CLI against the fixture helm database.
async fn checkout_config(state: &Path, args: impl IntoIterator<Item = OsString>) -> String {
    let output = tokio::process::Command::new(farhelm_bin())
        .args(["helm", "checkout-config"])
        .args(args)
        .arg("--state-dir")
        .arg(state)
        .output()
        .await
        .expect("run checkout-config");
    assert!(
        output.status.success(),
        "checkout-config failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("checkout-config output is UTF-8")
}

/// Change both effective fields through separate shipped CLI transactions.
async fn set_checkout_config(state: &Path, root: &Path, hook_marker: &Path) {
    checkout_config(
        state,
        [OsString::from("set-root"), root.as_os_str().to_os_string()],
    )
    .await;
    let hook = format!(
        "printf '%s\\n' \"$PWD\" >> {}",
        shell_words::quote(&hook_marker.to_string_lossy())
    );
    checkout_config(
        state,
        [OsString::from("set-post-clone"), OsString::from(hook)],
    )
    .await;
}

/// Wait for the local row's connected claim, retaining the last host reply.
async fn await_local_claim(client: &reqwest::Client, base: &str) -> HostClaim {
    let deadline = tokio::time::Instant::now() + REAL_STACK_SETTLE;
    let mut last = None;
    loop {
        let response =
            tokio::time::timeout_at(deadline, client.get(format!("{base}/api/hosts")).send())
                .await
                .unwrap_or_else(|_| {
                    panic!("local-host readiness timed out; last response: {last:?}")
                })
                .expect("local-host request reached the helm");
        let status = response.status();
        let body = tokio::time::timeout_at(deadline, response.text())
            .await
            .unwrap_or_else(|_| panic!("local-host body timed out; last response: {last:?}"))
            .expect("read local-host body");
        assert!(
            status.is_success(),
            "host readiness answered {status}: {body}"
        );
        let value: Value = serde_json::from_str(&body).expect("host readiness JSON");
        if let Some(row) = value["hosts"].as_array().and_then(|rows| {
            rows.iter()
                .find(|row| row["kind"] == "local" && row["state"]["phase"] == "connected")
        }) {
            let identity = row["identity"]
                .as_str()
                .expect("connected local host has a stable identity")
                .to_string();
            assert!(
                !identity.is_empty(),
                "local installation identity is nonempty"
            );
            return HostClaim {
                id: row["id"].clone(),
                incarnation: row["incarnation"]
                    .as_u64()
                    .expect("incarnation is a number"),
                identity,
            };
        }
        last = Some(value);
        assert!(
            tokio::time::Instant::now() < deadline,
            "the local host never connected; last response: {last:?}"
        );
        // sleep-ok: poll the authenticated local-host connection claim inside one deadline.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Obtain and validate the accepted preview the create body will freeze.
async fn preview(stack: &CheckoutStack, title: &str) -> AcceptedGithubPreview {
    let (status, body) = post(
        &stack.client,
        &format!("{}/api/github-checkout-preview", stack.helm().base),
        json!({
            "host": stack.claim.id,
            "expected_incarnation": stack.claim.incarnation,
            "repo": REPO_TEXT,
            "title": title,
        }),
    )
    .await;
    assert!(status.is_success(), "checkout preview failed: {body}");
    let preview: AcceptedGithubPreview =
        serde_json::from_str(&body).expect("accepted checkout preview JSON");
    assert_eq!(preview.incarnation, stack.claim.incarnation);
    assert_eq!(preview.installation_identity, stack.claim.identity);
    preview
}

/// Construct the exact immutable create bytes for one selector.
fn create_body(
    claim: &HostClaim,
    preview: &AcceptedGithubPreview,
    title: &str,
    key: &str,
    selector: &Selector,
) -> Vec<u8> {
    let mut body = json!({
        "cwd": "",
        "title": title,
        "host": claim.id,
        "expected_incarnation": claim.incarnation,
        "intent_key": key,
        "github_checkout": {
            "repo": REPO_TEXT,
            "title": title,
            "preview": preview,
        },
    });
    let object = body.as_object_mut().expect("create body is an object");
    match selector {
        Selector::Raw => {
            object.insert("invocation".into(), json!("codex --yolo"));
        }
        Selector::Structured(selection) => {
            object.insert("launch".into(), serde_json::to_value(selection).unwrap());
        }
        Selector::ProfileId => {
            object.insert("profile_id".into(), json!("builtin-codex-yolo"));
        }
        Selector::ProfileName => {
            object.insert("profile_name".into(), json!("codex-yolo"));
        }
    }
    serde_json::to_vec(&body).expect("serialize create body")
}

/// Write a complete authenticated POST but leave the response entirely unread.
async fn send_unread_create(stack: &CheckoutStack, body: &[u8]) -> tokio::net::TcpStream {
    let addr = stack.helm().addr();
    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connect raw create stream");
    let head = format!(
        "POST /api/sessions HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
        stack.secret,
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .await
        .expect("write raw create headers");
    stream
        .write_all(body)
        .await
        .expect("write complete raw create body");
    stream.flush().await.expect("flush raw create request");
    stream
}

/// Read one response from the exact raw writer used by the loss test.
async fn read_raw_response(mut stream: tokio::net::TcpStream) -> (u16, String) {
    tokio::time::timeout(REAL_STACK_SETTLE, async {
        let mut bytes = Vec::new();
        let header_end = loop {
            assert!(
                bytes.len() < 64 * 1024,
                "HTTP response headers exceeded 64 KiB"
            );
            let mut chunk = [0u8; 4096];
            let read = stream.read(&mut chunk).await.expect("read raw response");
            assert!(read > 0, "helm closed before completing response headers");
            bytes.extend_from_slice(&chunk[..read]);
            if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let head = std::str::from_utf8(&bytes[..header_end]).expect("HTTP headers are UTF-8");
        let status = head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u16>().ok())
            .expect("HTTP response has a numeric status");
        let content_length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .expect("JSON response carries Content-Length");
        while bytes.len() - header_end < content_length {
            let mut chunk = [0u8; 4096];
            let read = stream
                .read(&mut chunk)
                .await
                .expect("read raw response body");
            assert!(read > 0, "helm closed before completing response body");
            bytes.extend_from_slice(&chunk[..read]);
        }
        let body = String::from_utf8(bytes[header_end..header_end + content_length].to_vec())
            .expect("HTTP response body is UTF-8");
        (status, body)
    })
    .await
    .expect("raw create response arrived within the settle budget")
}

/// Connect a second client directly to the owned supervisor acceptance oracle.
async fn supervisor_client(state: &Path) -> Arc<SupervisorClient> {
    let transport = farhelm_supervisor::service::connect(state)
        .await
        .expect("dial owned supervisor");
    let (reader, writer) = tokio::io::split(transport);
    SupervisorClient::start(reader, writer)
        .await
        .expect("handshake owned supervisor client")
}

/// Learn the accepted session from the server, independent of its lost reply.
async fn await_server_session(state: &Path, title: &str) -> SessionInfo {
    let client = supervisor_client(state).await;
    let deadline = tokio::time::Instant::now() + REAL_STACK_SETTLE;
    let mut last = None;
    loop {
        let listing = tokio::time::timeout_at(deadline, client.list_sessions())
            .await
            .unwrap_or_else(|_| {
                panic!("server-side session oracle timed out; last listing: {last:?}")
            })
            .expect("server-side session listing");
        if let Some(session) = listing
            .sessions
            .iter()
            .find(|session| session.title == title)
        {
            assert!(
                session.github_repo.is_some(),
                "fresh session must carry repo provenance"
            );
            assert!(
                session.working_copy.is_some(),
                "fresh session must carry checkout identity"
            );
            return session.clone();
        }
        last = Some(listing.sessions);
        assert!(
            tokio::time::Instant::now() < deadline,
            "server never listed {title:?}; last listing: {last:?}"
        );
        // sleep-ok: poll the authoritative supervisor list for this accepted title inside one deadline.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Wait for clone and hook outputs, then prove the fake agent is live.
async fn await_prepared(
    stack: &CheckoutStack,
    session: &SessionInfo,
    hook_marker: &Path,
    generation: u32,
) -> String {
    let deadline = tokio::time::Instant::now() + REAL_STACK_SETTLE;
    let sentinel = Path::new(&session.cwd).join(SENTINEL_NAME);
    let mut last_sentinel;
    let mut last_hook;
    loop {
        last_sentinel = std::fs::read_to_string(&sentinel).ok();
        last_hook = std::fs::read_to_string(hook_marker).ok();
        let hook_ready = last_hook
            .as_deref()
            .is_some_and(|text| text.lines().filter(|line| *line == session.cwd).count() == 1);
        if last_sentinel.as_deref() == Some(SENTINEL_CONTENT) && hook_ready {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "checkout preparation never completed for {}: sentinel={last_sentinel:?}, hook={last_hook:?}",
            session.id
        );
        // sleep-ok: poll the committed clone sentinel and completed hook marker inside one deadline.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    stack.git.assert_clone(&session.cwd);
    wait_for_agent_ready(
        &stack.supervisor.state.path().join("tmux.sock"),
        &session.id,
    )
    .await;
    observed_argv_in_state(stack.supervisor.state.path(), &session.id, generation).await
}

/// Assert immutable checkout identity and selector-specific launch metadata.
fn assert_case_metadata(session: &SessionInfo, case: &AcceptedCase, browser_reply: bool) {
    assert_eq!(session.id, case.session.id, "{} session id", case.label);
    assert_eq!(session.cwd, case.preview.binding.cwd, "{} cwd", case.label);
    assert_eq!(session.canonical_cwd.as_deref(), Some(session.cwd.as_str()));
    assert_eq!(
        session.github_repo,
        Some(GithubRepo {
            owner: "acme".into(),
            name: "bar".into(),
        }),
        "{} repository provenance",
        case.label
    );
    let working_copy = session
        .working_copy
        .as_ref()
        .unwrap_or_else(|| panic!("{} checkout identity", case.label));
    assert_eq!(
        working_copy.id,
        case.session.working_copy.as_ref().unwrap().id
    );
    assert_eq!(working_copy.canonical_path, session.cwd);
    assert_eq!(working_copy.origin_session_id, session.id);
    match &case.selector {
        Selector::Raw => {
            assert_eq!(session.invocation, "codex --yolo");
            assert!(session.launch.is_none());
            assert!(session.source_profile.is_none());
        }
        Selector::Structured(selection) => {
            assert_eq!(session.launch.as_ref(), Some(selection));
            assert!(session.source_profile.is_none());
            assert!(session.invocation.starts_with("codex "));
        }
        Selector::ProfileId | Selector::ProfileName => {
            assert!(session.launch.is_none());
            assert_eq!(session.invocation, "codex --yolo");
            let profile = session
                .source_profile
                .as_ref()
                .expect("profile-backed session carries its snapshot");
            assert_eq!(profile.id, "builtin-codex-yolo");
            assert_eq!(profile.name, "codex-yolo");
            assert_eq!(
                profile.existence,
                if browser_reply {
                    ProfileExistence::Present
                } else {
                    ProfileExistence::Unresolved
                }
            );
        }
    }
}

/// Prove the selected launch reached the fake executable after hook integration.
fn assert_process_argv(case: &AcceptedCase, argv: &str) {
    let words = shell_words::split(argv).expect("fake executable argv is shell-safe");
    match &case.selector {
        Selector::Raw | Selector::ProfileId | Selector::ProfileName => {
            assert_eq!(
                words.first().map(String::as_str),
                Some("--yolo"),
                "{} must launch the selected invocation before hook integration arguments: {argv}",
                case.label
            );
        }
        Selector::Structured(selection) => {
            assert_eq!(selection.harness, LaunchHarness::Codex);
            assert!(words.windows(2).any(|pair| pair == ["-m", "gpt-5.6-terra"]));
            assert!(
                words
                    .iter()
                    .any(|word| word == "model_reasoning_effort=high")
            );
            assert!(words.iter().any(|word| word == "--yolo"));
        }
    }
}

/// Count only the exact checkout path written by the configured hook.
fn hook_count(path: &Path, cwd: &str) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|line| *line == cwd)
        .count()
}

/// Distinguish replay from a second launch by inspecting the session's own pane.
async fn assert_one_agent_execution(stack: &CheckoutStack, case: &AcceptedCase) {
    let output = tmux_query(
        &stack.supervisor.state.path().join("tmux.sock"),
        &[
            "capture-pane",
            "-p",
            "-S",
            "-",
            "-t",
            &format!("fh-{}", case.session.id),
        ],
    )
    .await;
    assert!(
        output.status.success(),
        "capture {} terminal: {}",
        case.label,
        String::from_utf8_lossy(&output.stderr)
    );
    let text = normalize_pane_text(&output.stdout);
    assert_eq!(
        text.matches("STRUCTURED-LAUNCH-GENERATION:").count(),
        1,
        "{} must have one fake executable start: {text}",
        case.label
    );
    assert!(
        text.contains(&format!("STRUCTURED-LAUNCH-GENERATION:{}", case.generation)),
        "{} must retain its original process generation: {text}",
        case.label
    );
}

/// Return a stable view of every checkout basename allocated under one root.
fn directory_names(root: &Path) -> Vec<String> {
    let mut names = std::fs::read_dir(root)
        .expect("read checkout root")
        .map(|entry| {
            entry
                .expect("read checkout entry")
                .file_name()
                .into_string()
                .expect("checkout basename is UTF-8")
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

/// Resubmit preserved serialized bytes without rebuilding their request identity.
async fn post_exact_body(client: &reqwest::Client, base: &str, body: Vec<u8>) -> reqwest::Response {
    client
        .post(format!("{base}/api/sessions"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await
        .expect("exact create body reached the helm")
}

/// The diagnostic proves the exact raw writer reaches a successful real clone.
///
/// It stays separate from recovery so malformed HTTP, Git rewrite, and launch
/// failures have a response and one narrow boundary.
#[farhelm_testtrace::test]
async fn a_raw_http_fresh_checkout_completes_the_real_clone_hook_and_agent() {
    let stack = CheckoutStack::start().await;
    let title = "raw-diagnostic";
    let selector = Selector::Raw;
    let accepted = preview(&stack, title).await;
    let body = create_body(&stack.claim, &accepted, title, "diagnostic-key", &selector);
    let stream = send_unread_create(&stack, &body).await;
    let (status, body) = read_raw_response(stream).await;
    assert_eq!(status, 200, "diagnostic create failed: {body}");
    let session: SessionInfo = serde_json::from_str(&body).expect("diagnostic session JSON");
    let case = AcceptedCase::new(title, selector, 1, Vec::new(), accepted, session.clone());
    assert_case_metadata(&session, &case, true);
    let argv = await_prepared(&stack, &session, &stack.hook_a(), 1).await;
    assert_process_argv(&case, &argv);
    assert_eq!(
        stack.git.head.len(),
        40,
        "fixture uses a full Git object id"
    );
}

/// R1.3 acceptance: each supported selector replays under its accepted snapshot.
///
/// The first replies are never read. After server-side readiness, root and hook
/// change, the helm is reaped and restarted, and byte-identical bodies retry.
/// A stale key is refused, while a fresh preview and key allocate under B.
#[farhelm_testtrace::test]
async fn lost_fresh_checkout_success_replays_after_settings_change_and_helm_restart() {
    let mut stack = CheckoutStack::start().await;
    let selectors = [
        ("raw-recovery", Selector::Raw),
        (
            "structured-recovery",
            Selector::Structured(LaunchSelection {
                harness: LaunchHarness::Codex,
                model: Some("gpt-5.6-terra".into()),
                effort: Some(LaunchEffort::High),
                permissions: Some(LaunchPermission::Yolo),
            }),
        ),
        ("profile-id-recovery", Selector::ProfileId),
        ("profile-name-recovery", Selector::ProfileName),
    ];
    let mut cases = Vec::new();
    for (index, (label, selector)) in selectors.into_iter().enumerate() {
        let accepted = preview(&stack, label).await;
        assert_eq!(
            accepted.binding.canonical_root,
            stack.root_a.path().to_string_lossy()
        );
        let body = create_body(
            &stack.claim,
            &accepted,
            label,
            &format!("{label}-key"),
            &selector,
        );
        let unread = send_unread_create(&stack, &body).await;
        let session = await_server_session(stack.supervisor.state.path(), label).await;
        let generation = u32::try_from(index + 1).unwrap();
        let case = AcceptedCase::new(label, selector, generation, body, accepted, session);
        assert_case_metadata(&case.session, &case, false);
        let argv = await_prepared(&stack, &case.session, &stack.hook_a(), generation).await;
        assert_process_argv(&case, &argv);
        drop(unread);
        cases.push(case);
    }

    let mut expected_a = cases
        .iter()
        .map(|case| case.preview.binding.basename.clone())
        .collect::<Vec<_>>();
    expected_a.sort();
    assert_eq!(directory_names(stack.root_a.path()), expected_a);
    for case in &cases {
        assert_eq!(hook_count(&stack.hook_a(), &case.session.cwd), 1);
        assert_one_agent_execution(&stack, case).await;
    }

    stack.configure_b().await;
    stack.restart_helm().await;

    for case in &cases {
        let response = post_exact_body(&stack.client, &stack.helm().base, case.body.clone()).await;
        let status = response.status();
        let text = response.text().await.expect("read replay response");
        assert!(status.is_success(), "{} replay failed: {text}", case.label);
        let replayed: SessionInfo = serde_json::from_str(&text).expect("replayed session JSON");
        assert_case_metadata(&replayed, case, true);
        let argv = observed_argv_in_state(
            stack.supervisor.state.path(),
            &case.session.id,
            case.generation,
        )
        .await;
        assert_process_argv(case, &argv);
    }

    assert_eq!(directory_names(stack.root_a.path()), expected_a);
    assert!(
        directory_names(stack.root_b.path()).is_empty(),
        "replays must not allocate under root B"
    );
    assert!(
        !stack.hook_b().exists(),
        "replays must not execute the replacement hook"
    );
    for case in &cases {
        assert_eq!(hook_count(&stack.hook_a(), &case.session.cwd), 1);
        assert_one_agent_execution(&stack, case).await;
    }

    let mut stale: Value = serde_json::from_slice(&cases[0].body).unwrap();
    stale["intent_key"] = json!("never-accepted-after-change");
    let response = post_exact_body(
        &stack.client,
        &stack.helm().base,
        serde_json::to_vec(&stale).unwrap(),
    )
    .await;
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(
        response
            .headers()
            .get("x-farhelm-create-outcome")
            .and_then(|value| value.to_str().ok()),
        Some("definitely-unaccepted")
    );
    let refusal = response.text().await.expect("read stale-key refusal");
    assert!(
        refusal.contains("checkout settings changed"),
        "stale unknown key must name the changed settings: {refusal}"
    );
    assert!(directory_names(stack.root_b.path()).is_empty());
    assert!(!stack.hook_b().exists());

    let fresh_title = "fresh-under-root-b";
    let fresh_preview = preview(&stack, fresh_title).await;
    assert_eq!(
        fresh_preview.binding.canonical_root,
        stack.root_b.path().to_string_lossy()
    );
    let fresh_selector = Selector::Raw;
    let fresh_body = create_body(
        &stack.claim,
        &fresh_preview,
        fresh_title,
        "fresh-root-b-key",
        &fresh_selector,
    );
    let response = post_exact_body(&stack.client, &stack.helm().base, fresh_body).await;
    let status = response.status();
    let text = response.text().await.expect("read fresh-B response");
    assert!(status.is_success(), "fresh root-B create failed: {text}");
    let fresh: SessionInfo = serde_json::from_str(&text).expect("fresh-B session JSON");
    assert_eq!(fresh.cwd, fresh_preview.binding.cwd);
    assert_eq!(
        directory_names(stack.root_b.path()),
        std::slice::from_ref(&fresh_preview.binding.basename)
    );
    let fresh_case = AcceptedCase::new(
        fresh_title,
        fresh_selector,
        5,
        Vec::new(),
        fresh_preview,
        fresh.clone(),
    );
    assert_case_metadata(&fresh, &fresh_case, true);
    let argv = await_prepared(&stack, &fresh, &stack.hook_b(), 5).await;
    assert_process_argv(&fresh_case, &argv);
    assert_eq!(hook_count(&stack.hook_b(), &fresh.cwd), 1);
    assert_eq!(directory_names(stack.root_a.path()), expected_a);
}
