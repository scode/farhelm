//! Two real hosts, one merged session list, through the REAL stack
//! (PLAN_M6.md item 5; M6 acceptance 1's first half).
//!
//! Everything else that covers aggregation is in-process: `farhelm-helm`'s
//! REST tests merge scripted hosts behind the real router, and its manager
//! tests drive the state machine against scripted peers. Both are the right
//! shape for what they cover, and neither can tell you that the whole thing
//! works when the hosts are actually separate processes talking over real
//! transports. That is what this file is for:
//!
//! - a real helm started through the desktop app's embedded entry
//!   (`run_embedded`, so the app's own startup assembles it), on a real
//!   loopback port, driven over real HTTP;
//! - two real supervisor processes, each with the terminal hosts of its
//!   sessions — one reached over the unix socket in the helm's own state
//!   directory (the reserved local row), one over the user's own `ssh` to
//!   `localhost` running `farhelm internal stdio` (an ssh row);
//! - the ssh row registered through `POST /api/hosts`, the helm's own
//!   registration route;
//! - sessions created, listed, and operated on across both hosts through
//!   `POST /api/sessions` and the lifecycle routes.
//!
//! NOTE: The helm runs inside the test process, as it runs inside the
//! desktop app's. This file used to run it as a process of its own; that
//! went with the decision to remove the standalone helm (2026-11-16), and
//! what is no longer shown is only the helm's own process boundary. Every
//! transport between the helm and the hosts is still real.
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
//!
//! The machinery for starting those pieces and talking HTTP to them
//! lives in `harness` (`supervisor_process`, `start_helm`,
//! `register_self_ssh_host`, `get_json`, `post`) rather than here: a second
//! real-stack module now needs the same fixtures, and one copy is what
//! keeps them from drifting into two subtly different stacks.

use crate::harness::*;
use crate::host_connection::self_ssh_available;

/// Wait until `/api/hosts` reports both hosts connected, and return the
/// list.
///
/// Polling rather than sleeping, with the last body in the panic message:
/// "the fleet never came up" is useless on its own, and the states are
/// exactly what says WHY — an ssh host that is skewed, mismatched, or
/// unreachable each fails here carrying its own evidence.
/// The deadline also covers each response body; receiving headers alone does
/// not make a probe complete.
async fn await_fleet_connected(client: &reqwest::Client, base: &str) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + REAL_STACK_SETTLE;
    let mut last_completed = None;
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "fleet never reached two connected hosts; last completed response (None means no response): {last_completed:?}"
        );
        let hosts =
            tokio::time::timeout_at(deadline, get_json(client, &format!("{base}/api/hosts")))
                .await
                .unwrap_or_else(|_| panic!(
                    "fleet HTTP probe exceeded readiness deadline; last completed response (None means no response): {last_completed:?}"
                ));
        let rows = hosts["hosts"].as_array().expect("hosts is an array");
        if rows.len() == 2 && rows.iter().all(|row| row["state"]["phase"] == "connected") {
            return hosts;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the fleet never reached two connected hosts; last seen {hosts}"
        );
        last_completed = Some(hosts);
        // sleep-ok: poll the two hosts' connection states before exercising the merged fleet.
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Wait until the merged list contains every id in `ids`, and return it.
///
/// The list trails a create by up to one refresh interval BY DESIGN — the
/// helm serves it from its cache and never by asking hosts — so polling is
/// the correct shape here rather than a concession to flakiness.
/// A stalled HTTP response shares that deadline with the polling loop.
async fn await_listed(client: &reqwest::Client, base: &str, ids: &[&str]) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + REAL_STACK_SETTLE;
    let mut last_completed = None;
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "merged list never carried {ids:?}; last completed response (None means no response): {last_completed:?}"
        );
        let listing =
            tokio::time::timeout_at(deadline, get_json(client, &format!("{base}/api/sessions")))
                .await
                .unwrap_or_else(|_| panic!(
                    "merged-list HTTP probe for {ids:?} exceeded readiness deadline; last completed response (None means no response): {last_completed:?}"
                ));
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
        last_completed = Some(listing);
        // sleep-ok: observe the cached merged listing until it contains every requested session.
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Two hosts — this machine and an ssh-to-localhost "remote" — must serve
/// ONE merged session list through the real stack, with every row naming
/// its host and sessions on both hosts operable.
///
/// This is M6's first acceptance criterion reduced to what one test can
/// prove without the desktop app: a host registered through the helm's API joins
/// the reserved local one, both hosts connect over their genuinely different
/// transports, creates route to the host the body names, the list is one
/// order across both, and a lifecycle operation reaches the right
/// supervisor on either side. The UI half — chips, notices, the create
/// dialog's host selector — is Playwright's, in the PRs that follow.
///
/// Skipped loudly where passwordless `ssh localhost` is unavailable: the
/// repo's established pattern for a precondition that belongs to the host
/// rather than to the code.
#[farhelm_testtrace::test]
async fn two_real_hosts_serve_one_merged_list_and_both_are_operable() {
    if !self_ssh_available().await {
        eprintln!(
            "SKIPPED two_real_hosts_serve_one_merged_list_and_both_are_operable: passwordless \
             `ssh localhost` is unavailable on this host"
        );
        return;
    }
    // Two supervisors (each with the terminal hosts of its sessions), one
    // helm: two slots, taken
    // together so this test never holds one while waiting for the other.
    let _slots = SLOTS
        .acquire_many(2)
        .await
        .expect("semaphore is never closed");

    let local = supervisor_process().await;
    let remote = supervisor_process().await;

    let helm = start_helm(local.state.path()).await;
    let client = helm.client();
    let registered = register_self_ssh_host(&helm, remote.state.path()).await;

    let hosts = await_fleet_connected(&client, &helm.base).await;
    let rows = hosts["hosts"].as_array().unwrap();
    let local_row = rows
        .iter()
        .find(|row| row["kind"] == "local")
        .expect("the helm's own machine is always in the list");
    let ssh_row = rows
        .iter()
        .find(|row| row["kind"] == "ssh")
        .expect("the registered remote is listed");
    assert_eq!(local_row["name"], "this machine");
    assert_eq!(ssh_row["destination"], "localhost");
    assert_ne!(
        local_row["identity"], ssh_row["identity"],
        "two separate installs must report two separate identities, or the merge is meaningless"
    );
    let local_id = local_row["id"].as_i64().expect("host id");
    let ssh_id = ssh_row["id"].as_i64().expect("host id");
    assert_eq!(
        ssh_id, registered,
        "the listed ssh row is the one the registration created"
    );

    // One session per host, created the only way sessions are created now.
    let work = farhelm_teststate::tempdir().expect("work dir");
    let mut created: Vec<(i64, String)> = Vec::new();
    for (host, title) in [(local_id, "on-this-machine"), (ssh_id, "on-the-remote")] {
        let (status, body) = post(
            &client,
            &format!("{}/api/sessions", helm.base),
            serde_json::json!({
                "cwd": work.path().to_string_lossy(),
                "command": {"command": fixture_cmd("fake-agent --script basic"), "yolo": false},
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
    // stay so until this function returns: they hold the processes, the
    // terminal hosts, and the state directories the helm and the ssh child are
    // both still using. Explicit drops would say nothing the scope does not
    // already, and would invite someone to "tidy" them into the wrong order.
}
