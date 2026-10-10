//! `RestartSession` against a real client and real tmux, pinning the
//! terminal-reuse behavior tmux itself provides on a respawned pane.

use crate::harness::*;

use crate::boot_id_durable_outcome::{listed, wait_for_dead_pane};
use crate::create_idempotency::handoff_to_new_supervisor;
use crate::hook_identity::{
    ServeTask, attach_ready, hook_log, hook_log_at, report as report_conversation, report_client,
};
use crate::structured_launches::{FakeHarness, fake_agent_launch, fake_harness, observed_argv};

/// Give compiled vendor names an owned executable and a private fixture home.
///
/// Restart-with compiles `claude` rather than the absolute fixture path used
/// at create. The owned login profile makes that name resolve inside the
/// launch shell without changing this test process's environment or reaching
/// any installed vendor agent. The returned [`ServeTask`] owns the socket
/// accept loop needed by the hook-reporting fixture; callers must keep it
/// alive for hook reports and stop it before handing the supervisor to a
/// successor.
async fn restart_with_harness() -> (Harness, FakeHarness, ServeTask) {
    let fixture = fake_harness();
    let shell = fixture.bash_shell().to_string_lossy().into_owned();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_env: vec![
                (
                    "HOME".to_string(),
                    fixture
                        .login_home_with_fake_path()
                        .to_string_lossy()
                        .into_owned(),
                ),
                ("SHELL".to_string(), shell.clone()),
            ],
            launch_shell: Some(shell),
            scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let accepting = ServeTask::spawn(&h.sup, h.state.path()).await;
    (h, fixture, accepting)
}

/// Create a Claude agent launch whose choices can be changed on resume.
///
/// The executable wrapper carries real argv through tmux; the launch is the
/// shape the helm's compiler composes, around the fixture's program.
async fn structured_claude_session(h: &Harness, fixture: &FakeHarness) -> SessionInfo {
    let selection = farhelm_proto::LaunchSelection {
        harness: farhelm_proto::LaunchHarness::Claude,
        model: None,
        effort: None,
        permissions: None,
        workspace_trust: None,
    };
    h.client
        .create_session_with_extras(
            &fixture.work().to_string_lossy(),
            fake_agent_launch(&fixture.hook_invocation(&selection), selection),
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect("create structured Claude session")
}

/// Read the launch settings a later supervisor or restart will actually use.
///
/// A visible reply alone cannot prove a refusal left SQLite untouched, and
/// the live entry can differ from the row while a relaunch is in progress.
async fn durable_launch_bundle(h: &Harness, id: &str) -> farhelm_proto::SessionLaunch {
    SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open durable store")
        .session(id)
        .await
        .expect("read durable session")
        .expect("session remains stored")
        .launch
}

/// The agent launch the helm composes for `selection`, as Restart with
/// sends it.
fn composed(selection: farhelm_proto::LaunchSelection) -> farhelm_proto::SessionLaunch {
    farhelm_helm::compile_agent_launch(selection).expect("the catalog composes the selection")
}

/// Establish the reported identity before any restart-with assertion uses it.
///
/// The test chooses an id no record carries and reports it through the hook,
/// so only an accepted report can make the session resumable.
async fn captured_claude_conversation(h: &Harness, session: &SessionInfo) -> String {
    let (channel, mut stream, mut seen) = attach_ready(h, session).await;
    let conversation = "restart-report";
    report_conversation(h, channel, &mut stream, &mut seen, conversation).await;
    let snapshot = snapshot_of(h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation.as_deref(),
        Some(conversation),
        "the explicit report must supply the identity; {}",
        hook_log(h, &session.id)
    );
    assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
    conversation.to_string()
}

/// A refused override cannot change the launch settings used by later restarts.
///
/// Check the conflict classification and the durable row, so a plausible
/// error message cannot hide an accidental store write.
async fn assert_restart_with_refused(
    h: &Harness,
    session: &SessionInfo,
    launch: farhelm_proto::SessionLaunch,
    says: &str,
) {
    let before = durable_launch_bundle(h, &session.id).await;
    let error = h
        .client
        .restart_session_with(&session.id, true, Some(launch), None)
        .await
        .expect_err("invalid restart-with must be refused");
    assert_eq!(
        error
            .downcast_ref::<SupervisorError>()
            .expect("supervisor classifies the refusal")
            .kind,
        ErrorKind::Conflict
    );
    // Every refusal here is a Conflict, so the message is what tells WHICH
    // rule refused: a kind change, a type change, or the offer.
    assert!(
        error.to_string().contains(says),
        "the refusal must be the one this case is about ({says:?}): {error:#}"
    );
    assert_eq!(durable_launch_bundle(h, &session.id).await, before);
    let live = listed(&h.client, &session.id).await;
    assert_eq!(live.launch, before);
}

/// A successful override must become both this run's metadata and the next run's default.
///
/// This catches a store-only update whose live `Arc` stays stale: the first
/// reply, a fresh list, and a second plain restart must all name YOLO, while
/// the two relaunched processes receive the flag and captured resume selector.
#[farhelm_testtrace::test]
async fn restart_with_claude_yolo_updates_live_and_future_restarts() {
    let (h, fixture, _accepting) = restart_with_harness().await;
    let session = structured_claude_session(&h, &fixture).await;
    let conversation = captured_claude_conversation(&h, &session).await;
    let mut yolo = session
        .launch
        .agent_selection()
        .cloned()
        .expect("agent launch premise");
    yolo.permissions = Some(farhelm_proto::LaunchPermission::Yolo);

    let restarted = h
        .client
        .restart_session_with(&session.id, true, Some(composed(yolo.clone())), None)
        .await
        .expect("restart with new permissions");
    assert_eq!(restarted.launch.agent_selection(), Some(&yolo));
    assert_eq!(
        restarted.invocation,
        "claude --dangerously-skip-permissions"
    );
    let first_argv = observed_argv(&h, &session.id, 2).await;
    let first_words = shell_words::split(&first_argv).expect("first resumed argv");
    assert!(first_words.contains(&"--dangerously-skip-permissions".to_string()));
    assert!(
        first_words
            .windows(2)
            .any(|pair| pair == ["--resume", &conversation])
    );

    let listed = listed(&h.client, &session.id).await;
    assert_eq!(listed.launch.agent_selection(), Some(&yolo));
    assert_eq!(listed.invocation, restarted.invocation);
    let stored = durable_launch_bundle(&h, &session.id).await;
    assert_eq!(stored.display_command(), restarted.invocation);
    assert_eq!(stored.agent_selection(), Some(&yolo));

    let second = h
        .client
        .restart_session(&session.id, true)
        .await
        .expect("plain restart uses saved settings");
    assert_eq!(second.launch.agent_selection(), Some(&yolo));
    assert_eq!(second.invocation, restarted.invocation);
    let second_words =
        shell_words::split(&observed_argv(&h, &session.id, 3).await).expect("second resumed argv");
    assert!(second_words.contains(&"--dangerously-skip-permissions".to_string()));
    assert!(
        second_words
            .windows(2)
            .any(|pair| pair == ["--resume", &conversation])
    );
}

/// Restart with keeps the launch kind: a command launch that can resume is
/// refused an agent launch, and its stored launch is left alone.
///
/// Why: SPEC.md keeps the launch kind, agent type, host and folder fixed
/// across Restart with; Replace with is how a session changes kind. The
/// refusal must leave the durable row untouched even after the captured
/// conversation makes a normal Resume legal.
#[farhelm_testtrace::test]
async fn restart_with_refuses_a_launch_kind_change_without_changing_settings() {
    let (h, fixture, _accepting) = restart_with_harness().await;
    let work = farhelm_teststate::tempdir().expect("command workdir");
    let selection = farhelm_proto::LaunchSelection {
        harness: farhelm_proto::LaunchHarness::Claude,
        model: None,
        effort: None,
        permissions: None,
        workspace_trust: None,
    };
    let invocation = fixture.hook_invocation(&selection);
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            declared_command(
                &format!("{invocation} {{farhelm_args}}"),
                farhelm_proto::LaunchHarness::Claude,
                Some(&format!(
                    "{invocation} --resume {{conversation}} {{farhelm_args}}"
                )),
            ),
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect("create a Claude command launch");
    assert_eq!(
        session.launch.launch_kind(),
        farhelm_proto::LaunchKind::Command,
        "premise: the session is a command launch"
    );
    captured_claude_conversation(&h, &session).await;
    assert_restart_with_refused(
        &h,
        &session,
        composed(farhelm_proto::LaunchSelection {
            permissions: Some(farhelm_proto::LaunchPermission::Yolo),
            ..selection
        }),
        "keeps the launch kind",
    )
    .await;
}

/// Captured identity does not permit a harness swap.
///
/// The conflict must leave the saved bundle alone, so the session can still
/// resume with its original Claude settings afterwards.
#[farhelm_testtrace::test]
async fn restart_with_refuses_a_harness_mismatch() {
    let (h, fixture, _accepting) = restart_with_harness().await;
    let session = structured_claude_session(&h, &fixture).await;
    captured_claude_conversation(&h, &session).await;
    let mut wrong_harness = session
        .launch
        .agent_selection()
        .cloned()
        .expect("agent launch premise");
    wrong_harness.harness = farhelm_proto::LaunchHarness::Codex;
    assert_restart_with_refused(
        &h,
        &session,
        composed(wrong_harness),
        "keeps the agent type",
    )
    .await;
}

/// Restart-with needs an actual captured conversation, not merely a structured launch.
///
/// A ready process that has reported nothing cannot resume, so Restart with
/// is unavailable; its stored selection and invocation must survive a
/// refused override.
#[farhelm_testtrace::test]
async fn restart_with_refuses_a_non_resume_offer_without_changing_settings() {
    let (h, fixture, _accepting) = restart_with_harness().await;
    let session = structured_claude_session(&h, &fixture).await;
    observed_argv(&h, &session.id, 1).await;
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::NotCaptured
    );
    let mut yolo = session
        .launch
        .agent_selection()
        .cloned()
        .expect("agent launch premise");
    yolo.permissions = Some(farhelm_proto::LaunchPermission::Yolo);
    assert_restart_with_refused(&h, &session, composed(yolo), "cannot be restarted").await;
}

/// Send one Restart with launch exactly as given, over a connection of its
/// own, and return the supervisor's answer.
///
/// The helm composes every agent launch it sends from a catalog selection,
/// so it cannot produce the malformed launches a broken or hostile client
/// could; this writes the `RestartSession` frame directly, the way the
/// rename tests drive their verb.
/// Unsolicited session notifications can precede the answer on this connection;
/// only a reply to request 1 answers the restart. All frames share one deadline.
async fn raw_restart_with(
    sup: &Arc<Supervisor>,
    session_id: &str,
    launch: farhelm_proto::SessionLaunch,
) -> ControlMsg {
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);
    handshake(&mut reader, &mut writer, "helm")
        .await
        .expect("handshake");
    writer
        .write_control(&ControlMsg::RestartSession {
            req_id: 1,
            session_id: session_id.to_string(),
            stop_if_running: true,
            with: Some(launch),
            expected_launch: None,
        })
        .await
        .expect("write restart-with");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let frame = tokio::time::timeout(remaining, reader.read_frame())
            .await
            .expect("timed out waiting for the restart-with reply")
            .expect("read frame")
            .expect("connection closed before the restart-with was answered");
        if frame.kind == FrameKind::Control {
            let message = parse_control(&frame).expect("parse control");
            if message.reply_req_id() == Some(1) {
                return message;
            }
        }
    }
}

/// Why: Restart with stores its new launch, and loading refuses a row that
/// fails create's checks, so one bad launch would have left the supervisor
/// unable to load its sessions after its next restart. Spec (SPEC.md,
/// Restart with: "The edited launch is validated exactly as a create
/// validates it before anything is stopped"): on a session where Restart
/// with is otherwise allowed (an agent launch, same agent type, Resume
/// offer with a captured conversation), a launch whose resume or start
/// command has `{conversation}` as its program is refused as
/// `InvalidRequest`; the running agent is not stopped, the stored launch is
/// unchanged, and a fresh supervisor over the same state loads the session.
#[farhelm_testtrace::test]
async fn restart_with_refuses_a_bundle_loading_would_refuse_and_stops_nothing() {
    let (h, fixture, accepting) = restart_with_harness().await;
    let session = structured_claude_session(&h, &fixture).await;
    captured_claude_conversation(&h, &session).await;
    let before = durable_launch_bundle(&h, &session.id).await;
    let selection = session
        .launch
        .agent_selection()
        .cloned()
        .expect("agent launch premise");
    let conversation = farhelm_supervisor::agent_kind::CONVERSATION_PLACEHOLDER;
    let argv = |words: &[&str]| {
        words
            .iter()
            .map(|word| word.to_string())
            .collect::<Vec<_>>()
    };
    for (label, start, resume) in [
        (
            "resume program",
            argv(&["claude", "{farhelm_args}"]),
            argv(&[conversation, "--resume", "{farhelm_args}"]),
        ),
        (
            "start program",
            argv(&[conversation, "{farhelm_args}"]),
            argv(&["claude", "--resume", conversation, "{farhelm_args}"]),
        ),
    ] {
        let launch = farhelm_proto::SessionLaunch::Agent {
            selection: selection.clone(),
            start,
            resume: Some(resume),
        };
        match raw_restart_with(&h.sup, &session.id, launch).await {
            ControlMsg::Error { kind, message, .. } => {
                assert_eq!(kind, ErrorKind::InvalidRequest, "{label}: {message}");
                assert!(
                    message.contains(conversation),
                    "{label}: the refusal names the placeholder: {message}"
                );
            }
            other => panic!("{label}: expected a refusal, got {other:?}"),
        }
        assert_eq!(
            durable_launch_bundle(&h, &session.id).await,
            before,
            "{label}: the stored launch is unchanged"
        );
        let live = listed(&h.client, &session.id).await;
        assert!(live.status.is_live(), "{label}: nothing was stopped");
        assert_eq!(live.restart_offer, farhelm_proto::RestartOffer::Resume);
    }

    accepting.stop().await;
    let Harness {
        client,
        sup,
        _tmux,
        state,
        _slot,
    } = h;
    let replacement = handoff_to_new_supervisor(state.path(), sup, client).await;
    let reopened = connect_client(&replacement).await;
    assert!(
        reopened
            .list_sessions()
            .await
            .expect("the fresh supervisor lists its sessions")
            .sessions
            .iter()
            .any(|listed| listed.id == session.id),
        "the fresh supervisor loads the session"
    );
}

/// Release connection-owned supervisor references before reopening its durable state.
///
/// The caller must first stop any accept loop and drop its clients, leaving
/// only the supervisor reference borrowed here. This observes connection
/// cleanup; the caller still owns dropping the supervisor and killing tmux
/// when the scenario models a reboot.
async fn wait_for_resume_connections_to_drain(sup: &Arc<Supervisor>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(sup) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        // sleep-ok: clients and accept loops have stopped; wait for their asynchronous connection tasks to release this supervisor.
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

// ---------------------------------------------------------------------
// Restart with resume (PLAN_M3.md item 9; M3 acceptance 9, plus the
// restart clauses of acceptance 4 and 5)
//
// Every test below drives the real `RestartSession` handler through the
// real client, against a real tmux — the terminal-reuse behavior these
// pin (a respawned pane keeping the prior run above it) is tmux's, not
// this crate's, so a faked driver would prove nothing about it.
// ---------------------------------------------------------------------

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

/// Observe the new banner after retained history, without using attachment replay as evidence.
///
/// The fixture emits only a bounded set of lines before this observation.
/// Both markers must appear in order in tmux's own history; the caller then
/// checks that the old visible grid was discarded. One deadline covers
/// commands and polling, and a timed-out tmux query is killed on drop.
async fn wait_for_new_run_below_history(sock: &std::path::Path, tmux_name: &str) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut capture = String::new();
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the new run never appeared below the prior run's output; capture:\n{capture}"
        );
        let output = tokio::time::timeout_at(
            deadline,
            tokio::process::Command::new("tmux")
                .arg("-S")
                .arg(sock)
                .args(["capture-pane", "-p", "-S", "-", "-t", tmux_name])
                .kill_on_drop(true)
                .output(),
        )
        .await
        .unwrap_or_else(|_| panic!("tmux stopped answering; last capture:\n{capture}"))
        .expect("capture query");
        assert!(
            output.status.success(),
            "capture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        capture = String::from_utf8_lossy(&output.stdout).into_owned();
        if let Some(marker) = capture.find("PRIOR-RUN-MARKER")
            && capture[marker..].contains("FAKE-AGENT READY")
        {
            return capture;
        }
        // sleep-ok: the relaunched agent prints asynchronously; observe its banner after the retained marker in the same pane history.
        tokio::time::sleep(
            Duration::from_millis(200)
                .min(deadline.saturating_duration_since(tokio::time::Instant::now())),
        )
        .await;
    }
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
#[farhelm_testtrace::test]
async fn restarting_a_live_session_stops_its_tree_and_reuses_the_terminal() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = farhelm_teststate::tempdir().unwrap();
    let session = create_resumable_session(
        &h,
        &work.path().to_string_lossy(),
        &fixture_cmd("fake-agent --script spawner"),
        80,
        24,
    )
    .await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tmux_name = format!("fh-{}", session.id);

    let (chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");
    let grandchild_pid = wait_for_child(child_pid, 10).await;
    let pane_before = pane_id_of(&sock, &tmux_name).await;

    // Stopping first is what the user consented to; without that consent
    // the request is refused outright (see the next test).
    let restarted = h
        .client
        .restart_session(&session.id, true)
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

    let alive = wait_for_live_status(&h.client, &session.id, 30).await;
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
/// restart against an agent the supervisor finds WORKING is refused with
/// `Conflict` and kills nothing at all. This harness runs no activity
/// sampler, so a freshly launched agent reads working for the whole test;
/// the idle and waiting cases, which restart without consent, are covered
/// by the supervisor's own unit test, where the reading can be set.
///
/// This is the TOCTOU guard, not a redundancy: a client's cached status can
/// say "idle" or "exited" while the agent is working (another client
/// relaunched it, or the status was simply stale), and the flag is what
/// tells the supervisor "the user was actually asked". So the assertion that matters
/// is the process still being alive afterwards, not just the error.
#[farhelm_testtrace::test]
async fn restarting_a_working_session_without_consent_is_refused_and_kills_nothing() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().unwrap();
    let session = create_resumable_session(
        &h,
        &work.path().to_string_lossy(),
        &fixture_cmd("fake-agent --script spawner"),
        80,
        24,
    )
    .await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let child_pid = extract_pid(&seen, "CHILD-PID:");

    let err = h
        .client
        .restart_session(&session.id, false)
        .await
        .expect_err("a working agent may not be restarted without consent to stop it");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("a refusal carries the supervisor's own classification");
    assert_eq!(err.kind, ErrorKind::Conflict);
    assert!(
        err.message.contains("is working"),
        "the refusal must say why, so a client can ask the user: {}",
        err.message
    );

    assert!(
        !process_is_gone(self_pid) && !process_is_gone(child_pid),
        "a refused restart must not have killed anything"
    );
    assert!(
        listed(&h.client, &session.id).await.status.is_live(),
        "and must leave the session exactly as it was"
    );
}

/// M3 acceptance 9: relaunching into a RETAINED terminal keeps the prior
/// run's HISTORY — SPEC.md: "whatever scrollback the terminal itself
/// retained is still there" — with the new run's output below it.
///
/// History, not the visible screen: the same SPEC paragraph says restart
/// "does NOT preserve the previous run's last visible screen: the pane is
/// blank until the new agent draws", and `respawn-pane` does exactly that
/// (keeps tmux's scrollback, reinitializes the grid). So the marker is
/// pushed off the 24-row screen with `spam 60` before the restart; what
/// this test then finds is the retained scrollback and nothing else. A
/// marker left on the visible grid would be wiped, and asserting on it
/// would be asserting the shrink-and-restore trick SPEC.md now forbids.
///
/// The marker is produced by TYPING into the first run rather than by its
/// startup banner, because both runs print the same banner: an assertion
/// on text only the first run could have produced is what makes this about
/// retention rather than about the relaunch having printed something.
#[farhelm_testtrace::test]
async fn a_reused_terminal_keeps_the_prior_run_above_the_new_one() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let (session, _work) = resumable_basic_session(&h).await;
    let tmux_name = format!("fh-{}", session.id);

    let (chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client
        .send_input(chan, b"PRIOR-RUN-MARKER\r".to_vec())
        .await;
    wait_for(&mut rx, &mut seen, "echo:", 10).await;
    wait_for(&mut rx, &mut seen, "PRIOR-RUN-MARKER", 10).await;
    // Into history: more lines than the window has rows. The last spam
    // line is the barrier, so the restart cannot land while the marker is
    // still on the grid the respawn is about to wipe.
    h.client.send_input(chan, b"spam 60\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "spam-line-60", 10).await;
    // And one marker that stays on the VISIBLE GRID only: typed after the
    // spam, so nothing ever scrolls it into history before the restart.
    // Its absence afterwards is the other half of the contract — finding
    // it would mean something preserved the visible grid across the
    // respawn, which is the shrink-and-restore trick SPEC.md now forbids.
    // Waited for AFTER the last spam line so the echo provably reached
    // the grid before the restart is issued.
    h.client
        .send_input(chan, b"GRID-ONLY-MARKER\r".to_vec())
        .await;
    wait_for_after(&mut rx, &mut seen, "spam-line-60", "GRID-ONLY-MARKER", 10).await;

    h.client
        .restart_session(&session.id, true)
        .await
        .expect("restart");
    wait_for_live_status(&h.client, &session.id, 30).await;

    // Read from tmux itself (`capture-pane -S -`, history included), and
    // wait for the new run's own banner to appear in the capture: the
    // relaunched agent starts asynchronously, so a single read can land
    // before it has printed anything.
    let capture = wait_for_new_run_below_history(&sock, &tmux_name).await;
    assert!(
        capture.contains("PRIOR-RUN-MARKER"),
        "the prior run's retained scrollback must survive the respawn: {capture}"
    );
    assert!(
        !capture.contains("GRID-ONLY-MARKER"),
        "the prior run's visible grid must NOT survive the respawn — its presence means \
         something preserved the grid (the forbidden shrink-and-restore): {capture}"
    );

    // And a client attaching after the restart sees the same thing, since
    // its replay covers that scrollback (SPEC.md: at least the current
    // screen plus 10,000 lines of it).
    h.client.detach(chan).await;
    let (_chan2, initial_replay, mut rx2) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach after restart");
    let mut replay = initial_replay;
    wait_for_after(
        &mut rx2,
        &mut replay,
        "PRIOR-RUN-MARKER",
        "FAKE-AGENT READY",
        20,
    )
    .await;
    assert!(
        !String::from_utf8_lossy(&replay).contains("GRID-ONLY-MARKER"),
        "the replay must not resurrect the prior run's visible grid either"
    );
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
#[farhelm_testtrace::test]
async fn a_restart_reaps_a_daemon_left_by_a_self_exited_agent() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().unwrap();
    let session = create_resumable_session(
        &h,
        &work.path().to_string_lossy(),
        &fixture_cmd("fake-agent --script spawner-reparent"),
        80,
        24,
    )
    .await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let self_pid = extract_pid(&seen, "SELF-PID:");
    let daemon_pid = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    // SAFETY: `self_pid` is a real, currently-live pid this test just read
    // out of the fake agent's own output.
    unsafe {
        libc::kill(self_pid as libc::pid_t, libc::SIGKILL);
    }
    wait_until_pid_gone(self_pid, 10).await;
    wait_for_non_live_status(&h.client, &session.id, 20).await;
    assert!(
        !process_is_gone(daemon_pid),
        "the daemon must outlive its parent, or this test proves nothing"
    );

    h.client
        .restart_session(&session.id, false)
        .await
        .expect("an agent that already exited needs no stop consent");

    // Again the end state rather than the interleaving: the daemon is gone
    // and a new run is up. The "before, not alongside" ordering is a
    // property of the handler (the sweep completes before the generation is
    // opened), not something this vantage point can witness.
    wait_until_pid_gone(daemon_pid, 15).await;
    wait_for_live_status(&h.client, &session.id, 30).await;
}

/// M3 acceptance 9: a vanished working directory fails the restart with an
/// error NAMING the directory, and the session survives untouched — its
/// stop annotation included, which is PLAN_M3.md item 4's "only a
/// SUCCESSFUL restart clears it".
///
/// The annotation is what makes this more than an error-message test: the
/// clear commits with the new launch generation, so a restart that never
/// gets a generation must leave the stopped outcome exactly as it was.
#[farhelm_testtrace::test]
async fn a_vanished_working_directory_refuses_the_restart_and_keeps_the_annotation() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().unwrap();
    let cwd = work.path().to_string_lossy().into_owned();
    let session =
        create_resumable_session(&h, &cwd, &fixture_cmd("fake-agent --script basic"), 80, 24).await;

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
        .restart_session(&session.id, false)
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
/// `argv0` is the kind-named symlink so the resumed process reports as the
/// same agent the session declared.
fn fixture_resume_template(
    argv0: &std::path::Path,
    kind: &str,
    record_home: &std::path::Path,
) -> Vec<String> {
    vec![
        "sh".to_string(),
        "-c".to_string(),
        format!(
            "{}=\"$2\" exec \"$0\" fake-agent --script {kind}-record --record-home \"$1\"",
            FAKE_AGENT_RESUME_ENV
        ),
        argv0.to_string_lossy().into_owned(),
        record_home.to_string_lossy().into_owned(),
        farhelm_supervisor::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
    ]
}

/// Find the Claude record the resumed fixture appended to. This is an actual
/// conversation effect, rather than an assertion about the stored capture
/// value, so a broken resume can neither pass on a substituted argv alone nor
/// hide behind the removed Codex directory-scan fixture.
fn resumed_record_file(
    home: &std::path::Path,
    work: &std::path::Path,
    conversation: &str,
) -> std::path::PathBuf {
    let canonical = std::fs::canonicalize(work).expect("canonicalize the workdir");
    std::fs::read_dir(
        home.join(".claude")
            .join("projects")
            .join(canonical.to_string_lossy().replace(['/', '.', '_'], "-")),
    )
    .expect("project dir")
    .map(|entry| entry.expect("dir entry").path())
    .find(|path| path.to_string_lossy().contains(conversation))
    .expect("the captured record still exists")
}
/// M3 acceptance 9 and 8 together: a session INTERRUPTED by a (simulated)
/// reboot restarts into a FRESH terminal — there is none left to reuse —
/// and `Resume` mode fills the snapshot's template with the identity that
/// was reported before the reboot, so the relaunched agent picks up the
/// same conversation.
///
/// The unstructured branch proves record adoption and append behavior. The
/// structured branch instead isolates integration-derived resume argv and
/// frozen launch metadata: its admission template is deliberately absent, so
/// the test must not claim the wrapper-supplied record behavior it does not
/// exercise.
///
/// This fixture keeps its scope to Claude's report-and-record path. Codex needs
/// a foreground-attributed locator, and that behavior belongs to the dedicated
/// fixture.
async fn interrupted_session_resumes_its_conversation(structured: bool) {
    let kind = "claude";
    // The structured branch extracts argv from terminal output. Use the wide
    // grid for creation and both attachments, and wait for READY before parsing;
    // an 80-column replay or a partial output frame can truncate that witness.
    let cols = if structured { WIDE_COLS } else { 80 };
    let home = farhelm_teststate::tempdir().expect("agent home");
    let bin = farhelm_teststate::tempdir().expect("agent bin");
    std::os::unix::fs::symlink(fixtures_bin(), bin.path().join(kind))
        .expect("symlink the farhelm binary under the agent's own name");
    let state = farhelm_teststate::tempdir().expect("state dir");
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let _tmux = TmuxServerGuard::new(state.path().join("tmux.sock"));

    let seams = |boot: &str| SupervisorSeams {
        boot_id: {
            let boot = boot.to_string();
            Arc::new(move || Ok(Some(boot.clone())))
        },

        ..SupervisorSeams::default()
    };

    let work = farhelm_teststate::tempdir().expect("workdir");
    let resume_template =
        (!structured).then(|| fixture_resume_template(&bin.path().join(kind), kind, home.path()));
    let selection = structured.then(|| farhelm_proto::LaunchSelection {
        harness: farhelm_proto::LaunchHarness::Claude,
        model: Some("claude-fable-5".to_string()),
        effort: Some(farhelm_proto::LaunchEffort::High),
        permissions: Some(farhelm_proto::LaunchPermission::Yolo),
        workspace_trust: None,
    });
    let structured_options = structured
        .then(|| "--model claude-fable-5 --effort high --dangerously-skip-permissions".to_string());
    let conversation = {
        let sup = Supervisor::new_with_seams(
            state.path(),
            farhelm_bin().into(),
            // Built by hand (not `harness()`) for the boot-id seam below;
            // `suite_timeouts()` still gives this real attach the suite's
            // loaded-CI tmux floors.
            suite_timeouts(),
            seams("boot-a"),
        )
        .await
        .expect("first supervisor");
        let accepting = ServeTask::spawn(&sup, state.path()).await;
        let client = connect_client(&sup).await;
        let invocation = format!(
            "{} fake-agent --script hook-report --record-home {} {}",
            shell_words::quote(&bin.path().join(kind).to_string_lossy()),
            shell_words::quote(&home.path().to_string_lossy()),
            structured_options.as_deref().unwrap_or("")
        );
        // The structured branch is an agent launch; the other a command
        // launch declaring Claude with the fixture's own resume command.
        let launch = match &selection {
            Some(selection) => fake_agent_launch(&invocation, selection.clone()),
            None => declared_command(
                &format!("{invocation} {{farhelm_args}}"),
                farhelm_proto::LaunchHarness::Claude,
                Some(&format!(
                    "{} {{farhelm_args}}",
                    shell_words::join(resume_template.as_ref().expect("command branch"))
                )),
            ),
        };
        let session = client
            .create_session_with_extras(
                &work.path().to_string_lossy(),
                launch,
                None,
                cols,
                ROWS,
                farhelm_helm::CreateExtras::default(),
            )
            .await
            .expect("create the hook-reporting session");

        let (chan, initial_replay, mut rx) = client
            .attach_live(&session.id, cols, ROWS)
            .await
            .expect("attach");
        let mut seen = initial_replay;
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
        if let Some(options) = &structured_options {
            wait_for(&mut rx, &mut seen, ARGV_MARKER, 20).await;
            let argv = argv_marker(&seen);
            // The fixture shell-quotes its argv; decode it before comparing
            // adjacent flag/value pairs, including values that require quotes.
            let words = shell_words::split(&argv).expect("structured boot-A argv");
            let options = shell_words::split(options).expect("structured boot-A options");
            assert!(
                words.windows(options.len()).any(|window| window == options),
                "boot-A {kind} argv lost selected options {options:?}: {argv}"
            );
        }
        client.send_input(chan, b"first prompt\r".to_vec()).await;
        wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;
        let conversation = marker_value(&seen, "RECORD-WRITTEN:");
        report_client(&sup, &client, chan, &mut rx, &mut seen, &conversation).await;

        // The report helper crosses the explicit reconciliation boundary.
        // Check both identity and source so a scan cannot satisfy this premise.
        let snapshot = sup
            .session_snapshot(&session.id)
            .await
            .expect("snapshot")
            .expect("session exists");
        assert_eq!(
            snapshot.captured_conversation.as_deref(),
            Some(conversation.as_str()),
            "the explicit report must be durable before reconstruction; {}",
            hook_log_at(state.path(), &session.id)
        );
        let stored = SessionStore::open(&state.path().join("supervisor.db"), false)
            .await
            .expect("open durable store")
            .session(&session.id)
            .await
            .expect("read reported session")
            .expect("reported session stored");
        assert_eq!(
            stored.conversation_source.as_deref(),
            Some("hook"),
            "the identity must come from the accepted report, not a scan claim"
        );
        assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
        if let Some(template) = &resume_template {
            // The declared resume command is the fixture's template with
            // `{farhelm_args}` appended, exactly as created.
            let declared: Vec<String> = template
                .iter()
                .cloned()
                .chain([farhelm_proto::session_launch::FARHELM_ARGS_PLACEHOLDER.to_string()])
                .collect();
            assert_eq!(
                snapshot.resume_template.as_deref(),
                Some(declared.as_slice()),
                "the resume template stored at creation must survive until reconstruction"
            );
        } else {
            let template = snapshot
                .resume_template
                .as_ref()
                .expect("structured restart stores its integration default");
            assert!(
                template.iter().any(|word| word == "--resume")
                    && template.iter().any(|word| word == "{conversation}"),
                "structured restart stores the Claude integration default, not a catalog-derived template: {template:?}"
            );
            let stored = SessionStore::open(&state.path().join("supervisor.db"), false)
                .await
                .expect("open durable store")
                .session(&session.id)
                .await
                .expect("read structured session")
                .expect("structured session stored");
            assert_eq!(
                stored.launch.agent_selection().cloned(),
                selection,
                "the nondefault structured selection is durable before reconstruction"
            );
        }
        accepting.stop().await;
        drop(client);
        wait_for_resume_connections_to_drain(&sup).await;
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
        // Same reasoning as the first supervisor above: this one attaches
        // for real too (the resume/reattach below).
        suite_timeouts(),
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
        .restart_session(&session.id, false)
        .await
        .expect("an interrupted session has nothing running to consent about");
    assert_eq!(
        restarted.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "the identity is the conversation's, not the run's — it survives the relaunch too"
    );
    assert_eq!(
        restarted.launch.agent_selection().cloned(),
        selection,
        "restart response retains the frozen structured selection"
    );

    let (chan, initial_replay, mut rx) = client
        .attach_live(&session.id, cols, ROWS)
        .await
        .expect("the relaunch built a fresh terminal to attach to");
    let mut seen = initial_replay;
    if structured {
        // READY follows the complete argv line; the marker alone could be
        // only the first chunk of live output.
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 30).await;
        let argv = crate::harness::argv_marker(&seen);
        let expected = format!("--resume {conversation}");
        assert!(
            argv.contains(&expected),
            "the reconstructed Claude successor must use its default resume form: {argv}"
        );
        let words = shell_words::split(&argv).expect("structured successor argv");
        let options =
            shell_words::split(structured_options.as_deref().expect("structured options"))
                .expect("structured successor options");
        assert!(
            words.windows(options.len()).any(|window| window == options),
            "reconstructed Claude argv lost selected options {options:?}: {argv}"
        );
        crate::structured_launches::assert_live_exchange(
            &client,
            chan,
            &mut rx,
            state.path(),
            &session.id,
        )
        .await;
        let live = listed(&client, &session.id).await;
        assert_eq!(
            live.launch.agent_selection().cloned(),
            selection,
            "live reconstruction projection retains the selected structured fields"
        );
        let stored = SessionStore::open(&state.path().join("supervisor.db"), false)
            .await
            .expect("open reconstructed durable store")
            .session(&session.id)
            .await
            .expect("read reconstructed session")
            .expect("session stored");
        assert_eq!(
            stored.launch.agent_selection().cloned(),
            selection,
            "stored reconstruction projection retains the selected structured fields"
        );
        return;
    }
    wait_for(
        &mut rx,
        &mut seen,
        &format!("RECORD-RESUMED:{conversation}"),
        30,
    )
    .await;
    // Do not infer launch-versus-resume from the echoed argv. Both commands
    // carry `--record-home`, and tmux replay represents a visual wrap as a
    // real newline because `capture-pane` intentionally runs without `-J`.
    // `RECORD-RESUMED:<id>` is the discriminating fact: the fixture prints it
    // only after the template-supplied id locates an existing record.

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
        std::fs::read(resumed_record_file(home.path(), work.path(), &conversation))
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
#[farhelm_testtrace::test]
async fn an_interrupted_session_resumes_its_conversation_in_a_fresh_terminal() {
    interrupted_session_resumes_its_conversation(false).await;
}

/// Structured selections survive boot-A/boot-B reconstruction without asking
/// today's catalog how an already accepted session should resume.
#[farhelm_testtrace::test]
async fn structured_claude_resume_survives_supervisor_reconstruction() {
    interrupted_session_resumes_its_conversation(true).await;
}

/// The same interrupted-then-resumed journey as
/// [`an_interrupted_session_resumes_its_conversation_in_a_fresh_terminal`],
/// but for a reported identity with no backing record. A supervisor restart
/// must preserve that identity without asking the record scanner to find it.
///
/// The reported id is deliberately one no record on disk carries. That is
/// the whole point of the mechanism (`/clear` mints an id the scan can
/// never correlate), and it also means nothing but the report could have
/// put this value in the resume argv.
///
/// It also bounds what the relaunched fixture can be asked to prove. A
/// real resume ADOPTS an existing record, and this conversation has none
/// to adopt — so the fixture's honest answer is
/// `RECORD-RESUME-MISSING:conv-h`, which is still the fixture naming the
/// id it was told to resume, from inside the relaunched process. That,
/// plus the substituted id in its echoed argv, is the whole proof
/// available here: the resumed run cannot be shown to continue a
/// conversation that never existed on disk.
///
/// This sibling reports an id no record carries and checks the fixture's
/// missing-record resume witness. The preceding tests report a record-backed
/// id and check adoption plus append behavior; all of them serve the hook over
/// the supervisor's unix socket.
#[farhelm_testtrace::test]
async fn an_interrupted_hook_reported_session_resumes_its_conversation() {
    let home = farhelm_teststate::tempdir().expect("agent home");
    let bin = farhelm_teststate::tempdir().expect("agent bin");
    std::os::unix::fs::symlink(fixtures_bin(), bin.path().join("claude"))
        .expect("symlink the farhelm binary under the agent's own name");
    let state = farhelm_teststate::tempdir().expect("state dir");
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let _tmux = TmuxServerGuard::new(state.path().join("tmux.sock"));

    let seams = |boot: &str| SupervisorSeams {
        boot_id: {
            let boot = boot.to_string();
            Arc::new(move || Ok(Some(boot.clone())))
        },

        ..SupervisorSeams::default()
    };
    let claude = bin.path().join("claude");
    let invocation = format!(
        "{} fake-agent --script hook-report --record-home {}",
        shell_words::quote(&claude.to_string_lossy()),
        shell_words::quote(&home.path().to_string_lossy())
    );
    // A template that makes the substituted id do BOTH jobs, which
    // neither of the two existing shapes does alone. `fixture_resume_template`
    // moves the id into `FAKE_AGENT_RESUME_ENV` (so the fixture adopts it)
    // but leaves it invisible in the argv; a bare `--resume <id>` argv
    // shows it (the fixture's `extra` catch-all lets clap accept the flag)
    // but the fixture never reads it. Here the id is the only evidence
    // there is — no record exists for it — so it is passed both ways: the
    // argv echo proves the supervisor substituted it, and the fixture's
    // own resume marker proves the relaunched process took it as the
    // conversation it was resuming.
    let template = vec![
        "sh".to_string(),
        "-c".to_string(),
        format!(
            "{FAKE_AGENT_RESUME_ENV}=\"$3\" exec \"$0\" fake-agent --script hook-report \
             --record-home \"$1\" \"$2\" \"$3\""
        ),
        claude.to_string_lossy().into_owned(),
        home.path().to_string_lossy().into_owned(),
        "--resume".to_string(),
        farhelm_supervisor::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
    ];

    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = {
        let sup = Supervisor::new_with_seams(
            state.path(),
            farhelm_bin().into(),
            suite_timeouts(),
            seams("boot-a"),
        )
        .await
        .expect("first supervisor");
        // The accept loop as a guard rather than a bare handle: the
        // assertions below it would otherwise leak a running loop (and the
        // `Arc<Supervisor>` it holds) on the way out of a panic, and this
        // test's whole premise is that the predecessor is gone before the
        // successor is built. See `hook_identity::ServeTask`.
        let accepting = crate::hook_identity::ServeTask::spawn(&sup, state.path()).await;
        let client = connect_client(&sup).await;
        let session = client
            .create_session_with_extras(
                &work.path().to_string_lossy(),
                declared_command(
                    &format!("{invocation} {{farhelm_args}}"),
                    farhelm_proto::LaunchHarness::Claude,
                    Some(&format!(
                        "{} {{farhelm_args}}",
                        shell_words::join(&template)
                    )),
                ),
                None,
                200,
                24,
                farhelm_helm::CreateExtras::default(),
            )
            .await
            .expect("create the hook-reporting session");

        let (chan, initial_replay, mut rx) = client
            .attach_live(&session.id, 200, 24)
            .await
            .expect("attach");
        let mut seen = initial_replay;
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
        client.send_input(chan, b"report conv-h\r".to_vec()).await;
        wait_for(&mut rx, &mut seen, "HOOK-REPORTED:conv-h", 30).await;
        // The hook has dropped its report; apply it now rather than waiting
        // for the ticker's pass.
        sup.reconcile_for_test().await;
        assert_eq!(
            sup.session_snapshot(&session.id)
                .await
                .expect("snapshot")
                .expect("present")
                .captured_conversation
                .as_deref(),
            Some("conv-h"),
            "the report must be durable before the reboot, or this proves nothing about \
             reload"
        );

        // Stop accepting BEFORE the drain: the accept loop holds a
        // supervisor reference of its own, so a drain that only dropped the
        // client would spin until its deadline.
        accepting.stop().await;
        drop(client);
        wait_for_resume_connections_to_drain(&sup).await;
        drop(sup);
        session
    };

    // The reboot: tmux dies with the host, and the next supervisor reads a
    // different boot id.
    kill_tmux_server_and_wait(&state.path().join("tmux.sock")).await;
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        suite_timeouts(),
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
        "a reported identity survives the supervisor that recorded it, so opening this \
         session offers a resume"
    );

    client
        .restart_session(&session.id, false)
        .await
        .expect("an interrupted session has nothing running to consent about");
    let (_chan, initial_replay, mut rx) = client
        .attach_live(&session.id, 200, 24)
        .await
        .expect("the relaunch built a fresh terminal to attach to");
    let mut seen = initial_replay;
    // Anchored on the substituted id rather than on the argv marker: the
    // relaunch is what has to produce it, and no earlier generation's
    // output contains it.
    wait_for(&mut rx, &mut seen, "--resume conv-h", 30).await;
    // And the fixture's own verdict on the id it was handed. It cannot
    // adopt a record for a conversation that only ever existed as a report
    // (see this test's docs), so "missing" IS the expected answer — what
    // matters is that it names conv-h, which only the relaunched process
    // could have read out of its own environment.
    wait_for(&mut rx, &mut seen, "RECORD-RESUME-MISSING:conv-h", 30).await;

    // The permit goes back only once the successor and its tmux server are
    // gone: `SLOTS` bounds concurrent tmux servers and supervisors, and
    // releasing it here — before the implicit drops at the end of scope —
    // would admit the next harness onto a machine still carrying this one.
    drop(client);
    drop(sup);
    drop(_tmux);
    drop(slot);
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
#[farhelm_testtrace::test]
async fn an_rc_file_change_between_launches_reaches_the_relaunched_agent() {
    let home = farhelm_teststate::tempdir().expect("fixture home");
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
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create_resumable_session(
        &h,
        &work.path().to_string_lossy(),
        &fixture_cmd("fake-agent --script env-echo"),
        80,
        24,
    )
    .await;

    let (chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
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
        .restart_session(&session.id, true)
        .await
        .expect("restart");
    wait_for_live_status(&h.client, &session.id, 30).await;

    // A restart detaches whatever was attached to the previous run (the
    // supervisor's `detach_for_restart`), so the client reattaches — which
    // is also how it gets the reused terminal's scrollback replayed,
    // first run's line included.
    h.client.detach(chan).await;
    let (_chan2, initial_replay, mut rx2) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach after restart");
    // Anchored AFTER the first run's own line, which is still in the
    // reused terminal's scrollback: an unanchored wait would match the
    // pre-restart value and pass without the relaunch having sourced
    // anything.
    let mut replay = initial_replay;
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
/// resume command that can, and a conversation recorded through the test
/// seam, which gives one session both a failing launch and a working
/// relaunch (restart only resumes). What that combination really exercises is the per-launch
/// sentinel lifecycle: the failed launch's sentinel sits at the very path
/// this relaunch's own would use, and a build that left it there would
/// classify a perfectly good agent as `error` forever.
#[farhelm_testtrace::test]
async fn a_restart_clears_a_previous_launch_error() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let missing_binary = work.path().join("no-such-farhelm-agent");
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            // Goose, for the reasons `create_resumable_session` gives.
            declared_command(
                &format!(
                    "{} {{farhelm_args}}",
                    shell_words::quote(&missing_binary.to_string_lossy())
                ),
                farhelm_proto::LaunchHarness::Goose,
                Some(
                    "sh -c 'echo RELAUNCHED-OK; sleep 300' farhelm-test-resume {conversation} \
                     {farhelm_args}",
                ),
            ),
            None,
            80,
            24,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect("create a session whose invocation cannot exec");
    h.sup
        .record_conversation_for_test(&session.id, RESUMABLE_TEST_CONVERSATION)
        .await;

    wait_for_dead_pane(&sock, &format!("fh-{}", session.id)).await;
    h.sup.reconcile_for_test().await;
    let errored = wait_for_non_live_status(&h.client, &session.id, 30).await;
    assert!(
        matches!(errored.status, SessionStatus::Error { .. }),
        "a launch that never execed is an error, not an exit: {errored:?}"
    );

    h.client
        .restart_session(&session.id, false)
        .await
        .expect("restart through the resume command");

    let alive = wait_for_live_status(&h.client, &session.id, 30).await;
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
