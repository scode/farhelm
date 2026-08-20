//! Error status reported through the launch shim's sentinel, and its
//! durability across restarts and simulated reboots.

use crate::harness::*;

use crate::boot_id_durable_outcome::{
    harness_believing_boot, listed, supervisor_believing_boot, supervisor_reading_boot,
    wait_for_dead_pane,
};

// ---------------------------------------------------------------------
// PLAN_M3.md item 3: error status via the launch shim's sentinel.
// ---------------------------------------------------------------------

/// M3 acceptance 3/4's core case: an invocation that cannot even `exec`
/// (argv0 names a file that simply does not exist, inside the session's
/// own throwaway tempdir) must list as **error**, carrying the shim's own
/// errno detail, and that classification must be DURABLE — surviving both
/// an ordinary supervisor restart and a simulated reboot, in the latter
/// case landing on error rather than the interrupted a plain reboot
/// conversion would otherwise produce.
///
/// The sentinel is witnessed here through an ordinary `list_sessions`
/// poll (`wait_for_non_live_status`), which is the common case: most
/// exec failures WILL be listed at least once before anything restarts.
/// [`a_reboot_never_interrupts_a_row_a_sentinel_already_claims_as_error`]
/// below covers the harder case — a reboot landing before any list ever
/// consumed the sentinel, with the row still `Running` in the store.
#[tokio::test]
async fn unexecutable_invocation_lists_as_error_and_outranks_a_reboot() {
    let h = harness_believing_boot("boot-a").await;
    let sock = h.state.path().join("tmux.sock");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
        )
        .await
        .expect("create a session whose invocation cannot exec");

    // The shim's own process — which has already exec'd over the login
    // shell by the time it attempts the REAL agent's exec — dies the
    // moment that second exec fails, so the pane goes dead almost
    // immediately, well before any list has a chance to observe it.
    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;

    let before = wait_for_non_live_status(&h.client, &session.id, 30).await;
    let detail = match before.status {
        SessionStatus::Error { detail } => detail,
        other => panic!("expected Error, an exec failure must never read as {other:?}"),
    };
    assert!(
        detail.contains("exec_failed") && detail.contains("errno="),
        "the shim's own errno detail must reach the wire verbatim: {detail}"
    );

    // Survives an ordinary supervisor restart on the SAME boot: by now the
    // sentinel FILE is gone (consumed once its Error outcome committed —
    // see `service.rs`'s reload/list sentinel-lifecycle comments), so this
    // proves the classification is durable store state, not a live
    // re-read of a file that no longer exists.
    let sup2 = supervisor_believing_boot(h.state.path(), Some("boot-a")).await;
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        listed(&client2, &session.id).await.status,
        SessionStatus::Error {
            detail: detail.clone()
        },
        "a durable error outcome survives a supervisor restart"
    );

    // Survives a simulated reboot too, and the sharper claim: as ERROR,
    // not INTERRUPTED. By the time this reboot lands the row was ALREADY
    // terminal (`Error`), so `record_boot`'s blanket interrupt conversion
    // (which only touches launching/running/stop-requested rows) never
    // even considers it — the ordinary terminal-state stickiness already
    // proven by `a_reboot_interrupts_live_sessions_and_preserves_ended_ones`
    // is what protects it here.
    kill_tmux_server_and_wait(&sock).await;
    let sup3 = supervisor_believing_boot(h.state.path(), Some("boot-b")).await;
    let client3 = connect_client(&sup3).await;
    assert_eq!(
        listed(&client3, &session.id).await.status,
        SessionStatus::Error { detail },
        "an exec failure must classify error across a reboot, never interrupted"
    );
}

/// PLAN_M3.md item 3's hardest precedence case, and the reason
/// `SessionStore::record_boot` takes a `sentinel_overrides` map instead of
/// leaving this to an ordinary `Transition`: a launch sentinel that
/// exists while its row is STILL `Running` in the store — because nothing
/// ever listed this session before the reboot to let the sentinel
/// classify it first — must still win the race against the blanket
/// interrupt conversion that same reboot triggers. Get the ordering wrong
/// (convert to `Interrupted` first, check sentinels after) and this row
/// is already terminal and immune to reclassification by the time
/// anything looks for its sentinel — exactly the bug this test exists to
/// catch.
#[tokio::test]
async fn a_reboot_never_interrupts_a_row_a_sentinel_already_claims_as_error() {
    let h = harness_believing_boot("boot-a").await;
    let sock = h.state.path().join("tmux.sock");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
        )
        .await
        .expect("create a session whose invocation cannot exec");

    // Deliberately no `list_sessions` call here at all: this row must
    // still be `Running` in the store (`ConfirmRunning` committed at
    // create time, nothing has observed the exec failure since) when the
    // reboot below lands.
    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;

    kill_tmux_server_and_wait(&sock).await;
    let sup2 = supervisor_believing_boot(h.state.path(), Some("boot-b")).await;
    let client2 = connect_client(&sup2).await;
    let status = listed(&client2, &session.id).await.status;
    assert!(
        matches!(status, SessionStatus::Error { .. }),
        "a sentinel-bearing launch must classify error even though its row was still \
         `Running` at the exact moment a reboot was detected: {status:?}"
    );
}

/// SPEC.md's other half of the error/exited split, pinned directly: an
/// agent whose invocation DOES exec successfully, and then exits with 126
/// or 127 (the codes a shell conventionally uses for "found but not
/// executable" and "command not found" — easy to mistake for exec
/// failure since they LOOK like one), must classify exited, never error.
/// Exit codes alone never carry classification weight; only the
/// sentinel's presence does.
#[tokio::test]
async fn exec_that_succeeds_and_exits_126_or_127_is_exited_never_error() {
    let h = harness().await;
    for code in [126, 127] {
        let work = farhelm_teststate::tempdir().expect("workdir");
        let session = h
            .client
            .create_session(
                &work.path().to_string_lossy(),
                &format!("sh -c 'exit {code}'"),
                None,
                80,
                24,
            )
            .await
            .expect("create");
        let status = wait_for_exit_code(&h.client, &session.id, code, 30)
            .await
            .status;
        assert!(
            matches!(
                status,
                SessionStatus::Exited { exit_code: Some(c) } if c == code
            ) || status == (SessionStatus::Exited { exit_code: None }),
            "exit code {code} must classify exited, never error: {status:?}"
        );
    }
}

/// PLAN_M3.md item 5's other launch-failure class: a spec that never even
/// reached `exec` at all — missing or malformed, in the shim's own
/// vocabulary (pinned at the unit level in `launch.rs`'s
/// `exec_launch_spec_records_a_sentinel_for_a_malformed_spec` and
/// `..._for_a_missing_spec`) — must classify identically to a real exec
/// failure once the supervisor reads whatever sentinel resulted.
///
/// Planted directly at the sentinel's own derived path
/// (`spec_path_for_session`/`status_path_for_spec`, both public exactly so
/// a test can agree with the shim on where its output lives) rather than
/// raced out of a genuine shim run: `create_session` never itself hands
/// the shim a torn or missing spec (the write-then-launch ordering in
/// `service.rs`'s `create_session` guarantees a valid spec exists before
/// the tmux window that would read it is even created), so reaching this
/// failure class end-to-end would mean deliberately corrupting
/// supervisor-internal state anyway. Planting the sentinel directly tests
/// the piece this PR actually owns — the SUPERVISOR's reader and
/// classifier — independent of which shim code path produced the file,
/// and does so deterministically rather than racing a real tmux window.
///
/// The session's REAL agent is left running throughout: this also proves
/// a sentinel outranks even a genuinely alive pane (PLAN_M3.md item 3's
/// "outranks every inference"), which only a fresh `reload_sessions`
/// checks unconditionally — the list path only checks a dead-or-absent
/// pane's sentinel (`service.rs`'s `ListSessions` handler docs) — so a
/// supervisor restart is what exercises the stronger check.
#[tokio::test]
async fn a_planted_malformed_spec_sentinel_classifies_error_with_its_detail() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    assert!(
        listed(&h.client, &session.id).await.status.is_live(),
        "the session's real agent must still be genuinely alive throughout this test"
    );

    let detail = format!(
        "launch spec at /state/launch/{}.json is malformed: EOF while parsing a value",
        session.id
    );
    let status_path = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    std::fs::write(&status_path, &detail).expect("plant the sentinel");

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("restart to trigger reload's unconditional sentinel check");
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        listed(&client2, &session.id).await.status,
        SessionStatus::Error {
            detail: detail.clone()
        },
        "a planted sentinel must classify error with its exact detail, even against a pane \
         that never stopped being genuinely alive"
    );
    assert!(
        !status_path.exists(),
        "a consumed sentinel is deleted once its Error outcome commits durably"
    );
}

/// Review-swarm fix batch item 5/19: a session `reload_sessions` classifies
/// `Error` via its sentinel must NOT lose its terminal in the process — the
/// bug this pins is that the sentinel branch used to `continue` before
/// ever recording the pane it had already found, leaving `Attach` refusing
/// a session whose dead pane genuinely still exists in tmux, and
/// `DeleteSession`'s kill sweep with nothing to act on at all (a leaked
/// tmux session). Attach must succeed (the dead pane is viewable, exactly
/// like any other exited session), and delete must actually tear the real
/// tmux session down, not merely drop the row.
#[tokio::test]
async fn a_reload_classified_error_session_keeps_its_terminal_for_attach_and_delete() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
        )
        .await
        .expect("create a session whose invocation cannot exec");
    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;

    // Reload — not list — is what this test targets: a fresh supervisor's
    // `reload_sessions` reconciliation is where item 5's bug lived,
    // separately from `ListSessions`'s own (already-correct) sentinel
    // branch. This supervisor goes on to attach below, so it needs the
    // suite's loaded-CI tmux floors.
    let sup2 = Supervisor::new_with_exe_and_timeouts(
        h.state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
    )
    .await
    .expect("reload onto the sentinel-classified row");
    let client2 = connect_client(&sup2).await;
    assert!(
        matches!(
            listed(&client2, &session.id).await.status,
            SessionStatus::Error { .. }
        ),
        "sanity: the reload really did classify this session as error"
    );

    let tmux_name = format!("fh-{}", session.id);
    let before = tmux_query(&sock, &["has-session", "-t", &format!("={tmux_name}")]).await;
    assert!(
        before.status.success(),
        "the real tmux session must still exist going into this test's real assertions"
    );

    // Attach succeeds: the dead pane genuinely exists, so this is exactly
    // like attaching to any other exited session, not a `NotFound`.
    client2
        .attach(&session.id, 80, 24)
        .await
        .expect("an error-classified session with a real dead pane must still be attachable");

    client2.delete_session(&session.id).await.expect("delete");
    let after = tmux_query(&sock, &["has-session", "-t", &format!("={tmux_name}")]).await;
    assert!(
        !after.status.success(),
        "delete must tear down the REAL tmux session, not merely drop the row — a session \
         reload never recorded a terminal for has nothing for the kill sweep to find"
    );
}

/// Review-swarm fix batch item 2: a launch sentinel this supervisor CANNOT
/// durably record (its boot-id read failed, so `may_record()` is false for
/// this instance's whole lifetime) must still surface as `error` in a
/// `ListSessions` reply — undurably — rather than silently reporting the
/// stale `Exited` a degraded pass used to fall back to by skipping the
/// sentinel read entirely. Once a LATER supervisor's boot id read succeeds,
/// the same classification lands durably, with the file consumed.
#[tokio::test]
async fn a_sentinel_survives_an_unreadable_boot_id_undurably_then_commits_once_readable() {
    let h = harness_believing_boot("boot-a").await;
    let sock = h.state.path().join("tmux.sock");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
        )
        .await
        .expect("create a session whose invocation cannot exec");
    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;

    let status_path = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    assert!(
        status_path.exists(),
        "the shim must have left its sentinel behind by now"
    );

    // A supervisor whose boot-id read fails outright: `may_record()` is
    // false for its whole lifetime (no reboot-vs-same-boot decision could
    // even be made), yet this is exactly the "degraded boot-id" case item
    // 2 requires the list path to still READ the sentinel for.
    let degraded = supervisor_reading_boot(h.state.path(), Err("/proc is unavailable")).await;
    let degraded_client = connect_client(&degraded).await;
    let undurable = listed(&degraded_client, &session.id).await;
    assert!(
        matches!(undurable.status, SessionStatus::Error { .. }),
        "a degraded supervisor must still SURFACE a sentinel it read, even though it cannot \
         record it: {:?}",
        undurable.status
    );
    assert!(
        status_path.exists(),
        "an undurable classification must retain the sentinel file for a later pass to \
         commit against"
    );

    // A later supervisor whose boot-id read succeeds: the same
    // classification now lands durably, and the file is consumed.
    let recovered = supervisor_believing_boot(h.state.path(), Some("boot-a")).await;
    let recovered_client = connect_client(&recovered).await;
    assert!(
        matches!(
            listed(&recovered_client, &session.id).await.status,
            SessionStatus::Error { .. }
        ),
        "the classification must also land once recording becomes possible"
    );
    assert!(
        !status_path.exists(),
        "once durably committed, the sentinel file must finally be consumed"
    );
}

/// Review-swarm fix batch item 3(a): `StopSession`'s own dead/absent-pane
/// exit-recording boundary must check the sentinel FIRST — the failure
/// this pins is a stop committing a plain `ObservedExit` before anything
/// ever reads the sentinel, which terminal-stickiness would then protect
/// forever, permanently hiding an `Error` classification the file already
/// had evidence for. Stop is called BEFORE any list, deliberately: a list
/// is how this supervisor would ordinarily witness the sentinel first, and
/// this test is specifically about the row still being `Running` (no
/// intervening observer at all) when the stop lands.
#[tokio::test]
async fn stop_before_any_list_on_an_exec_failed_session_still_ends_error() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");

    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
        )
        .await
        .expect("create a session whose invocation cannot exec");
    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;

    // No `list_sessions` call anywhere above: `stop_session` is the FIRST
    // and only observer this row ever gets before its own exit-recording
    // boundary runs.
    h.client
        .stop_session(&session.id)
        .await
        .expect("stop a session whose agent already never started");

    let status = listed(&h.client, &session.id).await.status;
    assert!(
        matches!(status, SessionStatus::Error { .. }),
        "a stop landing before any list must still end error, not a plain exit: {status:?}"
    );
}

/// Review-swarm fix batch item 1: a corrupt (invalid-UTF-8) sentinel must
/// fail the WHOLE `ListSessions` request rather than silently classifying
/// its row (or any other entry sharing the reply) from pane state alone.
/// Pinned against the list path specifically, since that is the site the
/// fix batch calls out as returning `Internal` for the request; the file
/// must survive the failed attempt, and a later pass with it repaired
/// (removed, here) must classify correctly.
#[tokio::test]
async fn a_corrupt_sentinel_fails_the_whole_list_request_and_survives() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    assert!(listed(&h.client, &session.id).await.status.is_live());

    // A genuinely alive pane never has its sentinel checked at all (the
    // dead-or-absent gate), so the pane is killed first — this is the
    // absent-terminal half of the gate, exercised deliberately rather than
    // the live half (covered by the dead-or-absent test elsewhere).
    let sock = h.state.path().join("tmux.sock");
    let out = tmux_query(
        &sock,
        &["kill-session", "-t", &format!("=fh-{}", session.id)],
    )
    .await;
    assert!(out.status.success(), "test setup: killing the tmux session");

    let status_path = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    std::fs::create_dir_all(status_path.parent().unwrap()).unwrap();
    std::fs::write(&status_path, [0xff, 0xfe, 0xfd]).expect("plant a corrupt sentinel");

    let err = h
        .client
        .list_sessions()
        .await
        .expect_err("a corrupt sentinel must fail the whole list request");
    assert!(
        format!("{err:#}").contains("launch sentinel"),
        "the failure must name what went wrong: {err:#}"
    );
    assert!(
        status_path.exists(),
        "the corrupt file must survive a failed read for a later, repaired pass"
    );

    // Repaired (by removing the corrupt file outright): a later list must
    // classify normally rather than staying wedged on the earlier failure.
    std::fs::remove_file(&status_path).expect("remove the corrupt sentinel");
    let status = listed(&h.client, &session.id).await.status;
    assert_eq!(
        status,
        SessionStatus::Exited { exit_code: None },
        "once repaired, a later pass must classify normally again: {status:?}"
    );
}

/// Review-swarm fix batch item 13: the dead-or-absent gate itself, both
/// directions in one test. A genuinely ALIVE pane must never have its
/// sentinel even READ (the live status wins outright, and the planted file is left
/// untouched) — planting a sentinel behind a still-running agent is
/// exactly the scenario that must NOT retroactively classify it error.
/// Once the pane goes dead, the SAME planted file is read on the very next
/// list and classifies error.
#[tokio::test]
async fn the_dead_or_absent_gate_ignores_a_sentinel_behind_a_live_pane_until_the_pane_dies() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    assert!(listed(&h.client, &session.id).await.status.is_live());

    let detail = "exec_failed argv0=/nope errno=2".to_string();
    let status_path = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    std::fs::write(&status_path, &detail).expect("plant a sentinel behind a live pane");

    assert!(
        listed(&h.client, &session.id).await.status.is_live(),
        "a live pane must win outright; its sentinel is not even consulted"
    );
    assert!(
        status_path.exists(),
        "an unconsulted sentinel must be left completely untouched"
    );

    // Kill the real agent so the pane goes dead; the SAME file is what the
    // next list reads.
    let sock = h.state.path().join("tmux.sock");
    let out = tmux_query(
        &sock,
        &["kill-session", "-t", &format!("=fh-{}", session.id)],
    )
    .await;
    assert!(out.status.success(), "test setup: killing the tmux session");

    let status = listed(&h.client, &session.id).await.status;
    assert_eq!(
        status,
        SessionStatus::Error { detail },
        "once the pane is dead-or-absent, the same planted sentinel classifies error: {status:?}"
    );
}
