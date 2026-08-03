//! Concurrent restarts of one session, and failure paths that must not
//! lose durable metadata (PR8 review-swarm fix batch, items 1 and 4).

use crate::harness::*;

use crate::boot_id_durable_outcome::{listed, wait_for_dead_pane};
use crate::conversation_identity_capture::{
    capture_harness, last_marker_value, provoke_record, record_session, settle_past_horizon,
    snapshot_of,
};
use crate::create_idempotency::handoff_to_new_supervisor;
use crate::restart_with_resume::{pane_capture, wait_for_alive_status};

// ---------------------------------------------------------------------
// Restart under concurrency, and the failure paths that must not lose
// durable metadata (PR8 review-swarm fix batch, items 1 and 4).
// ---------------------------------------------------------------------

/// The security case behind the per-session lifecycle claim: two restarts
/// of one session must never interleave into a kill nobody consented to.
///
/// Without serialization the sequence is entirely legal-looking and
/// entirely wrong: the first restart records its stop intent and starts a
/// kill sweep that takes seconds; the second restart, arriving mid-sweep,
/// probes the pane, finds it dead, concludes no consent is needed — and
/// then runs ITS marker sweep, which reaps the agent the first restart has
/// meanwhile launched. The user asked for a restart and got a stopped
/// session, with a live agent killed on the way.
///
/// The claim turns that into an ordinary serial pair, and this test pins
/// exactly that: the second restart runs AFTER the first has finished, so
/// it finds a LIVE agent and refuses without consent — and the session is
/// still running when both have returned.
///
/// `spawner-stubborn`'s SIGTERM-ignoring child is what makes the window
/// wide enough to aim at: it forces the first restart's sweep through the
/// full grace/quiesce/SIGKILL escalation rather than finishing instantly.
#[tokio::test]
async fn a_second_restart_cannot_reap_the_agent_the_first_one_just_launched() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-stubborn"),
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
    wait_for_file(&work.path().join("stubborn-ready"), 10).await;

    // The first restart, with consent: it will spend seconds in the sweep.
    let first_client = Arc::clone(&h.client);
    let first_id = session.id.clone();
    let first = tokio::spawn(async move {
        first_client
            .restart_session(&first_id, farhelm_proto::RestartMode::Fresh, true)
            .await
    });
    // Long enough to be inside that sweep, short enough to be well before
    // it ends (`kill_process_tree`'s grace period alone is ~1s).
    tokio::time::sleep(Duration::from_millis(300)).await;
    let second = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await;

    first
        .await
        .expect("the first restart task")
        .expect("the first restart succeeds");
    let refusal = second.expect_err(
        "the second restart must see the FIRST one's relaunched agent, alive, and refuse \
         without consent — never sweep it away",
    );
    assert_eq!(
        refusal
            .downcast_ref::<SupervisorError>()
            .expect("classified")
            .kind,
        ErrorKind::Conflict
    );
    // The whole point: something is still running when the dust settles.
    wait_for_alive_status(&h.client, &session.id, 30).await;
}

/// Delete racing a restart resolves to exactly one winner, with the loser
/// getting an honest error — never a session torn half-down.
///
/// The delete is issued while the restart is inside its kill sweep, which
/// is precisely where an unserialized delete would kill the tmux session
/// the relaunch is about to respawn into. Whichever order the claim
/// imposes, the invariants below hold: the session is gone afterwards, and
/// nothing carrying its marker is still running.
#[tokio::test]
async fn a_delete_racing_a_restart_leaves_no_session_and_no_survivors() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script spawner-stubborn"),
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
    wait_for_file(&work.path().join("stubborn-ready"), 10).await;

    let restart_client = Arc::clone(&h.client);
    let restart_id = session.id.clone();
    let restart = tokio::spawn(async move {
        restart_client
            .restart_session(&restart_id, farhelm_proto::RestartMode::Fresh, true)
            .await
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    h.client
        .delete_session(&session.id)
        .await
        .expect("the delete completes rather than colliding with the relaunch");
    // The restart either finished first (and its agent was then deleted)
    // or lost to the delete and said so; both are legitimate, and neither
    // may leave anything behind.
    let _ = restart.await.expect("the restart task");

    assert!(
        h.client
            .list_sessions()
            .await
            .expect("list")
            .sessions
            .iter()
            .all(|s| s.id != session.id),
        "the delete must win the session's existence outright"
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if marked_pids(&session.id).is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no process carrying this session's marker may outlive the delete"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// PLAN_M3.md item 4's binding contract, and the one this PR could most
/// easily have broken: a restart that FAILS leaves the stop annotation
/// exactly where it was.
///
/// The generation has to be opened before any side effect (item 2's
/// ordering rule), and opening it is what clears the annotation — so the
/// only way both promises hold is for a definitively-failed relaunch to put
/// the previous outcome back. This drives a real failure rather than a
/// simulated one: with the launch directory read-only, the spec write fails
/// and nothing external has happened, which is exactly the class of failure
/// the restore is defined for.
#[tokio::test]
async fn a_failed_restart_restores_the_stop_annotation_it_had_cleared() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    h.client.stop_session(&session.id).await.expect("stop");
    let stopped = listed(&h.client, &session.id).await;
    assert_eq!(stopped.annotation.as_deref(), Some("stopped by user"));

    // The launch directory becomes unwritable, so this restart's spec —
    // its first side effect — cannot land.
    let launch_dir = h.state.path().join("launch");
    let original = std::fs::metadata(&launch_dir)
        .expect("launch dir")
        .permissions();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&launch_dir, std::fs::Permissions::from_mode(0o500))
            .expect("make the launch dir read-only");
    }
    let refused = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await;
    std::fs::set_permissions(&launch_dir, original).expect("restore the launch dir");
    refused.expect_err("a launch spec that cannot be written fails the restart");

    let after = listed(&h.client, &session.id).await;
    assert!(
        matches!(after.status, SessionStatus::Exited { .. }),
        "the previous run's outcome is restored, not left as an unknown launching row: \
         {after:?}"
    );
    assert_eq!(
        after.annotation.as_deref(),
        Some("stopped by user"),
        "only a SUCCESSFUL restart clears the annotation (PLAN_M3.md item 4)"
    );

    // ...and the session is still restartable afterwards, which is what
    // makes the restore a recovery rather than a tidier failure.
    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect("restart once the directory is writable again");
    wait_for_alive_status(&h.client, &session.id, 30).await;
}

/// A relaunch into a directory that no longer resolves where it did at
/// create time is refused, naming both paths (fix-batch item 21).
///
/// The threat is specific: a session's cwd is a path, and a path can be a
/// symlink somebody repoints between launches. Relaunching a permissive
/// agent into a directory an attacker chose is not a decision the user
/// made, and `ensure_cwd_usable`'s existence check cannot see it — the
/// directory is perfectly usable, it is simply a different one.
#[tokio::test]
async fn a_repointed_working_directory_refuses_the_restart() {
    let h = harness().await;
    let real = tempfile::tempdir().expect("real cwd");
    let decoy = tempfile::tempdir().expect("decoy cwd");
    let link = tempfile::tempdir().expect("link parent");
    let link = link.path().join("cwd");
    std::os::unix::fs::symlink(real.path(), &link).expect("symlink");

    let session = h
        .client
        .create_session(
            &link.to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create through the symlink");
    h.client.stop_session(&session.id).await.expect("stop");

    // The repoint.
    std::fs::remove_file(&link).expect("drop the old link");
    std::os::unix::fs::symlink(decoy.path(), &link).expect("repoint");

    let err = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect_err("the session's directory is no longer the one it was created in");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("a precondition failure carries its classification");
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    let canonical_decoy = std::fs::canonicalize(decoy.path()).expect("canonicalize");
    assert!(
        err.message
            .contains(&canonical_decoy.to_string_lossy().into_owned()),
        "the refusal must name where the path leads NOW: {}",
        err.message
    );
    assert_eq!(
        listed(&h.client, &session.id).await.annotation.as_deref(),
        Some("stopped by user"),
        "a refusal this early cannot have touched the session's durable state"
    );
}

/// A relaunch that is not resuming a captured identity opens a FRESH
/// capture window (fix-batch items 5 and 15): the previous run's ambiguity
/// verdict and first-input anchor are per-LAUNCH state, and carrying them
/// forward would deny the new run any capture at all.
///
/// Two fixture sessions in one directory make the first run's correlation
/// ambiguous — the durable refusal SPEC.md's no-wrong-conversation rule
/// depends on. Restarting one of them fresh must then let it capture its
/// OWN conversation on the new run, which is only possible if the verdict
/// and the anchor were both cleared.
#[tokio::test]
async fn a_fresh_relaunch_opens_a_new_capture_window_after_an_ambiguity() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let first = record_session(&h, &fixtures, work.path(), "claude").await;
    let second = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_c1, _r1, _s1, _id1) = provoke_record(&h, &first).await;
    let (_c2, _r2, _s2, _id2) = provoke_record(&h, &second).await;
    settle_past_horizon(&h).await;
    let ambiguous = snapshot_of(&h, &first.id).await;
    assert!(ambiguous.capture_ambiguous, "the setup must be ambiguous");
    assert_eq!(
        ambiguous.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );

    // The rival is stopped first, so the new run's window has the
    // directory to itself — otherwise the ambiguity rule would (correctly)
    // refuse again and this test could not tell a cleared verdict from an
    // inherited one.
    h.client.stop_session(&second.id).await.expect("stop rival");
    h.client
        .restart_session(&first.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("fresh restart");
    wait_for_alive_status(&h.client, &first.id, 30).await;

    let after = snapshot_of(&h, &first.id).await;
    assert!(
        !after.capture_ambiguous,
        "the previous run's verdict describes a run this session no longer has"
    );
    assert_eq!(
        after.first_input_at, None,
        "and its first-input anchor points at a window that closed long ago"
    );

    // The new run captures its own conversation, which an inherited
    // ambiguity would have made impossible forever.
    let (chan, mut rx) = h.client.attach(&first.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    // Waited for by the ECHO of a prompt only this run has seen, and read
    // as the LAST marker: this attachment replays the reused terminal's
    // scrollback, which still holds the pre-restart run's own
    // `RECORD-WRITTEN:` line — waiting on the marker itself would return
    // on the OLD one, before the new run had written anything at all.
    h.client
        .send_input(chan, b"prompt-after-restart\r".to_vec())
        .await;
    // Anchored on the typed line's own echo, and read as the LAST marker:
    // this attachment replays the reused terminal's scrollback, so both an
    // earlier `RECORD-WRITTEN:` and an earlier `echo:` are already in the
    // transcript before the new run has produced anything at all.
    wait_for_after(
        &mut rx,
        &mut seen,
        "prompt-after-restart",
        "RECORD-WRITTEN:",
        20,
    )
    .await;
    let conversation = last_marker_value(&seen, "RECORD-WRITTEN:");
    settle_past_horizon(&h).await;
    let captured = snapshot_of(&h, &first.id).await;
    assert_eq!(
        captured.captured_conversation.as_deref(),
        Some(conversation.as_str()),
        "the fresh window captured the new run's own conversation"
    );
    assert_eq!(captured.restart_offer, farhelm_proto::RestartOffer::Resume);
}

/// Terminal reuse has to survive the case tmux's own respawn cannot carry
/// (fix-batch items 10 and 17): a TUI that dies on the ALTERNATE screen.
///
/// The shrink trick that preserves a primary-screen grid is powerless
/// here — an alternate-screen grid has no history to scroll into, which is
/// what the alternate screen IS — so the frame is captured before the kill
/// and re-emitted by the relaunched process itself
/// (`launch::LaunchSpec::preamble`). Without that, restarting an
/// agent-shaped TUI would silently discard everything the user was looking
/// at, which is exactly what SPEC.md's "the previous run's output stays in
/// scrollback" promises it will not.
///
/// The same restart also rotates the stored alt-screen snapshot: that file
/// describes the run being replaced, and leaving it behind would let the
/// NEXT natural death replay the OLD run's last screen as if it were the
/// new run's.
#[tokio::test]
async fn restarting_an_alt_screen_agent_carries_its_last_frame_into_the_new_run() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen-ignores-term"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let snapshot_path = h.state.path().join("snapshots").join(&session.id);

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "ALT-SCREEN APP", 20).await;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart a live alt-screen agent");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    assert!(
        !snapshot_path.exists(),
        "the previous run's alt-screen snapshot must not outlive the run it describes, or a \
         later natural death would replay it as the NEW run's last screen"
    );

    // A fresh attachment sees the prior frame above the new run — the
    // whole promise, from the side a user actually experiences.
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
        "ALT-SCREEN APP",
        "FAKE-AGENT READY",
        30,
    )
    .await;
}

/// Pane ids are assigned by a server-wide counter that restarts at `%0`
/// with the tmux server, so a remembered `%N` can name a pane belonging to
/// a completely different session — and `respawn-pane` REPLACES the
/// process in whatever it names. Binding the target to the session as well
/// (`=<session>:.<pane>`) is what makes that unconstructible.
///
/// This drives the reuse path itself rather than the pairing in isolation:
/// two sessions whose pane ids come from the same counter, one restarted,
/// and the other's agent must be entirely undisturbed.
#[tokio::test]
async fn a_restart_respawns_only_its_own_pane() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let (restarted, _work_a) = basic_session(&h).await;
    let (bystander, _work_b) = basic_session(&h).await;

    let (chan, mut rx) = h
        .client
        .attach(&bystander.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client
        .send_input(chan, b"BYSTANDER-MARKER\r".to_vec())
        .await;
    wait_for(&mut rx, &mut seen, "BYSTANDER-MARKER", 10).await;
    let bystander_pane = pane_id_of(&sock, &format!("fh-{}", bystander.id)).await;

    h.client
        .restart_session(&restarted.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart");
    wait_for_alive_status(&h.client, &restarted.id, 30).await;

    assert_eq!(
        listed(&h.client, &bystander.id).await.status,
        SessionStatus::Alive,
        "the bystander's agent must be untouched by another session's respawn"
    );
    assert_eq!(
        pane_id_of(&sock, &format!("fh-{}", bystander.id)).await,
        bystander_pane,
        "and it must still be the same pane"
    );
    // Its terminal content is intact too — a respawn would have cleared
    // the visible grid even where it left the pane in place.
    let content = pane_capture(&sock, &format!("fh-{}", bystander.id)).await;
    assert!(
        content.contains("BYSTANDER-MARKER"),
        "the bystander's own output must survive: {content}"
    );
}

/// The reason sentinel paths are generation-scoped at all
/// (`spec_path_for_launch`/`status_path_for_spec`, both keyed on the
/// launch's generation number): a stale sentinel from an EARLIER
/// generation must never be able to paint a LATER, unrelated launch as
/// `error`, even if something failed to clean it up.
///
/// `a_restart_clears_a_previous_launch_error` already pins that a real
/// failed launch's own sentinel is deleted on a successful restart — this
/// test pins the complementary, previously-untested half: a gen-0
/// sentinel that SURVIVES (planted directly, standing in for whatever
/// cleanup bug might one day leave one behind) still cannot describe
/// gen-1, because nothing ever looks a generation-0 path up on behalf of
/// a generation-1 session. The session here never actually failed to
/// launch; the sentinel is a pure fabrication written straight to the
/// gen-0 path AFTER the restart has already moved the session to
/// generation 1, so real cleanup has nothing left to race against.
///
/// The sentinel gate (`sentinel_could_still_apply` combined with
/// `dead_or_absent` in `service.rs`) is only ever consulted for a pane
/// that is dead or gone — a live gen-1 pane never reaches it regardless of
/// what the gate would have said, which would make an assertion against a
/// still-alive gen-1 vacuous. So generation 1's agent is SIGKILLed WITHOUT
/// an annotation (its own pane pid, not `stop_session` — an annotated
/// exit is never sentinel-superseded per `sentinel_could_still_apply`'s
/// own docs, which would make the assertion vacuous the other way) before
/// either read: this is the exact state — an unannotated dead pane on the
/// current generation — that a wrongly-scoped sentinel lookup would flip
/// from `Exited` to `Error`.
///
/// Checked twice: once against the live supervisor's `ListSessions`, and
/// once again after a full supervisor handoff to a fresh process that
/// actually owns the state directory (`handoff_to_new_supervisor`; a
/// second supervisor started while the first still holds the directory
/// would come up read-only and reconcile nothing, per
/// `Supervisor::owns_state_dir`'s own docs), which is `reload_sessions`'s
/// unconditional sentinel check — the stronger of the two reads and the
/// one most likely to re-surface a generation mismatch if the scoping
/// were ever accidentally loosened to "the session's latest sentinel"
/// instead of "this generation's".
#[tokio::test]
async fn a_stale_generation_zero_sentinel_cannot_taint_generation_one() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    assert_eq!(
        listed(&h.client, &session.id).await.status,
        SessionStatus::Alive,
        "the session must start out genuinely healthy"
    );

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart onto generation 1");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    // Planted AFTER the restart above, so the restart's own cleanup of
    // generation 0's real launch files (which never failed) cannot
    // interfere with this fabricated one.
    let stale_sentinel =
        status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    std::fs::write(&stale_sentinel, "exec_failed argv0=/nope errno=2")
        .expect("plant a stale generation-0 sentinel");

    // `Alive` only means the pane hasn't died yet, not that the shim's own
    // `exec` chain has reached the real agent (`wait_for_alive_status`'s
    // own docs) — killing on that signal alone would race the shim itself
    // and reproduce the WRAPPER-failure shape
    // (`a_failed_scope_wrapper_classifies_as_error_rather_than_a_plain_exit`),
    // not the one under test here. The shim unlinks its own spec the
    // moment it has read it, strictly before exec'ing the real agent
    // (`exec_launch_spec_with_seam`'s docs), so generation 1's spec file
    // going away is the earliest reliable proof that the shim has handed
    // off and the real fake agent — not its wrapper — now owns the pane.
    // (An attach-and-wait-for-the-ready-banner alternative was tried and
    // rejected: this pane's tmux scrollback can still hold generation 0's
    // OWN ready banner from before the restart, so a naive text search
    // matches instantly against stale output rather than generation 1's.)
    let gen1_spec = spec_path_for_launch(h.state.path(), &session.id, 1);
    let shim_handoff_deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while gen1_spec.exists() {
        assert!(
            tokio::time::Instant::now() < shim_handoff_deadline,
            "generation 1's shim never consumed its own spec"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Kill generation 1's pane's OWN process directly, bypassing
    // `stop_session`, so the pane goes dead (tmux keeps a dead pane around
    // rather than tearing down the session) with no annotation — the only
    // shape that would ever even attempt a sentinel read. `kill-session`
    // would destroy the tmux session outright instead of leaving a dead
    // pane behind, which is not the state under test here.
    let sock = h.state.path().join("tmux.sock");
    let tmux_name = format!("fh-{}", session.id);
    let pid_out = tmux_query(
        &sock,
        &["display-message", "-p", "-t", &tmux_name, "#{pane_pid}"],
    )
    .await;
    let pane_pid: u32 = String::from_utf8_lossy(&pid_out.stdout)
        .trim()
        .parse()
        .expect("a live pane must report a pid");
    // SAFETY: a real, currently-live pid this test just read from tmux.
    unsafe {
        libc::kill(pane_pid as libc::pid_t, libc::SIGKILL);
    }
    wait_for_dead_pane(&sock, &tmux_name).await;

    let live_read = listed(&h.client, &session.id).await;
    assert!(
        matches!(live_read.status, SessionStatus::Exited { .. }),
        "an unannotated dead generation-1 pane must classify as Exited, not be swallowed by \
         the gen-0 sentinel: {live_read:?}"
    );

    // Hand off to a replacement supervisor that actually owns the state
    // directory, so `reload_sessions`'s unconditional sentinel check runs
    // for real rather than against a read-only reconciler.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    let sup2 = handoff_to_new_supervisor(state.path(), sup, client).await;
    let client2 = connect_client(&sup2).await;
    let reloaded = listed(&client2, &session.id).await;
    assert!(
        matches!(reloaded.status, SessionStatus::Exited { .. }),
        "the stale generation-0 sentinel must still not taint generation 1 after a real, \
         owning reload: {reloaded:?}"
    );

    // `cleanup_launch_artifacts` only ever removes a launch's OWN files
    // once ITS OWN generation is classified `Error` (`service.rs`) —
    // generation 1 here is never classified `Error` and has no sentinel of
    // its own, so nothing in this path has any reason to touch generation
    // 0's leftover file. Confirmed empirically (not merely assumed) before
    // pinning: the real reload's cleanup does NOT sweep other generations'
    // files, so the plant survives untouched.
    assert!(
        stale_sentinel.exists(),
        "an unconsulted sentinel for the wrong generation is left untouched, not swept, by a \
         reload that never classified that generation as Error"
    );
}

/// PLAN_M3.md item 4 / acceptance 5's exact composition: stop, then
/// restart, then let the new run end on its own. The stop records
/// "stopped by user" on generation 0; the restart must clear that
/// annotation with the new generation (already pinned elsewhere); what is
/// untested until this is the THIRD leg — once generation 1 exits
/// NATURALLY, nothing must re-attach generation 0's annotation to it. The
/// real risk this guards against is not annotation storage (annotations
/// are intentionally kept on the session row and cleared whenever a new
/// generation opens) but a STALE-GENERATION OBSERVATION: generation 0's
/// exit or annotation being reported late and restored onto generation 1
/// despite the generation fence that is supposed to keep them apart. A bug
/// that let that happen would pass every other test here and only show up
/// in exactly this sequence.
///
/// The same invocation is used for both generations — `RestartMode::Fresh`
/// replays the session's original argv verbatim, so there is no
/// per-restart override to give the second run a different command. A
/// fixed sleep duration would race under load (generation 0 could exit
/// naturally before the stop lands, or generation 1 could exit before
/// `wait_for_alive_status` observes it), so both generations instead loop
/// on a marker FILE the test controls: `until [ -e released ]; do sleep
/// 0.2; done`, resolved against the session's own working directory.
/// Generation 0 is stopped while the marker provably does not exist yet,
/// so it cannot have exited on its own; generation 1 is left looping until
/// the test creates the marker, at which point it exits 0 naturally.
#[tokio::test]
async fn stop_then_restart_then_natural_exit_carries_no_stale_annotation() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let marker = work.path().join("released");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "sh -c 'until [ -e released ]; do sleep 0.2; done'",
            None,
            80,
            24,
        )
        .await
        .expect("create");
    wait_for_alive_status(&h.client, &session.id, 30).await;

    h.client
        .stop_session(&session.id)
        .await
        .expect("stop the running agent");
    let stopped = wait_for_non_alive_status(&h.client, &session.id, 30).await;
    assert!(
        matches!(stopped.status, SessionStatus::Exited { .. }),
        "the stop must end generation 0: {stopped:?}"
    );
    assert_eq!(
        stopped.annotation.as_deref(),
        Some("stopped by user"),
        "the stop's annotation must be recorded where it happens, exactly like the other \
         stop-annotation tests"
    );
    assert!(
        !marker.exists(),
        "test setup: generation 0 must still be looping, not having exited on its own, when \
         it was stopped"
    );

    // The session already exited, so no live agent needs consent to stop
    // it first.
    let restarted = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, false)
        .await
        .expect("restart the already-exited session");
    assert_eq!(
        restarted.annotation, None,
        "a fresh generation must never inherit the previous generation's annotation"
    );
    wait_for_alive_status(&h.client, &session.id, 30).await;

    // Release generation 1's loop deliberately, rather than leaving it to
    // a timer — this is the moment, and only this moment, at which it may
    // exit on its own.
    std::fs::write(&marker, "").expect("release generation 1's loop");
    let exited = wait_for_exit_code(&h.client, &session.id, 0, 30).await;
    assert_eq!(
        exited.annotation, None,
        "a natural exit must carry no annotation at all, stale or otherwise — only a stop \
         records one"
    );
}
