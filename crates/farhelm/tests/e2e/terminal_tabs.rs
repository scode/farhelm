//! Terminal tabs: the launch contract, refusals, close's reap,
//! rediscovery from a window marker, and the per-terminal properties
//! that only become observable once a session has a second terminal.

use crate::harness::*;

use crate::restart_with_resume::{RC_MARKER_VAR, write_rc_files};
use crate::terminal_backpressure::drain_for;

// ---------------------------------------------------------------------------
// Terminal tabs (PLAN_M4.md item 2)
//
// A tab is a tmux WINDOW on the session's tmux session running the user's
// login shell, rediscovered from a window marker rather than stored. The
// tests below are grouped by the promise each one pins: the launch
// contract, the refusals, close's reap, rediscovery, the marker split that
// keeps stop and restart off tabs, and the per-terminal properties that
// only become observable once a session has a second terminal at all.
// ---------------------------------------------------------------------------

/// A supervisor whose launches all run `shell` — the seam that makes a
/// tab's own launch drivable, since a tab has no invocation of its own.
///
/// Used two ways: to give the agent terminal a plain shell so the
/// conformance tests can drive BOTH terminals identically, and to give a
/// tab a shell that fails immediately so the dead-at-open-reply refusal is
/// reachable at all.
async fn harness_with_shell(shell: &str) -> Harness {
    harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_shell: Some(shell.to_string()),
            ..SupervisorSeams::default()
        },
    )
    .await
}

/// Wait for an attached SHELL terminal to be ready to accept a command.
///
/// A shell announces readiness with a prompt, whose text is the user's
/// business and not something a test may assume. So readiness is
/// established by round trip instead: send a command whose OUTPUT differs
/// from its own echo, and wait for the output. `printf 'X%sX\n' MARK`
/// echoes as its source text and prints `XMARKX`, so waiting on the latter
/// cannot be satisfied by the terminal merely echoing what was typed.
///
/// Retried rather than sent once: an interactive shell that has not
/// finished starting discards input, and there is no observable moment at
/// which it starts accepting it.
pub(crate) async fn wait_for_shell(
    client: &SupervisorClient,
    channel: u32,
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    marker: &str,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let command = format!("printf 'X%sX\\n' {marker}\r");
    let expected = format!("X{marker}X");
    loop {
        client
            .send_input(channel, command.clone().into_bytes())
            .await;
        let waited =
            tokio::time::timeout(Duration::from_secs(3), wait_for(rx, seen, &expected, 3)).await;
        if waited.is_ok() {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "shell on channel {channel} never answered a round trip; transcript so far:\n{}",
            String::from_utf8_lossy(seen)
        );
    }
}

/// Run `command` in an attached shell terminal and wait for `marker` in
/// its output. The caller is responsible for choosing a marker the
/// command's own echoed text does not contain (see [`wait_for_shell`]).
pub(crate) async fn run_in_shell(
    client: &SupervisorClient,
    channel: u32,
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    command: &str,
    marker: &str,
    secs: u64,
) {
    client
        .send_input(channel, format!("{command}\r").into_bytes())
        .await;
    wait_for(rx, seen, marker, secs).await;
}

/// Every pane on the harness's tmux server, as five `|`-separated fields:
/// `pane_id|session_name|window_id|tab_marker|agent_marker`.
///
/// The test-side mirror of the supervisor's own rediscovery query, used to
/// assert what tmux actually holds rather than trusting the supervisor's
/// report of it — which is the whole point for a feature whose only record
/// IS the tmux window marker. Both markers are included because both are
/// now READ by the supervisor: the tab marker is a tab's whole identity,
/// and the agent marker is what a pane-less reload prefers.
pub(crate) async fn window_rows(h: &Harness) -> Vec<String> {
    let out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &[
            "list-panes",
            "-a",
            "-F",
            "#{pane_id}|#{session_name}|#{window_id}|#{@farhelm-tab}|#{@farhelm-agent}",
        ],
    )
    .await;
    assert!(
        out.status.success(),
        "listing panes failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// The tmux pane id backing one tab, read from tmux's own window markers.
pub(crate) async fn tab_pane(h: &Harness, session_id: &str, tab_id: &str) -> String {
    let want_session = format!("fh-{session_id}");
    let rows = window_rows(h).await;
    rows.iter()
        .find_map(|row| {
            let mut fields = row.split('|');
            let pane = fields.next()?;
            let session = fields.next()?;
            let _window = fields.next()?;
            let tab = fields.next()?;
            (session == want_session && tab == tab_id).then(|| pane.to_string())
        })
        .unwrap_or_else(|| panic!("no pane carries tab {tab_id}; rows:\n{}", rows.join("\n")))
}

/// The tab ids `list_sessions` currently reports for a session, in the
/// order it reports them.
pub(crate) async fn listed_tabs(client: &SupervisorClient, session_id: &str) -> Vec<String> {
    client
        .list_sessions()
        .await
        .expect("list")
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .unwrap_or_else(|| panic!("session {session_id} is not listed"))
        .tabs
        .iter()
        .map(|tab| tab.id.clone())
        .collect()
}

/// Write a shell script that daemonizes a long `sleep` and records its
/// pid, returning the script's path.
///
/// The `( … & )` subshell is a deliberate double fork: the intermediate
/// subshell exits immediately, so the `sleep` reparents to init and no
/// PPID closure from the tab's pane can reach it. `setsid` puts it in its
/// own session so the pty hangup that follows `kill-window` cannot reach
/// it either — without that, a survivor could die from the window's own
/// teardown and prove nothing about the reap that is supposed to have
/// killed it.
///
/// `scrub_env` additionally strips the environment (`env -i`), which
/// removes both farhelm markers and so hides the process from the marker
/// scan entirely — the accidental-daemonization shape only a cgroup can
/// reach (lore/2026-07-27-m2-process-tree-stop.md).
pub(crate) fn write_daemon_script(
    dir: &std::path::Path,
    name: &str,
    pid_file: &std::path::Path,
    scrub_env: bool,
) -> std::path::PathBuf {
    let path = dir.join(name);
    let scrub = if scrub_env { "env -i " } else { "" };
    std::fs::write(
        &path,
        format!(
            "( setsid {scrub}/bin/sh -c 'echo $$ > {pid}; exec sleep 120' \
             </dev/null >/dev/null 2>&1 & )\n",
            pid = pid_file.display()
        ),
    )
    .expect("writing the daemonizer script");
    path
}

/// A tab opened in a session runs the user's shell in the SESSION's
/// working directory, is attachable by the id the open returned, and
/// appears in that session's authoritative tab list (PLAN_M4.md
/// acceptance 1).
///
/// The working directory is checked by asking the shell rather than by
/// inspecting tmux: what SPEC.md promises is that a command typed in the
/// tab shows the session's cwd, and only running one proves the `-c` was
/// applied to the process rather than merely to the window's metadata.
#[tokio::test]
async fn a_tab_runs_a_shell_in_the_sessions_working_directory() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()],
        "an opened tab must appear in the session's own tab list, which is the one place \
         ordering is authoritative"
    );

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

    // The session's cwd is a tempdir whose path may be a symlink (macOS
    // aside, `/tmp` is one on some Linux setups), so compare against what
    // the shell itself resolves rather than against the literal path.
    run_in_shell(
        &h.client,
        chan,
        &mut rx,
        &mut seen,
        "printf 'CW%s[%s]\\n' D \"$PWD\"",
        "CWD[",
        20,
    )
    .await;
    let transcript = String::from_utf8_lossy(&seen).into_owned();
    let reported = transcript
        .split("CWD[")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("the tab shell must report its working directory")
        .trim()
        .to_string();
    let expected = std::fs::canonicalize(work.path()).expect("canonical workdir");
    assert_eq!(
        std::fs::canonicalize(&reported).expect("canonical reported cwd"),
        expected,
        "a tab must start in the session's working directory, not the supervisor's"
    );
}

/// Opening a tab when the session's working directory has vanished fails
/// with an error NAMING the directory, and leaves the session untouched
/// (PLAN_M4.md acceptance 4).
///
/// The same precondition — and deliberately the same error shape — restart
/// makes, reused unchanged rather than reworded: a user who has seen one
/// of these refusals should recognize the other.
#[tokio::test]
async fn opening_a_tab_after_the_working_directory_vanished_names_it() {
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
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    work.close()
        .expect("removing the session's working directory");

    let err = h
        .client
        .open_tab(&session.id)
        .await
        .expect_err("a vanished working directory must refuse the open");
    assert!(
        err.to_string().contains(&cwd),
        "the refusal must name the directory that vanished, got: {err:#}"
    );
    assert!(
        listed_tabs(&h.client, &session.id).await.is_empty(),
        "a refused open must leave no tab behind"
    );
    // The session itself is untouched: its agent is still answering.
    h.client.send_input(_chan, b"still-here\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "still-here", 15).await;
}

/// Opening a tab on a session whose tmux session no longer exists is
/// refused with advice to restart the session first (PLAN_M4.md
/// acceptance 4).
///
/// Building a tab-only tmux session for an agent-less session would be a
/// strange half-alive state this system deliberately does not have, and
/// SPEC.md already puts re-adding tabs after the user's own restart. The
/// refusal has to SAY that, because "no such terminal" alone leaves the
/// user with no next step.
#[tokio::test]
async fn opening_a_tab_without_a_tmux_session_says_to_restart_first() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let sock = h.state.path().join("tmux.sock");
    let killed = tmux_query(
        &sock,
        &["kill-session", "-t", &format!("fh-{}", session.id)],
    )
    .await;
    assert!(
        killed.status.success(),
        "test setup: killing the session's tmux session must succeed: {}",
        String::from_utf8_lossy(&killed.stderr)
    );

    let err = h
        .client
        .open_tab(&session.id)
        .await
        .expect_err("a session with no tmux session cannot gain a tab");
    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("restart"),
        "the refusal must point at restarting the session, got: {rendered}"
    );
    assert_eq!(
        err.downcast_ref::<SupervisorError>()
            .expect("a refused open must carry a SupervisorError")
            .kind,
        ErrorKind::Conflict,
        "a session in the wrong state for this operation is a conflict, not a missing thing"
    );
}

/// A tab whose shell is already dead by the time the open would reply is
/// a REFUSED open carrying the pane's last words, with the window cleaned
/// up — never a silently "successful" tab holding a corpse (PLAN_M4.md
/// acceptance 4, SPEC.md's every-failed-operation rule).
///
/// The shell seam is what makes this reachable: a tab has no invocation of
/// its own, so the only way to drive its launch into failure is to choose
/// the shell. The fixture prints a recognizable line and exits, which is
/// exactly the shape a broken login shell has.
#[tokio::test]
async fn a_tab_whose_shell_is_dead_by_reply_time_is_refused_with_its_last_words() {
    let dying = tempfile::tempdir().unwrap();
    let shell = dying.path().join("dying-shell");
    std::fs::write(&shell, "#!/bin/sh\necho SHELL-REFUSED-TO-START\nexit 9\n")
        .expect("writing the failing shell fixture");
    std::fs::set_permissions(
        &shell,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .expect("making the failing shell executable");

    let h = harness_with_shell(&shell.to_string_lossy()).await;
    let work = tempfile::tempdir().unwrap();
    // The AGENT's own launch also runs through this shell, so it is given
    // a command it never reaches — this test is about the tab, and the
    // session only has to exist and hold a tmux session.
    let session = h
        .client
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

    let err = h
        .client
        .open_tab(&session.id)
        .await
        .expect_err("a shell that is already dead must refuse the open");
    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("SHELL-REFUSED-TO-START"),
        "the refusal must carry the pane's last words as its detail, got: {rendered}"
    );
    assert!(
        listed_tabs(&h.client, &session.id).await.is_empty(),
        "a refused open must clean its window up rather than leave a dead tab listed"
    );
    let rows = window_rows(&h).await;
    assert!(
        !rows
            .iter()
            .any(|row| row.contains(&format!("fh-{}|", session.id))
                && row.split('|').nth(3).is_some_and(|tab| !tab.is_empty())),
        "no marked tab window may survive a refused open; rows:\n{}",
        rows.join("\n")
    );
}

/// Closing a tab kills its shell AND a deliberately daemonized child of
/// that shell, while the agent terminal and the session's OTHER tab are
/// untouched (PLAN_M4.md acceptance 3).
///
/// The daemonized child is the whole point: it has reparented to init, so
/// no PPID walk from the pane reaches it, and only the tab's own marker
/// scan can. The second tab and the agent are the other half — a close
/// that reaped by the SESSION's marker instead of the tab's would end all
/// three and still look like a pass without them.
#[tokio::test]
async fn closing_a_tab_kills_its_shell_and_daemonized_child_and_nothing_else() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent terminal");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let doomed = h.client.open_tab(&session.id).await.expect("open the tab");
    let survivor = h
        .client
        .open_tab(&session.id)
        .await
        .expect("open a second tab");

    let (doomed_chan, mut doomed_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: doomed.id.clone(),
            },
            "one-client",
        )
        .await
        .expect("attach the doomed tab");
    let mut doomed_seen = Vec::new();
    wait_for_shell(
        &h.client,
        doomed_chan,
        &mut doomed_rx,
        &mut doomed_seen,
        "D",
    )
    .await;

    let (survivor_chan, mut survivor_rx) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: survivor.id.clone(),
            },
            "one-client",
        )
        .await
        .expect("attach the surviving tab");
    let mut survivor_seen = Vec::new();
    wait_for_shell(
        &h.client,
        survivor_chan,
        &mut survivor_rx,
        &mut survivor_seen,
        "S",
    )
    .await;

    let doomed_pid_file = work.path().join("doomed-daemon.pid");
    let script = write_daemon_script(work.path(), "doomed.sh", &doomed_pid_file, false);
    run_in_shell(
        &h.client,
        doomed_chan,
        &mut doomed_rx,
        &mut doomed_seen,
        &format!("sh {} && printf 'SPAWN%sED\\n' N", script.display()),
        "SPAWNNED",
        20,
    )
    .await;
    let daemon_pid = wait_for_pid_file(&doomed_pid_file, 10).await;
    let _daemon_cleanup = PidKillGuard::arm(daemon_pid);
    let doomed_pane_pid = pane_pid_of(&h, &tab_pane(&h, &session.id, &doomed.id).await).await;

    h.client
        .close_tab(&session.id, &doomed.id)
        .await
        .expect("close the tab");

    wait_until_pid_gone(doomed_pane_pid, 15).await;
    wait_until_pid_gone(daemon_pid, 15).await;
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![survivor.id.clone()],
        "closing one tab must leave the session's other tabs listed"
    );
    // The surviving tab and the agent both still answer, which is the
    // only proof their processes were never reaped.
    run_in_shell(
        &h.client,
        survivor_chan,
        &mut survivor_rx,
        &mut survivor_seen,
        "printf 'SURVIV%sR\\n' O",
        "SURVIVOR",
        20,
    )
    .await;
    h.client
        .send_input(agent_chan, b"agent-untouched\r".to_vec())
        .await;
    wait_for(&mut agent_rx, &mut agent_seen, "agent-untouched", 15).await;
}

/// The pid of the process tmux reports for `pane`.
async fn pane_pid_of(h: &Harness, pane: &str) -> u32 {
    let out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &["display-message", "-p", "-t", pane, "#{pane_pid}"],
    )
    .await;
    assert!(
        out.status.success(),
        "querying a pane's pid failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .expect("tmux reports a numeric pane pid")
}

/// The cgroup half of a tab close (PLAN_M4.md item 2), loudly skipped
/// where no user manager exists — M3's pattern, for M3's reason.
///
/// The fixture is the one shape neither half of the marker sweep can
/// reach: double-forked to init (so no PPID walk from the tab's pane
/// finds it) AND environment-scrubbed (so the tab-marker scan cannot see
/// it either). Its death can therefore only have come from the tab's own
/// `systemd-run --scope`, which is the whole claim this test makes.
///
/// A tab's scope is not recorded anywhere — tabs have no durable row — so
/// this also pins that `close_tab` re-derives the same unit name the open
/// created, from the session id and the tab id alone.
#[tokio::test]
async fn closing_a_tab_kills_an_environment_scrubbed_double_fork_through_its_scope() {
    let Some((h, _scopes)) = scope_gated_harness(
        "closing_a_tab_kills_an_environment_scrubbed_double_fork_through_its_scope",
    )
    .await
    else {
        return;
    };
    let (session, work) = basic_session(&h).await;
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

    let pid_file = work.path().join("cloaked-tab.pid");
    let script = write_daemon_script(work.path(), "cloak.sh", &pid_file, true);
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
    let cloaked = wait_for_pid_file(&pid_file, 10).await;
    let _cloaked_cleanup = PidKillGuard::arm(cloaked);
    assert!(
        !marked_pids(&session.id).contains(&cloaked),
        "test setup: the cloaked daemon must NOT carry the session marker — the whole point \
         is that only a cgroup can find it"
    );

    h.client
        .close_tab(&session.id, &tab.id)
        .await
        .expect("close the tab");
    wait_until_pid_gone(cloaked, 15).await;
}

/// Tabs survive a supervisor restart by the same mechanism the agent
/// terminal does — tmux outliving the supervisor — and a window someone
/// conjured behind the supervisor's back is never reported as one
/// (PLAN_M4.md acceptance 2 and 4).
///
/// The unmarked window is not a hypothetical: a pane's own processes
/// inherit `TMUX` and can create windows on the private server, which is
/// exactly why rediscovery is marker-based rather than positional. Here it
/// is created directly against the same socket, which is the same thing
/// from the supervisor's point of view.
///
/// Both tabs are checked, in order, because ordering is the one thing
/// `SessionInfo::tabs` promises beyond identity — and a rediscovery that
/// rebuilt the list from a hash map would pass an identity-only assertion
/// while shuffling the user's tab strip on every poll.
#[tokio::test]
async fn tabs_are_rediscovered_across_a_supervisor_restart_and_unmarked_windows_are_ignored() {
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

    let first = client.open_tab(&session.id).await.expect("open the first");
    let second = client.open_tab(&session.id).await.expect("open the second");

    // A window the supervisor never made, on its own private server.
    let sock = state.path().join("tmux.sock");
    let conjured = tmux_query(
        &sock,
        &[
            "new-window",
            "-d",
            "-P",
            "-F",
            "#{window_id}",
            "-t",
            &format!("=fh-{}:", session.id),
            "--",
            "sleep",
            "300",
        ],
    )
    .await;
    assert!(
        conjured.status.success(),
        "test setup: conjuring an unmarked window must succeed: {}",
        String::from_utf8_lossy(&conjured.stderr)
    );

    assert_eq!(
        listed_tabs(&client, &session.id).await,
        vec![first.id.clone(), second.id.clone()],
        "an unmarked window must never appear as a tab, and tabs list in creation order"
    );

    // Restart the supervisor over the same state directory, tmux and all
    // its windows untouched — the ordinary supervisor-restart shape.
    drop(client);
    drop(sup);
    let sup = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("second supervisor");
    let client = connect_client(&sup).await;

    assert_eq!(
        listed_tabs(&client, &session.id).await,
        vec![first.id.clone(), second.id.clone()],
        "tabs must be rediscovered from their window markers across a supervisor restart, in \
         the same order"
    );

    // Attachable, not merely listed: the ids the rediscovery reported have
    // to be the ones the attach machinery resolves.
    let (chan, mut rx) = client
        .attach_terminal(
            &session.id,
            80,
            24,
            TerminalSelector::Tab {
                id: second.id.clone(),
            },
            "after-restart",
        )
        .await
        .expect("attach a rediscovered tab");
    let mut seen = Vec::new();
    wait_for_shell(&client, chan, &mut rx, &mut seen, "REDISCOVERED").await;
    drop(slot);
}

/// Stopping the agent leaves a tab's shell AND its daemonized child
/// running (PLAN_M4.md acceptance 3, SPEC.md's "terminal tabs keep
/// running").
///
/// This is the marker split's whole reason to exist. Tab processes carry
/// the session marker the stop sweep is keyed on, so a stop that did not
/// subtract them would reap the very terminals SPEC.md promises survive
/// it. The daemonized child is what makes the assertion sharp: the tab's
/// shell might survive by luck of ancestry, but a reparented daemon is
/// reachable ONLY by the marker scan, so its survival is a statement about
/// the marker rule and nothing else.
///
/// The agent's own daemonized child is asserted dead in the same run, so a
/// stop that had simply stopped sweeping at all could not pass.
#[tokio::test]
async fn stopping_the_agent_leaves_a_tabs_shell_and_its_daemonized_child_running() {
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

    let (_agent_chan, mut agent_rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let agent_pid = extract_pid(&agent_seen, "SELF-PID:");
    let agent_daemon = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, mut tab_rx) = h
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
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    let tab_pid_file = work.path().join("tab-daemon.pid");
    let script = write_daemon_script(work.path(), "tab-daemon.sh", &tab_pid_file, false);
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        &format!("sh {} && printf 'SPAWN%sED\\n' N", script.display()),
        "SPAWNNED",
        20,
    )
    .await;
    let tab_daemon = wait_for_pid_file(&tab_pid_file, 10).await;
    let _tab_daemon_cleanup = PidKillGuard::arm(tab_daemon);
    assert!(
        marked_pids(&session.id).contains(&tab_daemon),
        "test setup: the tab's daemon must carry the SESSION marker — surviving without it \
         would prove nothing about the exclusion rule"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    // The agent's side of the sweep still works.
    wait_until_pid_gone(agent_pid, 15).await;
    wait_until_pid_gone(agent_daemon, 15).await;

    // The tab's shell is still answering, and its daemon is still alive.
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        "printf 'AFTER%sTOP\\n' S",
        "AFTERSTOP",
        20,
    )
    .await;
    assert!(
        !process_is_gone(tab_daemon),
        "stop must not reach a tab's daemonized child (pid {tab_daemon})"
    );
}

/// Restarting the agent touches the agent terminal alone: its attachment
/// is detached, while a tab stays attached, keeps answering, and keeps its
/// daemonized child (PLAN_M4.md acceptance 3, SPEC.md's "restart touches
/// the agent terminal only").
///
/// Two independent mechanisms have to hold for this and are asserted
/// together on purpose, because either one failing produces the same
/// user-visible complaint: the detach sweep is scoped to the agent's
/// attachment key, and the restart's pre-relaunch reap subtracts tab
/// processes.
#[tokio::test]
async fn restarting_the_agent_leaves_a_tab_attached_running_and_unswept() {
    let h = harness().await;
    let work = tempfile::tempdir().unwrap();
    let session = h
        .client
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

    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, mut tab_rx) = h
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
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    let pid_file = work.path().join("tab-daemon.pid");
    let script = write_daemon_script(work.path(), "tab-daemon.sh", &pid_file, false);
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        &format!("sh {} && printf 'SPAWN%sED\\n' N", script.display()),
        "SPAWNNED",
        20,
    )
    .await;
    let tab_daemon = wait_for_pid_file(&pid_file, 10).await;
    let _tab_daemon_cleanup = PidKillGuard::arm(tab_daemon);

    let restarted = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("restart");
    assert_eq!(
        restarted
            .tabs
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>(),
        vec![tab.id.clone()],
        "the restart's own reply must still report the tabs the restart did not touch"
    );

    // The AGENT's attachment is gone, and told why.
    let reason = expect_detached(&mut agent_rx, 15).await;
    assert!(
        reason.contains("restart"),
        "the agent's attachment must be detached for the restart, got: {reason:?}"
    );
    let _ = agent_chan;

    // The TAB's attachment survived: still delivering, on the same channel.
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        "printf 'AFTER%sESTART\\n' R",
        "AFTERRESTART",
        20,
    )
    .await;
    assert!(
        !process_is_gone(tab_daemon),
        "a restart's pre-relaunch reap must not reach a tab's daemonized child (pid \
         {tab_daemon})"
    );
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()],
        "a restart must leave the session's tabs listed"
    );
}

/// Deleting a session takes the agent, its tabs, and their daemonized
/// descendants (PLAN_M4.md acceptance 3).
///
/// The other side of the marker split: delete sweeps the session marker
/// INCLUSIVELY, so the exclusion that protects tabs from stop must not
/// leak into this path. A daemonized child of the tab is what proves it
/// reached past the tmux teardown, which would have killed the shell
/// regardless.
#[tokio::test]
async fn deleting_a_session_takes_its_tabs_and_their_daemonized_descendants() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
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

    let pid_file = work.path().join("tab-daemon.pid");
    let script = write_daemon_script(work.path(), "tab-daemon.sh", &pid_file, false);
    run_in_shell(
        &h.client,
        chan,
        &mut rx,
        &mut seen,
        &format!("sh {} && printf 'SPAWN%sED\\n' N", script.display()),
        "SPAWNNED",
        20,
    )
    .await;
    let tab_daemon = wait_for_pid_file(&pid_file, 10).await;
    let _tab_daemon_cleanup = PidKillGuard::arm(tab_daemon);
    let tab_pane_pid = pane_pid_of(&h, &tab_pane(&h, &session.id, &tab.id).await).await;

    h.client.delete_session(&session.id).await.expect("delete");

    wait_until_pid_gone(tab_pane_pid, 15).await;
    wait_until_pid_gone(tab_daemon, 15).await;
    assert!(
        marked_pids(&session.id).is_empty(),
        "delete must leave nothing carrying this session's marker"
    );
}

/// A session whose AGENT runs the same plain login shell a tab does, so
/// the conformance battery below can drive both terminals through one
/// code path instead of two.
///
/// The parameterization is the point: "a tab is the same terminal
/// machinery as the agent pane" is a claim, and the honest way to test a
/// claim of sameness is to run the same program through both and assert
/// the same properties, rather than to write a tab-shaped copy of each
/// agent-shaped test and hope they stayed equivalent.
async fn shell_session(h: &Harness) -> (SessionInfo, tempfile::TempDir) {
    let work = tempfile::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(&work.path().to_string_lossy(), "/bin/sh -i", None, 80, 24)
        .await
        .expect("create a shell session");
    (session, work)
}

/// The `COLSxROWS` tmux reports for the window containing `pane`.
async fn window_geometry(h: &Harness, pane: &str) -> String {
    let out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &[
            "display-message",
            "-p",
            "-t",
            pane,
            "#{window_width}x#{window_height}",
        ],
    )
    .await;
    assert!(
        out.status.success(),
        "querying a window's geometry failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Every terminal-fidelity promise SPEC.md makes, asserted once for the
/// AGENT terminal and once for a TAB of the same session — replay of
/// scrollback across a reattach, alternate-screen selection, pane-mode
/// restoration, and binary-clean live output.
///
/// Parameterized rather than duplicated (PLAN_M4.md's testing decisions):
/// the property under test is that the two are the SAME machinery, and a
/// second tab-shaped copy of each assertion would be free to drift from
/// the agent-shaped one it was supposed to mirror.
///
/// "The same program in both" is made true rather than assumed: the agent
/// is created with `/bin/sh -i` as its invocation AND the tab's own shell
/// is pinned to `/bin/sh` through `launch_shell`, since a tab otherwise
/// launches whatever this host's `$SHELL` resolves to and the two
/// terminals would be running different programs while claiming to
/// demonstrate sameness.
///
/// The battery is deliberately the REPLAY/MODE/BINARY subset — scrollback
/// replay across a reattach, bracketed-paste restoration, alternate-screen
/// selection, and byte-clean live output. Per-window resize and stall
/// scoping are the two conformance properties that are only meaningful
/// BETWEEN terminals rather than within one, and they have tests of their
/// own for that reason.
#[tokio::test]
async fn terminal_conformance_holds_for_the_agent_and_for_a_tab() {
    let h = harness_with_shell("/bin/sh").await;
    let (session, _work) = shell_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let targets = [
        ("agent", TerminalSelector::Agent),
        ("tab", TerminalSelector::Tab { id: tab.id.clone() }),
    ];

    for (label, selector) in targets {
        let (chan, mut rx) = h
            .client
            .attach_terminal(&session.id, 80, 24, selector.clone(), "conformance")
            .await
            .unwrap_or_else(|e| panic!("{label}: attach failed: {e:#}"));
        let mut seen = Vec::new();
        wait_for_shell(&h.client, chan, &mut rx, &mut seen, "READY").await;

        // Binary-clean live output: invalid UTF-8 is legitimate terminal
        // content and must cross the control-mode stream byte-for-byte.
        // Anchored on an adjacent marker rather than on the byte itself,
        // since a lossy conversion would REPLACE the byte while leaving
        // everything around it intact — which is exactly the bug.
        run_in_shell(
            &h.client,
            chan,
            &mut rx,
            &mut seen,
            "printf 'BIN\\377ARY%s\\n' -END",
            "ARY-END",
            20,
        )
        .await;
        assert!(
            seen.contains(&0xff),
            "{label}: live output replaced or dropped an invalid UTF-8 byte"
        );

        // Enough output to push the earliest of it off an 80x24 screen,
        // so the reattach replay below has to come from tmux HISTORY
        // rather than from the visible grid.
        run_in_shell(
            &h.client,
            chan,
            &mut rx,
            &mut seen,
            "i=0; while [ $i -lt 60 ]; do printf 'SCROLL%s-%s\\n' ED $i; i=$((i+1)); done",
            "SCROLLED-59",
            20,
        )
        .await;
        // Bracketed paste, the audited silent-loss mode (SPEC_impl.md):
        // content replay alone passing this test would be the bug.
        h.client
            .send_input(chan, b"printf '\\033[?2004h'\r".to_vec())
            .await;
        run_in_shell(
            &h.client,
            chan,
            &mut rx,
            &mut seen,
            "printf 'MODE%s\\n' -SET",
            "MODE-SET",
            20,
        )
        .await;

        h.client.detach(chan).await;
        let (chan2, mut rx2) = h
            .client
            .attach_terminal(&session.id, 80, 24, selector.clone(), "conformance")
            .await
            .unwrap_or_else(|e| panic!("{label}: reattach failed: {e:#}"));
        let mut replay = Vec::new();
        wait_for(&mut rx2, &mut replay, "SCROLLED-0", 20).await;
        assert!(
            String::from_utf8_lossy(&replay).contains("SCROLLED-59"),
            "{label}: replay lost the tail of the pre-detach history"
        );
        if tmux_has_format(&h, "bracket_paste_flag").await {
            wait_for(&mut rx2, &mut replay, "\x1b[?2004h", 10).await;
        } else {
            eprintln!("tmux lacks bracket_paste_flag; skipping {label} mode restoration");
        }
        // Live after replay, not just replayed: a control-client overlap
        // renders the replay and then never updates, and only fresh
        // output tells a live terminal from a frozen one.
        run_in_shell(
            &h.client,
            chan2,
            &mut rx2,
            &mut replay,
            "printf 'LIVE%sREATTACH\\n' -AFTER-",
            "LIVE-AFTER-REATTACH",
            20,
        )
        .await;

        // Alternate screen: the replay must select the alternate buffer
        // BEFORE prefilling it (the switch clears what it switches to) and
        // must not mix the normal screen's history in.
        run_in_shell(
            &h.client,
            chan2,
            &mut rx2,
            &mut replay,
            "printf '\\033[?1049hALT%sSCREEN\\n' -",
            "ALT-SCREEN",
            20,
        )
        .await;
        h.client.detach(chan2).await;
        let (chan3, mut rx3) = h
            .client
            .attach_terminal(&session.id, 80, 24, selector.clone(), "conformance")
            .await
            .unwrap_or_else(|e| panic!("{label}: alt-screen reattach failed: {e:#}"));
        let mut alt = Vec::new();
        wait_for(&mut rx3, &mut alt, "ALT-SCREEN", 20).await;
        let alt_text = String::from_utf8_lossy(&alt).into_owned();
        assert!(
            alt_text.contains("\x1b[?1049h"),
            "{label}: an alternate-screen pane's replay must select that buffer first"
        );
        assert!(
            !alt_text.contains("SCROLLED-0"),
            "{label}: alternate-screen replay must not mix in the normal screen's history"
        );
        h.client
            .send_input(chan3, b"printf '\\033[?1049l'\r".to_vec())
            .await;
        h.client.detach(chan3).await;
    }
}

/// A resize reflows ONLY the window of the terminal whose channel carried
/// it (PLAN_M4.md item 3: resize goes per window).
///
/// Before tabs, `resize-window` was targeted at the tmux SESSION, which
/// resolves to whichever window tmux last made current — unambiguous with
/// one window and silently wrong with two. This is the test that would
/// have caught that: it resizes each terminal in turn and requires the
/// other's geometry to stay put.
#[tokio::test]
async fn a_resize_reflows_only_the_named_terminals_window() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let agent_pane = pane_id_of(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;
    let tab_pane = tab_pane(&h, &session.id, &tab.id).await;

    let (agent_chan, mut agent_rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
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
            "one-client",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    h.client.resize(&session.id, tab_chan, 111, 33).await;
    wait_for_pane_geometry(&h, &tab_pane, "111x33").await;
    assert_eq!(
        window_geometry(&h, &agent_pane).await,
        "80x24",
        "resizing a tab must not reflow the agent's window"
    );

    h.client.resize(&session.id, agent_chan, 90, 30).await;
    wait_for_pane_geometry(&h, &agent_pane, "90x30").await;
    assert_eq!(
        window_geometry(&h, &tab_pane).await,
        "111x33",
        "resizing the agent terminal must not reflow a tab's window"
    );
}

/// Poll one window's geometry until it reaches `expected`. Resize is
/// fire-and-forget, so polling is the only observation available — the
/// per-window counterpart of `session_lifecycle`'s `wait_for_geometry`.
async fn wait_for_pane_geometry(h: &Harness, pane: &str, expected: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let got = window_geometry(h, pane).await;
        if got == expected {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "window of pane {pane} never reached {expected} (last: {got})"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Input sent on one terminal's channel reaches ONLY that terminal's pane
/// (PLAN_M4.md item 3).
///
/// Both channels belong to one client and one connection, so nothing but
/// the attachment key distinguishes them — which is exactly the thing a
/// regression here would collapse. Each terminal's transcript is checked
/// for the OTHER's marker as well as for its own, because "it arrived"
/// and "it arrived only here" are different claims.
#[tokio::test]
async fn input_reaches_only_the_terminal_it_was_sent_to() {
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
    let (tab_chan, mut tab_rx) = h
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
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    h.client
        .send_input(agent_chan, b"for-the-agent-only\r".to_vec())
        .await;
    wait_for(&mut agent_rx, &mut agent_seen, "for-the-agent-only", 15).await;
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        "printf 'FORTHE%sONLY\\n' TAB",
        "FORTHETABONLY",
        20,
    )
    .await;

    // Settle before asserting an ABSENCE: the other terminal's frames are
    // in flight over the same connection, and a single read could simply
    // have run ahead of them.
    let _ = drain_for(&mut agent_rx, &mut agent_seen, Duration::from_secs(1)).await;
    let _ = drain_for(&mut tab_rx, &mut tab_seen, Duration::from_secs(1)).await;
    assert!(
        !String::from_utf8_lossy(&agent_seen).contains("FORTHETABONLY"),
        "the tab's output leaked into the agent terminal's stream"
    );
    assert!(
        !String::from_utf8_lossy(&tab_seen).contains("for-the-agent-only"),
        "the agent terminal's output leaked into the tab's stream"
    );
}

/// A stalled viewer on one TAB pauses only that tab's stream: the agent
/// terminal keeps flowing throughout (PLAN_M4.md acceptance 5).
///
/// This is the reason per-terminal control clients exist rather than one
/// per session. tmux's `pause-after` flow control is a property of a
/// CONTROL CLIENT, so a client shared across a session's terminals would
/// let one wedged tab viewer pause the agent's stream — the terminal the
/// user is actually looking at.
///
/// The agent is round-tripped repeatedly for longer than
/// `TMUX_PAUSE_AFTER_SECS`, deliberately: the interesting window is the
/// one where tmux's own backstop fires on the stalled client, and a check
/// that finished before that would miss the very interaction under test.
///
/// Honest scope, because a passing run here does not by itself cover both
/// of tmux's answers to a lagging client. tmux either cuts the stalled
/// client with `%pause` or stops reading the pane it is behind on, and it
/// picks between them nondeterministically (see `TMUX_PAUSE_AFTER_SECS`);
/// this test observes whichever branch tmux happened to take on this run,
/// and cannot force the second one. Which branch occurs depends on how far
/// tmux read ahead, and the audit that reproduced the throttle branch at
/// all needed a 16 MB/s producer and still only hit it in four runs out of
/// five (recorded on `SessionSink`); nothing in the control protocol lets
/// a test choose.
///
/// What HAS changed is that the second branch is no longer a residual to
/// warn about. It used to be one — a stalled tab could stop the AGENT's
/// pane being read, blocking the agent's own writes for as long as the
/// stall lasted — and this test's earlier form said so. The session sink
/// closes it by guaranteeing tmux always has a client it can deliver every
/// pane to, and the coverage for that guarantee is the mechanism tests
/// (`only_the_sink_keeps_a_filtered_pane_readable` in `tmux.rs`, and the
/// `the_session_sink_*` lifecycle tests below), not this one. This test's
/// own claim stays what it always was: the supervisor's per-terminal
/// machinery keeps the agent flowing while a tab's viewer is wedged.
#[tokio::test]
async fn a_stalled_tab_viewer_does_not_pause_the_agents_stream() {
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
    let (tab_chan, mut tab_rx) = h
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
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    // Stall the tab's viewer, then give it something enormous to be
    // behind on. Input still flows to a paused terminal — only its OUTPUT
    // is held — which is what lets the producer be started at all.
    h.client.pause_output(tab_chan).await;
    h.client
        .send_input(
            tab_chan,
            b"i=0; while [ $i -lt 200000 ]; do printf 'FLOOD-%s\\n' $i; i=$((i+1)); done\r"
                .to_vec(),
        )
        .await;

    let until = tokio::time::Instant::now() + Duration::from_secs(8);
    let mut round = 0;
    while tokio::time::Instant::now() < until {
        round += 1;
        let marker = format!("agent-alive-{round}");
        h.client
            .send_input(agent_chan, format!("{marker}\r").into_bytes())
            .await;
        wait_for(&mut agent_rx, &mut agent_seen, &marker, 10).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(
        round > 1,
        "the agent must have been round-tripped several times while the tab was stalled"
    );

    // And the stalled tab recovers on resume rather than staying dead —
    // the other half of the isolation claim.
    h.client.resume_output(tab_chan).await;
    wait_for(&mut tab_rx, &mut tab_seen, "FLOOD-", 30).await;
}

/// The session sink's lifecycle: the first attachment on a session brings
/// one up, and the last one to go takes it down.
///
/// The sink is what keeps tmux reading every pane of a session whose
/// terminals are filtered or stalled (`tmux::SessionSink`), and it is
/// owned by refcount rather than by any teardown path — so both ends of
/// this are properties nothing else in the suite would notice breaking. A
/// sink that never started would surface only as the isolation guarantee
/// quietly not holding; a sink that outlived its last viewer would surface
/// only as a control client attached to a session nobody is watching,
/// forever.
///
/// Deliberately checks a session that is attached, detached, and attached
/// AGAIN: the second attach proves the registry's dangling-`Weak` handling
/// (the entry left behind by the dead sink must be replaced, not upgraded
/// into a corpse).
#[tokio::test]
async fn the_session_sink_lives_exactly_as_long_as_the_sessions_attachments() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tmux_name = format!("fh-{}", session.id);

    assert_eq!(
        h.sup.session_sink_pid(&tmux_name).await,
        None,
        "a session nobody has attached to must have no sink"
    );

    // Both terminals attach under ONE lease: the empty legacy lease sweeps
    // every other attachment on the session (`same_lease_client`), which
    // would leave this test with one terminal where it means to have two.
    let (chan, mut rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let first = h
        .sup
        .session_sink_pid(&tmux_name)
        .await
        .expect("the first attachment must bring a sink up");

    // A second terminal of the same session SHARES the sink rather than
    // starting a second one — the sink is per tmux session, not per
    // attachment, and a per-attachment one would put the drain cost back.
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, mut tab_rx) = h
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
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;
    assert_eq!(
        h.sup.session_sink_pid(&tmux_name).await,
        Some(first),
        "a second terminal on the same session must share its sink"
    );

    // One of two detaching is not the last: the sink stays. `Detach` has
    // no reply, so the round trip on the OTHER channel is what makes this
    // an ordered assertion rather than a race — a connection's frames are
    // handled in order, so an echo that came back proves the detach ahead
    // of it was processed.
    h.client.detach(tab_chan).await;
    h.client
        .send_input(chan, b"after-the-tab-detached\r".to_vec())
        .await;
    wait_for(&mut rx, &mut seen, "after-the-tab-detached", 20).await;
    assert_eq!(
        h.sup.session_sink_pid(&tmux_name).await,
        Some(first),
        "a sink must outlive a detach that leaves another terminal attached"
    );

    // The last detach leaves no channel to round-trip on, so this one is
    // polled. It is still an assertion about the sink going away, not
    // about how fast: the deadline is generous and the failure message
    // names the property.
    h.client.detach(chan).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while h.sup.session_sink_pid(&tmux_name).await.is_some() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the last detach must take the sink down"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // The registry forgetting is not the claim — the PROCESS being gone
    // is. A sink whose handle was dropped but whose client survived would
    // pass every check above while leaving an attached tmux client behind
    // for the life of the server, which is precisely the leak the refcount
    // lifecycle exists to prevent.
    await_process_gone(first, "the last detach must kill the sink's client").await;
    assert_eq!(
        attached_control_clients(&h).await,
        0,
        "no control client may remain attached once the session has no terminals"
    );

    let (chan, mut rx) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let second = h
        .sup
        .session_sink_pid(&tmux_name)
        .await
        .expect("a later attach must bring a fresh sink up");
    assert_ne!(
        second, first,
        "the second sink must be a new process, not the registry handing back a dead one"
    );
    h.client.detach(chan).await;
}

/// Wait for `pid` to be gone, or fail saying what was expected of it.
///
/// `/proc/<pid>` rather than `kill -0`: the test never owns these
/// processes (they are the supervisor's children), so a signal-based probe
/// would report on a pid it has no right to signal, and on a machine that
/// recycled the pid it would report on a stranger.
async fn await_process_gone(pid: u32, what: &str) {
    let path = format!("/proc/{pid}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while std::path::Path::new(&path).exists() {
        assert!(tokio::time::Instant::now() < deadline, "{what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// How many control-mode clients are attached to this harness's tmux
/// server right now.
///
/// The sink's contract is about attached CLIENTS, so counting them is the
/// only way to tell "the supervisor forgot its sink" apart from "the sink
/// is really gone" — and the only way to catch a duplicate sink, which no
/// amount of registry inspection would reveal.
async fn attached_control_clients(h: &Harness) -> usize {
    count_control_clients(&h.state.path().join("tmux.sock")).await
}

/// A sink killed out from under a live attachment comes back, and the
/// terminal it was protecting never notices.
///
/// This is the failure the supervising task exists for. A sink is a plain
/// process: it can be OOM-killed, or swept up by something aiming at a
/// tmux client. Without self-healing, the session would keep every
/// appearance of health — terminals attached, output flowing — while
/// silently having lost the pane-read guarantee for the rest of its life,
/// which is exactly the class of degradation nobody discovers until a tab
/// wedges months later.
///
/// `kill -9` rather than a graceful signal on purpose: the sink holds no
/// state worth flushing, and the ungraceful case is the one where the
/// supervising task's own bookkeeping (its client handle, its pid
/// publication) is most likely to be left inconsistent.
///
/// Killed UNDER LOAD, with a tab flooding throughout, because that is the
/// only configuration where the respawn window has anything to lose: a
/// busy pane every terminal client has filtered off is exactly the pane
/// the sink is keeping readable, so if a respawn left the session without
/// one for good, this is the shape that would show it — as a tab that
/// went quiet and never came back.
#[tokio::test]
async fn a_killed_session_sink_comes_back_while_its_terminals_stay_attached() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tmux_name = format!("fh-{}", session.id);

    let (chan, mut rx) = h
        .client
        .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // A second terminal, kept busy for the whole test: its pane is
    // filtered off on the agent's client, so it is the sink that keeps
    // tmux reading it.
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, mut tab_rx) = h
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
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;
    h.client
        .send_input(
            tab_chan,
            b"i=0; while [ $i -lt 200000 ]; do printf 'HEAL-FLOOD-%s\\n' $i; i=$((i+1)); done\r"
                .to_vec(),
        )
        .await;
    wait_for(&mut tab_rx, &mut tab_seen, "HEAL-FLOOD-", 20).await;

    let doomed = h
        .sup
        .session_sink_pid(&tmux_name)
        .await
        .expect("an attached session must have a sink");
    kill_verified_tmux_client(doomed, &h).await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let healed = loop {
        if let Some(pid) = h.sup.session_sink_pid(&tmux_name).await
            && pid != doomed
        {
            break pid;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the sink never came back after being killed"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    await_process_gone(doomed, "the killed sink's process must be gone").await;
    assert!(
        std::path::Path::new(&format!("/proc/{healed}")).exists(),
        "the replacement sink must be a live process"
    );

    // The attachments must have been undisturbed throughout — the sink is
    // invisible infrastructure, and a self-heal that cost the user their
    // terminal would be no better than the failure.
    h.client
        .send_input(chan, b"after-the-sink-died\r".to_vec())
        .await;
    wait_for(&mut rx, &mut seen, "after-the-sink-died", 20).await;
    // And the busy tab is still being read: its flood kept arriving
    // across the respawn rather than stopping at the moment the sink died.
    let before = tab_seen.len();
    tab_seen.clear();
    wait_for(&mut tab_rx, &mut tab_seen, "HEAL-FLOOD-", 30).await;
    assert!(
        before > 0,
        "test premise: the tab must have been flooding before the sink was killed"
    );
}

/// `kill -9` a pid, having first confirmed it really is a tmux client on
/// THIS harness's socket.
///
/// A bare check-then-kill on a pid read from somewhere else is how a test
/// eventually kills an unrelated process: the supervisor could replace its
/// sink between the read and the signal, the pid could be recycled, and on
/// a loaded machine both are more than theoretical. Reading
/// `/proc/<pid>/cmdline` and requiring this harness's own socket path in
/// it makes the signal safe to send — the socket is a per-test temporary
/// directory, so nothing outside this test can match.
async fn kill_verified_tmux_client(pid: u32, h: &Harness) {
    let sock = h.state.path().join("tmux.sock");
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline"))
        .unwrap_or_else(|e| panic!("reading /proc/{pid}/cmdline: {e}"));
    let cmdline = String::from_utf8_lossy(&cmdline).replace('\0', " ");
    assert!(
        cmdline.contains(&sock.to_string_lossy().to_string()),
        "refusing to kill pid {pid}: its command line ({cmdline:?}) does not name this \
         harness's tmux socket"
    );
    let killed = tokio::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .await
        .expect("running kill");
    assert!(
        killed.success(),
        "test setup: killing pid {pid} must succeed"
    );
}

/// Two terminals of one session attaching CONCURRENTLY converge on one
/// sink, and leave no second client behind.
///
/// `ensure_session_sink` deliberately does not hold its registry lock
/// across the client spawn — that would serialize a process spawn against
/// every other session's first attach — so two first-attaches really can
/// both reach the spawn. What the design promises instead is that the
/// loser's client is dropped and killed on the spot rather than left as an
/// attached tmux client nothing owns, which would be an invisible leak
/// with a real cost: an extra copy of the session's entire output stream,
/// forever.
///
/// Counting attached CLIENTS is what makes that observable. Per attached
/// terminal the supervisor runs exactly two (output and input), plus one
/// sink for the session — so the total is the arithmetic below, and an
/// orphaned loser shows up as one more.
#[tokio::test]
async fn concurrent_first_attaches_share_one_sink_and_orphan_nothing() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tmux_name = format!("fh-{}", session.id);
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");

    // Both attaches are in flight before either completes, which is the
    // only way to reach the double-checked path at all.
    let agent =
        h.client
            .attach_terminal(&session.id, 80, 24, TerminalSelector::Agent, "one-client");
    let tab_attach = h.client.attach_terminal(
        &session.id,
        80,
        24,
        TerminalSelector::Tab { id: tab.id.clone() },
        "one-client",
    );
    let (agent, tab_attach) = tokio::join!(agent, tab_attach);
    let (agent_chan, mut agent_rx) = agent.expect("attach the agent");
    let (tab_chan, mut tab_rx) = tab_attach.expect("attach the tab");
    let mut agent_seen = Vec::new();
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let mut tab_seen = Vec::new();
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    let pid = h
        .sup
        .session_sink_pid(&tmux_name)
        .await
        .expect("the session must have a sink");
    assert_eq!(
        attached_control_clients(&h).await,
        5,
        "expected two clients per attached terminal plus exactly one sink (pid {pid}); a \
         different count means a losing first-attach left its client attached"
    );

    h.client.detach(tab_chan).await;
    h.client.detach(agent_chan).await;
}

/// A supervisor killed outright leaves no orphaned sink client behind,
/// and its replacement brings up exactly one fresh sink.
///
/// The sink's teardown is `kill_on_drop`, which a `SIGKILL`ed supervisor
/// never gets to run — so "the owner dies" is precisely the case that
/// mechanism cannot cover, and the one where an orphan would be silent
/// and permanent: an attached control client draining a session's entire
/// output stream into a dead process's pipe, for as long as the tmux
/// server lives, invisible to every later supervisor.
///
/// What saves it is not a sweep but the protocol: tmux control mode ends
/// at stdin EOF, and the dead supervisor's end of that pipe closes when
/// the kernel reaps it. This test is what pins that reasoning to
/// observed behavior rather than leaving it as an argument — and what
/// would fail loudly if a future change gave the sink a stdin it holds
/// open some other way.
///
/// Runs the supervisor as a real child process, because an in-process one
/// cannot be `SIGKILL`ed without taking the test with it.
#[tokio::test]
async fn a_killed_supervisor_leaves_no_orphaned_sink_client() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let work = tempfile::tempdir().expect("workdir");
    let sock = state.path().join("tmux.sock");
    let _tmux = TmuxServerGuard(sock.clone());

    let mut supervisor = tokio::process::Command::new(farhelm_bin())
        .args(["supervisor", "run", "--state-dir"])
        .arg(state.path())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn the supervisor process");
    wait_for_socket(&state.path().join("supervisor.sock")).await;

    let session = {
        let client = connect_over_socket(state.path()).await;
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
        let (chan, mut rx) = client.attach(&session.id, 80, 24).await.expect("attach");
        let mut seen = Vec::new();
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
        // One attached terminal: its output client, its input client, and
        // the session's sink.
        assert_eq!(
            count_control_clients(&sock).await,
            3,
            "test premise: an attached terminal must have brought a sink up"
        );
        let _ = chan;
        session
    };
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    // The whole point: no graceful shutdown, no `Drop`, no chance to kill
    // anything it owns.
    supervisor.start_kill().expect("kill the supervisor");
    let _ = supervisor.wait().await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while count_control_clients(&sock).await > 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "a control client outlived the supervisor that owned it: {} still attached",
            count_control_clients(&sock).await
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // A replacement supervisor adopts the surviving tmux session, and its
    // first attach must produce exactly one sink — not a second one
    // alongside something left over.
    let mut replacement = tokio::process::Command::new(farhelm_bin())
        .args(["supervisor", "run", "--state-dir"])
        .arg(state.path())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn the replacement supervisor");
    wait_for_socket(&state.path().join("supervisor.sock")).await;
    {
        let client = connect_over_socket(state.path()).await;
        let (_chan, mut rx) = client
            .attach(&session.id, 80, 24)
            .await
            .expect("reattach through the replacement supervisor");
        let mut seen = Vec::new();
        wait_for(&mut rx, &mut seen, "FAKE-AGENT", 20).await;
        assert_eq!(
            count_control_clients(&sock).await,
            3,
            "the replacement's attach must leave exactly one sink attached"
        );
    }
    replacement.start_kill().expect("kill the replacement");
    let _ = replacement.wait().await;
}

/// A client talking to an out-of-process supervisor over its unix socket,
/// retrying until it is actually accepting.
///
/// The in-process [`connect_client`] cannot be used by tests that need a
/// supervisor they can kill, which is why this exists rather than being a
/// second way to do the same thing. Retrying rather than dialling once
/// because a socket FILE is not a listener: a killed supervisor leaves its
/// socket path behind, so a replacement's file exists (and
/// [`wait_for_socket`] is satisfied) well before the replacement has
/// unlinked it, bound, and begun accepting.
async fn connect_over_socket(state_dir: &std::path::Path) -> Arc<SupervisorClient> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        match farhelm_supervisor::service::connect(state_dir).await {
            Ok(stream) => {
                let (r, w) = tokio::io::split(stream);
                return SupervisorClient::start(r, w).await.expect("handshake");
            }
            Err(e) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "the supervisor never began accepting: {e:#}"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

/// How many control-mode clients are attached to the tmux server on
/// `sock` — see [`attached_control_clients`], which is the same question
/// asked of a [`Harness`].
async fn count_control_clients(sock: &std::path::Path) -> usize {
    let listed = tmux_query(sock, &["list-clients", "-F", "#{client_flags}"]).await;
    if !listed.status.success() {
        return 0;
    }
    String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

/// The sink registry does not grow with the number of sessions that have
/// ever been attached to.
///
/// Entries are `Weak`, so a dead one is harmless to behavior and invisible
/// to every other test — which is exactly what makes an unbounded map here
/// the kind of leak that ships. A supervisor serving short-lived sessions
/// all day would accumulate one dead key per session id, forever.
#[tokio::test]
async fn the_sink_registry_does_not_grow_with_dead_sessions() {
    let h = harness().await;
    for _ in 0..4 {
        let (session, _work) = basic_session(&h).await;
        let _cleanup = MarkerCleanupGuard::new(session.id.clone());
        let tmux_name = format!("fh-{}", session.id);
        let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
        let mut seen = Vec::new();
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
        h.client.detach(chan).await;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        while h.sup.session_sink_pid(&tmux_name).await.is_some() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "a detached session must lose its sink"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        h.client
            .delete_session(&session.id)
            .await
            .expect("delete the session");
    }
    // Pruning is opportunistic, so the bound is "does not accumulate one
    // per session", not "is exactly zero at every instant": the last
    // session's entry may still be present until the next lookup sweeps
    // it.
    let registered = h.sup.session_sink_registry_len().await;
    assert!(
        registered <= 1,
        "the sink registry kept {registered} entries after four sessions came and went"
    );
}

/// A tab's launch evaluates the environment at OPEN time, so an rc-file
/// change made between two opens is visible to the second (SPEC.md's
/// environment contract, extended to tabs by PLAN_M4.md item 2's
/// same-interactive-login-contract rule).
///
/// The agent-side version of this promise is already pinned
/// (`an_rc_file_change_between_launches_reaches_the_relaunched_agent`);
/// this is the half that would silently break if a tab ever resolved its
/// shell once and cached it, or launched through anything other than an
/// interactive login shell.
///
/// The rc files live in a private HOME injected through
/// `SupervisorSeams::launch_env` — never by mutating this process's
/// environment, which this repo forbids and which every concurrently
/// running harness would share anyway. A host whose login shell reads
/// none of the files [`write_rc_files`] knows how to write is an honest,
/// loud skip rather than a silent pass.
#[tokio::test]
async fn an_rc_file_change_between_two_tab_opens_reaches_the_second_tab() {
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
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    /// Open a tab, attach it, and ask its shell what the marker holds.
    async fn tab_marker_value(h: &Harness, session_id: &str, ready: &str) -> String {
        let tab = h.client.open_tab(session_id).await.expect("open a tab");
        let (chan, mut rx) = h
            .client
            .attach_terminal(
                session_id,
                80,
                24,
                TerminalSelector::Tab { id: tab.id.clone() },
                "rc-lease",
            )
            .await
            .expect("attach the tab");
        let mut seen = Vec::new();
        wait_for_shell(&h.client, chan, &mut rx, &mut seen, ready).await;
        run_in_shell(
            &h.client,
            chan,
            &mut rx,
            &mut seen,
            &format!("printf 'EN%s[%s]\\n' V \"${RC_MARKER_VAR}\""),
            "ENV[",
            20,
        )
        .await;
        h.client.detach(chan).await;
        let text = String::from_utf8_lossy(&seen).into_owned();
        text.split("ENV[")
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    let first = tab_marker_value(&h, &session.id, "ONE").await;
    if first != "first" {
        // Deterministic, not a shrug — same reasoning as the agent-side
        // test: for every shell family `write_rc_files` covers, the value
        // MUST have arrived, so anything else is a host this harness
        // genuinely cannot reach, named so the gap is diagnosable.
        let shell = farhelm_supervisor::launch::resolve_shell().await;
        let family = std::path::Path::new(&shell)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| shell.clone());
        assert!(
            !["bash", "zsh", "sh", "dash", "ksh"].contains(&family.as_str()),
            "tabs launch through {shell}, which sources one of the rc files this test writes, \
             so the first tab should have seen the value; it reported {first:?} instead"
        );
        eprintln!(
            "SKIPPED an_rc_file_change_between_two_tab_opens_reaches_the_second_tab: this host \
             launches tabs through {shell}, which sources none of the rc files this test knows \
             how to write"
        );
        return;
    }

    write_rc_files(home.path(), "second");
    assert_eq!(
        tab_marker_value(&h, &session.id, "TWO").await,
        "second",
        "a tab opened after an rc-file edit must see the edit — the environment is evaluated \
         at each launch, not resolved once per supervisor"
    );
}
