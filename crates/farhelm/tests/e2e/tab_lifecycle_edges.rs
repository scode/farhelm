//! Tab lifecycle edge cases: restart-gap conflicts and other boundary
//! conditions a tab's open/close path must resolve consistently.
//!

use crate::harness::*;

use crate::terminal_tabs::{listed_tabs, run_in_shell, wait_for_shell, write_daemon_script};

/// Observe that a delete racing a tab operation left no process with the owned session marker.
///
/// Either operation may win. The caller checks its protocol outcome first;
/// this independent process scan then covers descendants that could outlive
/// an otherwise successful response. Keep the cleanup guard alive around it.
async fn wait_for_tab_race_cleanup(session_id: &str, context: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let pids = marked_pids(session_id);
        if pids.is_empty() {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{context}: marked processes survived: {pids:?}"
        );
        // sleep-ok: repeat the owned-marker scan within the existing cleanup observation window, retaining the final survivor assertion.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// ---------------------------------------------------------------------------
// Tab lifecycle edges
// ---------------------------------------------------------------------------

/// Opening a tab on a RESTART-GAP session — one whose row survived a
/// supervisor restart but whose terminal did not — is the restart-first
/// conflict, and it must not start a tab host as a side effect.
///
/// The entry has no terminal at all, which is the branch that answers
/// before any host is asked; an entry whose host is still held but has
/// ended gives the same advice, because they are the same fact for a
/// user.
#[farhelm_testtrace::test]
async fn opening_a_tab_on_a_restart_gap_session_is_a_restart_first_conflict() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("tempdir");
    let work = farhelm_teststate::tempdir().expect("workdir");

    let sup = supervisor_on(state.path()).await.expect("supervisor");
    let client = connect_client(&sup).await;
    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &fixture_cmd("fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    // End every terminal and reload: the row comes back terminal-less,
    // which is the restart gap PLAN_M2.md names.
    drop(client);
    drop(sup);
    end_all_terminals(state.path()).await;
    let sup = supervisor_on(state.path())
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
            .kind(),
        ErrorKind::Conflict
    );
    // A refused open must not have built a tab host as a side effect: no launch record for a tab of this session, and no
    // live process serving a socket in the state directory.
    let tab_records: Vec<_> = std::fs::read_dir(state.path().join("launch"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with(&format!("{}.tab-", session.id)))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        tab_records.is_empty(),
        "a refused open must not have recorded a tab host: {tab_records:?}"
    );
    assert!(
        crate::host_creates::host_processes_in(state.path()).is_empty(),
        "a refused open must not have started a tab host"
    );
    drop(slot);
}

/// Closing a tab id that is well-formed but unknown is `NotFound`, and
/// costs the session's real terminals nothing.
///
/// The shape matters: a valid-looking id exercises the lookup rather than
/// the syntax check, which is the path a client holding a selector from
/// before a reboot actually takes.
#[farhelm_testtrace::test]
async fn closing_an_unknown_but_well_formed_tab_id_is_not_found_and_harms_nothing() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (agent_chan, initial_replay, mut agent_rx) = h
        .client
        .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = initial_replay;
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let err = h
        .client
        .close_tab(&session.id, "0e5d9a11-0000-4000-8000-00000000dead")
        .await
        .expect_err("an unknown tab id must be refused");
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a refused close must carry a SupervisorError")
            .kind(),
        ErrorKind::NotFound,
    );
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()],
        "a refused close must leave the session's real tabs alone"
    );
    h.client
        .send_keys(agent_chan, b"agent-unharmed\r".to_vec())
        .await;
    wait_for(&mut agent_rx, &mut agent_seen, "agent-unharmed", 15).await;
}

/// A closed tab's attached client is told, on that tab's own channel.
///
/// The client must learn why its stream ended, on that tab's own channel
/// (`detach_closed_tab`), which is why the notice is asserted directly
/// rather than through "the terminal stopped updating".
#[farhelm_testtrace::test]
async fn a_closed_tabs_channel_receives_its_detached_notice() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (chan, initial_replay, mut rx) = h
        .client
        .attach_terminal_live(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "tab-lease",
        )
        .await
        .expect("attach the tab");
    let mut seen = initial_replay;
    wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;

    h.client
        .close_tab(&session.id, &tab.id)
        .await
        .expect("close the tab");
    let reason = expect_detached(&mut rx, 15).await;
    assert!(
        reason.code == farhelm_proto::DetachCode::TabClosed,
        "the notice must say the tab closed, got: {reason:?}"
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
#[farhelm_testtrace::test]
async fn a_second_lease_takes_over_both_terminals_of_one_session_only() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (bystander, _bystander_work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let _bystander_cleanup = MarkerCleanupGuard::new(bystander.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (_agent_chan, initial_replay, mut agent_rx) = h
        .client
        .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "first-lease")
        .await
        .expect("attach the agent");
    let mut agent_seen = initial_replay;
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let (tab_chan, initial_replay, mut tab_rx) = h
        .client
        .attach_terminal_live(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "first-lease",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = initial_replay;
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;
    // The SAME client also holds the bystander session, under the same
    // lease: a lease groups a client's channels per session, never across
    // sessions.
    let (bystander_chan, initial_replay, mut bystander_rx) = h
        .client
        .attach_terminal_live(
            &bystander.id,
            80,
            24,
            TerminalSelector::Agent,
            "first-lease",
        )
        .await
        .expect("attach the bystander");
    let mut bystander_seen = initial_replay;
    wait_for(
        &mut bystander_rx,
        &mut bystander_seen,
        "FAKE-AGENT READY",
        20,
    )
    .await;

    // A different lease attaches to just ONE of the two terminals.
    let second = h.second_client().await;
    let (_winner_chan, initial_replay, mut winner_rx) = second
        .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "second-lease")
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
        agent_reason.code == farhelm_proto::DetachCode::TakenOver,
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
        .send_keys(bystander_chan, b"bystander-alive\r".to_vec())
        .await;
    wait_for(
        &mut bystander_rx,
        &mut bystander_seen,
        "bystander-alive",
        15,
    )
    .await;
    // And the winner really has the terminal.
    let mut winner_seen = initial_replay;
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
#[farhelm_testtrace::test]
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

    let (_agent_chan, initial_replay, mut agent_rx) = h
        .client
        .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = initial_replay;
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let mut tab_streams = Vec::new();
    let mut cloaked = Vec::new();
    let mut guards = Vec::new();
    for which in ["a", "b"] {
        let tab = h.client.open_tab(&session.id).await.expect("open a tab");
        let (chan, initial_replay, mut rx) = h
            .client
            .attach_terminal_live(
                &session.id,
                80,
                24,
                TerminalSelector::Tab { id: tab.id.clone() },
                "one-client",
            )
            .await
            .expect("attach the tab");
        let mut seen = initial_replay;
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
        agent_reason.reason.contains("deleted"),
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

/// The pid of the shell tab `tab_id` of session `session_id` runs, as
/// `sup`'s runtime for the tab's terminal host reports it, waiting (bounded)
/// for a supervisor that is still finding its tabs again to publish it.
async fn tab_host_shell(sup: &Supervisor, session_id: &str, tab_id: &str) -> u32 {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(host) = sup.tab_host_for_test(session_id, tab_id).await {
            return host.status().child_pid;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "tab {tab_id} has no terminal host runtime"
        );
        // sleep-ok: a restarted supervisor publishes the tabs it found again on its own schedule; poll until this one is published.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Tabs survive a supervisor restart as the SAME shells, with their
/// scrollback (PLAN_M4.md acceptance 2).
///
/// The sibling rediscovery test pins that the ids and the ordering come
/// back. This pins the thing that actually matters to a user: the process
/// never noticed. Comparing the tab shell's pid across the restart (the
/// tab host's child, as each supervisor's runtime for it reports it) is
/// what distinguishes "found the same shell again" from "quietly started a
/// new one", and replaying content written before the restart is what
/// distinguishes a live reattachment from a fresh, empty terminal.
#[farhelm_testtrace::test]
async fn a_supervisor_restart_leaves_a_tabs_shell_and_scrollback_untouched() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("tempdir");
    let work = farhelm_teststate::tempdir().expect("workdir");

    let sup = supervisor_on(state.path()).await.expect("supervisor");
    let client = connect_client(&sup).await;
    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &fixture_cmd("fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tab = client.open_tab(&session.id).await.expect("open a tab");

    let (chan, initial_replay, mut rx) = client
        .attach_terminal_live(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "before-restart",
        )
        .await
        .expect("attach the tab");
    let mut seen = initial_replay;
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

    let shell_before = tab_host_shell(&sup, &session.id, &tab.id).await;
    assert!(
        !process_is_gone(shell_before),
        "premise: the tab's shell {shell_before} runs before the restart"
    );

    client.detach(chan).await;
    retire_supervisor(state.path(), sup, client).await;
    let sup = supervisor_on(state.path())
        .await
        .expect("second supervisor");
    let client = connect_client(&sup).await;

    assert_eq!(
        tab_host_shell(&sup, &session.id, &tab.id).await,
        shell_before,
        "a supervisor restart must find the SAME shell again, not start a new one"
    );

    let (_chan2, initial_replay, mut rx2) = client
        .attach_terminal_live(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "after-restart",
        )
        .await
        .expect("attach the rediscovered tab");
    let mut replay = initial_replay;
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
#[farhelm_testtrace::test]
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

    let (agent_chan, initial_replay, mut agent_rx) = h
        .client
        .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = initial_replay;
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let (sibling_chan, initial_replay, mut sibling_rx) = h
        .client
        .attach_terminal_live(
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
    let mut sibling_seen = initial_replay;
    wait_for_shell(
        &h.client,
        sibling_chan,
        &mut sibling_rx,
        &mut sibling_seen,
        "SIB",
    )
    .await;
    let (stalling_chan, initial_replay, mut stalling_rx) = h
        .client
        .attach_terminal_live(
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
    let mut stalling_seen = initial_replay;
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
        .send_keys(
            stalling_chan,
            b"i=0; while [ $i -lt 500 ]; do printf 'FLOOD-%s\\n' $i; i=$((i+1)); done\r".to_vec(),
        )
        .await;

    let reason = expect_detached(&mut stalling_rx, 60).await;
    assert!(
        reason.code == farhelm_proto::DetachCode::Stalled,
        "the stalled tab must be detached as stalled, got: {reason:?}"
    );

    // The agent and the sibling tab were never disturbed and still work.
    h.client
        .send_keys(agent_chan, b"agent-untouched\r".to_vec())
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
    let (_again, initial_replay, mut again_rx) = h
        .client
        .attach_terminal_live(
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
    let mut replay = initial_replay;
    wait_for(&mut again_rx, &mut replay, "BEFORE-STALL", 30).await;
}

/// A tab OPEN racing a session DELETE resolves to one coherent winner and
/// leaves no orphan, at any interleaving.
///
/// The session's lifecycle claim is what makes that true: without it a
/// delete could finish its process-tree sweep and an open could then start
/// a shell beside the session the delete is about to tear down, leaving
/// that shell's daemonized children alive with no row left to reap them
/// from. Serialized, both orders are correct — an open that wins is swept
/// by the delete behind it, and an open that loses finds no session at
/// all — so the assertion is on the OUTCOMES rather than on which one won.
///
/// Staggered offsets rather than one fixed timing: the interesting
/// interleavings are near the boundary and no single delay reliably lands
/// on them.
#[farhelm_testtrace::test]
async fn an_open_tab_racing_a_delete_leaves_one_coherent_winner() {
    for offset_ms in [0, 5, 20, 60] {
        let h = harness().await;
        let (session, _work) = basic_session(&h).await;
        let cleanup = MarkerCleanupGuard::new(session.id.clone());

        let (_chan, initial_replay, mut rx) = h
            .client
            .attach_live(&session.id, 80, 24)
            .await
            .expect("attach");
        let mut seen = initial_replay;
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

        let opener = h.second_client().await;
        let open_id = session.id.clone();
        let opening =
            tokio::spawn(async move { opener.open_tab(&open_id).await.map(|tab| tab.id) });
        // sleep-ok: vary the open/delete race's scheduling offset; either order is permitted, and deletion and cleanup are checked without assuming readiness.
        tokio::time::sleep(Duration::from_millis(offset_ms)).await;
        let deleted = h.client.delete_session(&session.id).await;
        let opened = opening.await.expect("the open task must not panic");

        assert!(
            deleted.is_ok(),
            "offset {offset_ms}: the delete must not be defeated by a concurrent open: {:#}",
            deleted.unwrap_err()
        );
        // Whichever way it went, nothing of the session may remain: no
        // row, no tab host, and no marked process — including a shell
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
        wait_for_tab_race_cleanup(
            &session.id,
            &format!(
                "offset {offset_ms}: delete racing open (which {})",
                if opened.is_ok() { "won" } else { "lost" }
            ),
        )
        .await;
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
#[farhelm_testtrace::test]
async fn a_close_tab_racing_a_delete_leaves_one_coherent_winner() {
    for offset_ms in [0, 10, 40] {
        let h = harness().await;
        let (session, _work) = basic_session(&h).await;
        let cleanup = MarkerCleanupGuard::new(session.id.clone());

        let (_chan, initial_replay, mut rx) = h
            .client
            .attach_live(&session.id, 80, 24)
            .await
            .expect("attach");
        let mut seen = initial_replay;
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
        let tab = h.client.open_tab(&session.id).await.expect("open a tab");

        let closer = h.second_client().await;
        let close_session = session.id.clone();
        let close_tab_id = tab.id.clone();
        let closing =
            tokio::spawn(async move { closer.close_tab(&close_session, &close_tab_id).await });
        // sleep-ok: vary the close/delete race's scheduling offset without requiring either operation to have reached a particular state.
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
                    .kind(),
                ErrorKind::NotFound,
                "offset {offset_ms}: a close that lost the race must report the session or tab \
                 as gone, not a teardown failure: {e:#}"
            );
        }
        wait_for_tab_race_cleanup(
            &session.id,
            &format!("offset {offset_ms}: delete racing close"),
        )
        .await;
        drop(cleanup);
    }
}
