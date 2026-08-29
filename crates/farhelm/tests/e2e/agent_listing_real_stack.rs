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
//! This module therefore injects NOTHING: it starts the products, creates
//! a session (or two) through the helm's own API, attaches to it the way a
//! browser does, and runs the built `farhelm agent` commands as child
//! processes carrying only the credentials a real session's agent is
//! given — both the two read-only listings (`hosts`/`sessions`) and, in
//! this file's second test, the three lifecycle verbs
//! (`rename`/`stop`/`archive`), each verified against the real,
//! post-mutation state a browser's own REST call would see.
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
use farhelm_proto::{STOP_ANNOTATION, SessionInfo, SessionStatus};

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

/// Spawn one built `farhelm agent <verb...>` as an agent inside `session`
/// would, and hand back its raw output without judging success.
///
/// Exists (rather than folding straight into [`agent_command_args`]) so
/// [`hosts_until_attached`] below can inspect a FAILED attempt's stderr —
/// something `agent_command_args`'s success-or-panic contract has no way to
/// hand back — while both still build the exact same child, with the exact
/// same three environment variables the production launch shim injects and
/// nothing else (this repo's tests never mutate their own process's
/// environment).
///
/// Takes a whole ARGV slice rather than a single verb word — every call
/// site names `hosts`/`sessions` as a one-element slice — because a
/// forwarding single-verb wrapper would be exactly as long as the slice
/// literal it replaced at each of the (few) call sites.
async fn spawn_agent_command_args(
    args: &[&str],
    session: &str,
    token: &str,
    socket: &std::path::Path,
) -> std::process::Output {
    tokio::process::Command::new(farhelm_bin())
        .arg("agent")
        .args(args)
        .env(farhelm_supervisor::launch::SESSION_ID_ENV_VAR, session)
        .env(farhelm_supervisor::launch::SESSION_TOKEN_ENV_VAR, token)
        .env(farhelm_supervisor::launch::SUPERVISOR_SOCK_ENV_VAR, socket)
        .output()
        .await
        .expect("run the agent command")
}

/// Run one built `farhelm agent <verb...>` as an agent inside `session`
/// would, and return its stdout — the lifecycle verbs' extra positional and
/// `--session` arguments fit the same `args` slice `hosts`/`sessions` pass
/// as a single element.
async fn agent_command_args(
    args: &[&str],
    session: &str,
    token: &str,
    socket: &std::path::Path,
) -> String {
    let output = spawn_agent_command_args(args, session, token, socket).await;
    assert!(
        output.status.success(),
        "`farhelm agent {args:?}` failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the command's stdout is UTF-8")
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
        let output = spawn_agent_command_args(&["hosts"], session, token, socket).await;
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

    let sessions = agent_command_args(&["sessions"], &session_id, &token, &socket).await;
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

/// Spec: the shipped `farhelm agent rename/stop/archive`, run against the
/// real stack, each produce the UI-visible effect `GET /api/sessions`
/// reports — a real rename, a real process kill, and a real archive —
/// through the exact assembly a browser's own REST call would use.
///
/// This is the lifecycle counterpart to
/// [`the_shipped_agent_commands_are_answered_by_the_real_helm`], and
/// exercises what that test cannot: `HelmAgentRequests::handle`'s `Rename`/
/// `Stop`/`Archive` arms calling the SAME `sessions.rs` functions
/// (`do_rename_session`/`do_stop_session`/`do_archive_session`) the REST
/// routes call, through a real routed connection to a real supervisor
/// managing real (fake-agent) processes. A unit test driving the handler
/// directly cannot see a startup-wiring or routing regression that only
/// shows up once the whole assembly is running; this test starts the
/// products and drives them exactly as a session's agent would.
///
/// ## Two sessions, deliberately — a "controller" and a "target"
///
/// Every `farhelm agent` call here authenticates as the CONTROLLER, and
/// `stop`/`archive` are pointed at the TARGET with an explicit `--session`
/// — which is also, on its own, the strongest evidence a fixture can offer
/// for the wide-authority half of `AgentVerb`'s contract ("act on the
/// asker OR any session the helm knows"): the reply names a session the
/// asking connection was never attached to at all.
///
/// This is not only a style choice. `stop` and `archive` end with the
/// supervisor's process-tree SWEEP, which claims every same-user process
/// on the machine whose environment carries the TARGET session's exact
/// `FARHELM_SESSION_ID` marker (`sweep.rs`'s environment-marker scan,
/// documented there as intentionally host-wide — it is what catches a
/// descendant that broke its PPID chain). A `farhelm agent` process
/// necessarily carries that marker FOR THE SESSION IT AUTHENTICATES AS, so
/// a bare CLI invocation that both asks-as and acts-on the SAME session
/// races its own triggered sweep — confirmed empirically while writing
/// this test, where a stand-alone `farhelm agent archive` (no `--session`)
/// was reliably SIGTERM'd by the sweep its own request caused, before it
/// could print a confirmation. That is a property of an unattended CLI
/// process racing a kill sweep it started against itself, not a defect in
/// the relay or in `HelmAgentRequests`, and a real in-pane agent survives
/// it exactly because stopping or archiving ONESELF is supposed to end the
/// whole tree, calling CLI included. Routing `stop`/`archive` at a
/// SEPARATE target session sidesteps the race entirely — the controller's
/// marker never matches the target's sweep — which is what makes this
/// fixture deterministic. `rename` triggers no sweep at all, so it is
/// exercised the other way, WITHOUT `--session`, to cover the
/// "defaults to the asker" half on a verb that carries no such hazard.
///
/// `GET /api/sessions/{id}` is the read used to observe each effect rather
/// than the listing table, because it is documented to answer LIVE for a
/// connected host (never the cache) — see `sessions::get_session`'s own
/// docs — so the assertion after `stop` cannot be racing the helm's
/// periodic refresh the way reading the cached list could.
#[tokio::test]
async fn the_shipped_agent_lifecycle_commands_act_through_the_real_helm() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");

    let supervisor = supervisor_process().await;
    let helm = helm_process(supervisor.state.path(), None).await;
    let secret = device_secret(supervisor.state.path(), &helm.base).await;
    let client = client_with_secret(&secret);
    await_local_host(&client, &helm.base).await;

    let work = farhelm_teststate::tempdir().expect("work dir");
    let create_session = |title: &'static str| {
        let client = client.clone();
        let base = helm.base.clone();
        let cwd = work.path().to_string_lossy().into_owned();
        async move {
            let (status, body) = post(
                &client,
                &format!("{base}/api/sessions"),
                serde_json::json!({
                    "cwd": cwd,
                    "invocation": agent_cmd("internal fake-agent --script basic"),
                    "title": title,
                }),
            )
            .await;
            assert!(status.is_success(), "creating {title} failed: {body}");
            let session: serde_json::Value =
                serde_json::from_str(&body).expect("created session JSON");
            session["id"].as_str().expect("session id").to_string()
        }
    };
    let controller_id = create_session("controller").await;
    let target_id = create_session("before-rename").await;

    // Only the CONTROLLER needs an attachment: the supervisor forwards an
    // upcall to the helm holding the ASKING session's attachment, and
    // every `farhelm agent` call below authenticates as the controller —
    // see the module docs for why the target is named by `--session`
    // instead of also being attached to.
    let _terminal = attach_terminal(&helm, &secret, &controller_id).await;
    let token = session_token(supervisor.state.path(), &controller_id).await;
    let socket = supervisor.state.path().join("supervisor.sock");
    let target_url = format!("{}/api/sessions/{target_id}", helm.base);

    // Same CI-runner race as the read-only test above (see
    // `hosts_until_attached`'s own docs): the controller's `Attach` upcall
    // reaching the supervisor is not guaranteed by the WebSocket's 101
    // response alone, and every lifecycle verb below authenticates as the
    // controller, so its attachment has to be confirmed before the first
    // one is issued.
    hosts_until_attached(&controller_id, &token, &socket).await;

    // `rename`, with NO `--session`: defaults to the asker (the controller
    // itself), and triggers no sweep, so it is the safe verb to exercise
    // that half of the contract on.
    let renamed = agent_command_args(
        &["rename", "renamed-controller"],
        &controller_id,
        &token,
        &socket,
    )
    .await;
    assert_eq!(
        renamed,
        format!("renamed {controller_id} to \"renamed-controller\"\n")
    );
    let controller_detail = get_json(
        &client,
        &format!("{}/api/sessions/{controller_id}", helm.base),
    )
    .await;
    assert_eq!(
        controller_detail["title"], "renamed-controller",
        "the rename must be visible through the same read a browser uses: {controller_detail}"
    );

    // `stop`/`archive`, both with an explicit `--session` naming the
    // TARGET — see the module docs for why acting on a different session
    // than the asker is what keeps this fixture out of the sweep's way.
    let stopped = agent_command_args(
        &["stop", "--session", &target_id],
        &controller_id,
        &token,
        &socket,
    )
    .await;
    assert_eq!(stopped, format!("stopped {target_id}\n"));
    let target_detail = get_json(&client, &target_url).await;
    // `status` is `SessionStatus`'s own internally-tagged shape (a nested
    // object under an `state` field), not a bare word — so the ORIGINAL
    // form of this assertion, `assert_ne!(target_detail["status"],
    // "running")`, compared a JSON object against a string literal and
    // could never have failed no matter what `stop` actually did. Beyond
    // fixing that, `waiting`/`idle` are also "alive" the way `running` is,
    // so even a string-shaped comparison against just `"running"` would
    // pass on a no-op stop. Decoding into the typed `SessionInfo` and
    // asserting the SPECIFIC ended state — `Exited`, carrying the stop's
    // own annotation — is what actually proves the agent's process is
    // gone rather than merely not-currently-observed-as-running.
    let target_info: SessionInfo = serde_json::from_value(target_detail.clone())
        .expect("the session detail decodes as a SessionInfo");
    assert!(
        matches!(target_info.status, SessionStatus::Exited { .. }),
        "a stopped session's agent must have actually exited, not merely stopped reading as \
         running: {target_detail}"
    );
    assert_eq!(
        target_info.annotation.as_deref(),
        Some(STOP_ANNOTATION),
        "a stop through the agent relay must record the same annotation a REST stop would: \
         {target_detail}"
    );

    let archived = agent_command_args(
        &["archive", "--session", &target_id],
        &controller_id,
        &token,
        &socket,
    )
    .await;
    assert_eq!(archived, format!("archived {target_id}\n"));
    let fleet = get_json(
        &client,
        &format!("{}/api/sessions?include_archived=true", helm.base),
    )
    .await;
    let rows = fleet["sessions"].as_array().expect("sessions is an array");
    let row = rows
        .iter()
        .find(|row| row["id"] == target_id)
        .unwrap_or_else(|| panic!("the archived session must still be listed: {fleet}"));
    assert_eq!(
        row["archived"], true,
        "the archive must be visible in the same listing the UI polls: {row}"
    );
    let default_view = get_json(&client, &format!("{}/api/sessions", helm.base)).await;
    assert!(
        default_view["sessions"]
            .as_array()
            .expect("sessions is an array")
            .iter()
            .all(|row| row["id"] != target_id),
        "an archived session must not appear in the default, non-archived view: {default_view}"
    );

    // `helm`, `supervisor` and `work` deliberately outlive this test's last
    // assertion — see the read-only test above for why an explicit drop
    // would only invite someone to reorder them.
}
