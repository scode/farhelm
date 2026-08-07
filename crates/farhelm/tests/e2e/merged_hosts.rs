//! Two real hosts, one merged session list, through the REAL stack
//! (PLAN_M6.md item 5; M6 acceptance 1's first half).
//!
//! Everything else that covers aggregation is in-process: `farhelm-helm`'s
//! REST tests merge scripted hosts behind the real router, and its manager
//! tests drive the state machine against scripted peers. Both are the right
//! shape for what they cover, and neither can tell you that the whole thing
//! works when the pieces are actually separate processes talking over real
//! transports. That is what this file is for, and it is why it runs the
//! SHIPPED binary rather than assembling a helm in-process:
//!
//! - a real `farhelm helm run`, with no session flags, on a real loopback
//!   port, driven over real HTTP the way a browser drives it;
//! - two real supervisors, each with its own private tmux server — one
//!   reached over the unix socket in the helm's own state directory (the
//!   reserved local row), one over the user's own `ssh` to `localhost`
//!   running `farhelm internal stdio` (an ssh row);
//! - the ssh row registered by `--ensure-hosts`, so the startup file is
//!   exercised as the feature it is rather than only as a parser;
//! - sessions created, listed, and operated on across both hosts through
//!   `POST /api/sessions` and the lifecycle routes.
//!
//! The "remote" supervisor runs on this same machine with an ISOLATED state
//! directory, passed as the registry row's `remote_state_dir` — the field
//! that replaced M1's `--remote-state-dir` — which is what keeps a test
//! that says "remote" from ever touching the developer's real
//! `~/.local/state/farhelm`.
//!
//! Skipped loudly, never silently, where passwordless `ssh localhost` is
//! unavailable: that is a property of the host, not of the code, and CI
//! provisions self-ssh so the skip never hides this path there.

use crate::harness::*;
use crate::host_connection::self_ssh_available;

/// How long this test waits for a multi-process stack to reach a state.
///
/// Generous rather than tight: two supervisors, two tmux servers, an ssh
/// handshake and a helm all have to come up, and on a loaded runner that is
/// genuinely slow. The failure this bound reports — "it never got there" —
/// is not diagnosed any better by a shorter wait.
const SETTLE: Duration = Duration::from_secs(90);

/// One running supervisor process plus everything that must die with it.
///
/// Field order is drop order and is load-bearing, the same rule
/// [`TmuxServerGuard`]'s own docs set out: the process first, then its tmux
/// server, then the directory holding both their sockets.
///
/// The child needs no guard of its own — every process here is spawned with
/// `kill_on_drop(true)`, so dropping the `Child` is what kills it, and a
/// wrapper would only restate that. Which matters for the same reason the
/// order does: a test that fails an assertion never reaches an explicit
/// teardown, and a leaked helm holding a loopback port is exactly the
/// debris that makes the NEXT run fail for an unrelated reason.
struct SupervisorProcess {
    _child: tokio::process::Child,
    _tmux: TmuxServerGuard,
    state: tempfile::TempDir,
}

/// Start a real `farhelm supervisor run` on a fresh state directory and
/// wait for its socket to accept.
///
/// The socket wait is what makes the rest of the test deterministic: a helm
/// started against a directory with no socket yet is not wrong — it simply
/// retries — but it would turn every later assertion into a race with the
/// reconnect ladder.
async fn supervisor_process() -> SupervisorProcess {
    let state = tempfile::tempdir().expect("supervisor state dir");
    let child = tokio::process::Command::new(farhelm_bin())
        .args(["supervisor", "run", "--state-dir"])
        .arg(state.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn supervisor");
    let tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let socket = state.path().join("supervisor.sock");
    let deadline = tokio::time::Instant::now() + SETTLE;
    while !socket.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the supervisor at {} never bound its socket",
            state.path().display()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    SupervisorProcess {
        _child: child,
        _tmux: tmux,
        state,
    }
}

/// A running helm process and the loopback base URL it printed.
struct HelmProcess {
    _child: tokio::process::Child,
    base: String,
}

/// Start a real `farhelm helm run` on an ephemeral port, against a state
/// directory somebody else owns, and read back the URL it prints.
///
/// `--port 0` plus parsing stdout, rather than picking a port and hoping:
/// this suite runs concurrently with itself and with whatever else is on
/// the machine, and a hardcoded port is a flake waiting for a second
/// worktree.
///
/// The state directory is the LOCAL supervisor's, deliberately. The local
/// row is reached through whatever listens in the helm's own state
/// directory, so sharing one directory is not a shortcut — it is the
/// production arrangement, the one where helm.db and `supervisor.sock` are
/// siblings and the local host needs no registering at all.
async fn helm_process(state_dir: &std::path::Path, ensure_hosts: &std::path::Path) -> HelmProcess {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let mut child = tokio::process::Command::new(farhelm_bin())
        .args(["helm", "run", "--port", "0", "--state-dir"])
        .arg(state_dir)
        .arg("--ensure-hosts")
        .arg(ensure_hosts)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn helm");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut lines = BufReader::new(stdout).lines();
    let line = tokio::time::timeout(SETTLE, lines.next_line())
        .await
        .expect("the helm printed its URL within the settle budget")
        .expect("reading the helm's stdout")
        .expect("the helm printed a line before exiting");
    let base = line
        .split_once("http://")
        .map(|(_, url)| format!("http://{}", url.trim_end_matches('/')))
        .unwrap_or_else(|| panic!("the helm's first stdout line named no URL: {line:?}"));
    HelmProcess {
        _child: child,
        base,
    }
}

/// Exchange the shipped CLI's bootstrap token for the explicit device secret
/// every later request in this real-stack test carries.
async fn authenticated_client(state_dir: &std::path::Path, base: &str) -> reqwest::Client {
    let output = tokio::process::Command::new(farhelm_bin())
        .args(["helm", "token", "show", "--state-dir"])
        .arg(state_dir)
        .output()
        .await
        .expect("run token show");
    assert!(
        output.status.success(),
        "token show failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let token = String::from_utf8(output.stdout)
        .expect("the token is UTF-8")
        .trim()
        .to_string();
    assert!(!token.is_empty(), "token show must print a credential");

    let response = reqwest::Client::new()
        .post(format!("{base}/api/auth/token"))
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .expect("token exchange reached the helm");
    assert!(
        response.status().is_success(),
        "token exchange answered {}",
        response.status()
    );
    let exchange: serde_json::Value = response.json().await.expect("decode device exchange");
    let secret = exchange["device_secret"]
        .as_str()
        .expect("the exchange returns a device secret");
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Bearer {secret}"))
            .expect("the device secret is an Authorization value"),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("build authenticated client")
}

/// GET a JSON body from the helm, failing the test on a non-2xx.
async fn get_json(client: &reqwest::Client, url: &str) -> serde_json::Value {
    let response = client.get(url).send().await.expect("GET reached the helm");
    let status = response.status();
    let body = response.text().await.expect("read body");
    assert!(status.is_success(), "GET {url} answered {status}: {body}");
    serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("GET {url} body is not JSON ({e}): {body}"))
}

/// POST a JSON body to the helm, returning the status and the body text —
/// both, because a refusal's body is prose and is half of what this stack
/// promises about refusals.
async fn post(
    client: &reqwest::Client,
    url: &str,
    body: serde_json::Value,
) -> (reqwest::StatusCode, String) {
    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .expect("POST reached the helm");
    let status = response.status();
    (status, response.text().await.expect("read body"))
}

/// Wait until `/api/hosts` reports both hosts connected, and return the
/// list.
///
/// Polling rather than sleeping, with the last body in the panic message:
/// "the fleet never came up" is useless on its own, and the states are
/// exactly what says WHY — an ssh host that is skewed, mismatched, or
/// unreachable each fails here carrying its own evidence.
async fn await_fleet_connected(client: &reqwest::Client, base: &str) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + SETTLE;
    loop {
        let hosts = get_json(client, &format!("{base}/api/hosts")).await;
        let rows = hosts["hosts"].as_array().expect("hosts is an array");
        if rows.len() == 2 && rows.iter().all(|row| row["state"]["phase"] == "connected") {
            return hosts;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the fleet never reached two connected hosts; last seen {hosts}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Wait until the merged list contains every id in `ids`, and return it.
///
/// The list trails a create by up to one refresh interval BY DESIGN — the
/// helm serves it from its cache and never by asking hosts — so polling is
/// the correct shape here rather than a concession to flakiness.
async fn await_listed(client: &reqwest::Client, base: &str, ids: &[&str]) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + SETTLE;
    loop {
        let listing = get_json(client, &format!("{base}/api/sessions")).await;
        let present: Vec<&str> = listing["sessions"]
            .as_array()
            .expect("sessions is an array")
            .iter()
            .map(|row| row["id"].as_str().expect("id is a string"))
            .collect();
        if ids.iter().all(|id| present.contains(id)) {
            return listing;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the merged list never carried {ids:?}; last seen {listing}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Two hosts — this machine and an ssh-to-localhost "remote" — must serve
/// ONE merged session list through the real stack, with every row naming
/// its host and sessions on both hosts operable.
///
/// This is M6's first acceptance criterion reduced to what one test can
/// prove without a browser: the registry is populated by `--ensure-hosts`
/// before serving begins, both hosts connect over their genuinely different
/// transports, creates route to the host the body names, the list is one
/// order across both, and a lifecycle operation reaches the right
/// supervisor on either side. The UI half — chips, notices, the create
/// dialog's host selector — is Playwright's, in the PRs that follow.
///
/// Skipped loudly where passwordless `ssh localhost` is unavailable: the
/// repo's established pattern for a precondition that belongs to the host
/// rather than to the code.
#[tokio::test]
async fn two_real_hosts_serve_one_merged_list_and_both_are_operable() {
    if !self_ssh_available().await {
        eprintln!(
            "SKIPPED two_real_hosts_serve_one_merged_list_and_both_are_operable: passwordless \
             `ssh localhost` is unavailable on this host"
        );
        return;
    }
    // Two supervisors, two tmux servers, one helm: two slots, taken
    // together so this test never holds one while waiting for the other.
    let _slots = SLOTS
        .acquire_many(2)
        .await
        .expect("semaphore is never closed");

    let local = supervisor_process().await;
    let remote = supervisor_process().await;

    let ensure = local.state.path().join("ensure-hosts.json5");
    tokio::fs::write(
        &ensure,
        serde_json::json!({
            "hosts": [{
                "ssh": "localhost",
                "remote_farhelm": farhelm_bin(),
                "remote_state_dir": remote.state.path().to_string_lossy(),
            }],
        })
        .to_string(),
    )
    .await
    .expect("write the ensure-hosts file");

    let helm = helm_process(local.state.path(), &ensure).await;
    let client = authenticated_client(local.state.path(), &helm.base).await;

    let hosts = await_fleet_connected(&client, &helm.base).await;
    let rows = hosts["hosts"].as_array().unwrap();
    let local_row = rows
        .iter()
        .find(|row| row["kind"] == "local")
        .expect("the helm's own machine is always in the list");
    let ssh_row = rows
        .iter()
        .find(|row| row["kind"] == "ssh")
        .expect("--ensure-hosts registered the remote before serving began");
    assert_eq!(local_row["name"], "this machine");
    assert_eq!(ssh_row["destination"], "localhost");
    assert_ne!(
        local_row["identity"], ssh_row["identity"],
        "two separate installs must report two separate identities, or the merge is meaningless"
    );
    let local_id = local_row["id"].as_i64().expect("host id");
    let ssh_id = ssh_row["id"].as_i64().expect("host id");

    // One session per host, created the only way sessions are created now.
    let work = tempfile::tempdir().expect("work dir");
    let mut created: Vec<(i64, String)> = Vec::new();
    for (host, title) in [(local_id, "on-this-machine"), (ssh_id, "on-the-remote")] {
        let (status, body) = post(
            &client,
            &format!("{}/api/sessions", helm.base),
            serde_json::json!({
                "cwd": work.path().to_string_lossy(),
                "invocation": agent_cmd("internal fake-agent --script basic"),
                "title": title,
                "host": host,
            }),
        )
        .await;
        assert!(
            status.is_success(),
            "creating on host {host} failed: {body}"
        );
        let session: serde_json::Value = serde_json::from_str(&body).expect("created session JSON");
        created.push((
            host,
            session["id"].as_str().expect("session id").to_string(),
        ));
    }

    let ids: Vec<&str> = created.iter().map(|(_, id)| id.as_str()).collect();
    let listing = await_listed(&client, &helm.base, &ids).await;
    assert_eq!(
        listing["total"], 2,
        "the merged total counts both hosts, not one: {listing}"
    );
    for (host, id) in &created {
        let row = listing["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id.as_str())
            .expect("both sessions are in the one list");
        assert_eq!(row["host"], *host, "every row names the host it lives on");
        assert_eq!(
            row["stale"], false,
            "both hosts are connected, so neither list is last-known knowledge"
        );
    }
    let names: Vec<&str> = listing["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["host_name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"this machine") && names.contains(&"localhost"),
        "the rows must name their hosts distinguishably: {names:?}"
    );

    // Operable on BOTH sides: a stop has to reach the supervisor that owns
    // the session, which is the whole point of owner-lookup routing — and a
    // single-host stack could not tell that apart from "sent to the only
    // connection there is".
    for (host, id) in &created {
        let (status, body) = post(
            &client,
            &format!("{}/api/sessions/{id}/stop", helm.base),
            serde_json::json!({}),
        )
        .await;
        assert!(
            status.is_success(),
            "stopping session {id} on host {host} failed: {body}"
        );
    }

    // `helm`, `remote`, and `local` are deliberately still alive here, and
    // stay so until this function returns: they hold the processes, the tmux
    // servers, and the state directories the helm and the ssh child are
    // both still using. Explicit drops would say nothing the scope does not
    // already, and would invite someone to "tidy" them into the wrong order.
}
