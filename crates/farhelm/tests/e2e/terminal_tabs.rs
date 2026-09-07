//! Terminal tabs: the launch contract, refusals, close's reap,
//! rediscovery from a window marker, and the per-terminal properties
//! that only become observable once a session has a second terminal.

use crate::harness::*;

use crate::boot_id_durable_outcome::listed;
use crate::restart_with_resume::{RC_MARKER_VAR, write_rc_files};
use crate::terminal_backpressure::drain_for;
use std::sync::Arc;

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
/// Used to give the agent terminal a plain shell so the conformance tests
/// can drive BOTH terminals identically. The dead-at-open-reply refusal
/// needs this same seam pointed at a shell that exits immediately, but it
/// must ALSO hold the settle boundary, so it assembles its seams itself
/// rather than growing a second parameter here.
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

/// The shape both pause hooks below share, spelled once so the constructor's
/// signature stays readable; identical to the supervisor's own alias for
/// these seams.
type PauseHook =
    Arc<dyn Fn() -> Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>;

/// One of the supervisor's pause hooks, plus the two notifications that
/// drive it: `entered` fires when the supervisor reaches the boundary, and
/// the hook returns once `release` is notified.
///
/// The two hooks this file installs share one shape — a zero-argument
/// `Fn() -> BoxFuture<()>` (the supervisor's `ForwarderCleanupGate` and
/// `TabSettleGate` are both that alias) — so one constructor serves both,
/// and the seam FIELD a test installs it in is what names the boundary
/// being held. The staged hooks (`ArchiveGate` takes a stage and answers
/// with a `Result`) are a different shape and are not served here. The
/// two boundaries in this file:
/// a forwarder's cleanup publication (its deferred-publication interval is
/// only a scheduler race, and finalization must be provable while the output
/// client is already safe but its per-terminal barrier still reads as
/// pending), and a tab open's dead-shell settle (see
/// [`a_tab_whose_shell_is_dead_by_reply_time_is_refused_with_its_last_words`]).
///
/// `notify_one` before anyone waits still stores a permit, so a test that
/// reaches its `notified()` after the supervisor has already entered the
/// gate is not left waiting for a notification that has been and gone.
fn notifying_gate() -> (
    PauseHook,
    Arc<tokio::sync::Notify>,
    Arc<tokio::sync::Notify>,
) {
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let gate_entered = Arc::clone(&entered);
    let gate_release = Arc::clone(&release);
    let gate = Arc::new(move || {
        let entered = Arc::clone(&gate_entered);
        let release = Arc::clone(&gate_release);
        let future: Pin<Box<dyn std::future::Future<Output = ()> + Send>> = Box::pin(async move {
            entered.notify_one();
            release.notified().await;
        });
        future
    });
    (gate, entered, release)
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
///
/// The two nested waits carry deliberately UNEQUAL budgets. `wait_for`
/// panics at its own deadline, so whenever that deadline can arrive first
/// it destroys the retry it was nested inside: the panic escapes the
/// `timeout` that exists to turn a silent round into another send, and one
/// slow first round fails the test with most of the budget unspent (seen
/// 2026-08-18 as "timed out waiting for XREADYX" with an empty transcript,
/// 27 of 30 seconds left). So each round's inner wait is given a deadline
/// longer than the round — the whole budget, for lack of a better number —
/// purely so its panic can never race the wrapper's `timeout`: the wrapper
/// alone ends a round, and the explicit deadline assertion below alone
/// ends the retries.
pub(crate) async fn wait_for_shell(
    client: &SupervisorClient,
    channel: u32,
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    marker: &str,
) {
    // How long one send is given to produce an answer before the shell is
    // assumed to have discarded it, and how long the retries as a whole
    // get. Only the ratio matters: the round must be the shorter one.
    const ROUND: Duration = Duration::from_secs(3);
    const BUDGET: Duration = Duration::from_secs(30);

    let started = tokio::time::Instant::now();
    let deadline = started + BUDGET;
    let command = format!("printf 'X%sX\\n' {marker}\r");
    let expected = format!("X{marker}X");
    let mut rounds = 0_u32;
    loop {
        client
            .send_input(channel, command.clone().into_bytes())
            .await;
        rounds += 1;
        let waited =
            tokio::time::timeout(ROUND, wait_for(rx, seen, &expected, BUDGET.as_secs())).await;
        if waited.is_ok() {
            return;
        }
        // Distinguishes a shell that never spoke from one that spoke but
        // never answered: the first is a dead or never-started shell, the
        // second is a genuinely overrun budget.
        assert!(
            tokio::time::Instant::now() < deadline,
            "shell on channel {channel} never answered a round trip: {rounds} sends over {:?}, \
             {} bytes received; transcript so far:\n{}",
            started.elapsed(),
            seen.len(),
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

/// Block until tmux ITSELF reports a dead pane on a marked tab window of
/// this session.
///
/// The barrier behind the dead-at-open-reply refusal test: `pane_dead` is
/// the exact fact the supervisor's settle is polling for, so waiting on it
/// here — from outside, while the open is held at the settle boundary —
/// turns "the fixture shell dies before the settle gives up" from a bet the
/// test places into a precondition it has already observed. Reading it from
/// tmux rather than from the supervisor is deliberate for the same reason
/// [`window_rows`] does: the tmux server is the authority, and the
/// supervisor's view of it is the thing under test.
///
/// Polled rather than awaited because tmux offers no event for it, but the
/// poll is not a sprint: nothing is asserted until the fact holds, and the
/// budget exists only so a shell that never dies fails by name instead of
/// hanging the suite.
async fn wait_for_dead_tab_pane(h: &Harness, session_id: &str) {
    let sock = h.state.path().join("tmux.sock");
    let want_session = format!("fh-{session_id}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let out = tmux_query(
            &sock,
            &[
                "list-panes",
                "-a",
                "-F",
                "#{session_name}|#{@farhelm-tab}|#{pane_dead}",
            ],
        )
        .await;
        assert!(
            out.status.success(),
            "listing panes failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let listing = String::from_utf8_lossy(&out.stdout);
        let rows = listing.lines().collect::<Vec<&str>>();
        let dead = rows.iter().any(|row| {
            let mut fields = row.split('|');
            matches!(
                (fields.next(), fields.next(), fields.next()),
                (Some(session), Some(tab), Some("1")) if session == want_session && !tab.is_empty()
            )
        });
        if dead {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no marked tab pane of session {session_id} ever became dead; rows:\n{}",
            rows.join("\n")
        );
        // 100ms like the suite's other dead-pane waits: each poll is a tmux
        // process, and the gate means nothing is gained by noticing faster.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
async fn opening_a_tab_after_the_working_directory_vanished_names_it() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().unwrap();
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

    let (_chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
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
#[farhelm_testtrace::test]
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
///
/// The settle gate is what makes it DETERMINISTIC, and it earns its keep:
/// "dead by reply time" was previously a bet on the fixture shell dying
/// inside the supervisor's bounded settle, and under load the shell can
/// lose that race — with three e2e binaries pinned to this module on a
/// four-core box (2026-08-18) the open returned a live tab in roughly half
/// the runs. What the test means to pin has nothing to do with which of
/// the two wins: it is what the supervisor does about a shell that IS
/// already dead. So the open is held at the boundary just before the
/// settle, the shell's death is observed in tmux first, and only then is
/// the settle allowed to look. The bounded settle's own timing is left
/// entirely alone — widening it would have hidden the race rather than
/// removed it, and is production behavior besides.
#[farhelm_testtrace::test]
async fn a_tab_whose_shell_is_dead_by_reply_time_is_refused_with_its_last_words() {
    let dying = farhelm_teststate::tempdir().unwrap();
    let shell = dying.path().join("dying-shell");
    std::fs::write(&shell, "#!/bin/sh\necho SHELL-REFUSED-TO-START\nexit 9\n")
        .expect("writing the failing shell fixture");
    std::fs::set_permissions(
        &shell,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .expect("making the failing shell executable");

    let (settle_gate, settle_entered, settle_release) = notifying_gate();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_shell: Some(shell.to_string_lossy().into_owned()),
            tab_settle_gate: Some(settle_gate),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().unwrap();
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

    // The open and its barrier run as one: `open_tab` parks on the gate
    // once the window exists and is marked, the other half waits for tmux
    // to report that window's pane dead, and only then is the open let
    // through to its settle.
    let (opened, ()) = tokio::join!(h.client.open_tab(&session.id), async {
        // Bounded so that an open which fails BEFORE the gate — a broken
        // fixture, a refused precondition — fails by name here instead of
        // parking the test on a notification that is never coming.
        tokio::time::timeout(Duration::from_secs(60), settle_entered.notified())
            .await
            .expect("the open never reached the dead-shell settle; it failed before the gate");
        wait_for_dead_tab_pane(&h, &session.id).await;
        settle_release.notify_one();
    });
    let err = opened.expect_err("a shell that is already dead must refuse the open");
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
#[farhelm_testtrace::test]
async fn closing_a_tab_kills_its_shell_and_daemonized_child_and_nothing_else() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let (agent_chan, initial_replay, mut agent_rx) = h
        .client
        .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent terminal");
    let mut agent_seen = initial_replay;
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let doomed = h.client.open_tab(&session.id).await.expect("open the tab");
    let survivor = h
        .client
        .open_tab(&session.id)
        .await
        .expect("open a second tab");

    let (doomed_chan, initial_replay, mut doomed_rx) = h
        .client
        .attach_terminal_live(
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
    let mut doomed_seen = initial_replay;
    wait_for_shell(
        &h.client,
        doomed_chan,
        &mut doomed_rx,
        &mut doomed_seen,
        "D",
    )
    .await;

    let (survivor_chan, initial_replay, mut survivor_rx) = h
        .client
        .attach_terminal_live(
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
    let mut survivor_seen = initial_replay;
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

/// A closed tab tells its viewer the irreversible truth even when output
/// cleanup is still being finalized.
///
/// Once tmux has killed the window, retrying close can only report that the
/// tab is missing. The first request must therefore send `Detached` before it
/// returns the pending-cleanup error, and the removed tab must stay absent
/// while the runtime-owned barrier completes.
#[farhelm_testtrace::test]
async fn close_notifies_and_removes_a_tab_while_output_cleanup_is_pending() {
    let (cleanup_gate, cleanup_entered, cleanup_release) = notifying_gate();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_shell: Some("/bin/sh".to_string()),
            forwarder_cleanup_gate: Some(cleanup_gate),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (channel, initial_replay, mut rx) = h
        .client
        .attach_terminal_live(
            &session.id,
            80,
            24,
            TerminalSelector::Tab { id: tab.id.clone() },
            "close-cleanup-owner",
        )
        .await
        .expect("attach the tab");
    let mut seen = initial_replay;
    wait_for_shell(&h.client, channel, &mut rx, &mut seen, "CLOSE-READY").await;

    let error = h
        .client
        .close_tab(&session.id, &tab.id)
        .await
        .expect_err("the close must report its still-published cleanup barrier");
    tokio::time::timeout(Duration::from_secs(10), cleanup_entered.notified())
        .await
        .expect("the forwarder cleanup result reaches its publication gate");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("terminal-output client is still being cleaned up"),
        "the close must name its pending output cleanup: {rendered}"
    );
    let reason = expect_detached(&mut rx, 10).await;
    assert!(
        reason.contains("terminal tab closed"),
        "the viewer must hear the final tab verdict despite cleanup delay: {reason}"
    );
    assert!(
        listed_tabs(&h.client, &session.id).await.is_empty(),
        "the already-killed tab must remain absent after cleanup reports pending"
    );

    cleanup_release.notify_one();
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
async fn tabs_are_rediscovered_across_a_supervisor_restart_and_unmarked_windows_are_ignored() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("tempdir");
    let work = farhelm_teststate::tempdir().expect("workdir");
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
    // This second supervisor is the one that attaches a rediscovered tab
    // below, so — unlike the first supervisor above, which only lists and
    // never attaches — it needs the suite's loaded-CI tmux floors.
    let sup =
        Supervisor::new_with_exe_and_timeouts(state.path(), farhelm_bin().into(), suite_timeouts())
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
    let (chan, initial_replay, mut rx) = client
        .attach_terminal_live(
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
    let mut seen = initial_replay;
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
#[farhelm_testtrace::test]
async fn stopping_the_agent_leaves_a_tabs_shell_and_its_daemonized_child_running() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().unwrap();
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

    let (_agent_chan, initial_replay, mut agent_rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut agent_seen = initial_replay;
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let agent_pid = extract_pid(&agent_seen, "SELF-PID:");
    let agent_daemon = wait_for_pid_file(&work.path().join("reparented.pid"), 10).await;

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, initial_replay, mut tab_rx) = h
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
    let mut tab_seen = initial_replay;
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
#[farhelm_testtrace::test]
async fn restarting_the_agent_leaves_a_tab_attached_running_and_unswept() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().unwrap();
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

    let (agent_chan, initial_replay, mut agent_rx) = h
        .client
        .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = initial_replay;
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, initial_replay, mut tab_rx) = h
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
    let mut tab_seen = initial_replay;
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

/// A restart whose terminal cleanup is deferred restores the stopped session.
///
/// The restart has already opened a durable generation and temporarily
/// unpublished the session when it discovers the output barrier. That failure
/// must take the normal rollback path: the viewer hears why it detached, the
/// row returns with its stopped outcome, and it is not stranded as an unknown
/// launch until the supervisor itself restarts.
#[farhelm_testtrace::test]
async fn restart_restores_and_notifies_while_output_cleanup_is_pending() {
    let (cleanup_gate, cleanup_entered, cleanup_release) = notifying_gate();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            forwarder_cleanup_gate: Some(cleanup_gate),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let (channel, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    let error = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect_err("pending terminal cleanup must fail this restart definitively");
    tokio::time::timeout(Duration::from_secs(10), cleanup_entered.notified())
        .await
        .expect("the forwarder cleanup result reaches its publication gate");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("terminal-output client is still being cleaned up"),
        "the restart must preserve the cleanup cause: {rendered}"
    );
    let reason = expect_detached(&mut rx, 10).await;
    assert!(
        reason.contains("session restarted"),
        "the removed viewer must receive the restart verdict: {reason}"
    );
    let restored = listed(&h.client, &session.id).await;
    assert!(
        matches!(restored.status, SessionStatus::Exited { .. }),
        "the failed relaunch must restore a stopped outcome, not leave Launching: {restored:?}"
    );
    assert_eq!(
        restored.annotation.as_deref(),
        Some("stopped by user"),
        "the failed relaunch must republish the outcome created by its completed stop"
    );

    let _ = channel;
    cleanup_release.notify_one();
}

/// Deleting a session takes the agent, its tabs, and their daemonized
/// descendants (PLAN_M4.md acceptance 3).
///
/// The other side of the marker split: delete sweeps the session marker
/// INCLUSIVELY, so the exclusion that protects tabs from stop must not
/// leak into this path. A daemonized child of the tab is what proves it
/// reached past the tmux teardown, which would have killed the shell
/// regardless.
#[farhelm_testtrace::test]
async fn deleting_a_session_takes_its_tabs_and_their_daemonized_descendants() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
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
async fn shell_session(h: &Harness) -> (SessionInfo, farhelm_teststate::TestDir) {
    let work = farhelm_teststate::tempdir().expect("workdir");
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

/// Capture tmux's own view after a terminal attach unexpectedly fails.
///
/// This runs only on the failure path. Keeping the probes out of the successful
/// path matters here: the test is chasing a timing-dependent disappearance, so
/// routine diagnostics must not add enough scheduling work to hide it. The
/// output distinguishes a dead server from a missing window and from a pane
/// that exited while its window marker remained.
async fn tmux_state_after_attach_failure(h: &Harness) -> String {
    let socket = h.state.path().join("tmux.sock");
    let sessions = bounded_tmux_diagnostic(
        "list-sessions",
        &socket,
        &[
            "list-sessions",
            "-F",
            "#{session_name}|windows=#{session_windows}|attached=#{session_attached}",
        ],
    )
    .await;
    let panes = bounded_tmux_diagnostic(
        "list-panes",
        &socket,
        &[
            "list-panes",
            "-a",
            "-F",
            "#{session_name}|#{window_id}|#{pane_id}|dead=#{pane_dead}|status=#{pane_dead_status}|command=#{pane_current_command}|agent=#{@farhelm-agent}|tab=#{@farhelm-tab}",
        ],
    )
    .await;
    let clients = bounded_tmux_diagnostic(
        "list-clients",
        &socket,
        &[
            "list-clients",
            "-F",
            "pid=#{client_pid}|session=#{session_name}|flags=#{client_flags}",
        ],
    )
    .await;

    format!("{sessions}\n{panes}\n{clients}")
}

/// Run one failure-only tmux probe with a hard process-lifetime bound.
///
/// These diagnostics execute when the private server may already be wedged.
/// An unbounded probe would replace the useful attach failure with a hung test
/// and leave its child behind when the harness times out.
async fn bounded_tmux_diagnostic(label: &str, socket: &std::path::Path, args: &[&str]) -> String {
    let mut command = tokio::process::Command::new("tmux");
    command.arg("-S").arg(socket).args(args).kill_on_drop(true);
    match tokio::time::timeout(Duration::from_secs(2), command.output()).await {
        Ok(Ok(output)) => format!(
            "{label} status={:?}\nstdout:\n{}stderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        Ok(Err(error)) => format!("{label} could not start or finish: {error}"),
        Err(_) => format!("{label} timed out after 2s; its child was killed"),
    }
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
#[farhelm_testtrace::test]
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
        let attached = h
            .client
            .attach_terminal_live(&session.id, 80, 24, selector.clone(), "conformance")
            .await;
        let (chan, mut seen, mut rx) = match attached {
            Ok(attached) => attached,
            Err(error) => {
                let tmux_state = tmux_state_after_attach_failure(&h).await;
                panic!("{label}: attach failed: {error:#}\ntmux state:\n{tmux_state}");
            }
        };
        // `attach_terminal_live` has separated the initial snapshot, so
        // bytes appended after this offset are the only candidates for the
        // binary-clean live-output witness below.
        let live_from = seen.len();
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
            seen[live_from..].contains(&0xff),
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
        let (chan2, mut replay, mut rx2) = h
            .client
            .attach_terminal_live(&session.id, 80, 24, selector.clone(), "conformance")
            .await
            .unwrap_or_else(|e| panic!("{label}: reattach failed: {e:#}"));
        let replay_text = String::from_utf8_lossy(&replay);
        assert!(
            replay_text.contains("SCROLLED-0"),
            "{label}: replay lost the head of the pre-detach history"
        );
        assert!(
            replay_text.contains("SCROLLED-59"),
            "{label}: replay lost the tail of the pre-detach history"
        );
        if tmux_has_format(&h, "bracket_paste_flag").await {
            assert!(
                replay_text.contains("\x1b[?2004h"),
                "{label}: replay lost bracketed-paste mode"
            );
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
        let (chan3, alt, _rx3) = h
            .client
            .attach_terminal_live(&session.id, 80, 24, selector.clone(), "conformance")
            .await
            .unwrap_or_else(|e| panic!("{label}: alt-screen reattach failed: {e:#}"));
        let alt_text = String::from_utf8_lossy(&alt).into_owned();
        assert!(
            alt_text.contains("ALT-SCREEN"),
            "{label}: alternate-screen replay lost its visible content"
        );
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
        // Deliberately do not wait for the echo or mode change. A user may
        // close a terminal while output is queued, and tmux 3.7b used to abort
        // the whole private server when Farhelm detached the control client
        // before tmux had discarded that queue. The next attach makes that
        // server loss observable: the next target does it after the agent leg,
        // and the probe below does it after the final tab leg.
        h.client.detach(chan3).await;
    }

    // This probe is intentionally left at the raw boundary: the preceding
    // detach exercised the queued-output edge, and this reply observes that
    // the tmux server survived it before a replay wait can obscure that.
    let (_probe_channel, _probe_rx) = h
        .client
        .attach_terminal_at_boundary(&session.id, 80, 24, TerminalSelector::Agent, "conformance")
        .await
        .expect("the tmux server survives the tab's immediate detach");
}

/// A resize reflows ONLY the window of the terminal whose channel carried
/// it (PLAN_M4.md item 3: resize goes per window).
///
/// Before tabs, `resize-window` was targeted at the tmux SESSION, which
/// resolves to whichever window tmux last made current — unambiguous with
/// one window and silently wrong with two. This is the test that would
/// have caught that: it resizes each terminal in turn and requires the
/// other's geometry to stay put.
#[farhelm_testtrace::test]
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

    let (agent_chan, initial_replay, mut agent_rx) = h
        .client
        .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
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
            "one-client",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = initial_replay;
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
#[farhelm_testtrace::test]
async fn input_reaches_only_the_terminal_it_was_sent_to() {
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
    let (tab_chan, initial_replay, mut tab_rx) = h
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
    let mut tab_seen = initial_replay;
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
/// (`only_the_sink_keeps_a_filtered_pane_readable` in `tmux/sink.rs`, and the
/// `the_session_sink_*` lifecycle tests below), not this one. This test's
/// own claim stays what it always was: the supervisor's per-terminal
/// machinery keeps the agent flowing while a tab's viewer is wedged.
#[farhelm_testtrace::test]
async fn a_stalled_tab_viewer_does_not_pause_the_agents_stream() {
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
    let (tab_chan, initial_replay, mut tab_rx) = h
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
    let mut tab_seen = initial_replay;
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
/// AGAIN. The second attach is the handoff boundary: final-owner release
/// publishes a per-session reaping barrier before teardown leaves the request,
/// and the replacement must wait until the old control client is confirmed
/// gone. A registry that merely forgot the old handle would pass the lifetime
/// assertions while still allowing two tmux clients to overlap.
#[farhelm_testtrace::test]
async fn the_session_sink_lives_exactly_as_long_as_the_sessions_attachments() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tmux_name = format!("fh-{}", session.id);

    assert_eq!(
        h.sup.session_sink_pid(&tmux_name),
        None,
        "a session nobody has attached to must have no sink"
    );

    // Both terminals attach under ONE lease: the empty legacy lease sweeps
    // every other attachment on the session (`same_lease_client`), which
    // would leave this test with one terminal where it means to have two.
    let (chan, initial_replay, mut rx) = h
        .client
        .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let first = h
        .sup
        .session_sink_pid(&tmux_name)
        .expect("the first attachment must bring a sink up");

    // A second terminal of the same session SHARES the sink rather than
    // starting a second one — the sink is per tmux session, not per
    // attachment, and a per-attachment one would put the drain cost back.
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, initial_replay, mut tab_rx) = h
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
    let mut tab_seen = initial_replay;
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;
    assert_eq!(
        h.sup.session_sink_pid(&tmux_name),
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
        h.sup.session_sink_pid(&tmux_name),
        Some(first),
        "a sink must outlive a detach that leaves another terminal attached"
    );

    // The browser reload shape is deliberately immediate: `Detach` has no
    // reply, and the following attach is the only ordering barrier. Its
    // success must therefore mean the last-owner sink teardown finished,
    // including process death, before the replacement was opened.
    h.client.detach(chan).await;
    let (chan, mut rx) = h
        .client
        .attach_at_boundary(&session.id, 80, 24)
        .await
        .expect("reattach");
    assert!(
        !std::path::Path::new(&format!("/proc/{first}")).exists(),
        "a reattach reply overtook the old sink client's exit"
    );
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let second = h
        .sup
        .session_sink_pid(&tmux_name)
        .expect("a later attach must bring a fresh sink up");
    assert_ne!(
        second, first,
        "the second sink must be a new process, not the registry handing back a dead one"
    );
    h.client.detach(chan).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while h.sup.session_sink_pid(&tmux_name).is_some() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the final detach must take the replacement sink down"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    await_process_gone(second, "the final detach must kill the sink's client").await;
    assert_eq!(
        attached_control_clients(&h).await,
        0,
        "no control client may remain attached once the session has no terminals"
    );
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

/// The process ids of every currently attached tmux client.
///
/// The connection-loss regression below needs process identity, not merely a
/// count: a replacement is allowed to have the same number of clients as its
/// predecessor, but none of those clients may be the predecessor still dying
/// behind an already-successful attach reply.
async fn attached_control_client_pids(h: &Harness) -> Vec<u32> {
    let output = tmux_query(
        &h.state.path().join("tmux.sock"),
        &["list-clients", "-F", "#{client_pid}"],
    )
    .await;
    assert!(
        output.status.success(),
        "listing tmux client pids failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| {
            line.parse::<u32>()
                .expect("tmux printed a numeric client pid")
        })
        .collect()
}

/// Wait until tmux's own visible pane contains a producer-completion marker.
///
/// The attachment is deliberately paused in the callers below, so observing
/// through its output channel would defeat the queued-output fixture. Polling
/// the pane makes producer completion a real state transition rather than a
/// sleep whose margin changes with host load.
async fn wait_for_pane_text(h: &Harness, pane: &str, needle: &str, timeout_secs: u64) {
    let socket = h.state.path().join("tmux.sock");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let capture = tmux_query(&socket, &["capture-pane", "-p", "-t", pane]).await;
        assert!(
            capture.status.success(),
            "capturing the producer's pane failed: {}",
            String::from_utf8_lossy(&capture.stderr)
        );
        if String::from_utf8_lossy(&capture.stdout).contains(needle) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the pane never contained producer marker {needle:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Input-client failure reaps queued output before a replacement attaches.
///
/// The input and output paths use separate tmux clients. Killing the input one
/// and then sending a frame drives the handler branch that used to abort the
/// output forwarder. With output paused under a busy pane, that shortcut can
/// hit tmux 3.7b's queued-pane teardown abort; the cooperative path must report
/// the failure, keep the server alive, and let a fresh attachment work.
#[farhelm_testtrace::test]
async fn input_client_failure_safely_reaps_queued_output_before_reattach() {
    let h = harness_with_shell("/bin/sh").await;
    let (session, _work) = shell_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let (channel, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for_shell(&h.client, channel, &mut rx, &mut seen, "INPUT-READY").await;

    h.client
        .send_input(
            channel,
            b"yes INPUT-FAIL-FLOOD | head -c 1048576; printf '\nINPUT-FLOOD-%s\n' DONE\r".to_vec(),
        )
        .await;
    h.client.pause_output(channel).await;
    // Observe completion from tmux's own pane rather than through the paused
    // attachment. The marker proves the finite producer is no longer adding
    // backlog before the input client is broken; replacement recovery then
    // measures teardown alone while the old output client still holds the
    // already-queued flood.
    let socket = h.state.path().join("tmux.sock");
    let tmux_name = format!("fh-{}", session.id);
    let pane = pane_id_of(&socket, &tmux_name).await;
    wait_for_pane_text(&h, &pane, "INPUT-FLOOD-DONE", 20).await;
    let input_pid = h
        .sup
        .attachment_input_client_pid(&session.id, None)
        .await
        .expect("the live attachment has an input client");
    let old_sink = h
        .sup
        .session_sink_pid(&tmux_name)
        .expect("the attachment has a session sink");
    let old_clients = attached_control_client_pids(&h).await;
    assert!(
        old_clients.len() >= 3
            && old_clients.contains(&input_pid)
            && old_clients.contains(&old_sink),
        "the attachment must own input, output, and sink clients before failure: {old_clients:?}"
    );
    kill_verified_tmux_client(input_pid, &h).await;
    // The attachment still owns the child's wait handle, so SIGKILL can leave
    // a zombie until the failure branch removes it. A zombie has already
    // closed its pipes and is therefore the exact precondition this send needs.
    wait_until_pid_gone(input_pid, 20).await;

    h.client
        .send_input(channel, b"trigger-input-failure\r".to_vec())
        .await;
    let reason = expect_detached(&mut rx, 20).await;
    assert!(
        reason.contains("input") && reason.contains("failed"),
        "the detach must describe the input-client failure: {reason}"
    );
    assert!(
        tmux_query(
            &h.state.path().join("tmux.sock"),
            &["has-session", "-t", &format!("fh-{}", session.id)],
        )
        .await
        .status
        .success(),
        "input failure cleanup must not abort the private tmux server"
    );

    let (replacement, mut replacement_rx) = h
        .client
        .attach_at_boundary(&session.id, 80, 24)
        .await
        .expect("replacement attach");
    for pid in old_clients.into_iter().filter(|pid| *pid != old_sink) {
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "replacement attach replied before old per-terminal tmux client {pid} was reaped"
        );
    }
    let mut replacement_seen = Vec::new();
    wait_for_shell(
        &h.client,
        replacement,
        &mut replacement_rx,
        &mut replacement_seen,
        "INPUT-RECOVERED",
    )
    .await;
}

/// Connection loss finishes queued-output teardown before replacement attach.
///
/// Losing the transport bypasses the explicit `Detach` request. The connection
/// tail must therefore retain attachment ownership while it shuts down the
/// per-terminal clients and releases its sink lease; otherwise another helm
/// can see an empty slot, overlap the old output client, and reproduce tmux
/// 3.7b's queued-pane abort. The session sink may be handed directly to the
/// replacement because it never carries output. A successful replacement
/// reply is the observable boundary for every output-bearing client.
#[farhelm_testtrace::test]
async fn connection_loss_safely_reaps_queued_output_before_reattach() {
    let h = harness_with_shell("/bin/sh").await;
    let (session, _work) = shell_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let owner = h.second_client().await;
    let (channel, initial_replay, mut owner_rx) = owner
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut owner_seen = initial_replay;
    wait_for_shell(
        &owner,
        channel,
        &mut owner_rx,
        &mut owner_seen,
        "CONNECTION-READY",
    )
    .await;

    owner
        .send_input(
            channel,
            b"yes CONNECTION-FAIL-FLOOD | head -c 1048576; printf '\nCONNECTION-FLOOD-%s\n' DONE\r"
                .to_vec(),
        )
        .await;
    owner.pause_output(channel).await;
    // The marker proves the finite producer has stopped while the attachment
    // still owns its already-queued output. Connection teardown is therefore
    // the only moving part when the replacement races it below.
    let socket = h.state.path().join("tmux.sock");
    let tmux_name = format!("fh-{}", session.id);
    let pane = pane_id_of(&socket, &tmux_name).await;
    wait_for_pane_text(&h, &pane, "CONNECTION-FLOOD-DONE", 20).await;
    let old_clients = attached_control_client_pids(&h).await;
    let old_sink = h
        .sup
        .session_sink_pid(&format!("fh-{}", session.id))
        .expect("the final attachment has a session sink");
    let old_input = h
        .sup
        .attachment_input_client_pid(&session.id, None)
        .await
        .expect("the attachment has an input client");
    assert!(
        old_clients.len() >= 3,
        "the final attachment must own input, output, and sink clients before connection loss: {old_clients:?}"
    );
    assert!(old_clients.contains(&old_sink));
    assert!(old_clients.contains(&old_input));

    drop(owner);
    // The replacement reply is the cleanup-order observation. Waiting for
    // replay here could let a stale output client disappear before the test
    // checks that this reply already reaped it.
    let replacement = tokio::time::timeout(
        Duration::from_secs(20),
        h.client.attach_at_boundary(&session.id, 80, 24),
    )
    .await;
    let (replacement, mut replacement_rx) = match replacement {
        Ok(result) => result.expect("replacement attach after connection loss"),
        Err(_) => panic!(
            "replacement attach timed out while connection cleanup held the terminal:\n{}",
            tmux_state_after_attach_failure(&h).await
        ),
    };

    for pid in old_clients.into_iter().filter(|pid| *pid != old_sink) {
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "replacement attach replied before old per-terminal tmux client {pid} was reaped"
        );
    }
    assert!(
        tmux_query(
            &h.state.path().join("tmux.sock"),
            &["has-session", "-t", &format!("fh-{}", session.id)],
        )
        .await
        .status
        .success(),
        "connection-loss cleanup must not abort the private tmux server"
    );

    let mut replacement_seen = Vec::new();
    wait_for_shell(
        &h.client,
        replacement,
        &mut replacement_rx,
        &mut replacement_seen,
        "CONNECTION-RECOVERED",
    )
    .await;
}

/// Explicit takeover wins a race with a natural output-client failure.
///
/// The forwarder can finish and derive a stream-failure reason just before a
/// takeover removes its attachment. The later arbiter must identity-check the
/// map instead of sending that stale verdict: the old client should receive
/// only the takeover reason that actually ended its ownership.
#[farhelm_testtrace::test]
async fn takeover_reason_wins_over_a_gated_natural_detach() {
    let natural_entered = Arc::new(tokio::sync::Notify::new());
    let natural_release = Arc::new(tokio::sync::Notify::new());
    let gate_entered = Arc::clone(&natural_entered);
    let gate_release = Arc::clone(&natural_release);
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_shell: Some("/bin/sh".to_string()),
            natural_detach_gate: Some(Arc::new(move || {
                let entered = Arc::clone(&gate_entered);
                let release = Arc::clone(&gate_release);
                Box::pin(async move {
                    entered.notify_one();
                    release.notified().await;
                })
            })),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let (session, _work) = shell_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let (channel, initial_replay, mut owner_rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut owner_seen = initial_replay;
    wait_for_shell(
        &h.client,
        channel,
        &mut owner_rx,
        &mut owner_seen,
        "NATURAL-RACE-READY",
    )
    .await;

    let tmux_name = format!("fh-{}", session.id);
    let sink = h
        .sup
        .session_sink_pid(&tmux_name)
        .expect("the attachment has a sink");
    let input = h
        .sup
        .attachment_input_client_pid(&session.id, None)
        .await
        .expect("the attachment has an input client");
    let output = attached_control_client_pids(&h)
        .await
        .into_iter()
        .find(|pid| *pid != sink && *pid != input)
        .expect("the attachment has an output client");
    kill_verified_tmux_client(output, &h).await;
    tokio::time::timeout(Duration::from_secs(20), natural_entered.notified())
        .await
        .expect("the natural detach reached its arbitration boundary");

    let replacement_client = h.second_client().await;
    let (replacement, _replay, mut replacement_rx) = replacement_client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("takeover attach");
    let reason = expect_detached(&mut owner_rx, 20).await;
    assert!(
        reason.contains("another client"),
        "the explicit takeover reason must win, got: {reason}"
    );

    natural_release.notify_one();
    tokio::task::yield_now().await;
    let mut replacement_seen = Vec::new();
    wait_for_shell(
        &replacement_client,
        replacement,
        &mut replacement_rx,
        &mut replacement_seen,
        "NATURAL-RACE-RECOVERED",
    )
    .await;
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
#[farhelm_testtrace::test]
async fn a_killed_session_sink_comes_back_while_its_terminals_stay_attached() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let tmux_name = format!("fh-{}", session.id);

    let (chan, initial_replay, mut rx) = h
        .client
        .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    // A second terminal, kept busy for the whole test: its pane is
    // filtered off on the agent's client, so it is the sink that keeps
    // tmux reading it.
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let (tab_chan, initial_replay, mut tab_rx) = h
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
    let mut tab_seen = initial_replay;
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
        .expect("an attached session must have a sink");
    kill_verified_tmux_client(doomed, &h).await;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let healed = loop {
        if let Some(pid) = h.sup.session_sink_pid(&tmux_name)
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
#[farhelm_testtrace::test]
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
            .attach_terminal_live(&session.id, 80, 24, TerminalSelector::Agent, "one-client");
    let tab_attach = h.client.attach_terminal_live(
        &session.id,
        80,
        24,
        TerminalSelector::Tab { id: tab.id.clone() },
        "one-client",
    );
    let (agent, tab_attach) = tokio::join!(agent, tab_attach);
    let (agent_chan, mut agent_seen, mut agent_rx) = agent.expect("attach the agent");
    let (tab_chan, mut tab_seen, mut tab_rx) = tab_attach.expect("attach the tab");
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;

    let pid = h
        .sup
        .session_sink_pid(&tmux_name)
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

/// A supervisor killed outright leaves no orphaned control clients once
/// its replacement arrives, and the replacement's first attach brings up
/// exactly one fresh sink.
///
/// The sink's teardown is `kill_on_drop`, which a `SIGKILL`ed supervisor
/// never gets to run — so "the owner dies" is precisely the case that
/// mechanism cannot cover, and the one where an orphan would be silent
/// and permanent: an attached control client holding a session's output
/// hostage for as long as the tmux server lives, invisible to every
/// later supervisor.
///
/// Two mechanisms compose against that, and this test pins both. First,
/// teardown by protocol: tmux control mode ends at stdin EOF, and the
/// dead supervisor's pipe ends close when the kernel reaps it — that is
/// what usually clears the clients, with no cleanup code involved. But
/// the protocol is not a guarantee: tmux defers a control client's exit
/// until its pending output drains, and a client whose reader died with
/// output queued can never finish exiting — observed live 2026-08-18,
/// after three CI flakes of this very test, as a session sink outliving
/// a 120-second deadline with EOF deliverable and a complete /proc scan
/// showing no write end of its stdin left anywhere. The GUARANTEE is
/// therefore the second mechanism: a starting supervisor reaps every
/// control-mode client already attached to its private server
/// (`reap_stale_control_clients`). The test fabricates a client the
/// protocol cannot reap — its stdin write end held open by the test
/// itself — so the sweep is exercised deterministically on every run,
/// not only on the rare runs where tmux's deferred-exit trap fires.
///
/// Runs the supervisor as a real child process, because an in-process one
/// cannot be `SIGKILL`ed without taking the test with it.
#[farhelm_testtrace::test]
async fn a_killed_supervisor_leaves_no_orphaned_sink_client() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("tempdir");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let sock = state.path().join("tmux.sock");
    let _tmux = TmuxServerGuard(sock.clone());

    let mut supervisor = tokio::process::Command::new(farhelm_bin())
        .args(["supervisor", "run", "--state-dir"])
        .arg(state.path())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn the supervisor process");
    wait_for_supervisor_ready(state.path()).await;

    // The client, its channel, and its stream all stay alive until after
    // the kill below. Dropping them first would close the connection, and
    // a supervisor that notices its client hung up tears the attachment —
    // and the sink with it — down GRACEFULLY, which is the one path this
    // test is not about. Held open, the SIGKILL is what ends them: the
    // crash shape is preserved, EOF-driven protocol teardown is then
    // observed opportunistically during the grace period below, and the
    // replacement's startup reap provides the final cleanup this test
    // actually asserts.
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
    let (_chan, initial_replay, mut rx) = client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    // One attached terminal: its output client, its input client, and
    // the session's sink.
    assert_eq!(
        count_control_clients(&sock).await,
        3,
        "test premise: an attached terminal must have brought a sink up"
    );
    // Captured while all three clients are attached: the failure report
    // prints the survivors NEXT TO this complete trio, so the clients
    // that did die name themselves by absence. tmux can still describe
    // whoever remains after the kill — what it cannot reconstruct is
    // what a full roster looked like before the teardown began.
    let pre_kill_roster = control_client_roster(&sock).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    // The whole point: no graceful shutdown, no `Drop`, no chance to kill
    // anything it owns.
    supervisor.start_kill().expect("kill the supervisor");
    let _ = supervisor.wait().await;
    // Only now, with the owner already reaped, does the test let go of its
    // end of the connection.
    drop(rx);
    drop(client);

    // Two stale control clients the PROTOCOL cannot reap: their stdin
    // write ends live in this very test process, held open, so EOF never
    // arrives. Two rather than one so a sweep that stopped after its
    // first victim would still fail. Their stdouts are piped and NEVER
    // read, so the spam burst below leaves genuinely queued output
    // behind each of them — the exact shape that aborts tmux 3.7b when
    // a client is closed without the acknowledged no-output boundary,
    // which is what makes this test the regression for that hazard and
    // not merely for "some client got removed".
    let mut stale_clients = Vec::new();
    let mut held_stdins = Vec::new();
    let mut held_stdouts = Vec::new();
    for _ in 0..2 {
        let mut stale = tokio::process::Command::new("tmux")
            .arg("-S")
            .arg(&sock)
            .args(["-C", "attach", "-t", &format!("fh-{}", session.id)])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn a deliberately stale control client");
        // Taken OUT of the child before any `wait()`: tokio's `wait`
        // drops a stored stdin to avoid pipe deadlocks, which would
        // close the very write end this fixture depends on holding open
        // and let the client exit naturally — masking a sweep that did
        // nothing.
        held_stdins.push(stale.stdin.take().expect("piped stale stdin"));
        held_stdouts.push(stale.stdout.take().expect("piped stale stdout"));
        stale_clients.push(stale);
    }
    let stale_pids: Vec<u32> = stale_clients
        .iter()
        .map(|c| c.id().expect("stale client pid"))
        .collect();
    // The sweep can only reap what is attached when the replacement
    // starts, so both fabricated clients must be visibly attached first.
    // The expiry panic describes each fabricated child's fate — an
    // attach the server refused exits immediately, with the reason on
    // stderr — plus the roster, whose error text is itself evidence
    // (e.g. "server exited unexpectedly" means the tmux SERVER died in
    // the dead supervisor's teardown storm, a different bug than the
    // clients merely being slow to register).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let attached = roster_pids(&control_client_roster(&sock).await);
        if stale_pids.iter().all(|pid| attached.contains(pid)) {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            use tokio::io::AsyncReadExt as _;
            let mut fates = String::new();
            for (i, stale) in stale_clients.iter_mut().enumerate() {
                use std::fmt::Write as _;
                match stale.try_wait() {
                    Ok(Some(status)) => {
                        let mut stderr_text = String::new();
                        if let Some(mut err) = stale.stderr.take() {
                            let _ = tokio::time::timeout(
                                Duration::from_secs(2),
                                err.read_to_string(&mut stderr_text),
                            )
                            .await;
                        }
                        let _ = writeln!(
                            fates,
                            "fabricated client {i} (pid {}): EXITED {status:?}, stderr: {}",
                            stale_pids[i],
                            stderr_text.trim()
                        );
                    }
                    Ok(None) => {
                        let _ = writeln!(
                            fates,
                            "fabricated client {i} (pid {}): still running, never listed",
                            stale_pids[i]
                        );
                    }
                    Err(e) => {
                        let _ = writeln!(
                            fates,
                            "fabricated client {i} (pid {}): try_wait failed: {e}",
                            stale_pids[i]
                        );
                    }
                }
            }
            panic!(
                "test setup: the fabricated stale clients never attached\n{fates}roster now:\n{}",
                control_client_roster(&sock).await
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // Queue real output behind every attached client. The fake agent
    // still runs — the session outlives its supervisor — so keystrokes
    // injected straight through tmux still produce pane output, which
    // every attached client receives: the two unread fabricated clients
    // accumulate it, and so does the dead supervisor's sink, which
    // reliably arms the deferred-exit trap this whole fix exists for.
    let sent = tmux_query(
        &sock,
        &[
            "send-keys",
            "-t",
            &format!("fh-{}", session.id),
            "spam 3000",
            "Enter",
        ],
    )
    .await;
    assert!(
        sent.status.success(),
        "test setup: injecting the spam burst failed: {}",
        String::from_utf8_lossy(&sent.stderr)
    );

    // Teardown by protocol gets a grace period to clear what it CAN —
    // the dead supervisor's output and input clients. Its sink is
    // expected to survive (the queued spam armed the deferred-exit
    // trap), so the wait is for "fabricated clients plus at most the
    // sink", not for a clean board; whatever remains is reported, and
    // then checked for the one shape that must still fail loudly here:
    // a survivor whose stdin has a LIVE write-end holder is a leaked
    // duplicate — the fd-leak bug this test originally hunted — not the
    // known drain stall, and the sweep must not be allowed to mask it.
    let grace = tokio::time::Instant::now() + Duration::from_secs(20);
    while count_control_clients(&sock).await > stale_pids.len() + 1 {
        if tokio::time::Instant::now() >= grace {
            println!(
                "note: control clients beyond the expected set outlived the grace period\n{}",
                orphaned_client_report(&sock, &pre_kill_roster).await
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    for pid in roster_pids(&control_client_roster(&sock).await) {
        if stale_pids.contains(&pid) {
            continue;
        }
        let scan = stdin_pipe_holder_scan(pid);
        assert!(
            !scan.contains(" mode WRITE:"),
            "a surviving client of the DEAD supervisor still has a live stdin write-end \
             holder — that is a leaked fd, not the known drain stall:\n{scan}"
        );
    }

    // The replacement supervisor's startup reap must clear the board —
    // both fabricated clients and the trapped sink alike, each behind
    // the acknowledged no-output boundary first — and its first attach
    // must then produce exactly one sink, not a second one alongside
    // something left over.
    let mut replacement = tokio::process::Command::new(farhelm_bin())
        .args(["supervisor", "run", "--state-dir"])
        .arg(state.path())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn the replacement supervisor");
    wait_for_supervisor_ready(state.path()).await;
    // Both fabricated clients' PROCESSES must be dead, and dead of
    // SIGKILL specifically — the reap kills the process because that is
    // the only lever that works on a client wedged in the deferred-exit
    // trap, and an exit by any other cause would mean this fixture
    // cleaned itself up rather than being swept.
    for mut stale in stale_clients {
        let status = tokio::time::timeout(Duration::from_secs(10), stale.wait())
            .await
            .expect("the replacement's startup reap must kill the stale control clients")
            .expect("waiting on a reaped stale client");
        assert_eq!(
            std::os::unix::process::ExitStatusExt::signal(&status),
            Some(9),
            "the stale client must die of the reap's SIGKILL, not exit on its own: {status:?}"
        );
    }
    drop(held_stdins);
    drop(held_stdouts);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while count_control_clients(&sock).await > 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the replacement's startup reap must leave a clean client roster\n{}",
            orphaned_client_report(&sock, &pre_kill_roster).await
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    {
        let client = connect_over_socket(state.path()).await;
        let (_chan, initial_replay, mut rx) = client
            .attach_live(&session.id, 80, 24)
            .await
            .expect("reattach through the replacement supervisor");
        let mut seen = initial_replay;
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
/// socket path behind, so a replacement's file exists well before the
/// replacement has unlinked it, bound, and begun accepting. That is the
/// same hazard [`wait_for_supervisor_ready`] now dials for; this one goes
/// on to complete a handshake, so it stays separate rather than being
/// folded into it.
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
///
/// A failed query is NOT zero clients, and conflating the two is how a
/// test that polls "until no clients remain" passes by never having asked.
/// The one failure that genuinely means zero is tmux saying there is no
/// server to answer for at all, which it does in three exact shapes; the
/// discrimination mirrors the driver's own `is_definitively_empty` (see
/// `farhelm-supervisor`'s `tmux.rs`), including its rule that the match is
/// against the WHOLE trimmed stderr rather than a substring — the socket
/// path is embedded in one of these messages, so a `contains` check could
/// launder an unrelated failure that merely mentions that path.
async fn count_control_clients(sock: &std::path::Path) -> usize {
    let listed = tmux_query(sock, &["list-clients", "-F", "#{client_flags}"]).await;
    if !listed.status.success() {
        let stderr = String::from_utf8_lossy(&listed.stderr);
        let stderr = stderr.trim();
        let definitively_empty = stderr == "no current target"
            || stderr == "server exited unexpectedly"
            || stderr == format!("no server running on {}", sock.display());
        assert!(
            definitively_empty,
            "list-clients failed on {} with an unrecognized diagnostic, which is not evidence \
             that no clients are attached: {stderr}",
            sock.display()
        );
        return 0;
    }
    String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

/// Every attached control client, one line each, in the shape the orphan
/// diagnostics want: pid, name, flags, session, and the created/activity
/// timestamps.
///
/// The flags identify each client's ROLE. Verified against a live roster
/// (tmux 3.7b, 2026-08-18): the input client attaches `-f no-output`
/// permanently, so that flag names it for certain; the replay client is
/// the one carrying `pause-after=N` (the per-terminal flow control from
/// PLAN_M4 is set on exactly that client); the sink is the one with
/// neither. Where flags are not enough, [`stdin_pipe_holder_scan`]'s
/// cmdline capture settles it — the three attach argument shapes differ.
///
/// This one listing is a diagnostic consumer's ENTIRE view of the server:
/// pids for the per-process scans are parsed back out of these lines (see
/// [`roster_pids`]) rather than fetched by a second query, because clients
/// exit asynchronously and two queries can describe two different worlds
/// (observed live: a graceful teardown dropped two of three clients
/// between back-to-back listings). Bounded, and returns the failure or
/// timeout text instead of panicking: every caller is either asserting a
/// premise or assembling a failure report, and a probe that can hang — or
/// panic inside a panic's evidence-gathering — would trade the real
/// report for a worse one.
async fn control_client_roster(sock: &std::path::Path) -> String {
    // A dedicated command rather than `tmux_query`, for `kill_on_drop`:
    // when the timeout abandons the output future, the spawned tmux
    // process must die with it, not linger while the diagnostic loop
    // keeps polling a wedged server.
    let mut query = tokio::process::Command::new("tmux");
    query
        .arg("-S")
        .arg(sock)
        .args([
            "list-clients",
            "-F",
            "pid=#{client_pid} name=#{client_name} flags=[#{client_flags}] \
             session=#{client_session} created=#{client_created} activity=#{client_activity}",
        ])
        .kill_on_drop(true);
    let listed = tokio::time::timeout(Duration::from_secs(5), query.output()).await;
    match listed {
        Err(_) => {
            "list-clients timed out after 5s; the tmux server itself may be wedged".to_string()
        }
        Ok(Err(e)) => format!("list-clients could not be spawned: {e}"),
        Ok(Ok(listed)) if listed.status.success() => String::from_utf8_lossy(&listed.stdout)
            .trim_end()
            .to_string(),
        Ok(Ok(listed)) => format!(
            "list-clients failed: {}",
            String::from_utf8_lossy(&listed.stderr).trim()
        ),
    }
}

/// The client pids named by a [`control_client_roster`] listing.
///
/// Parsing the roster rather than re-querying keeps every consumer on the
/// same single snapshot (the roster's docstring says why). Error and
/// timeout text parses to an empty list; the caller prints the roster
/// verbatim, so the failure itself stays visible.
fn roster_pids(roster: &str) -> Vec<u32> {
    roster
        .lines()
        .filter_map(|line| line.strip_prefix("pid=")?.split(' ').next()?.parse().ok())
        .collect()
}

/// The full story of an expired orphan-drain deadline: who is still
/// attached, who was attached before the kill, and — the part that decides
/// the investigation — which live processes still hold the survivor's
/// stdin pipe, in which direction.
///
/// The question this exists to answer: did the survivor simply not exit
/// YET (deadline too tight under load), or can it NEVER exit because some
/// process other than the dead supervisor holds a duplicate of its stdin
/// write end, so EOF will never arrive? A write-end holder that is not the
/// supervisor is the second mechanism caught red-handed — and a real bug in
/// exactly the guarantee this test pins. No write-end holder at all points
/// back at the first. The read-end holders are expected chatter (the tmux
/// server holds each control client's stdin via fd passing) but are listed
/// anyway; an unexpected reader would be its own lead.
async fn orphaned_client_report(sock: &std::path::Path, pre_kill_roster: &str) -> String {
    let survivors = control_client_roster(sock).await;
    let pids = roster_pids(&survivors);
    let mut report = format!(
        "still attached at deadline expiry:\n{survivors}\nroster before the kill:\n{pre_kill_roster}\n",
    );
    if pids.is_empty() {
        report.push_str(
            "no client pids parsed from the survivor listing above; per-process pipe scans skipped\n",
        );
    }
    for pid in pids {
        report.push_str(&stdin_pipe_holder_scan(pid));
    }
    report
}

/// Trace one process's stdin pipe through /proc: its own state, the pipe's
/// identity, and every live process holding an end of that same pipe, with
/// each holder's access mode spelled out.
///
/// Linux-only by construction — every path here starts at /proc — and
/// degrades to a note rather than failing where /proc or a permission is
/// missing. Reads race process exit by nature; a process that vanishes
/// mid-scan (NotFound) is skipped silently, which is the right bias for
/// evidence-gathering — a vanished process was not keeping anything
/// alive. Foreign-UID processes are skipped silently too, on different
/// grounds: an unprivileged run cannot read their fd tables, and they
/// cannot have INHERITED this test's pipe ends through any unprivileged
/// path, so counting their routine permission errors would mark every
/// scan on a real system incomplete and drown the signal. Every other
/// inspection failure — same-UID processes included, and unreadable or
/// unparsable fdinfo — is counted and the scan marked incomplete, because
/// "no write-end holder found" is the finding that exonerates the fd-leak
/// mechanism, and it is only worth anything from a scan without blind
/// spots.
fn stdin_pipe_holder_scan(pid: u32) -> String {
    stdin_pipe_holder_scan_at(std::path::Path::new("/proc"), pid)
}

/// [`stdin_pipe_holder_scan`] against an explicit proc root, so the gap
/// accounting and hostile-entry handling are testable against a
/// fabricated tree (a real /proc cannot be made to misbehave on demand).
/// The same-UID gate still consults the REAL /proc for this process's
/// uid; the fabricated tree's entries are owned by the test user, so they
/// pass it.
fn stdin_pipe_holder_scan_at(proc_root: &std::path::Path, pid: u32) -> String {
    use std::fmt::Write as _;
    use std::os::unix::fs::MetadataExt as _;
    let mut out = String::new();
    let status = std::fs::read_to_string(proc_root.join(format!("{pid}/status")))
        .map(|s| {
            s.lines()
                .filter(|l| l.starts_with("State:") || l.starts_with("PPid:"))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|e| format!("status unreadable: {e}"));
    let _ = writeln!(out, "surviving client pid {pid}: {status}");
    let stdin = match std::fs::read_link(proc_root.join(format!("{pid}/fd/0"))) {
        Ok(target) => target,
        Err(e) => {
            let _ = writeln!(out, "  stdin not inspectable (no /proc, or gone): {e}");
            return out;
        }
    };
    let _ = writeln!(out, "  stdin is {}", stdin.display());
    if !stdin.to_string_lossy().starts_with("pipe:") {
        return out;
    }
    let my_uid = std::fs::metadata(format!("/proc/{}", std::process::id()))
        .map(|m| m.uid())
        .ok();
    let procs = match std::fs::read_dir(proc_root) {
        Ok(entries) => entries,
        Err(e) => {
            let _ = writeln!(out, "  /proc scan unavailable: {e}");
            return out;
        }
    };
    let mut gaps: Vec<String> = Vec::new();
    for proc_entry in procs {
        let proc_entry = match proc_entry {
            Ok(entry) => entry,
            Err(e) => {
                gaps.push(format!("proc listing error ({e})"));
                continue;
            }
        };
        let holder = proc_entry.file_name();
        let Some(holder) = holder
            .to_str()
            .filter(|n| n.bytes().all(|b| b.is_ascii_digit()))
        else {
            continue;
        };
        // The foreign-UID gate (see the docstring): skip silently rather
        // than gap-account processes this scan could neither read nor
        // suspect.
        match proc_entry.metadata() {
            Ok(meta) if my_uid.is_some() && Some(meta.uid()) != my_uid => continue,
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                gaps.push(format!("pid {holder}: not statable ({e})"));
                continue;
            }
        }
        let fds = match std::fs::read_dir(proc_root.join(format!("{holder}/fd"))) {
            Ok(fds) => fds,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                gaps.push(format!("pid {holder}: fd dir unreadable ({e})"));
                continue;
            }
        };
        for fd_entry in fds {
            let fd_entry = match fd_entry {
                Ok(entry) => entry,
                Err(e) => {
                    gaps.push(format!("pid {holder}: fd listing error ({e})"));
                    continue;
                }
            };
            match std::fs::read_link(fd_entry.path()) {
                Ok(target) if target == stdin => {}
                Ok(_) => continue,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    gaps.push(format!(
                        "pid {holder}: fd {} unreadable ({e})",
                        fd_entry.file_name().to_string_lossy()
                    ));
                    continue;
                }
            }
            let fd = fd_entry.file_name();
            let fd = fd.to_string_lossy();
            // O_ACCMODE from fdinfo's octal flags line is what turns "holds
            // the pipe" into "holds the WRITE end" — the whole point of the
            // scan. A failure here is BOTH displayed ("mode ?") and gap-
            // accounted: a holder with an unknown direction leaves the
            // no-leaked-writer question open just as surely as a process
            // the scan could not read.
            let mode =
                match std::fs::read_to_string(proc_root.join(format!("{holder}/fdinfo/{fd}"))) {
                    Ok(info) => match info
                        .lines()
                        .find_map(|l| l.strip_prefix("flags:"))
                        .map(str::trim)
                        .and_then(|flags| u32::from_str_radix(flags, 8).ok())
                    {
                        Some(flags) => match flags & 0o3 {
                            0 => "read",
                            1 => "WRITE",
                            _ => "read-write",
                        },
                        None => {
                            gaps.push(format!("pid {holder}: fdinfo/{fd} unparsable"));
                            "?"
                        }
                    },
                    Err(e) => {
                        gaps.push(format!("pid {holder}: fdinfo/{fd} unreadable ({e})"));
                        "?"
                    }
                };
            let comm = std::fs::read_to_string(proc_root.join(format!("{holder}/comm")))
                .map(|c| c.trim().to_string())
                .unwrap_or_else(|_| "?".to_string());
            // Raw bytes, decoded lossily: an argv with invalid UTF-8 must
            // degrade to mojibake in that argument, not erase the whole
            // command line from the report.
            let cmdline = std::fs::read(proc_root.join(format!("{holder}/cmdline")))
                .map(|c| {
                    String::from_utf8_lossy(&c)
                        .replace('\0', " ")
                        .trim()
                        .to_string()
                })
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "  held by pid {holder} ({comm}) fd {fd} mode {mode}: {cmdline}"
            );
        }
    }
    if !gaps.is_empty() {
        let shown = gaps[..gaps.len().min(3)].join("; ");
        let _ = writeln!(
            out,
            "  scan INCOMPLETE ({} inspection failures — an absent write-end holder is not \
             conclusive): {shown}",
            gaps.len()
        );
    }
    out
}

/// The /proc pipe-holder scan executes only on a rare CI failure path, so
/// this pins its two load-bearing claims deterministically: the traversal
/// finds EVERY live holder of a pipe — not just the first reader and
/// writer — and the fdinfo access-mode decoding labels each end exactly.
/// Reversed modes or a broken traversal would otherwise compile fine and
/// stay invisible until the one CI occurrence the scan exists for — and
/// then spoil it.
///
/// The controlled pipe is a spawned `cat` with piped stdin, plus a second
/// deliberate write-end holder (a `sleep` whose stdout is a duplicate of
/// the pipe's write end — it never writes, it just HOLDS the fd). That
/// second writer is the scan's actual quarry in production: an unexpected
/// extra process keeping EOF from ever arriving. All three holders must
/// appear with exact modes.
#[farhelm_testtrace::test]
async fn the_pipe_holder_scan_names_every_holder_of_a_live_pipe() {
    use std::os::fd::AsFd as _;
    if !std::path::Path::new("/proc/self/fd").exists() {
        println!("SKIPPED: no /proc on this platform, nothing to scan");
        return;
    }
    // std rather than tokio spawns: the test needs the ChildStdin as a
    // borrowable fd to duplicate, and nothing here awaits.
    let mut cat = std::process::Command::new("cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn cat");
    let write_end = cat.stdin.take().expect("piped stdin");
    let dup = write_end
        .as_fd()
        .try_clone_to_owned()
        .expect("duplicate the pipe's write end");
    let mut extra_writer = std::process::Command::new("sleep")
        .arg("60")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(dup))
        .spawn()
        .expect("spawn the extra write-end holder");
    let report = stdin_pipe_holder_scan(cat.id());
    let me = std::process::id();
    let cat_pid = cat.id();
    let extra_pid = extra_writer.id();
    // " mode read:" (with the colon) and not a bare "mode read", so a
    // decoder that misreports read-write cannot sneak past the substring.
    let holds = |pid: u32, mode: &str| {
        report
            .lines()
            .any(|l| l.contains(&format!("held by pid {pid} ")) && l.contains(mode))
    };
    assert!(
        holds(me, " mode WRITE:"),
        "the scan must name this test process as a write-end holder:\n{report}"
    );
    assert!(
        holds(extra_pid, " mode WRITE:"),
        "the scan must name the extra duplicate-holding process as a write-end holder:\n{report}"
    );
    assert!(
        holds(cat_pid, " mode read:"),
        "the scan must name the child as the read-end holder:\n{report}"
    );
    let _ = extra_writer.kill();
    let _ = extra_writer.wait();
    drop(write_end);
    let _ = cat.wait();
}

/// The scan's gap accounting is what keeps a blind spot from being read
/// as "no leaked writer", and nothing on a healthy system exercises it —
/// a real /proc cannot be made to misbehave on demand. So: a fabricated
/// proc root with one hostile entry (an fd directory the scan cannot
/// read) and one complete fake holder whose argv is deliberately invalid
/// UTF-8. Pins the INCOMPLETE marker, the holder still being reported
/// with its mode, and the lossy cmdline decode preserving the rest of the
/// command line.
#[farhelm_testtrace::test]
async fn the_pipe_holder_scan_reports_blind_spots_instead_of_swallowing_them() {
    use std::os::unix::fs::PermissionsExt as _;
    if !std::path::Path::new("/proc/self/fd").exists() {
        println!("SKIPPED: no /proc on this platform, nothing to scan");
        return;
    }
    if std::fs::metadata(format!("/proc/{}", std::process::id()))
        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
        .ok()
        == Some(0)
    {
        println!("SKIPPED: running as root, permission-denied entries cannot be fabricated");
        return;
    }
    let root = tempfile::tempdir().expect("fake proc root");
    let root_path = root.path();
    // The scanned "client": pid 1000001, stdin is pipe:[4242]. Dangling
    // symlinks are fine — read_link returns the target text either way.
    std::fs::create_dir_all(root_path.join("1000001/fd")).expect("client dirs");
    std::fs::create_dir_all(root_path.join("1000001/fdinfo")).expect("client fdinfo");
    std::fs::write(
        root_path.join("1000001/status"),
        "State:\tS (sleeping)\nPPid:\t1\n",
    )
    .expect("client status");
    std::os::unix::fs::symlink("pipe:[4242]", root_path.join("1000001/fd/0")).expect("client fd0");
    // The client holds its own read end; a complete fixture keeps it out
    // of the gap accounting so the hostile entry below is the ONLY gap.
    std::fs::write(root_path.join("1000001/fdinfo/0"), "pos:\t0\nflags:\t00\n")
        .expect("client fdinfo file");
    std::fs::write(root_path.join("1000001/comm"), "fake-client\n").expect("client comm");
    // A complete fake holder of the same pipe's write end, argv invalid
    // UTF-8 in the second argument.
    std::fs::create_dir_all(root_path.join("1000002/fd")).expect("holder dirs");
    std::fs::create_dir_all(root_path.join("1000002/fdinfo")).expect("holder fdinfo");
    std::os::unix::fs::symlink("pipe:[4242]", root_path.join("1000002/fd/7")).expect("holder fd7");
    std::fs::write(
        root_path.join("1000002/fdinfo/7"),
        "pos:\t0\nflags:\t0100001\n",
    )
    .expect("holder fdinfo file");
    std::fs::write(root_path.join("1000002/comm"), "evil\n").expect("holder comm");
    std::fs::write(
        root_path.join("1000002/cmdline"),
        b"evil\0arg-\xff\xfe-bytes\0last\0",
    )
    .expect("holder cmdline");
    // The hostile entry: an fd directory that exists but cannot be read.
    std::fs::create_dir_all(root_path.join("1000003/fd")).expect("hostile dirs");
    std::fs::set_permissions(
        root_path.join("1000003/fd"),
        std::fs::Permissions::from_mode(0o000),
    )
    .expect("hostile perms");

    let report = stdin_pipe_holder_scan_at(root_path, 1000001);

    // Restore permissions so the tempdir can be deleted.
    let _ = std::fs::set_permissions(
        root_path.join("1000003/fd"),
        std::fs::Permissions::from_mode(0o700),
    );
    assert!(
        report
            .lines()
            .any(|l| l.contains("held by pid 1000002 (evil) fd 7 mode WRITE:")),
        "the fake write-end holder must be reported with its exact mode:\n{report}"
    );
    assert!(
        report.contains("last"),
        "the lossy cmdline decode must preserve the valid arguments around invalid UTF-8:\n{report}"
    );
    assert!(
        report.contains("scan INCOMPLETE (1 inspection failures"),
        "the unreadable fd directory must be counted and announced:\n{report}"
    );
    assert!(
        report.contains("pid 1000003: fd dir unreadable"),
        "the gap note must name the process the scan could not inspect:\n{report}"
    );
}

/// The orphan report's role recovery rests entirely on the flag
/// signatures this pins: exactly one attached client carries `no-output`
/// (the input client), exactly one carries `pause-after` (the replay
/// client), and exactly one carries neither (the sink). If a tmux release
/// renamed these flags — or the supervisor changed which client gets
/// which — the report would silently regress to anonymous clients, and
/// nobody would notice until the next CI failure arrived undecipherable.
/// Also pins [`roster_pids`] against the roster's real output shape.
#[farhelm_testtrace::test]
async fn the_client_roster_identifies_all_three_roles_by_flags() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let (chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    let sock = h.state.path().join("tmux.sock");
    // The sink comes up from a spawned task, so the roster is polled to
    // three rather than asserted immediately.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let roster = loop {
        let roster = control_client_roster(&sock).await;
        if roster.lines().count() == 3 {
            break roster;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "an attached terminal never reached three control clients: {roster}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    assert_eq!(
        roster_pids(&roster).len(),
        3,
        "every roster line must parse to a pid: {roster}"
    );
    let count = |flag: &str| roster.lines().filter(|l| l.contains(flag)).count();
    assert_eq!(
        count("no-output"),
        1,
        "exactly one input client carries no-output: {roster}"
    );
    assert_eq!(
        count("pause-after"),
        1,
        "exactly one replay client carries pause-after: {roster}"
    );
    assert_eq!(
        roster
            .lines()
            .filter(|l| !l.contains("no-output") && !l.contains("pause-after"))
            .count(),
        1,
        "exactly one sink carries neither flag: {roster}"
    );
    // Uniqueness of the flags is not yet identity: pin the neither-flag
    // line to the supervisor's OWN record of the sink's pid, so a future
    // change swapping which client carries which flag cannot pass by
    // symmetry. The input and replay clients' pids have no supervisor
    // accessor to cross-check against, so the sink is the one anchor
    // available — and one anchored role breaks any flag rotation.
    let sink_pid = h
        .sup
        .session_sink_pid(&format!("fh-{}", session.id))
        .expect("an attached session has a sink pid on record");
    assert!(
        roster
            .lines()
            .find(|l| !l.contains("no-output") && !l.contains("pause-after"))
            .is_some_and(|l| l.contains(&format!("pid={sink_pid} "))),
        "the neither-flag line must be the supervisor's recorded sink (pid {sink_pid}): {roster}"
    );
    h.client.detach(chan).await;
}

/// The sink registry does not grow with the number of sessions that have
/// ever been attached to.
///
/// Entries are `Weak`, so a dead one is harmless to behavior and invisible
/// to every other test — which is exactly what makes an unbounded map here
/// the kind of leak that ships. A supervisor serving short-lived sessions
/// all day would accumulate one dead key per session id, forever.
#[farhelm_testtrace::test]
async fn the_sink_registry_does_not_grow_with_dead_sessions() {
    let h = harness().await;
    for _ in 0..4 {
        let (session, _work) = basic_session(&h).await;
        let _cleanup = MarkerCleanupGuard::new(session.id.clone());
        let tmux_name = format!("fh-{}", session.id);
        let (chan, initial_replay, mut rx) = h
            .client
            .attach_live(&session.id, 80, 24)
            .await
            .expect("attach");
        let mut seen = initial_replay;
        wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
        h.client.detach(chan).await;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        while h.sup.session_sink_pid(&tmux_name).is_some() {
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
    let registered = h.sup.session_sink_registry_len();
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
#[farhelm_testtrace::test]
async fn an_rc_file_change_between_two_tab_opens_reaches_the_second_tab() {
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
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    /// Open a tab, retain its initial replay in the transcript, and ask its
    /// shell what the marker holds.
    ///
    /// The live helper consumes replay completion before the readiness round
    /// trip. Keeping the snapshot matters because an interactive shell can
    /// have already printed rc-file output before that round trip starts.
    async fn tab_marker_value(h: &Harness, session_id: &str, ready: &str) -> String {
        let tab = h.client.open_tab(session_id).await.expect("open a tab");
        let (chan, initial_replay, mut rx) = h
            .client
            .attach_terminal_live(
                session_id,
                80,
                24,
                TerminalSelector::Tab { id: tab.id.clone() },
                "rc-lease",
            )
            .await
            .expect("attach the tab");
        let mut seen = initial_replay;
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

/// A freshly opened tab's window is already the AGENT window's size when
/// the open reply publishes it (BUGS_BURNDOWN.md issue 4).
///
/// `new-window` inherits the tmux SESSION default size, not the agent
/// window's (resize is per-window), so without the open path's explicit
/// pre-sizing a tab's first attach almost always carried a real resize —
/// and the shell's SIGWINCH repaint raced the attach's snapshot capture,
/// which is the residue-beside-the-prompt bug. The geometry divergence is
/// manufactured the way real clients produce it: the session is CREATED
/// at one size (which becomes the session default) and the agent is then
/// ATTACHED at another (a per-window resize the session default never
/// follows). Creating the session at the odd size instead would prove
/// nothing — the tab would inherit it through the session default with no
/// pre-sizing at all, which is exactly the vacuous first version of this
/// test.
#[farhelm_testtrace::test]
async fn a_new_tab_window_is_presized_to_the_agent_windows_geometry() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().unwrap();
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

    // The client's attach is what moves the agent WINDOW off the session
    // default; 123x37 is the tell — nothing defaults to it.
    let (chan, initial_replay, mut rx) = h
        .client
        .attach_terminal_live(&session.id, 123, 37, TerminalSelector::Agent, "presize")
        .await
        .expect("attach the agent at the odd geometry");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let pane = tab_pane(&h, &session.id, &tab.id).await;
    let out = tmux_query(
        &h.state.path().join("tmux.sock"),
        &[
            "display-message",
            "-p",
            "-t",
            &pane,
            "#{window_width} #{window_height}",
        ],
    )
    .await;
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "123 37",
        "the tab window must be published at the ATTACHED agent window's geometry, \
         not the session default"
    );
    h.client.detach(chan).await;
}
