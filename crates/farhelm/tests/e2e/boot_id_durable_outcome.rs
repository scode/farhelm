//! Boot-id classification, the durable last-known outcome, and the
//! durable stop annotation across simulated reboots.

use crate::harness::*;

// ---------------------------------------------------------------------
// PLAN_M3.md item 2/4: boot-id classification, the durable last-known
// outcome, and the durable stop annotation.
//
// "Simulated reboot" throughout means two things together, because a real
// reboot does both: the boot id the supervisor reads changes (injected
// through `SupervisorSeams`), and the private tmux server is gone. Tests
// that changed only the boot id would leave live panes behind for the
// reload to find, which is not a reboot at all.
// ---------------------------------------------------------------------

/// A create-lifecycle seam that fails at exactly one stage and lets every
/// other one through (PLAN_M3.md items 2 and 6).
///
/// One stage at a time is the point: each of `CreateStage`'s boundaries
/// leaves durable state in a different shape, and a test that crashed at
/// several of them at once could not tell which shape its assertions were
/// actually about.
pub(crate) fn crash_at(stage: CreateStage) -> CreateCrashSeam {
    Arc::new(move |reached| {
        if reached == stage {
            anyhow::bail!("simulated crash at {stage:?}");
        }
        Ok(())
    })
}

/// A supervisor on an existing state dir whose boot-id source answers
/// `boot` — the stand-in for a machine that has (or has not) rebooted
/// since the last supervisor ran.
///
/// The three answers are deliberately distinct, because M3 treats them
/// differently: `Ok(Some(id))` is a positive identification, `Ok(None)` is
/// a host that publishes no boot id at all (permanently evidence-free),
/// and `Err` is a host that HAS one this read could not get — which must
/// not be allowed to produce the irreversible answers a successful read
/// would.
pub(crate) async fn supervisor_reading_boot(
    state: &std::path::Path,
    boot: Result<Option<&str>, &str>,
) -> Arc<Supervisor> {
    let boot = boot
        .map(|id| id.map(str::to_string))
        .map_err(str::to_string);
    Supervisor::new_with_seams(
        state,
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            boot_id: Arc::new(move || match &boot {
                Ok(id) => Ok(id.clone()),
                Err(e) => Err(anyhow::anyhow!("{e}")),
            }),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor construction on an existing state dir")
}

/// The common case of [`supervisor_reading_boot`]: a host that reports
/// exactly this boot id.
pub(crate) async fn supervisor_believing_boot(
    state: &std::path::Path,
    boot: Option<&str>,
) -> Arc<Supervisor> {
    supervisor_reading_boot(state, Ok(boot)).await
}

/// Like [`harness`], but the supervisor reads `boot` as the host's boot
/// id, so a later supervisor can be told a different one without the test
/// depending on whether this host publishes a real boot id at all.
pub(crate) async fn harness_believing_boot(boot: &str) -> Harness {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("tempdir");
    let sup = supervisor_believing_boot(state.path(), Some(boot)).await;
    let guard = TmuxServerGuard(state.path().join("tmux.sock"));
    let client = connect_client(&sup).await;
    Harness {
        client,
        sup,
        _tmux: guard,
        state,
        _slot: slot,
    }
}

/// Wait until tmux itself reports `tmux_name`'s pane dead.
///
/// Deliberately asks tmux directly instead of polling `list_sessions`: the
/// same-boot test needs the agent to have exited WITHOUT the supervisor
/// ever observing it (that is what "with the supervisor down" means for an
/// in-process supervisor), and a list is exactly such an observation.
pub(crate) async fn wait_for_dead_pane(sock: &std::path::Path, tmux_name: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let out = tmux_query(
            sock,
            &["display-message", "-p", "-t", tmux_name, "#{pane_dead}"],
        )
        .await;
        if String::from_utf8_lossy(&out.stdout).trim() == "1" {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "pane of {tmux_name} never died"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// One session out of a listing, by id.
pub(crate) async fn listed(client: &SupervisorClient, id: &str) -> SessionInfo {
    client
        .list_sessions()
        .await
        .expect("list")
        .sessions
        .into_iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("session {id} must still be listed"))
}

/// M3 acceptance 2: on the SAME boot, classification is per session and
/// stays M2's live probing — nothing is interrupted, because nothing
/// happened to the host.
///
/// Three sessions, three different fates while no supervisor is watching:
/// one agent exits on its own (its pane survives holding the code), one
/// has its tmux session killed outright (nothing survives to ask), and one
/// is simply left alone. The reloading supervisor must report exactly
/// those three answers — including the true exit code from the surviving
/// dead pane, which is the "retained knowledge is not a guess" half of the
/// contract, and exited-UNKNOWN where nothing retained anything.
///
/// The exits deliberately happen without any intervening `list_sessions`:
/// a list is how this supervisor witnesses an exit, so listing first would
/// test the recording path rather than the reload path this test is about.
#[tokio::test]
async fn same_boot_classification_is_per_session_and_never_interrupted() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let work = farhelm_teststate::tempdir().expect("workdir");

    let exiting = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "sh -c 'sleep 0.5; exit 3'",
            None,
            80,
            24,
        )
        .await
        .expect("create the self-exiting session");
    let (killed, _killed_work) = basic_session(&h).await;
    let (untouched, _untouched_work) = basic_session(&h).await;

    wait_for_dead_pane(&sock, &format!("fh-{}", exiting.id)).await;
    let out = tmux_query(
        &sock,
        &["kill-session", "-t", &format!("=fh-{}", killed.id)],
    )
    .await;
    assert!(
        out.status.success(),
        "test setup: killing one session's tmux session must succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor on the same boot");
    let client2 = connect_client(&sup2).await;

    // Asserted against what the dead pane ACTUALLY retains, not against a
    // version gate: on a tmux that records exit statuses reliably that is
    // `3`, and on one that loses them under load it is empty — either way
    // the supervisor must report exactly what tmux still holds and never
    // more (`tmux_records_exit_codes_reliably` documents the 3.4 behavior
    // this tolerates). A supervisor that fabricated a code, or dropped one
    // tmux still had, fails this on every host.
    let retained = tmux_query(
        &sock,
        &[
            "display-message",
            "-p",
            "-t",
            &format!("fh-{}", exiting.id),
            "#{pane_dead_status}",
        ],
    )
    .await;
    let retained: Option<i32> = String::from_utf8_lossy(&retained.stdout)
        .trim()
        .parse()
        .ok();
    if tmux_records_exit_codes_reliably() {
        assert_eq!(
            retained,
            Some(3),
            "a tmux this test trusts for codes must have kept this one"
        );
    }
    let exited = listed(&client2, &exiting.id).await;
    assert_eq!(
        exited.status,
        SessionStatus::Exited {
            exit_code: retained
        },
        "the supervisor must report exactly the code the surviving dead pane retains — \
         retained knowledge, never a guess and never a loss"
    );
    assert_eq!(
        listed(&client2, &killed.id).await.status,
        SessionStatus::Exited { exit_code: None },
        "nothing survived this session to hold a code, and none may be invented"
    );
    // An untouched session continues live across a supervisor restart.
    // Waited for rather than read once: a single list can degrade to an
    // empty pane map on a tolerated tmux diagnostic and report a live
    // session exited (see `wait_for_listing`).
    wait_for_live_status(&client2, &untouched.id, 30).await;
}

/// M3 acceptance 3 and 5: after a reboot, sessions that were live become
/// **interrupted** — an explicit lost-track state — while sessions that
/// had already ended keep their status, their codes, and their stop
/// annotations. Interrupted then persists: opening it (attach) fails
/// without changing anything, and further supervisor restarts on the same
/// boot leave it exactly as it was.
///
/// The stop annotation riding through this is the durable half of
/// PLAN_M3.md item 4: it was written when the user stopped the session,
/// and the tmux pane that stop happened in no longer exists by the time it
/// is read back here — so this proves the annotation comes from the
/// supervisor's own durable record and nowhere else.
#[tokio::test]
async fn a_reboot_interrupts_live_sessions_and_preserves_ended_ones() {
    let h = harness_believing_boot("boot-a").await;
    let (live, _live_work) = basic_session(&h).await;
    let (stopped, _stopped_work) = basic_session(&h).await;
    // A session that ends on its own AND is listed before the reboot: listing
    // is where its exit code is witnessed, so what survives below is the
    // supervisor's durable recording rather than anything recovered from the
    // pane after it is gone.
    let work = farhelm_teststate::tempdir().expect("workdir");
    let ended = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "sh -c 'sleep 0.5; exit 3'",
            None,
            80,
            24,
        )
        .await
        .expect("create the self-exiting session");
    let ended_settled = wait_for_exit_code(&h.client, &ended.id, 3, 30).await;

    h.client
        .stop_session(&stopped.id)
        .await
        .expect("stop the session the user ends deliberately");
    let before = listed(&h.client, &stopped.id).await;
    assert_eq!(
        before.annotation.as_deref(),
        Some("stopped by user"),
        "a user-initiated stop annotates the session immediately, not only after a restart"
    );

    // A plain supervisor restart first — same boot, tmux still up, so the
    // stopped session's pane is still there to be probed. This is the path
    // where a live probe and the durable record BOTH have something to
    // say, and the annotation has to come from the record even though the
    // status comes from tmux.
    let sup_restarted = supervisor_believing_boot(h.state.path(), Some("boot-a")).await;
    let client_restarted = connect_client(&sup_restarted).await;
    assert_eq!(
        listed(&client_restarted, &stopped.id)
            .await
            .annotation
            .as_deref(),
        Some("stopped by user"),
        "the annotation survives a supervisor restart, not merely a reboot"
    );
    // No reboot happened yet, so the live session is untouched. Waited for
    // rather than read once, for the reason `wait_for_listing` documents.
    wait_for_live_status(&client_restarted, &live.id, 30).await;

    // Older tmux versions can first report an exited pane without its code,
    // then enrich that durable outcome on a later list. Accept only that
    // monotonic refinement before using the final observation as the snapshot
    // the reboot must preserve; a broader allowance would hide corruption by
    // the same-boot supervisor restart above.
    let ended_before_reboot = listed(&client_restarted, &ended.id).await.status;
    assert!(
        ended_before_reboot == ended_settled.status
            || matches!(
                (&ended_settled.status, &ended_before_reboot),
                (
                    SessionStatus::Exited { exit_code: None },
                    SessionStatus::Exited { exit_code: Some(3) }
                )
            ),
        "an ended session may gain its known exit code before reboot, but must not otherwise \
         change across a same-boot supervisor restart (settled: {:?}, final: \
         {ended_before_reboot:?})",
        ended_settled.status
    );

    // The reboot: tmux dies with the host, and the next supervisor reads a
    // different boot id.
    kill_tmux_server_and_wait(&h.state.path().join("tmux.sock")).await;
    let sup2 = supervisor_believing_boot(h.state.path(), Some("boot-b")).await;
    let client2 = connect_client(&sup2).await;

    assert_eq!(
        listed(&client2, &live.id).await.status,
        SessionStatus::Interrupted,
        "a session that was running when the host rebooted lost its terminal to that \
         reboot; that is knowable, unlike how (or whether) its agent ended"
    );
    let after = listed(&client2, &stopped.id).await;
    assert!(
        matches!(after.status, SessionStatus::Exited { .. }),
        "an already-ended session keeps its status across a reboot: {after:?}"
    );
    assert_eq!(
        after.annotation.as_deref(),
        Some("stopped by user"),
        "the stop annotation is durable session metadata (SPEC.md), so it survives the \
         terminal it was recorded against"
    );

    let ended_after = listed(&client2, &ended.id).await;
    assert_eq!(
        ended_after.status, ended_before_reboot,
        "an exit keeps the final status observed before the reboot, even though the pane that \
         held it is gone"
    );
    assert_eq!(
        ended_after.annotation, None,
        "an agent that ended on its own is never credited to the user"
    );

    // Attaching to an interrupted session fails, and — the part that
    // matters — leaves the classification exactly as it was: nothing
    // relaunches, and nothing gets downgraded to exited-unknown by the
    // attempt. This is SPEC.md's "opening and declining changes nothing"
    // as far as this PR can go; the resume OFFER itself is item 9.
    // This is intentionally the raw form: the refusal is the premise, so
    // there can be no stream or replay boundary for the ready helper to own.
    let refusal = client2
        .attach_at_boundary(&live.id, 80, 24)
        .await
        .expect_err("an interrupted session has no terminal to attach to")
        .to_string();
    // The refusal is what a browser paints when it opens the session, so
    // its wording is part of the contract: it names the reboot and the way
    // forward, and never claims to know that the agent ended BEFORE the
    // restart — the ordering SPEC.md's "interrupted" exists to leave open.
    // The way forward is worded per the session's restart offer, and this
    // plain-shell session has no captured conversation, so what it is
    // promised is a fresh launch — never a resume it would not get.
    assert!(
        refusal.contains("host rebooted") && refusal.contains("restart launches a fresh agent"),
        "the refusal must name the reboot and the fresh-launch offer: {refusal}"
    );
    assert!(
        !refusal.contains("resume"),
        "a session with no captured conversation must not be promised a resume: {refusal}"
    );
    assert!(
        !refusal.contains("after the agent ended"),
        "the refusal must not order the agent's end against the restart: {refusal}"
    );
    assert_eq!(
        listed(&client2, &live.id).await.status,
        SessionStatus::Interrupted,
        "declining the offer changes nothing"
    );

    // A further restart on the SAME boot must not reclassify either.
    let sup3 = supervisor_believing_boot(h.state.path(), Some("boot-b")).await;
    let client3 = connect_client(&sup3).await;
    assert_eq!(
        listed(&client3, &live.id).await.status,
        SessionStatus::Interrupted,
        "interrupted is a durable outcome, not a per-startup inference"
    );
    assert_eq!(
        listed(&client3, &stopped.id).await.annotation.as_deref(),
        Some("stopped by user")
    );
}

/// M3 acceptance 3's pre-M3 clause: a database with no stored boot id
/// (every database written before this milestone) must NOT be read as a
/// reboot on its first M3 startup. There is no evidence either way, and
/// the no-guessing rule cuts both ways — so the same-boot path runs and a
/// still-live session keeps listing as alive.
///
/// Modelled by a first supervisor that reads no boot id at all, which
/// stores nothing, leaving the database in exactly the state a pre-M3 one
/// is in. The second half proves the id really is adopted from then on:
/// once a boot id HAS been stored, a later differing one does interrupt.
#[tokio::test]
async fn a_database_without_a_stored_boot_id_does_not_claim_a_reboot() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let sup1 = supervisor_believing_boot(state.path(), None).await;
    let client1 = connect_client(&sup1).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = client1
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    // Hand-built rather than through `basic_session`, because this
    // test owns two supervisors over one state dir; the barrier is the
    // same one for the same reason — the live-status check below must be
    // about an agent that execed, not a login shell holding a pane.
    wait_for_agent_ready(&state.path().join("tmux.sock"), &session.id).await;

    let sup2 = supervisor_believing_boot(state.path(), Some("boot-b")).await;
    let client2 = connect_client(&sup2).await;
    // With nothing stored to compare against, a differing boot id is not
    // evidence of a reboot — and the live tmux session proves the point
    // independently. Waited for rather than read once, for the reason
    // `wait_for_listing` documents.
    wait_for_live_status(&client2, &session.id, 30).await;

    // `boot-b` is stored now, so a THIRD boot id is a real reboot.
    kill_tmux_server_and_wait(&state.path().join("tmux.sock")).await;
    let sup3 = supervisor_believing_boot(state.path(), Some("boot-c")).await;
    let client3 = connect_client(&sup3).await;
    assert_eq!(
        listed(&client3, &session.id).await.status,
        SessionStatus::Interrupted,
        "once a boot id has been adopted, a change in it is the reboot evidence this \
         classification runs on"
    );
    drop(slot);
}

/// PLAN_M3.md item 2's ordering rule, pinned at the boundary it exists
/// for: the durable **launching** record must be committed BEFORE any
/// external side effect of the launch.
///
/// A crash injected immediately after that commit (the create seam skips
/// every cleanup path a graceful failure would run — a real crash gets no
/// cleanup either) must leave evidence that a launch was attempted. Under
/// M2's ordering there would be nothing at all: the row was written only
/// after tmux had the session, so this crash would have left silence.
///
/// What the next startup does with that evidence is the second half, and
/// it is deliberately NOT "exited": SPEC.md's exited means the agent RAN,
/// and a launch whose side effects were never found has not established
/// that. The row stays pending and lists as **unknown** — the honest
/// not-yet-classified answer — until PLAN_M3.md item 3's sentinel can call
/// it an error or item 6's reservation can retry it. Never alive (nothing
/// is running), never interrupted (no reboot happened), and never exited
/// (nothing was observed to run).
#[tokio::test]
async fn a_crash_after_the_launching_record_leaves_evidence_and_stays_pending() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let sup1 = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            create_crash: Some(crash_at(CreateStage::AfterRecord)),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let client1 = connect_client(&sup1).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    client1
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect_err("the injected crash must fail the create");

    // Nothing external happened: no tmux session was ever created.
    let sessions = tmux_query(&state.path().join("tmux.sock"), &["list-sessions"]).await;
    assert!(
        !String::from_utf8_lossy(&sessions.stdout).contains("fh-"),
        "the crash landed before the tmux side effect, so no session may exist: {}",
        String::from_utf8_lossy(&sessions.stdout)
    );

    let sup2 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("the next startup");
    let client2 = connect_client(&sup2).await;
    let listing = client2.list_sessions().await.expect("list");
    assert_eq!(
        listing.sessions.len(),
        1,
        "the launching record is the evidence the crash left behind: {:?}",
        listing.sessions
    );
    assert_eq!(
        listing.sessions[0].status,
        SessionStatus::Unknown,
        "a launch whose side effects were never found has not been shown to have run, so \
         it stays pending rather than claiming an exit"
    );

    // And it stays pending across further restarts — nothing degrades it
    // into a fabricated exit later.
    let sup3 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("a further startup");
    let client3 = connect_client(&sup3).await;
    assert_eq!(
        client3.list_sessions().await.expect("list").sessions[0].status,
        SessionStatus::Unknown
    );
    drop(slot);
}

/// PLAN_M3.md item 4 end to end: a stop's annotation is written where the
/// stop happens and read back by a supervisor that never saw it.
///
/// The two sessions are the contrast that gives the assertion meaning: one
/// ends because the user stopped it, the other because its tmux session
/// was killed out from under it (a stand-in for any ending the user had
/// nothing to do with). Both come back exited from the fresh supervisor —
/// only one of them says who did it. Without that contrast, an annotation
/// applied to every ended session would pass just as well.
///
/// The reconciliation of a stop INTERRUPTED mid-sweep is a different edge
/// and is pinned where it can be provoked deterministically, against the
/// reload itself (`service.rs`'s
/// `reload_reconciles_a_stop_intent_against_the_pane_it_left_behind`).
#[tokio::test]
async fn a_stop_annotation_is_written_where_it_happens_and_read_back_elsewhere() {
    let h = harness().await;
    let sock = h.state.path().join("tmux.sock");
    let (stopped, _stopped_work) = basic_session(&h).await;
    let (killed, _killed_work) = basic_session(&h).await;

    h.client.stop_session(&stopped.id).await.expect("stop");
    let out = tmux_query(
        &sock,
        &["kill-session", "-t", &format!("=fh-{}", killed.id)],
    )
    .await;
    assert!(out.status.success(), "test setup: kill the other session");

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("the next startup");
    let client2 = connect_client(&sup2).await;

    let after = listed(&client2, &stopped.id).await;
    assert!(
        matches!(after.status, SessionStatus::Exited { .. }),
        "the stopped session ended: {after:?}"
    );
    assert_eq!(
        after.annotation.as_deref(),
        Some("stopped by user"),
        "the annotation is durable session metadata, not something the stopping process \
         merely held in memory"
    );

    let other = listed(&client2, &killed.id).await;
    assert!(
        matches!(other.status, SessionStatus::Exited { .. }),
        "the other session ended too: {other:?}"
    );
    assert_eq!(
        other.annotation, None,
        "an ending the user did not cause must never be credited to them"
    );
}

/// The list-versus-stop race, driven through the REAL handlers rather than
/// the store: a client polling `ListSessions` while a `StopSession` runs
/// must not end up with a session that lists as a plain exit.
///
/// This is the concrete loss seven review lenses converged on. The window
/// is not exotic — `kill_process_tree` spends seconds on SIGTERM, a grace
/// period, re-enumeration and SIGKILL, tmux marks the pane dead the
/// instant the process actually dies, and the UI lists every couple of
/// seconds — so a list observing that death mid-sweep is the ORDINARY
/// case, not a corner one. The poll loop below runs as fast as it can for
/// exactly that reason: it is trying to be the observer that gets there
/// first.
///
/// The setup waits for the agent to have actually STARTED
/// (`basic_session`) rather than only for `create` to have replied,
/// and that is not a timing tweak: the subject of this test is a stop of a
/// RUNNING agent, and a launch that died before the exec shim classifies
/// as `Error` — the assertion below then reports the annotation as lost
/// when the session never ran at all (observed 2026-08-18 under
/// full-suite load; see `wait_for_agent_ready`). The barrier turns that
/// upstream failure into a setup failure with its own name.
#[tokio::test]
async fn a_list_polling_through_a_stop_never_erases_the_annotation() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let poller = h.second_client().await;
    let id = session.id.clone();
    let polling = tokio::spawn(async move {
        for _ in 0..200 {
            if poller.list_sessions().await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    h.client.stop_session(&session.id).await.expect("stop");
    polling.await.expect("the poller must not panic");

    let stopped = listed(&h.client, &id).await;
    assert!(
        matches!(stopped.status, SessionStatus::Exited { .. }),
        "the stop ended the session: {stopped:?}"
    );
    assert_eq!(
        stopped.annotation.as_deref(),
        Some("stopped by user"),
        "no amount of concurrent listing may erase who ended this session"
    );

    // And it is DURABLE, not just what this supervisor happens to hold.
    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("the next startup");
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        listed(&client2, &id).await.annotation.as_deref(),
        Some("stopped by user")
    );
}

/// Stopping a session whose agent had ALREADY exited on its own must
/// record a plain exit, never the stop annotation: the user pressed stop,
/// but they did not end this run, and SPEC.md's annotation says who did.
///
/// Read back through a fresh supervisor because the claim is about the
/// durable record, not about what one process happens to be holding.
#[tokio::test]
async fn stopping_an_already_exited_session_records_no_annotation() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            "sh -c 'sleep 0.5; exit 5'",
            None,
            80,
            24,
        )
        .await
        .expect("create");
    wait_for_dead_pane(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;

    h.client
        .stop_session(&session.id)
        .await
        .expect("stopping an already-dead session still succeeds");

    let sup2 = Supervisor::new_with_exe(h.state.path(), farhelm_bin().into())
        .await
        .expect("the next startup");
    let client2 = connect_client(&sup2).await;
    let after = listed(&client2, &session.id).await;
    assert!(
        matches!(after.status, SessionStatus::Exited { .. }),
        "the agent had already ended: {after:?}"
    );
    assert_eq!(
        after.annotation, None,
        "the user did not end this run, so nothing may say they did"
    );
}

/// PLAN_M3.md item 2's read-failure clause: a boot id that cannot be READ
/// is not the same as a host that HAS none, and treating it as such would
/// let a transient `/proc` failure produce an irreversible answer — every
/// still-live session durably recorded as exited, on evidence that never
/// arrived.
///
/// The sequence pins both halves: the failed read must neither clear nor
/// replace the stored id (so nothing is reclassified on it), and the
/// LATER successful read of a different id must still see the original
/// stored value and interrupt exactly as it would have without the
/// failure in between.
#[tokio::test]
async fn an_unreadable_boot_id_defers_rather_than_deciding() {
    let h = harness_believing_boot("boot-a").await;
    let (session, _work) = basic_session(&h).await;

    // The reboot happens; the next supervisor cannot read the boot id.
    kill_tmux_server_and_wait(&h.state.path().join("tmux.sock")).await;
    let degraded = supervisor_reading_boot(h.state.path(), Err("/proc is unavailable")).await;
    let degraded_client = connect_client(&degraded).await;
    assert_eq!(
        listed(&degraded_client, &session.id).await.status,
        SessionStatus::Exited { exit_code: None },
        "with no boot id to compare, this pass can only report what it can see — and it \
         must not durably decide anything on that"
    );

    // The read succeeds on the next startup: the stored id is still
    // `boot-a`, so the reboot IS detected, and the session that the
    // degraded pass could have written off as a plain exit is correctly
    // interrupted instead.
    let recovered = supervisor_believing_boot(h.state.path(), Some("boot-b")).await;
    let recovered_client = connect_client(&recovered).await;
    assert_eq!(
        listed(&recovered_client, &session.id).await.status,
        SessionStatus::Interrupted,
        "the deferred classification must still be reachable once the read works"
    );
}
