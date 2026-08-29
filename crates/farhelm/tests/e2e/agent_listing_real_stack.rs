//! The shipped `farhelm agent` commands, answered by a real helm through
//! the real assembly — separate processes, a real socket, a real
//! attachment.
//!
//! # What this covers that nothing else can
//!
//! Every other test of the agent relay bypasses at least one of the seams
//! that make the feature work in production. `farhelm-helm`'s
//! `agent_requests` tests call the production handler directly with an
//! origin the test built. Its `client` tests construct the handler slot
//! themselves. The supervisor's relay tests (`agent_relay`, next door)
//! install a scripted handler. The process-level CLI tests
//! (`tests/agent_cli.rs`) answer the built command from a mock supervisor.
//!
//! So the STARTUP wiring — the helm installing `HelmAgentRequests` into
//! its connection manager, the manager handing that slot to every
//! connection it dials, and each connection minting the `AgentOrigin` from
//! its own host id and connection id — is exercised by none of them.
//! Delete the installation, reorder it past the manager's start, or build
//! the origin from the wrong host, and every existing test stays green
//! while every shipped `farhelm agent` command fails or marks the wrong
//! row as the asker's own.
//!
//! This test therefore injects NOTHING: it starts the products, creates a
//! session through the helm's own API, attaches to it the way a browser
//! does, and runs the built `farhelm agent hosts` and `farhelm agent
//! sessions` as child processes carrying only the credentials a real
//! session's agent is given.
//!
//! # Why the attachment is part of the fixture
//!
//! The supervisor forwards an upcall to the helm holding an attachment to
//! the ASKING session (`service::agent_relay`). With nothing attached
//! there is no helm to ask and the command is refused — correctly — so
//! attaching is not decoration here: it is what makes the question
//! answerable at all, and it is the same act (a terminal WebSocket) that
//! makes it answerable in production.

use crate::harness::*;

/// Open the helm's terminal WebSocket for `session`, and hold it.
///
/// Hand-rolled down to the upgrade request because no WebSocket client is a
/// dependency of this workspace and this test needs exactly one thing from
/// one: that the upgrade SUCCEEDS, so the helm attaches. It never reads a
/// frame and never sends one — the returned stream exists only to keep the
/// socket, and therefore the attachment, alive for the rest of the test.
///
/// The credential rides `Sec-WebSocket-Protocol` rather than a header,
/// which is not a quirk of the test: browsers cannot set headers on a
/// WebSocket handshake, so this is the only way the helm's own client
/// authenticates one (`farhelm-helm`'s `auth`).
async fn attach_terminal(helm: &HelmProcess, secret: &str, session: &str) -> tokio::net::TcpStream {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let addr = helm.addr();
    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connect to the helm");
    // A fixed key is fine: the accept hash defends against caching proxies,
    // and nothing here verifies it.
    let request = format!(
        "GET /api/sessions/{session}/term HTTP/1.1\r\nHost: {addr}\r\nConnection: Upgrade\r\n\
         Upgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Key: AAAAAAAAAAAAAAAAAAAAAA==\r\n\
         Sec-WebSocket-Protocol: farhelm, farhelm-device-{secret}\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .expect("send the upgrade request");
    // Read exactly to the end of the response headers, leaving anything
    // that followed them in the socket.
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        let n = stream
            .read(&mut byte)
            .await
            .expect("read the upgrade response");
        assert!(n > 0, "the helm closed during the WebSocket upgrade");
        head.push(byte[0]);
    }
    let response = String::from_utf8_lossy(&head).into_owned();
    assert!(
        response.starts_with("HTTP/1.1 101"),
        "the terminal upgrade was refused: {response}"
    );
    stream
}

/// Read one session's credential out of the supervisor's own store.
///
/// This is the credential the supervisor injects into the agent's
/// environment at launch (`launch::SESSION_TOKEN_ENV_VAR`), and reading it
/// from the store is how a test can hand it to a child process without
/// racing the launch shim, which consumes and deletes its spec file as soon
/// as it has read it.
///
/// Polled rather than read once: the store is a live database another
/// process is writing, and a create that has returned over HTTP may still
/// be a moment away from being readable by a second connection.
async fn session_token(state_dir: &std::path::Path, session: &str) -> String {
    let deadline = tokio::time::Instant::now() + REAL_STACK_SETTLE;
    loop {
        if let Ok(store) = SessionStore::open(&state_dir.join("supervisor.db"), false).await
            && let Ok(Some(token)) = store.session_token(session).await
        {
            return token;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the supervisor never published a credential for session {session}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Spawn one built `farhelm agent <verb>` as an agent inside `session`
/// would, and hand back its raw output without judging success.
///
/// Factored out of [`agent_command`] so [`hosts_until_attached`] below can
/// inspect a FAILED attempt's stderr — something `agent_command`'s
/// success-or-panic contract has no way to hand back — while both still
/// build the exact same child, with the exact same three environment
/// variables the production launch shim injects and nothing else (this
/// repo's tests never mutate their own process's environment).
async fn spawn_agent_command(
    verb: &str,
    session: &str,
    token: &str,
    socket: &std::path::Path,
) -> std::process::Output {
    tokio::process::Command::new(farhelm_bin())
        .args(["agent", verb])
        .env(farhelm_supervisor::launch::SESSION_ID_ENV_VAR, session)
        .env(farhelm_supervisor::launch::SESSION_TOKEN_ENV_VAR, token)
        .env(farhelm_supervisor::launch::SUPERVISOR_SOCK_ENV_VAR, socket)
        .output()
        .await
        .expect("run the agent command")
}

/// Run one built `farhelm agent <verb>` as an agent inside `session` would,
/// and return its stdout.
async fn agent_command(verb: &str, session: &str, token: &str, socket: &std::path::Path) -> String {
    let output = spawn_agent_command(verb, session, token, socket).await;
    assert!(
        output.status.success(),
        "`farhelm agent {verb}` failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the listing is UTF-8")
}

/// The supervisor's own refusal text when no helm holds a session's
/// attachment (`service::agent_relay::NO_HELM_ATTACHED`, private to that
/// crate — matched here by the same literal substring `tests/agent_cli.rs`
/// and `e2e/agent_relay.rs` already pin it by, rather than by a shared
/// export).
const NO_HELM_ATTACHED_REFUSAL: &str = "no helm is attached to this session";

/// Run `farhelm agent hosts` for `session`, retrying while the CLI's
/// failure is EXACTLY the supervisor's "no helm is attached" refusal, and
/// return the eventual successful stdout.
///
/// # Why this exists: the upgrade completing is not the attachment existing
///
/// `attach_terminal` above only proves the WebSocket's 101 response
/// arrived. Axum answers that upgrade and hands the socket to
/// `serve_term`'s `on_upgrade` callback as two separate steps, and it is
/// that CALLBACK — running strictly after the 101 already reached this
/// test's raw socket — which sends the helm's `Attach` onward to the
/// supervisor and waits for it to land in `Supervisor::attachments`. So a
/// caller that treats "the upgrade returned" as "the attachment exists"
/// is racing the supervisor, not waiting on it.
///
/// That race is exactly what turned this test flaky on GitHub's loaded,
/// `--test-threads=4` 4-vCPU runner (see this file's own module docs):
/// `farhelm agent hosts` reached `relay_agent_request` before the
/// `attachments` insert did, and got the same
/// [`NO_HELM_ATTACHED_REFUSAL`] a session nobody has open in the UI gets —
/// correctly, for a question asked a moment too early. Locally, and on an
/// unloaded worker, the window is too narrow to ever lose.
///
/// A single frame read on the terminal socket was considered instead and
/// rejected: nothing in `farhelm-helm`'s terminal handler (`serve_term`,
/// `Forwarder::run`) promises that the FIRST frame a freshly attached,
/// freshly created, output-free session ever sends is bound to already
/// having its row in `attachments` — `Forwarder::run` spawns as a
/// separate task while the handler still holds that map's lock, so an
/// aggressively scheduled forwarder could in principle send its
/// `ReplayComplete` marker before the handler's own `attachments.insert`
/// runs. Retrying the CLI call instead polls the one fact this test
/// actually needs, with no assumption about internal task-scheduling
/// order to keep sound.
///
/// Retrying is rearmed by PROGRESS rather than by sleeping a flat budget
/// (this repo's flake-fix convention): each attempt either succeeds, fails
/// with this one specific transient refusal (attach still in flight — try
/// again), or fails some other way, in which case retrying cannot help and
/// the failure is surfaced immediately instead of spending the rest of the
/// 20-second budget on it. Once `hosts` succeeds the attachment is proven
/// present and stays that way for the rest of the test (nothing detaches
/// the held terminal socket), so nothing later needs to repeat this dance.
async fn hosts_until_attached(session: &str, token: &str, socket: &std::path::Path) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let output = spawn_agent_command("hosts", session, token, socket).await;
        if output.status.success() {
            return String::from_utf8(output.stdout).expect("the listing is UTF-8");
        }
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            stderr.contains(NO_HELM_ATTACHED_REFUSAL),
            "`farhelm agent hosts` failed ({}) for a reason retrying cannot fix: {stderr}",
            output.status
        );
        assert!(
            tokio::time::Instant::now() < deadline,
            "the helm's Attach never reached the supervisor's attachments map within 20s \
             (still refused: {stderr})"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// The rows of `output` that the CLI marked with its current-row marker.
///
/// The marker is the one value in either listing that neither the CLI nor
/// the supervisor can reconstruct — only the helm knows which connection
/// the question arrived on — so it is what this test is really reading.
fn marked_rows(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.starts_with('*'))
        .collect()
}

/// Wait until the helm reports its own machine connected.
///
/// Every later step depends on it: a create is routed to the local row, and
/// an upcall can only be answered by a connection that exists. Waiting here
/// turns "the supervisor was not up yet" into one clear failure instead of
/// an unrelated one three steps later.
async fn await_local_host(client: &reqwest::Client, base: &str) {
    let deadline = tokio::time::Instant::now() + REAL_STACK_SETTLE;
    loop {
        let hosts = get_json(client, &format!("{base}/api/hosts")).await;
        let rows = hosts["hosts"].as_array().expect("hosts is an array");
        if rows
            .iter()
            .any(|row| row["kind"] == "local" && row["state"]["phase"] == "connected")
        {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the helm's own host never connected; last seen {hosts}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Spec: an agent inside a real session, running the shipped `farhelm agent
/// hosts` and `farhelm agent sessions`, is answered by the real helm — and
/// exactly its own host and its own session carry the current-row marker.
///
/// The whole point is that nothing is injected. The handler reaches the
/// connection through the helm's own startup, the origin is minted by the
/// connection the manager dialled, the request travels the supervisor's
/// relay over a real unix socket, and the answer is the same listing the
/// UI's panels are built from. Each of those seams is invisible to every
/// other test in the stack (see this module's own docs), and each of them
/// would fail this test loudly.
///
/// The marker assertions are the sharp end. `current` is computed from
/// WHERE the upcall arrived and nowhere else, so a helm that marked the
/// first row, the local row unconditionally, or none at all would satisfy
/// every serialized-shape assertion in the suite while telling an agent it
/// is sitting on a machine it is not.
#[tokio::test]
async fn the_shipped_agent_commands_are_answered_by_the_real_helm() {
    // A supervisor with its own tmux server, plus a session's agent: one
    // harness slot, held for the whole test.
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");

    let supervisor = supervisor_process().await;
    let helm = helm_process(supervisor.state.path(), None).await;
    let secret = device_secret(supervisor.state.path(), &helm.base).await;
    let client = client_with_secret(&secret);
    await_local_host(&client, &helm.base).await;

    let work = farhelm_teststate::tempdir().expect("work dir");
    let (status, body) = post(
        &client,
        &format!("{}/api/sessions", helm.base),
        serde_json::json!({
            "cwd": work.path().to_string_lossy(),
            "invocation": agent_cmd("internal fake-agent --script basic"),
            "title": "the-asking-session",
        }),
    )
    .await;
    assert!(status.is_success(), "creating the session failed: {body}");
    let session: serde_json::Value = serde_json::from_str(&body).expect("created session JSON");
    let session_id = session["id"].as_str().expect("session id").to_string();

    // Held for the rest of the test: the attachment is what tells the
    // supervisor which helm to forward this session's questions to.
    let _terminal = attach_terminal(&helm, &secret, &session_id).await;
    let token = session_token(supervisor.state.path(), &session_id).await;
    let socket = supervisor.state.path().join("supervisor.sock");

    let hosts = hosts_until_attached(&session_id, &token, &socket).await;
    assert!(
        hosts.contains("NAME") && hosts.contains("KIND") && hosts.contains("STATE"),
        "the hosts listing must be the CLI's table: {hosts:?}"
    );
    let marked = marked_rows(&hosts);
    assert_eq!(
        marked.len(),
        1,
        "exactly one host is the asking session's own: {hosts:?}"
    );
    assert!(
        marked[0].contains("this machine") && marked[0].contains("local"),
        "the marked host must be the helm's own machine: {hosts:?}"
    );

    let sessions = agent_command("sessions", &session_id, &token, &socket).await;
    assert!(
        sessions.contains(&session_id),
        "the fleet listing must carry the session that asked: {sessions:?}"
    );
    assert!(
        sessions.contains("the-asking-session"),
        "the listing carries the title the create supplied: {sessions:?}"
    );
    let marked = marked_rows(&sessions);
    assert_eq!(
        marked.len(),
        1,
        "exactly one session is the asker's own: {sessions:?}"
    );
    assert!(
        marked[0].contains(&session_id),
        "the marked row must be the asking session: {sessions:?}"
    );

    // `helm`, `supervisor` and `work` are deliberately still alive here:
    // they hold the processes, the tmux server, and the directories both
    // are still using. See `merged_hosts` for why an explicit drop would
    // only invite someone to reorder them.
}
