//! Tab lifecycle edge cases: restart-gap conflicts and other boundary
//! conditions a tab's open/close path must resolve consistently.

use crate::harness::*;

use crate::terminal_backpressure::drain_for;
use crate::terminal_tabs::{
    listed_tabs, run_in_shell, tab_pane, wait_for_shell, window_rows, write_daemon_script,
};

// ---------------------------------------------------------------------------
// Tab lifecycle edges
// ---------------------------------------------------------------------------

/// Opening a tab on a RESTART-GAP session — one whose row survived a
/// supervisor restart but whose tmux did not — is the restart-first
/// conflict, and it must not build a tmux session as a side effect.
///
/// Distinct from the killed-tmux-session test: there the entry still
/// holds a terminal and tmux disagrees; here the entry has no terminal at
/// all, which is the branch that answers before tmux is consulted. Both
/// must produce the same advice, because they are the same fact for a
/// user.
#[tokio::test]
async fn opening_a_tab_on_a_restart_gap_session_is_a_restart_first_conflict() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let work = tempfile::tempdir().expect("workdir");
    let guard = TmuxServerGuard(state.path().join("tmux.sock"));

    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let client = connect_client(&sup).await;
    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    // Kill the tmux server and reload: the row comes back terminal-less,
    // which is the restart gap PLAN_M2.md names.
    drop(client);
    drop(sup);
    kill_tmux_server_and_wait(&state.path().join("tmux.sock")).await;
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor");
    let client = connect_client(&sup).await;

    let err = client
        .open_tab(&session.id)
        .await
        .expect_err("a terminal-less session cannot gain a tab");
    assert!(
        format!("{err:#}").contains("restart"),
        "the refusal must point at restarting the session, got: {err:#}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a refused open must carry a SupervisorError")
            .kind,
        ErrorKind::Conflict
    );
    let sessions = tmux_query(&state.path().join("tmux.sock"), &["list-sessions"]).await;
    assert!(
        !sessions.status.success()
            || !String::from_utf8_lossy(&sessions.stdout).contains(&format!("fh-{}", session.id)),
        "a refused open must not have built a tmux session as a side effect"
    );
    drop(guard);
    drop(slot);
}

/// A tab whose shell EXITED on its own is not a closed tab: it stays
/// listed, stays attachable with its scrollback, and still closes
/// cleanly.
///
/// SPEC.md gives an established tab the same `remain-on-exit` contract the
/// agent terminal has — a dead pane is viewable, not gone — and the
/// dead-at-OPEN refusal is deliberately a different thing. This is the
/// test that keeps the two from being conflated into "a dead shell means
/// no tab".
#[tokio::test]
async fn a_tab_whose_shell_exited_stays_listed_replayable_and_closable() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (chan, mut rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    let mut seen = Vec::new();
    wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;
    run_in_shell(
        &h.client,
        chan,
        &mut rx,
        &mut seen,
        "printf 'BEFORE%sEXIT\\n' -",
        "BEFORE-EXIT",
        20,
    )
    .await;
    h.client.send_input(chan, b"exit\r".to_vec()).await;

    // The pane goes dead; the tab does not.
    let pane = tab_pane(&h, &session.id, &tab.id).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let out = tmux_query(
            &h.state.path().join("tmux.sock"),
            &["display-message", "-p", "-t", &pane, "#{pane_dead}"],
        )
        .await;
        if String::from_utf8_lossy(&out.stdout).trim() == "1" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the tab's shell never exited"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()],
        "a tab whose shell exited is still a tab"
    );
    h.client.detach(chan).await;
    let (_chan2, mut rx2) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("a dead tab pane must still be attachable");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "BEFORE-EXIT", 20).await;

    h.client
        .close_tab(&session.id, &tab.id)
        .await
        .expect("a tab whose shell already exited must still close");
    assert!(listed_tabs(&h.client, &session.id).await.is_empty());
}

/// Closing a tab id that is well-formed but unknown is `NotFound`, and
/// costs the session's real terminals nothing.
///
/// The shape matters: a valid-looking id exercises the lookup rather than
/// the syntax check, which is the path a client holding a selector from
/// before a reboot actually takes.
#[tokio::test]
async fn closing_an_unknown_but_well_formed_tab_id_is_not_found_and_harms_nothing() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let err = h
        .client
        .close_tab(&session.id, "0e5d9a11-0000-4000-8000-00000000dead")
        .await
        .expect_err("an unknown tab id must be refused");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a refused close must carry a SupervisorError")
            .kind,
        ErrorKind::NotFound,
    );
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()],
        "a refused close must leave the session's real tabs alone"
    );
    h.client
        .send_input(agent_chan, b"agent-unharmed\r".to_vec())
        .await;
    wait_for(&mut agent_rx, &mut agent_seen, "agent-unharmed", 15).await;
}

/// A closed tab's attached client is told, on that tab's own channel.
///
/// A tab's forwarder holds a control client attached to the tmux SESSION,
/// so losing the tab's WINDOW does not end it — the stream would simply
/// go quiet forever. `detach_closed_tab` is the only thing that turns
/// that into a visible event, which is why it is asserted directly rather
/// than through "the terminal stopped updating".
#[tokio::test]
async fn a_closed_tabs_channel_receives_its_detached_notice() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (chan, mut rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    let mut seen = Vec::new();
    wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;

    h.client
        .close_tab(&session.id, &tab.id)
        .await
        .expect("close the tab");
    let reason = expect_detached(&mut rx, 15).await;
    assert!(
        reason.contains("tab"),
        "the notice must say the tab closed, got: {reason:?}"
    );
}

/// A tab open whose MARKING fails leaves nothing behind: no window, no
/// shell, no tab in the list — and the error says so rather than
/// claiming a clean removal it did not perform.
///
/// The marking is the one step whose failure strands something no
/// rediscovery can ever see again (an unmarked window is, by
/// construction, not a tab), so the unwind is worth proving rather than
/// reasoning about. The seam is the only way to reach that state: the
/// tmux call before it either works or leaves nothing.
#[tokio::test]
async fn a_tab_open_that_cannot_mark_its_window_leaves_nothing_behind() {
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            tab_open_fault: Some(Arc::new(|stage| {
                assert_eq!(
                    stage,
                    farhelm_supervisor::service::TabOpenStage::BeforeMarking
                );
                Err(anyhow::anyhow!("injected marking failure"))
            })),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let before = window_rows(&h).await.len();
    let err = h
        .client
        .open_tab(&session.id)
        .await
        .expect_err("an unmarkable window must fail the open");
    assert!(
        format!("{err:#}").contains("injected marking failure"),
        "the refusal must carry the cause, got: {err:#}"
    );
    assert!(
        listed_tabs(&h.client, &session.id).await.is_empty(),
        "a failed open must leave no tab"
    );
    let after = window_rows(&h).await;
    assert_eq!(
        after.len(),
        before,
        "a failed open must leave no window either; rows:\n{}",
        after.join("\n")
    );
}

/// A second lease attaching to EITHER of a session's terminals detaches
/// BOTH of the first client's channels, as one event — and leaves an
/// unrelated session's attachment alone (PLAN_M4.md acceptance 5).
///
/// SPEC.md's one-attached-client rule is per SESSION, not per terminal,
/// and only a session holding two terminals can show the difference: the
/// takeover has to sweep the whole lease and stop exactly there. The
/// unrelated session is the other half — a lease is never cross-session,
/// so one client may hold terminals in several sessions and taking one
/// over must not disturb the rest.
#[tokio::test]
async fn a_second_lease_takes_over_both_terminals_of_one_session_only() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (bystander, _bystander_work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let _bystander_cleanup = MarkerCleanupGuard::new(bystander.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (_agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "first-lease")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let (tab_chan, mut tab_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "first-lease",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;
    // The SAME client also holds the bystander session, under the same
    // lease: a lease groups a client's channels per session, never across
    // sessions.
    let (bystander_chan, mut bystander_rx) = h
        .client
        .attach_terminal(
            &bystander.id,
            80,
            24,
            TerminalSelector::Agent,
            "first-lease",
        )
        .await
        .expect("attach the bystander");
    let mut bystander_seen = Vec::new();
    wait_for(
        &mut bystander_rx,
        &mut bystander_seen,
        "FAKE-AGENT READY",
        20,
    )
    .await;

    // A different lease attaches to just ONE of the two terminals.
    let second = h.second_client().await;
    let (_winner_chan, mut winner_rx) = second
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "second-lease")
        .await
        .expect("take over");

    let agent_reason = expect_detached(&mut agent_rx, 15).await;
    let tab_reason = expect_detached(&mut tab_rx, 15).await;
    assert_eq!(
        agent_reason, tab_reason,
        "both channels of the losing lease must be told the SAME reason, which is what lets a \
         client coalesce them into one banner"
    );
    assert!(
        agent_reason.contains("another client"),
        "the reason must name a takeover, got: {agent_reason:?}"
    );

    // The bystander session's attachment, held by the SAME losing lease,
    // is untouched.
    let disturbed = drain_for(
        &mut bystander_rx,
        &mut bystander_seen,
        Duration::from_millis(500),
    )
    .await;
    assert_eq!(
        disturbed, None,
        "a takeover on one session must not detach the same client's terminals in another"
    );
    h.client
        .send_input(bystander_chan, b"bystander-alive\r".to_vec())
        .await;
    wait_for(
        &mut bystander_rx,
        &mut bystander_seen,
        "bystander-alive",
        15,
    )
    .await;
    // And the winner really has the terminal.
    let mut winner_seen = Vec::new();
    wait_for(&mut winner_rx, &mut winner_seen, "FAKE-AGENT READY", 20).await;
}

/// Deleting a session detaches EVERY channel it had — agent and tabs —
/// with the deletion notice, and reaps two environment-scrubbed tab
/// daemons through their own cgroups.
///
/// Two tabs rather than one because a delete names each tab's scope
/// separately (a cgroup kill reaches only what its own `systemd-run`
/// placed there), so a bug that named just one would pass with a single
/// tab. Loudly skipped where no user manager exists — the cloaked daemons
/// are unreachable by any marker scan by construction, which is the whole
/// point of the fixture.
#[tokio::test]
async fn deleting_a_session_detaches_every_channel_and_reaps_scrubbed_tab_daemons() {
    let Some((h, _scopes)) = scope_gated_harness(
        "deleting_a_session_detaches_every_channel_and_reaps_scrubbed_tab_daemons",
    )
    .await
    else {
        return;
    };
    let (session, work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let mut tab_streams = Vec::new();
    let mut cloaked = Vec::new();
    let mut guards = Vec::new();
    for which in ["a", "b"] {
        let tab = h.client.open_tab(&session.id).await.expect("open a tab");
        let (chan, mut rx) = h
            .client
            .attach_terminal(
                &session.id,
                80,
                24,
                TerminalSelector::Tab { id: tab.id.clone() },
                "one-client",
            )
            .await
            .expect("attach the tab");
        let mut seen = Vec::new();
        wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;
        let pid_file = work.path().join(format!("cloaked-{which}.pid"));
        let script =
            write_daemon_script(work.path(), &format!("cloak-{which}.sh"), &pid_file, true);
        run_in_shell(
            &h.client,
            chan,
            &mut rx,
            &mut seen,
            &format!("sh {} && printf 'CLOAK%sD\\n' E", script.display()),
            "CLOAKED",
            20,
        )
        .await;
        let pid = wait_for_pid_file(&pid_file, 10).await;
        guards.push(PidKillGuard::arm(pid));
        assert!(
            !marked_pids(&session.id).contains(&pid),
            "test setup: tab {which}'s cloaked daemon must carry no marker at all"
        );
        cloaked.push(pid);
        tab_streams.push(rx);
    }

    h.client.delete_session(&session.id).await.expect("delete");

    // Generous, because the failure this bounds is "it never died" and
    // the path to death is three D-Bus round trips per scope on a host
    // running several of these harnesses at once.
    for pid in cloaked {
        wait_until_pid_gone(pid, 30).await;
    }
    let agent_reason = expect_detached(&mut agent_rx, 15).await;
    assert!(
        agent_reason.contains("deleted"),
        "the agent's channel must be told the session was deleted, got: {agent_reason:?}"
    );
    for (index, mut rx) in tab_streams.into_iter().enumerate() {
        let reason = expect_detached(&mut rx, 15).await;
        assert_eq!(
            reason, agent_reason,
            "tab {index}'s channel must get the same deletion notice the agent's did"
        );
    }
}

/// Tabs survive a supervisor restart as the SAME shells, with their
/// scrollback (PLAN_M4.md acceptance 2).
///
/// The sibling rediscovery test pins that the ids and the ordering come
/// back. This pins the thing that actually matters to a user: the process
/// never noticed. Comparing pane PIDs across the restart is what
/// distinguishes "rediscovered the same shell" from "quietly started a
/// new one", and replaying content written before the restart is what
/// distinguishes a live reattachment from a fresh, empty terminal.
#[tokio::test]
async fn a_supervisor_restart_leaves_a_tabs_shell_and_scrollback_untouched() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let work = tempfile::tempdir().expect("workdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let client = connect_client(&sup).await;
    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tab = client.open_tab(&session.id).await.expect("open a tab");

    let (chan, mut rx) = client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "before-restart",
        )
        .await
        .expect("attach the tab");
    let mut seen = Vec::new();
    wait_for_shell(&client, chan, &mut rx, &mut seen, "READY").await;
    run_in_shell(
        &client,
        chan,
        &mut rx,
        &mut seen,
        "printf 'BEFORE%sRESTART\\n' -",
        "BEFORE-RESTART",
        20,
    )
    .await;

    let sock = state.path().join("tmux.sock");
    let pane_before =
        tmux_query(&sock, &["list-panes", "-a", "-F", "#{pane_id} #{pane_pid}"]).await;
    let pids_before = String::from_utf8_lossy(&pane_before.stdout).into_owned();

    drop(client);
    drop(sup);
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor");
    let client = connect_client(&sup).await;

    let pane_after = tmux_query(&sock, &["list-panes", "-a", "-F", "#{pane_id} #{pane_pid}"]).await;
    assert_eq!(
        String::from_utf8_lossy(&pane_after.stdout),
        pids_before,
        "a supervisor restart must rediscover the SAME shells, not start new ones"
    );

    let (_chan2, mut rx2) = client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "after-restart",
        )
        .await
        .expect("attach the rediscovered tab");
    let mut replay = Vec::new();
    wait_for(&mut rx2, &mut replay, "BEFORE-RESTART", 20).await;
    drop(slot);
}

/// A stalled TAB viewer takes the stall detach ALONE: the agent terminal
/// and a sibling tab stay usable throughout, and the stalled tab
/// reattaches as an ordinary reconnect with its scrollback (PLAN_M4.md
/// acceptance 5, item 3's deliberate per-terminal reading).
///
/// The stall timeout is shortened through the same seam every other
/// stall test uses, because the production value is a minute. What this
/// pins is the DETACH being scoped to one terminal — a client whose
/// background tab wedged must not lose the terminal it is looking at.
///
/// Scope: this exercises whichever branch tmux happened to take for the
/// stalled client (it answers a lagging client either by cutting it with
/// `%pause` or by not reading the pane it is behind on,
/// nondeterministically — see `TMUX_PAUSE_AFTER_SECS`), and pins the
/// SUPERVISOR's per-terminal teardown, which is the same on both. The
/// second branch used to make the surrounding terminals collateral damage
/// until this detach fired; the session sink closes that, so what is left
/// here is genuinely about the detach being scoped to one terminal.
#[tokio::test]
async fn a_stalled_tab_takes_the_stall_detach_alone_and_reattaches() {
    let stall = Duration::from_secs(3);
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: stall,
        ..SupervisorTimeouts::default()
    })
    .await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let stalling = h.client.open_tab(&session.id).await.expect("open a tab");
    let sibling = h
        .client
        .open_tab(&session.id)
        .await
        .expect("open a second tab");

    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let (sibling_chan, mut sibling_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: sibling.id.clone(),
            },
            "one-client",
        )
        .await
        .expect("attach the sibling tab");
    let mut sibling_seen = Vec::new();
    wait_for_shell(
        &h.client,
        sibling_chan,
        &mut sibling_rx,
        &mut sibling_seen,
        "SIB",
    )
    .await;
    let (stalling_chan, mut stalling_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: stalling.id.clone(),
            },
            "one-client",
        )
        .await
        .expect("attach the stalling tab");
    let mut stalling_seen = Vec::new();
    wait_for_shell(
        &h.client,
        stalling_chan,
        &mut stalling_rx,
        &mut stalling_seen,
        "STALL",
    )
    .await;
    run_in_shell(
        &h.client,
        stalling_chan,
        &mut stalling_rx,
        &mut stalling_seen,
        "printf 'BEFORE%sSTALL\\n' -",
        "BEFORE-STALL",
        20,
    )
    .await;

    // Wedge exactly one terminal and give it something to be behind on.
    // Deliberately a MODEST flood: the stall deadline is absolute from
    // the moment the pause is recorded, so volume is not what triggers
    // it, and a flood large enough to overrun `HISTORY_LIMIT` would push
    // the pre-stall marker out of the scrollback this test reattaches to
    // read.
    h.client.pause_output(stalling_chan).await;
    h.client
        .send_input(
            stalling_chan,
            b"i=0; while [ $i -lt 500 ]; do printf 'FLOOD-%s\\n' $i; i=$((i+1)); done\r".to_vec(),
        )
        .await;

    let reason = expect_detached(&mut stalling_rx, 60).await;
    assert!(
        reason.contains("stall"),
        "the stalled tab must be detached as stalled, got: {reason:?}"
    );

    // The agent and the sibling tab were never disturbed and still work.
    h.client
        .send_input(agent_chan, b"agent-untouched\r".to_vec())
        .await;
    wait_for(&mut agent_rx, &mut agent_seen, "agent-untouched", 20).await;
    run_in_shell(
        &h.client,
        sibling_chan,
        &mut sibling_rx,
        &mut sibling_seen,
        "printf 'SIBLING%sOK\\n' -",
        "SIBLING-OK",
        20,
    )
    .await;

    // And the stalled terminal reattaches like any reconnect, with the
    // scrollback it had before it wedged.
    let (_again, mut again_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: stalling.id.clone(),
            },
            "one-client",
        )
        .await
        .expect("a stall detach must leave the tab reattachable");
    let mut replay = Vec::new();
    wait_for(&mut again_rx, &mut replay, "BEFORE-STALL", 30).await;
}

/// A tab OPEN racing a session DELETE resolves to one coherent winner and
/// leaves no orphan, at any interleaving.
///
/// The session's lifecycle claim is what makes that true: without it a
/// delete could finish its process-tree sweep and an open could then start
/// a shell in the tmux session the delete is about to tear down, leaving
/// that shell's daemonized children alive with no row left to reap them
/// from. Serialized, both orders are correct — an open that wins is swept
/// by the delete behind it, and an open that loses finds no session at
/// all — so the assertion is on the OUTCOMES rather than on which one won.
///
/// Staggered offsets rather than one fixed timing, the same technique
/// `a_stall_teardown_racing_a_takeover_never_detaches_the_winner` uses:
/// the interesting interleavings are near the boundary and no single
/// delay reliably lands on them.
#[tokio::test]
async fn an_open_tab_racing_a_delete_leaves_one_coherent_winner() {
    for offset_ms in [0, 5, 20, 60] {
        let h = harness().await;
        let (session, _work) = basic_session(&h).await;
        let cleanup = MarkerCleanupGuard::new(session.id.clone());

        let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
        let mut seen = Vec::new();
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

        let opener = h.second_client().await;
        let open_id = session.id.clone();
        let opening =
            tokio::spawn(async move { opener.open_tab(&open_id).await.map(|tab| tab.id) });
        tokio::time::sleep(Duration::from_millis(offset_ms)).await;
        let deleted = h.client.delete_session(&session.id).await;
        let opened = opening.await.expect("the open task must not panic");

        assert!(
            deleted.is_ok(),
            "offset {offset_ms}: the delete must not be defeated by a concurrent open: {:#}",
            deleted.unwrap_err()
        );
        // Whichever way it went, nothing of the session may remain: no
        // row, no tmux session, and no marked process — including a shell
        // an open that WON would have started.
        assert!(
            h.client
                .list_sessions()
                .await
                .expect("list")
                .sessions
                .iter()
                .all(|listed| listed.id != session.id),
            "offset {offset_ms}: the deleted session must be gone from the list"
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while !marked_pids(&session.id).is_empty() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "offset {offset_ms}: a delete racing an open (which {}) left marked processes \
                 behind: {:?}",
                if opened.is_ok() { "won" } else { "lost" },
                marked_pids(&session.id)
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        drop(cleanup);
    }
}

/// A tab CLOSE racing a session DELETE likewise resolves coherently.
///
/// The same claim serializes them, and the failure it prevents is uglier
/// than the open's: two sweeps and two teardowns racing over one window,
/// with either able to report success while the other was mid-reap. Both
/// orders are acceptable outcomes — a close that wins is followed by a
/// delete that finds one fewer tab, and a close that loses finds no
/// session — so this asserts the delete succeeds, the close's own answer
/// is one of those two shapes, and nothing survives.
#[tokio::test]
async fn a_close_tab_racing_a_delete_leaves_one_coherent_winner() {
    for offset_ms in [0, 10, 40] {
        let h = harness().await;
        let (session, _work) = basic_session(&h).await;
        let cleanup = MarkerCleanupGuard::new(session.id.clone());

        let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
        let mut seen = Vec::new();
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
        let tab = h.client.open_tab(&session.id).await.expect("open a tab");

        let closer = h.second_client().await;
        let close_session = session.id.clone();
        let close_tab_id = tab.id.clone();
        let closing =
            tokio::spawn(async move { closer.close_tab(&close_session, &close_tab_id).await });
        tokio::time::sleep(Duration::from_millis(offset_ms)).await;
        let deleted = h.client.delete_session(&session.id).await;
        let closed = closing.await.expect("the close task must not panic");

        assert!(
            deleted.is_ok(),
            "offset {offset_ms}: the delete must not be defeated by a concurrent close: {:#}",
            deleted.unwrap_err()
        );
        if let Err(e) = &closed {
            assert_eq!(
                e.downcast_ref::<SupervisorError>()
                    .expect("a refused close must carry a SupervisorError")
                    .kind,
                ErrorKind::NotFound,
                "offset {offset_ms}: a close that lost the race must report the session or tab \
                 as gone, not a teardown failure: {e:#}"
            );
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while !marked_pids(&session.id).is_empty() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "offset {offset_ms}: a delete racing a close left marked processes behind: {:?}",
                marked_pids(&session.id)
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        drop(cleanup);
    }
}
