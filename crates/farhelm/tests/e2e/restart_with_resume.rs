//! `RestartSession` against a real client and real tmux, pinning the
//! terminal-reuse behavior tmux itself provides on a respawned pane.

use crate::harness::*;

use crate::boot_id_durable_outcome::{listed, wait_for_dead_pane};
use crate::conversation_identity_capture::{
    capture_harness, marker_value, provoke_record, record_session, settle_past_horizon,
    snapshot_of, test_capture_bounds,
};

// ---------------------------------------------------------------------
// Restart with resume (PLAN_M3.md item 9; M3 acceptance 9, plus the
// restart clauses of acceptance 4 and 5)
//
// Every test below drives the real `RestartSession` handler through the
// real client, against a real tmux — the terminal-reuse behavior these
// pin (a respawned pane keeping the prior run above it) is tmux's, not
// this crate's, so a faked driver would prove nothing about it.
// ---------------------------------------------------------------------

/// Poll `list_sessions` until `session_id` reports `Alive`.
///
/// The mirror image of [`wait_for_non_alive_status`], and needed for the
/// same reason: a restart's reply says the pane exists, not that the agent
/// inside it has execed yet, so "the relaunch is running" is only
/// observable by asking tmux — which `ListSessions` does, freshly, on every
/// call.
pub(crate) async fn wait_for_alive_status(
    client: &SupervisorClient,
    session_id: &str,
    secs: u64,
) -> SessionInfo {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let listed = client.list_sessions().await.expect("list while polling");
        if let Some(found) = listed.sessions.iter().find(|s| s.id == session_id)
            && found.status == SessionStatus::Alive
        {
            return found.clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "session {session_id} never became Alive within {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// The whole visible content of a session's pane, scrollback included —
/// asked of tmux directly rather than through an attachment.
///
/// The scrollback assertions below are about what the TERMINAL holds after
/// a respawn, which is precisely the thing an attachment's replay is
/// derived from; reading tmux itself keeps those assertions from passing
/// (or failing) for a reason that lives in the replay path instead.
pub(crate) async fn pane_capture(sock: &std::path::Path, tmux_name: &str) -> String {
    let out = tmux_query(sock, &["capture-pane", "-p", "-S", "-", "-t", tmux_name]).await;
    assert!(
        out.status.success(),
        "capture-pane for {tmux_name} must succeed, got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// M3 acceptance 9, first clause: a restart on a LIVE session confirms
/// (`stop_if_running`), stops the whole process tree, and relaunches into
/// the SAME terminal.
///
/// The proof that the stop lifecycle really ran is the tree's death, not
/// the annotation: a successful restart deliberately CLEARS the annotation
/// with its new generation (PLAN_M3.md item 4), so a stopped-then-restarted
/// session must come back carrying none — which this asserts too, since a
/// stale "stopped by user" on a session that is running again is exactly
/// the bug that clearing exists to prevent.
///
/// The `spawner` fixture is used rather than `basic` because a
/// single-process agent cannot distinguish a tree kill from a plain one,
/// and "reaps the prior run before relaunching, never alongside" is the
/// clause under test.
#[tokio::test]
async fn restarting_a_live_session_stops_its_tree_and_reuses_the_terminal() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tmux_name = format!("fh-{}", session.id);

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");
    let grandchild_pid = wait_for_child(child_pid, 10).await;
    let pane_before = pane_id_of(&sock, &tmux_name).await;

    // Stopping first is what the user consented to; without that consent
    // the request is refused outright (see the next test).
    let restarted = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart with consent to stop the running agent");
    assert_eq!(restarted.id, session.id);
    assert_eq!(
        restarted.annotation, None,
        "the new generation clears the previous run's stop annotation"
    );

    // The whole PRIOR tree is gone — including the grandchild, which only
    // a tree sweep ever reaches. What this observes is the END STATE (no
    // survivors, a live new run), not the interleaving: proving "before,
    // never alongside" from the outside would need launch-time
    // instrumentation this harness does not have. The supervisor's own
    // ordering is asserted where it is decided instead — the sweep runs to
    // completion before `begin_relaunch` is called at all.
    wait_until_pid_gone(self_pid, 15).await;
    wait_until_pid_gone(child_pid, 15).await;
    wait_until_pid_gone(grandchild_pid, 15).await;

    let alive = wait_for_alive_status(&h.client, &session.id, 30).await;
    assert_eq!(
        alive.annotation, None,
        "a running session must never carry the previous run's annotation"
    );
    assert_eq!(
        pane_id_of(&sock, &tmux_name).await,
        pane_before,
        "SPEC.md: restart reuses the session's terminal when it still exists — same pane, \
         not a replacement one"
    );

    h.client.detach(chan).await;
}

/// The other half of the confirm contract: without `stop_if_running`, a
/// restart against an agent the supervisor finds ALIVE is refused with
/// `Conflict` and kills nothing at all.
///
/// This is the TOCTOU guard, not a redundancy: a client's cached status can
/// say "exited" while the agent is running (another client relaunched it,
/// or the status was simply stale), and the flag is what tells the
/// supervisor "the user was actually asked". So the assertion that matters
/// is the process still being alive afterwards, not just the error.
#[tokio::test]
async fn restarting_a_live_session_without_consent_is_refused_and_kills_nothing() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");

    let err = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect_err("a live agent may not be restarted without consent to stop it");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("a refusal carries the supervisor's own classification");
    assert_eq!(err.kind, ErrorKind::Conflict);
    assert!(
        err.message.contains("still running"),
        "the refusal must say why, so a client can ask the user: {}",
        err.message
    );

    assert!(
        !process_is_gone(self_pid) && !process_is_gone(child_pid),
        "a refused restart must not have killed anything"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.status,
        SessionStatus::Alive,
        "and must leave the session exactly as it was"
    );
}

/// M3 acceptance 9: relaunching into a RETAINED terminal keeps the prior
/// run in scrollback — "the previous run's output stays in scrollback"
/// (SPEC.md), with the new run's output below it.
///
/// The marker is produced by TYPING into the first run rather than by its
/// startup banner, because both runs print the same banner: an assertion
/// on text only the first run could have produced is what makes this about
/// retention rather than about the relaunch having printed something.
#[tokio::test]
async fn a_reused_terminal_keeps_the_prior_run_above_the_new_one() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let (session, _work) = basic_session(&h).await;
    let tmux_name = format!("fh-{}", session.id);

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client
        .send_input(chan, b"PRIOR-RUN-MARKER\r".to_vec())
        .await;
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    wait_for(&mut rx, &mut seen, "PRIOR-RUN-MARKER", 10).await;

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    // Read from tmux itself, and wait for the new run's own banner to
    // appear in the capture: the relaunched agent starts asynchronously,
    // so a single read can land before it has printed anything.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let capture = loop {
        let capture = pane_capture(&sock, &tmux_name).await;
        if let Some(marker) = capture.find("PRIOR-RUN-MARKER")
            && capture[marker..].contains("FAKE-AGENT READY")
        {
            break capture;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the new run never appeared below the prior run's output; capture:\n{capture}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    assert!(
        capture.contains("PRIOR-RUN-MARKER"),
        "the prior run's output must survive the respawn: {capture}"
    );

    // And a client attaching after the restart sees the same thing, since
    // its replay is that scrollback.
    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach after restart");
    let mut replay = Vec::new();
    wait_for_after(
        &mut rx2,
        &mut replay,
        "PRIOR-RUN-MARKER",
        "FAKE-AGENT READY",
        20,
    )
    .await;
}

/// M3 acceptance 9: leftover descendants of a prior run are reaped BEFORE
/// the relaunch, never left running alongside it — including a daemon left
/// behind by an agent that exited on its own, which SPEC.md says only the
/// session's next restart (or teardown) goes hunting for.
///
/// The agent is killed directly rather than stopped, for the same reason
/// `stop_kills_a_reparented_daemon_with_no_live_pane_to_walk_from` does it:
/// a stop would already have reaped the daemon through the live-pane path,
/// proving nothing about the restart's own sweep. The daemon has fully
/// reparented to init by then, so only the environment-marker scan can
/// find it at all.
#[tokio::test]
async fn a_restart_reaps_a_daemon_left_by_a_self_exited_agent() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-reparent"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    // SAFETY: `self_pid` is a real, currently-live pid this test just read
    // out of the fake agent's own output.
    unsafe {
        libc::kill(self_pid as libc::pid_t, libc::SIGKILL);
    }
    wait_until_pid_gone(self_pid, 10).await;
    wait_for_non_alive_status(&h.client, &session.id, 20).await;
    assert!(
        !process_is_gone(daemon_pid),
        "the daemon must outlive its parent, or this test proves nothing"
    );

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect("an agent that already exited needs no stop consent");

    // Again the end state rather than the interleaving: the daemon is gone
    // and a new run is up. The "before, not alongside" ordering is a
    // property of the handler (the sweep completes before the generation is
    // opened), not something this vantage point can witness.
    wait_until_pid_gone(daemon_pid, 15).await;
    wait_for_alive_status(&h.client, &session.id, 30).await;
}

/// M3 acceptance 9: a vanished working directory fails the restart with an
/// error NAMING the directory, and the session survives untouched — its
/// stop annotation included, which is PLAN_M3.md item 4's "only a
/// SUCCESSFUL restart clears it".
///
/// The annotation is what makes this more than an error-message test: the
/// clear commits with the new launch generation, so a restart that never
/// gets a generation must leave the stopped outcome exactly as it was.
#[tokio::test]
async fn a_vanished_working_directory_refuses_the_restart_and_keeps_the_annotation() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let cwd = work.path().to_string_lossy().into_owned();
    let session = h
        .client
        .create_session(
            &cwd,
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    h.client.stop_session(&session.id).await.expect("stop");
    assert_eq!(
        listed(&h.client, &session.id).await.annotation.as_deref(),
        Some("stopped by user")
    );

    // The directory goes away under the session, exactly as a user
    // deleting a worktree would leave it.
    work.close().expect("remove the working directory");

    let err = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect_err("a session whose working directory is gone cannot be relaunched");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("a precondition failure carries its classification");
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    assert!(
        err.message.contains(&cwd),
        "the error must name the directory (SPEC.md): {}",
        err.message
    );

    let after = listed(&h.client, &session.id).await;
    assert!(
        matches!(after.status, SessionStatus::Exited { .. }),
        "the session itself survives a refused restart: {after:?}"
    );
    assert_eq!(
        after.annotation.as_deref(),
        Some("stopped by user"),
        "a restart that never opened a launch generation cannot have cleared the annotation"
    );
}

/// The staleness contract in the direction that actually happens
/// (`ControlMsg::RestartSession`'s docs): conversation capture upgrades a
/// session's offer from fresh-only to resumable AFTER a client read its
/// `SessionInfo`, so the mode that client picked is no longer the one the
/// supervisor will accept — and the refusal has to NAME the current offer,
/// because the client's next move is to re-present it rather than retry.
///
/// Driven through the ordinary client rather than a raw frame writer: the
/// staleness this exercises is a property of the SUPERVISOR's revalidation,
/// and reproducing it only needs the request to be sent with a mode that
/// was correct a moment earlier.
#[tokio::test]
async fn a_capture_that_lands_after_the_clients_read_makes_a_fresh_restart_conflict() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    // What a client that listed BEFORE the first prompt would have cached.
    assert_eq!(
        session.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );

    let (_chan, _rx, _seen, conversation) = provoke_record(&h, &session).await;
    settle_past_horizon(&h).await;
    assert_eq!(
        snapshot_of(&h, &session.id)
            .await
            .captured_conversation
            .as_deref(),
        Some(conversation.as_str()),
        "the capture must have landed, or there is no staleness to test"
    );

    let err = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect_err("a fresh restart is not a legal answer to a resumable session");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("a stale-offer refusal carries its classification");
    assert_eq!(err.kind, ErrorKind::Conflict);
    assert!(
        err.message.contains("resum"),
        "the refusal must name the CURRENT offer so the client can re-present it: {}",
        err.message
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "and the offer the client should re-present is the one it can now read"
    );
}

/// The environment variable the record-writing fixture reads a resumed
/// conversation id from (`fake_agent::RESUME_ENV_VAR`).
///
/// Duplicated rather than imported because this crate has no library
/// target — an integration test cannot reach `fake_agent`'s items at all
/// (the same duplication `FLOOD_RECORDS` accepts, for the same reason).
/// Drift is loud rather than silent: the fixture would report no resume at
/// all and the test below would fail waiting for its marker.
const FAKE_AGENT_RESUME_ENV: &str = "FARHELM_FAKE_AGENT_RESUME";

/// A resume template that runs the record-writing fixture and hands it the
/// substituted conversation id.
///
/// The `sh -c` wrapper exists for one mundane reason with a real payoff:
/// this binary's argument parser lives in `main.rs`, so the fixture cannot
/// grow a `--resume` flag from the test side — the wrapper moves the
/// substituted argv element into the environment variable the fixture reads
/// instead (`fake_agent::RESUME_ENV_VAR`). What it does NOT change is the
/// property under test: `{conversation}` is still its OWN argv element,
/// substituted slot-for-slot by the supervisor rather than spliced into any
/// string, which is exactly what keeps an id from ever becoming part of a
/// different command.
///
/// `argv0` must stay the kind-named symlink so the session still derives
/// its integration from its own invocation, as a real one would.
fn fixture_resume_template(
    argv0: &std::path::Path,
    kind: &str,
    record_home: &std::path::Path,
) -> Vec<String> {
    vec![
        "sh".to_string(),
        "-c".to_string(),
        format!(
            "{}=\"$2\" exec \"$0\" internal fake-agent --script {kind}-record --record-home \"$1\"",
            FAKE_AGENT_RESUME_ENV
        ),
        argv0.to_string_lossy().into_owned(),
        record_home.to_string_lossy().into_owned(),
        farhelm_supervisor::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
    ]
}

/// Where [`interrupted_session_resumes_its_conversation`]'s final assertion
/// finds the record the resumed run appended to, once it knows `kind` and
/// the conversation id.
///
/// Claude's tree is partitioned by working directory, so a listing of the
/// one project directory the fixture writes into is enough. Codex's is not
/// — it nests by CALENDAR DATE instead (see `record_path` in
/// `fake_agent.rs`) — so this walks the whole `.codex/sessions` tree rather
/// than duplicating that date math: a test-side reimplementation of the
/// fixture's own path formula would only prove the two agree with each
/// other, not that either matches what a real resumed Codex session does.
fn resumed_record_file(
    home: &std::path::Path,
    kind: &str,
    work: &std::path::Path,
    conversation: &str,
) -> std::path::PathBuf {
    match kind {
        "claude" => {
            let canonical = std::fs::canonicalize(work).expect("canonicalize the workdir");
            std::fs::read_dir(home.join(".claude").join("projects").join(
                farhelm_supervisor::agent_kind::munge_cwd(&canonical.to_string_lossy()),
            ))
            .expect("project dir")
            .map(|entry| entry.expect("dir entry").path())
            .find(|path| path.to_string_lossy().contains(conversation))
            .expect("the captured record still exists")
        }
        "codex" => {
            fn walk(dir: &std::path::Path, id: &str) -> Option<std::path::PathBuf> {
                for entry in std::fs::read_dir(dir).expect("read the sessions tree") {
                    let path = entry.expect("dir entry").path();
                    if path.is_dir() {
                        if let Some(found) = walk(&path, id) {
                            return Some(found);
                        }
                    } else if path.to_string_lossy().contains(id) {
                        return Some(path);
                    }
                }
                None
            }
            walk(&home.join(".codex").join("sessions"), conversation)
                .expect("the captured record still exists")
        }
        other => panic!("resumed_record_file: unknown kind {other}"),
    }
}

/// M3 acceptance 9 and 8 together: a session INTERRUPTED by a (simulated)
/// reboot restarts into a FRESH terminal — there is none left to reuse —
/// and `Resume` mode fills the snapshot's template with the identity that
/// was captured before the reboot, so the relaunched agent picks up the
/// same conversation.
///
/// Both halves are asserted from the fixture's own output rather than
/// inferred: it echoes the argv it was launched with (so the substituted id
/// is visible as a fact about what RAN), and it reports adopting the
/// existing record rather than starting a new one — which is what "resumes
/// exactly that conversation" means on disk.
///
/// Shared by both agent kinds ([`an_interrupted_session_resumes_its_conversation_in_a_fresh_terminal`]
/// and [`an_interrupted_codex_session_resumes_its_conversation_in_a_fresh_terminal`]):
/// the resume path is kind-agnostic once `fixture_resume_template` has
/// filled in the placeholder, and the only kind-specific step left is
/// finding where the record landed on disk ([`resumed_record_file`]).
async fn interrupted_session_resumes_its_conversation(kind: &str) {
    let home = tempfile::tempdir().expect("agent home");
    let bin = tempfile::tempdir().expect("agent bin");
    std::os::unix::fs::symlink(farhelm_bin(), bin.path().join(kind))
        .expect("symlink the farhelm binary under the agent's own name");
    let state = tempfile::tempdir().expect("state dir");
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    let seams = |boot: &str| SupervisorSeams {
        boot_id: {
            let boot = boot.to_string();
            Arc::new(move || Ok(Some(boot.clone())))
        },
        agent_home: Some(home.path().to_path_buf()),
        capture_window: test_capture_bounds(),
        ..SupervisorSeams::default()
    };

    let work = tempfile::tempdir().expect("workdir");
    let conversation = {
        let sup = Supervisor::new_with_seams(
            state.path(),
            farhelm_bin().into(),
            SupervisorTimeouts::default(),
            seams("boot-a"),
        )
        .await
        .expect("first supervisor");
        let client = connect_client(&sup).await;
        let session = client
            .create_session_with_extras(
                &work.path().to_string_lossy(),
                &format!(
                    "{} internal fake-agent --script {kind}-record --record-home {}",
                    shell_words::quote(&bin.path().join(kind).to_string_lossy()),
                    shell_words::quote(&home.path().to_string_lossy())
                ),
                None,
                80,
                24,
                farhelm_helm::CreateExtras {
                    resume_template: Some(fixture_resume_template(
                        &bin.path().join(kind),
                        kind,
                        home.path(),
                    )),
                    ..farhelm_helm::CreateExtras::default()
                },
            )
            .await
            .expect("create the record-writing session");

        let (chan, mut rx) = client.attach(&session.id, 80, 24).await.expect("attach");
        let mut seen = Vec::new();
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
        client.send_input(chan, b"first prompt\r".to_vec()).await;
        wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;
        let conversation = marker_value(&seen, "RECORD-WRITTEN:");

        // Let the claim become durable before the reboot: an identity that
        // only ever existed in memory would prove nothing about a session
        // whose supervisor is about to be replaced.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            client.list_sessions().await.expect("list drives capture");
            let snapshot = sup
                .session_snapshot(&session.id)
                .await
                .expect("snapshot")
                .expect("present");
            if snapshot.captured_conversation.is_some() {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the fixture's identity was never captured"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        drop(client);
        let drain = tokio::time::Instant::now() + Duration::from_secs(10);
        while Arc::strong_count(&sup) > 1 {
            assert!(tokio::time::Instant::now() < drain, "connection drain");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        drop(sup);
        (conversation, session)
    };
    let (conversation, session) = conversation;

    // The reboot: tmux dies with the host, and the next supervisor reads a
    // different boot id.
    kill_tmux_server_and_wait(&state.path().join("tmux.sock")).await;
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        seams("boot-b"),
    )
    .await
    .expect("post-reboot supervisor");
    assert!(sup.owns_state_dir(), "the predecessor must be gone");
    let client = connect_client(&sup).await;
    let interrupted = listed(&client, &session.id).await;
    assert_eq!(interrupted.status, SessionStatus::Interrupted);
    assert_eq!(
        interrupted.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "the captured identity survived the reboot, so opening this session offers a resume"
    );

    let restarted = client
        .restart_session(&session.id, farhelm_proto::RestartMode::Resume, false)
        .await
        .expect("an interrupted session has nothing running to consent about");
    assert_eq!(
        restarted.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "the identity is the conversation's, not the run's — it survives the relaunch too"
    );

    let (chan, mut rx) = client
        .attach(&session.id, 80, 24)
        .await
        .expect("the relaunch built a fresh terminal to attach to");
    let mut seen = Vec::new();
    wait_for(
        &mut rx,
        &mut seen,
        &format!("RECORD-RESUMED:{conversation}"),
        30,
    )
    .await;
    let argv_line = String::from_utf8_lossy(&seen);
    let argv_line = argv_line
        .split("FAKE-AGENT ARGV:")
        .nth(1)
        .expect("the fixture echoes its own argv")
        .lines()
        .next()
        .expect("a line");
    assert!(
        argv_line.contains("--record-home"),
        "the resume ran the TEMPLATE, not the launch invocation: {argv_line}"
    );
    // The substituted id itself is not visible in this argv, and that is a
    // property of the FIXTURE, not of the product: the template's
    // `{conversation}` element is consumed by the `sh -c` wrapper that
    // moves it into the environment variable the fixture reads (see
    // `fixture_resume_template`). What proves the id reached the relaunched
    // process is the `RECORD-RESUMED:<id>` marker waited on above, which
    // the fixture only prints for the exact id it was handed.

    // The resumed conversation genuinely continues: the fixture's
    // `append` command is its stand-in for a real agent writing more of
    // the SAME conversation (see `record_agent`'s docs), and it can only
    // do that because the relaunch handed it the id it was resuming.
    client.send_input(chan, b"append\r".to_vec()).await;
    wait_for(
        &mut rx,
        &mut seen,
        &format!("RECORD-APPENDED:{conversation}"),
        20,
    )
    .await;
    let record = String::from_utf8(
        std::fs::read(resumed_record_file(
            home.path(),
            kind,
            work.path(),
            &conversation,
        ))
        .expect("read the record"),
    )
    .expect("the fixture writes UTF-8");
    assert!(
        record.lines().count() >= 2,
        "the resumed run must append to the captured conversation, not replace it: {record}"
    );
    drop(slot);
}

/// Thin wrapper around [`interrupted_session_resumes_its_conversation`] for
/// the Claude-shaped fixture. Kept as its own `#[tokio::test]` (rather than
/// folded into a loop) so a failure names the agent kind directly in the
/// test binary's output.
#[tokio::test]
async fn an_interrupted_session_resumes_its_conversation_in_a_fresh_terminal() {
    interrupted_session_resumes_its_conversation("claude").await;
}

/// The Codex half of PLAN_M3.md acceptance 8: until this test existed, the
/// "both fixture pairs restart-resume their own conversation" claim was
/// only pinned for Codex up to offer-and-argv (`snapshot.resume_offer`,
/// `resume_argv`) — nothing actually EXECUTED a resume relaunch and
/// confirmed the SAME conversation record grew on disk afterward, the way
/// [`an_interrupted_session_resumes_its_conversation_in_a_fresh_terminal`]
/// already does for Claude. The resume machinery itself is kind-agnostic
/// (see `fixture_resume_template`'s docs), but only running it end to end
/// against Codex's differently-shaped, date-nested record tree
/// (`resumed_record_file`) rules out a Claude-only bug hiding behind a
/// kind-agnostic-looking code path.
#[tokio::test]
async fn an_interrupted_codex_session_resumes_its_conversation_in_a_fresh_terminal() {
    interrupted_session_resumes_its_conversation("codex").await;
}

/// SPEC.md's verbatim fallback resume, which only an explicitly configured
/// placeholder-free template can produce (PLAN_M3.md item 7): the session
/// offers `FallbackTemplate`, and restarting it runs that template rather
/// than the launch invocation.
///
/// The two commands are deliberately distinguishable in the terminal — the
/// launch prints one marker and the fallback another — because "ran the
/// right command" is the whole claim, and a template that silently fell
/// back to the launch invocation would otherwise look identical.
#[tokio::test]
async fn a_configured_fallback_template_is_what_a_restart_runs() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            "sh -c 'echo LAUNCH-INVOCATION; sleep 300'",
            None,
            80,
            24,
            farhelm_helm::CreateExtras {
                // Placeholder-free, on a session whose basename derives no
                // integration: SPEC.md's "the profile's resume invocation
                // verbatim".
                resume_template: Some(vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo FALLBACK-RESUME; sleep 300".to_string(),
                ]),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create with a configured fallback resume command");
    assert_eq!(
        session.restart_offer,
        farhelm_proto::RestartOffer::FallbackTemplate,
        "a configured placeholder-free template is an offer in its own right, not a fresh launch"
    );

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "LAUNCH-INVOCATION", 20).await;

    // The mode has to match the offer exactly — a `Fresh` restart of a
    // session with a configured fallback is refused, not silently honored.
    let refused = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect_err("fresh is not a legal mode for a fallback-template offer");
    assert_eq!(
        refused
            .downcast_ref::<SupervisorError>()
            .expect("classified")
            .kind,
        ErrorKind::Conflict
    );

    h.client
        .restart_session(
            &session.id,
            farhelm_proto::RestartMode::FallbackTemplate,
            true,
        )
        .await
        .expect("restart through the configured fallback");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach after restart");
    let mut replay = Vec::new();
    wait_for_after(
        &mut rx2,
        &mut replay,
        "LAUNCH-INVOCATION",
        "FALLBACK-RESUME",
        20,
    )
    .await;
}

/// The variable the `env-echo` fixture reports (`fake_agent::RC_MARKER_VAR`),
/// duplicated for the same reason [`FAKE_AGENT_RESUME_ENV`] is: this crate
/// has no library target for a test to import from. Drift fails the test
/// rather than weakening it — the fixture would report an empty value and
/// the assertions below would not find the one they wait for.
pub(crate) const RC_MARKER_VAR: &str = "FARHELM_RC_MARKER";

/// Write rc files exporting [`RC_MARKER_VAR`] as `value` into a private
/// HOME, covering every shell family this launch chain might resolve to.
///
/// The launch shell is whatever the supervisor's own `$SHELL`/passwd entry
/// says (`launch::resolve_shell`), which no test may change — so instead of
/// guessing one, this writes the file each family reads for an INTERACTIVE
/// LOGIN shell (`-l -i`, the shape `window_command` uses): bash reads
/// `.bash_profile` (and `.bashrc` when a profile sources it, as this one
/// does), zsh reads `.zshenv`/`.zprofile`/`.zshrc` under `ZDOTDIR`, and a
/// POSIX `sh` reads `$ENV`. Whichever one the host uses, the value arrives
/// by the route a user's own rc file would take.
pub(crate) fn write_rc_files(home: &std::path::Path, value: &str) {
    let export = format!("export {RC_MARKER_VAR}={value}\n");
    std::fs::write(home.join(".bashrc"), &export).expect("write .bashrc");
    std::fs::write(
        home.join(".bash_profile"),
        format!(". \"$HOME/.bashrc\"\n{export}"),
    )
    .expect("write .bash_profile");
    for name in [".zshenv", ".zprofile", ".zshrc", ".profile", ".shinit"] {
        std::fs::write(home.join(name), &export).unwrap_or_else(|e| panic!("write {name}: {e}"));
    }
}

/// M3 acceptance 9's last clause, and SPEC.md's environment contract: "the
/// environment is evaluated at each launch: edit your rc files and the next
/// launch or restart sees the change".
///
/// The rc files live in a private HOME injected through
/// `SupervisorSeams::launch_env` — never by mutating this process's
/// environment, which this repo forbids and which every concurrently
/// running harness would share anyway.
///
/// If the host's login shell reads none of the files this test can write,
/// it says so loudly and stops rather than asserting something it cannot
/// observe: a silent pass would be worse than an honest skip, and a
/// failure would blame the product for the harness's blind spot.
#[tokio::test]
async fn an_rc_file_change_between_launches_reaches_the_relaunched_agent() {
    let home = tempfile::tempdir().expect("fixture home");
    write_rc_files(home.path(), "first");
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_env: vec![
                (
                    "HOME".to_string(),
                    home.path().to_string_lossy().into_owned(),
                ),
                (
                    "ZDOTDIR".to_string(),
                    home.path().to_string_lossy().into_owned(),
                ),
                (
                    "ENV".to_string(),
                    home.path().join(".shinit").to_string_lossy().into_owned(),
                ),
            ],
            ..SupervisorSeams::default()
        },
    )
    .await;
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script env-echo"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, &format!("ENV:{RC_MARKER_VAR}="), 20).await;
    let observed = marker_value(&seen, &format!("ENV:{RC_MARKER_VAR}="));
    if observed != "first" {
        // Deterministic, not a shrug: the rc files this test writes cover
        // the shell families this launch chain can resolve to (see
        // `write_rc_files`), so for any of them the value MUST have
        // arrived. Anything else is a host whose login shell this harness
        // genuinely cannot reach, which is a skip — and one that names the
        // shell, so the gap is diagnosable rather than mysterious.
        let shell = farhelm_supervisor::launch::resolve_shell().await;
        let family = std::path::Path::new(&shell)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| shell.clone());
        assert!(
            !["bash", "zsh", "sh", "dash", "ksh"].contains(&family.as_str()),
            "the launch shell is {shell}, which sources one of the rc files this test writes, \
             so the relaunched agent should have seen the value; it reported {observed:?} \
             instead"
        );
        eprintln!(
            "SKIPPED an_rc_file_change_between_launches_reaches_the_relaunched_agent: this \
             host launches sessions through {shell}, which sources none of the rc files this \
             test knows how to write"
        );
        return;
    }

    // The edit a user would make between launches.
    write_rc_files(home.path(), "second");
    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    // A restart detaches whatever was attached to the previous run (the
    // supervisor's `detach_for_restart`), so the client reattaches — which
    // is also how it gets the reused terminal's scrollback replayed,
    // first run's line included.
    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("attach after restart");
    // Anchored AFTER the first run's own line, which is still in the
    // reused terminal's scrollback: an unanchored wait would match the
    // pre-restart value and pass without the relaunch having sourced
    // anything.
    let mut replay = Vec::new();
    wait_for_after(
        &mut rx2,
        &mut replay,
        &format!("ENV:{RC_MARKER_VAR}=first"),
        &format!("ENV:{RC_MARKER_VAR}=second"),
        30,
    )
    .await;
}

/// M3 acceptance 4's restart clause: after a successful restart, the
/// previous launch's `error` is gone — status, detail, and the sentinel
/// file that produced it.
///
/// The session is created with an invocation that cannot exec plus a
/// configured resume command that can, which is the only way (before M6.75's
/// profiles) to give one session both a failing launch and a working
/// relaunch. What that combination really exercises is the per-launch
/// sentinel lifecycle: the failed launch's sentinel sits at the very path
/// this relaunch's own would use, and a build that left it there would
/// classify a perfectly good agent as `error` forever.
#[tokio::test]
async fn a_restart_clears_a_previous_launch_error() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = tempfile::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &missing_binary.to_string_lossy(),
            None,
            80,
            24,
            farhelm_helm::CreateExtras {
                resume_template: Some(vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo RELAUNCHED-OK; sleep 300".to_string(),
                ]),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create a session whose invocation cannot exec");

    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;
    let errored = wait_for_non_alive_status(&h.client, &session.id, 30).await;
    assert!(
        matches!(errored.status, SessionStatus::Error { .. }),
        "a launch that never execed is an error, not an exit: {errored:?}"
    );

    h.client
        .restart_session(
            &session.id,
            farhelm_proto::RestartMode::FallbackTemplate,
            false,
        )
        .await
        .expect("restart through the configured resume command");

    let alive = wait_for_alive_status(&h.client, &session.id, 30).await;
    assert!(
        !matches!(alive.status, SessionStatus::Error { .. }),
        "the previous launch's error describes a run this session no longer has"
    );
    // Sentinel paths are generation-scoped, so even a surviving gen-0 file
    // could never describe the relaunch's generation. What this pins is the
    // cleanup half: the consumed sentinel is removed rather than left as an
    // orphan for every future reload to re-read and re-classify.
    let sentinel = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    assert!(
        !sentinel.exists(),
        "the failed launch's sentinel must not outlive the launch it described: {}",
        sentinel.display()
    );
}
