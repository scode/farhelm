//! Terminal tabs: the launch contract, refusals, close's reap, and the
//! per-terminal properties that only become observable once a session has
//! a second terminal.
//!
//! A tab is a terminal host of its own. Host tabs' own mechanics are
//! tested in `host_tabs.rs`.

use crate::harness::*;

use crate::boot_id_durable_outcome::listed;
use crate::restart_with_resume::{RC_MARKER_VAR, write_rc_files};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Terminal tabs (PLAN_M4.md item 2)
//
// A tab is a terminal host of its own running the user's login shell. The
// tests below are grouped by the promise each one pins: the launch
// contract, the refusals, close's reap, the marker split that keeps the
// shared agent reap off tabs, and the per-terminal properties that only
// become observable once a session has a second terminal at all.
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

/// Agent startup must undo an rc-file cd, while a tab preserves the user's normal
/// shell startup behavior. Both report physical cwd and PWD; the private profile's
/// witness proves it actually changed directory before either observation.
#[farhelm_testtrace::test]
async fn agent_restores_session_directory_but_tab_keeps_startup_cd() {
    let home = farhelm_teststate::tempdir().unwrap();
    let startup = farhelm_teststate::tempdir().unwrap();
    let work = farhelm_teststate::tempdir().unwrap();
    // macOS resolves /tmp through a symlink. Use physical fixture spellings so
    // the assertions isolate startup ordering rather than shell PWD spelling.
    let startup_path = std::fs::canonicalize(startup.path()).unwrap();
    let work_path = std::fs::canonicalize(work.path()).unwrap();
    let witness = home.path().join("profile-entered");
    std::fs::write(
        home.path().join(".bash_profile"),
        format!(
            "cd {} || exit 1\nprintf '%s\\n' \"$PWD\" >> {}\n",
            farhelm_proto::text::shell_quote(startup_path.to_str().unwrap()),
            farhelm_proto::text::shell_quote(witness.to_str().unwrap()),
        ),
    )
    .unwrap();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_shell: Some("/bin/bash".into()),
            launch_env: vec![("HOME".into(), home.path().to_str().unwrap().into())],
            ..SupervisorSeams::default()
        },
    )
    .await;
    let session = h
        .client
        .create_session(work_path.to_str().unwrap(), "/bin/sh", None, 80, 24)
        .await
        .unwrap();
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());
    let (channel, initial, mut rx) = h
        .client
        .attach_terminal_live(
            &session.id,
            120,
            24,
            TerminalSelector::Agent,
            "directory-agent",
        )
        .await
        .unwrap();
    let mut seen = initial;
    wait_for_shell(&h.client, channel, &mut rx, &mut seen, "AGENT-READY").await;
    assert_eq!(
        std::fs::read_to_string(&witness).unwrap().trim(),
        startup_path.to_str().unwrap()
    );
    run_in_shell(
        &h.client,
        channel,
        &mut rx,
        &mut seen,
        "printf 'CW%s[%s|%s]%s\\n' D \"$(pwd -P)\" \"$PWD\" DWC",
        "]DWC",
        20,
    )
    .await;
    let expected = work_path;
    assert!(
        String::from_utf8_lossy(&seen).contains(&format!("CWD[{0}|{0}]DWC", expected.display())),
        "agent must restore cwd/PWD after profile cd: {}",
        String::from_utf8_lossy(&seen)
    );

    let tab = h.client.open_tab(&session.id).await.unwrap();
    let (channel, initial, mut rx) = h
        .client
        .attach_terminal_live(
            &session.id,
            120,
            24,
            TerminalSelector::Tab { id: tab.id },
            "directory-tab",
        )
        .await
        .unwrap();
    let mut seen = initial;
    wait_for_shell(&h.client, channel, &mut rx, &mut seen, "TAB-READY").await;
    assert_eq!(
        std::fs::read_to_string(&witness).unwrap().lines().count(),
        2,
        "the tab must also execute the private startup profile"
    );
    run_in_shell(
        &h.client,
        channel,
        &mut rx,
        &mut seen,
        "printf 'CW%s[%s|%s]%s\\n' D \"$(pwd -P)\" \"$PWD\" DWC",
        "]DWC",
        20,
    )
    .await;
    assert!(
        String::from_utf8_lossy(&seen)
            .contains(&format!("CWD[{0}|{0}]DWC", startup_path.display())),
        "tab shell startup remains free to cd: {}",
        String::from_utf8_lossy(&seen)
    );
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
/// `host_tabs::a_tab_shell_dead_at_open_is_refused_with_its_last_words_and_leaves_nothing`).
///
/// `notify_one` before anyone waits still stores a permit, so a test that
/// reaches its `notified()` after the supervisor has already entered the
/// gate is not left waiting for a notification that has been and gone.
pub(crate) fn notifying_gate() -> (
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
            .send_keys(channel, command.clone().into_bytes())
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
        .send_keys(channel, format!("{command}\r").into_bytes())
        .await;
    wait_for(rx, seen, marker, secs).await;
}

/// The pid of the shell tab `tab_id` of session `session_id` runs, read
/// without a listing: its terminal host's child.
pub(crate) async fn tab_shell_pid(h: &Harness, session_id: &str, tab_id: &str) -> u32 {
    h.sup
        .tab_host_for_test(session_id, tab_id)
        .await
        .unwrap_or_else(|| panic!("session {session_id} has no host tab {tab_id}"))
        .status()
        .child_pid
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
/// own session and ignores SIGHUP before publishing its pid, so the pty
/// hangup that follows `kill-window` cannot reach it either — without that,
/// a survivor could die from the window's own
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
            "( setsid {scrub}/bin/sh -c 'trap \"\" HUP; echo $$ > {pid}; exec sleep 120' \
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
/// The working directory is checked by asking the shell: what SPEC.md
/// promises is that a command typed in the tab shows the session's cwd,
/// and only running one proves the directory was applied to the process
/// rather than merely recorded.
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
    // The closing marker is assembled at runtime, so the echoed command
    // cannot satisfy the wait before the shell's whole result arrives.
    run_in_shell(
        &h.client,
        chan,
        &mut rx,
        &mut seen,
        "printf 'CW%s[%s]%s\\n' D \"$PWD\" DWC",
        "]DWC",
        20,
    )
    .await;
    let transcript = String::from_utf8_lossy(&seen).into_owned();
    let reported = transcript
        .split("CWD[")
        .nth(1)
        .and_then(|rest| rest.split_once("]DWC").map(|(value, _)| value))
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
            &fixture_cmd("fake-agent --script basic"),
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
    h.client.send_keys(_chan, b"still-here\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "still-here", 15).await;
}

/// Closing a tab returns well inside the SIGTERM grace even though the
/// tab's interactive shell ignores SIGTERM: the teardown hangs the tab up
/// as well, and a shell exits on SIGHUP.
///
/// Why this exists: an interactive bash ignores SIGTERM absent a trap, so
/// a teardown that only terminated would sit out the whole grace and end
/// the shell with the SIGKILL, and every tab close would cost the full
/// bound. The bound is small in the commit that adds this test and grows
/// to five seconds afterwards; the four-second threshold is written
/// against that larger bound (a close that sat the grace out cannot come
/// in under it, and a hung-up shell ends the close in well under a
/// second, so a loaded host has seconds of margin) and passes trivially
/// until then.
///
/// The shell is pinned to bash rather than taken from the host, so the
/// premise — a pane shell that ignores SIGTERM — is made true instead of
/// hoped for, and asserted on Linux, where `/proc` can say so; a shell
/// that died to SIGTERM would let this pass without exercising the
/// hangup at all.
#[farhelm_testtrace::test]
async fn closing_a_tab_returns_well_inside_the_sigterm_grace() {
    let h = harness_with_shell("/bin/bash").await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open the tab");
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
    wait_for_shell(&h.client, chan, &mut rx, &mut seen, "T").await;
    let pane_pid = tab_shell_pid(&h, &session.id, &tab.id).await;
    let _pane_cleanup = PidKillGuard::arm(pane_pid);

    // The fixture premise, asserted where the kernel can be asked: the
    // tab's shell really does ignore SIGTERM (bit 15 of `SigIgn`), so the
    // close cannot be ending it with the SIGTERM alone.
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{pane_pid}/status"))
            .expect("test setup: reading the tab shell's /proc status");
        let ignored = status
            .lines()
            .find_map(|line| line.strip_prefix("SigIgn:"))
            .map(|mask| u64::from_str_radix(mask.trim(), 16).expect("SigIgn is a hex mask"))
            .expect("test setup: /proc status has a SigIgn line");
        assert!(
            ignored & (1 << (libc::SIGTERM - 1)) != 0,
            "test setup: the tab's bash must ignore SIGTERM (SigIgn {ignored:#x}), or the latency \
             below proves nothing about the hangup"
        );
    }

    let started = tokio::time::Instant::now();
    h.client
        .close_tab(&session.id, &tab.id)
        .await
        .expect("close the tab");
    let elapsed = started.elapsed();
    wait_until_pid_gone(pane_pid, 15).await;
    assert!(
        elapsed < Duration::from_secs(4),
        "closing a tab must not sit out the SIGTERM grace, but the close took {elapsed:?}"
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
    let doomed_pane_pid = tab_shell_pid(&h, &session.id, &doomed.id).await;

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
        .send_keys(agent_chan, b"agent-untouched\r".to_vec())
        .await;
    wait_for(&mut agent_rx, &mut agent_seen, "agent-untouched", 15).await;
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

/// Stop ends a host tab's shell and its SIGHUP-immune daemon, as well as
/// the agent's descendants. Killing only the host or its pty would leave
/// the daemon alive, so this pins the tab's own sweep as part of Stop.
#[farhelm_testtrace::test]
async fn stopping_the_agent_closes_a_host_tab_and_its_daemonized_child() {
    assert_stop_closes_tab_tree(harness().await).await;
}

/// Exercise the shared Stop contract against either terminal substrate.
/// The daemon reparents before Stop and ignores hangup, distinguishing
/// explicit process cleanup from the shell dying with its terminal.
async fn assert_stop_closes_tab_tree(h: Harness) {
    let work = farhelm_teststate::tempdir().unwrap();
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &fixture_cmd("fake-agent --script spawner-reparent"),
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
    run_in_shell(
        &h.client,
        tab_chan,
        &mut tab_rx,
        &mut tab_seen,
        "printf 'TAB-%s:%s\\n' SHELL \"$$\"",
        "TAB-SHELL:",
        20,
    )
    .await;
    let tab_shell = extract_pid(&tab_seen, "TAB-SHELL:");

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
        "premise: the tab's reparented daemon carries the session marker"
    );
    assert!(
        crate::host_tabs::ignores_hangup(tab_daemon),
        "premise: the tab daemon ignores SIGHUP"
    );

    h.client.stop_session(&session.id).await.expect("stop");

    // The agent's side of the sweep still works.
    wait_until_pid_gone(agent_pid, 15).await;
    wait_until_pid_gone(agent_daemon, 15).await;

    wait_until_pid_gone(tab_shell, 15).await;
    wait_until_pid_gone(tab_daemon, 15).await;
    assert!(listed_tabs(&h.client, &session.id).await.is_empty());
    assert_eq!(
        expect_detached(&mut tab_rx, 10).await.code,
        farhelm_proto::DetachCode::TabClosed
    );
}

/// A Stop clicked with no host tabs cannot kill a tab opened while the
/// request was in flight. A confirmed Stop covers later tabs too, and a
/// second Stop must close a tab opened after the agent has already ended.
#[farhelm_testtrace::test]
async fn stop_tab_consent_is_checked_for_host_tabs_and_for_an_ended_agent() {
    assert_stop_tab_consent(harness().await).await;
}

/// Pin consent at the request boundary rather than merely testing a tab
/// count helper. A refused Stop leaves both the live agent and tab intact.
async fn assert_stop_tab_consent(h: Harness) {
    let (session, _work) = basic_session(&h).await;
    let (channel, mut seen, mut stream) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    wait_for(&mut stream, &mut seen, "FAKE-AGENT READY", 20).await;
    assert!(
        listed_tabs(&h.client, &session.id).await.is_empty(),
        "premise: the first click displayed no tabs"
    );
    let first = h
        .client
        .open_tab(&session.id)
        .await
        .expect("tab opened after the display");
    let refused = h
        .client
        .stop_session_guarded(&session.id, true, false)
        .await
        .expect_err("a no-tabs click cannot close a new tab");
    assert_eq!(
        refused
            .downcast_ref::<SupervisorError>()
            .map(SupervisorError::kind),
        Some(ErrorKind::Conflict)
    );
    seen.clear();
    h.client
        .send_keys(channel, b"NO-TABS-GUARD-ALIVE\r".to_vec())
        .await;
    wait_for_after(&mut stream, &mut seen, "echo:", "NO-TABS-GUARD-ALIVE", 20).await;
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![first.id.clone()]
    );
    // The confirmation now names tabs. A second tab opened after that
    // display is covered by the same consent, as for session Delete.
    let second = h
        .client
        .open_tab(&session.id)
        .await
        .expect("tab after the confirmation display");
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![first.id, second.id]
    );
    let stopped = h
        .client
        .stop_session_guarded(&session.id, false, false)
        .await
        .expect("confirmed Stop");
    assert_eq!(stopped.notice, None);
    assert!(matches!(
        listed(&h.client, &session.id).await.status,
        SessionStatus::Exited { .. }
    ));
    assert!(listed_tabs(&h.client, &session.id).await.is_empty());
    let reopened = h
        .client
        .open_tab(&session.id)
        .await
        .expect("tabs can open after Stop");
    assert_eq!(listed_tabs(&h.client, &session.id).await, vec![reopened.id]);
    let stopped_again = h
        .client
        .stop_session_guarded(&session.id, false, false)
        .await
        .expect("Stop an ended agent's tab");
    assert_eq!(stopped_again.notice, None);
    assert!(listed_tabs(&h.client, &session.id).await.is_empty());
}

/// An ended-agent Stop display cannot authorize killing a host agent that
/// another client restarted, whether the display named tabs or was tabless.
/// The unchanged tab ID and live shell round trip distinguish a refusal
/// from a Stop that killed only the agent or only its tabs.
#[farhelm_testtrace::test]
async fn ended_agent_stop_consent_refuses_a_restarted_host_agent() {
    assert_ended_agent_stop_consent(harness().await).await;
}

/// Exercise both ended-agent consent states across an actual Restart, then
/// prove the same guards succeed once the agent really ends. Live echoes
/// use this test's post-replay attachments, never old Ready text carried
/// into the restarted terminal's scrollback.
async fn assert_ended_agent_stop_consent(h: Harness) {
    for with_tab in [false, true] {
        let work = farhelm_teststate::tempdir().expect("workdir");
        let session = create_resumable_session(
            &h,
            &work.path().to_string_lossy(),
            &fixture_cmd("fake-agent --script basic"),
            80,
            24,
        )
        .await;
        let (channel, mut seen, mut stream) = h
            .client
            .attach_terminal_live(
                &session.id,
                80,
                24,
                TerminalSelector::Agent,
                "stop-agent-consent",
            )
            .await
            .expect("attach initial agent");
        wait_for(&mut stream, &mut seen, "FAKE-AGENT READY", 20).await;
        let tab = if with_tab {
            Some(h.client.open_tab(&session.id).await.expect("open tab"))
        } else {
            None
        };
        h.client.send_keys(channel, b"quit\r".to_vec()).await;
        wait_for_exit_code(&h.client, &session.id, 0, 20).await;
        let expected_tabs: Vec<String> = tab.iter().map(|tab| tab.id.clone()).collect();
        assert_eq!(
            listed_tabs(&h.client, &session.id).await,
            expected_tabs,
            "premise: the ended-agent display either named this tab or showed no tabs"
        );
        let competing = connect_client(&h.sup).await;
        competing
            .restart_session(&session.id, true)
            .await
            .expect("another client restarts the ended agent");
        let (restarted_channel, mut live, mut restarted_stream) = h
            .client
            .attach_terminal_live(
                &session.id,
                80,
                24,
                TerminalSelector::Agent,
                "stop-agent-consent",
            )
            .await
            .expect("attach restarted agent");
        live.clear();
        h.client
            .send_keys(restarted_channel, b"AGENT-CONSENT-PREMISE\r".to_vec())
            .await;
        wait_for_after(
            &mut restarted_stream,
            &mut live,
            "echo:",
            "AGENT-CONSENT-PREMISE",
            20,
        )
        .await;
        let refused = h
            .client
            .stop_session_guarded(&session.id, !with_tab, true)
            .await
            .expect_err("ended-agent consent cannot stop the restarted agent");
        assert_eq!(
            refused
                .downcast_ref::<SupervisorError>()
                .map(SupervisorError::kind),
            Some(ErrorKind::Conflict)
        );
        assert!(
            format!("{refused:#}").contains("agent is running again"),
            "{refused:#}"
        );
        live.clear();
        h.client
            .send_keys(restarted_channel, b"AGENT-CONSENT-PRESERVED\r".to_vec())
            .await;
        wait_for_after(
            &mut restarted_stream,
            &mut live,
            "echo:",
            "AGENT-CONSENT-PRESERVED",
            20,
        )
        .await;
        assert_eq!(listed_tabs(&h.client, &session.id).await, expected_tabs);
        if let Some(tab) = tab {
            let (tab_channel, mut tab_seen, mut tab_stream) = h
                .client
                .attach_terminal_live(
                    &session.id,
                    80,
                    24,
                    TerminalSelector::Tab { id: tab.id },
                    "stop-agent-consent",
                )
                .await
                .expect("the refused Stop kept the original tab");
            wait_for_shell(
                &h.client,
                tab_channel,
                &mut tab_stream,
                &mut tab_seen,
                "CONSENT-PRESERVED",
            )
            .await;
        }
        h.client
            .send_keys(restarted_channel, b"quit\r".to_vec())
            .await;
        wait_for_exit_code(&h.client, &session.id, 0, 20).await;
        let stopped = h
            .client
            .stop_session_guarded(&session.id, !with_tab, true)
            .await
            .expect("the same consent is valid once the agent has really ended");
        assert_eq!(stopped.notice, None);
        assert!(listed_tabs(&h.client, &session.id).await.is_empty());
        h.client
            .delete_session(&session.id)
            .await
            .expect("delete stopped fixture");
    }
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
    let session = create_resumable_session(
        &h,
        &work.path().to_string_lossy(),
        &fixture_cmd("fake-agent --script basic"),
        80,
        24,
    )
    .await;
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
        .restart_session(&session.id, true)
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
        reason.reason.contains("restart"),
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
/// INCLUSIVELY, so the exclusion that protects tabs from the agent reap must not
/// leak into this path. A daemonized child of the tab is what proves it
/// reached past ending the tab's terminal host, whose hangup would have
/// killed the shell regardless.
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
    let tab_pane_pid = tab_shell_pid(&h, &session.id, &tab.id).await;

    h.client.delete_session(&session.id).await.expect("delete");

    wait_until_pid_gone(tab_pane_pid, 15).await;
    wait_until_pid_gone(tab_daemon, 15).await;
    assert!(
        marked_pids(&session.id).is_empty(),
        "delete must leave nothing carrying this session's marker"
    );
}

/// A resize changes the size of ONLY the terminal whose channel carried it
/// (PLAN_M4.md item 3: resize goes per terminal).
///
/// Why: the agent and each tab are separate terminals of one session, and
/// a viewer sizes the one it shows. A resize that reached a sibling would
/// reflow a terminal nobody is looking at, at a size nobody asked for,
/// which is the bug this test was first written for. Spec: with the agent
/// and a tab, made at 80x24, both attached at 81x25 by one client (premise:
/// both hosts reach 81x25), a resize of the tab's channel to 111x33 brings
/// the tab's host to 111x33 while the agent's stays at 81x25, and a resize
/// of the agent's channel to 90x30 brings the agent's host to 90x30 while
/// the tab's stays at 111x33. Sizes are read from each terminal host's
/// runtime, which reports a size once the host's own resize record has
/// arrived.
#[farhelm_testtrace::test]
async fn a_resize_reflows_only_the_named_terminal() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let _cleanup = MarkerCleanupGuard::new(session.id.clone());

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let agent_host = h
        .sup
        .agent_host_for_test(&session.id)
        .await
        .expect("the agent runs in a terminal host");
    let tab_host = crate::host_tabs::tab_host(&h.sup, &session.id, &tab.id).await;

    let (agent_chan, initial_replay, mut agent_rx) = h
        .client
        .attach_terminal_live(&session.id, 81, 25, TerminalSelector::Agent, "one-client")
        .await
        .expect("attach the agent");
    let mut agent_seen = initial_replay;
    wait_for(&mut agent_rx, &mut agent_seen, "FAKE-AGENT READY", 20).await;
    let (tab_chan, initial_replay, mut tab_rx) = h
        .client
        .attach_terminal_live(
            &session.id,
            81,
            25,
            TerminalSelector::Tab { id: tab.id.clone() },
            "one-client",
        )
        .await
        .expect("attach the tab");
    let mut tab_seen = initial_replay;
    wait_for_shell(&h.client, tab_chan, &mut tab_rx, &mut tab_seen, "READY").await;
    // Premise: both attaches sized their terminals (81x25, not the 80x24
    // the session and its tab were made at), so each terminal acts on the
    // resizes of its own channel and a size seen later can only have come
    // from the resizes below.
    wait_for_host_size(&agent_host, "the agent", "81x25").await;
    wait_for_host_size(&tab_host, "the tab", "81x25").await;

    h.client.resize(&session.id, tab_chan, 111, 33).await;
    wait_for_host_size(&tab_host, "the tab", "111x33").await;
    assert_eq!(
        host_size(&agent_host),
        "81x25",
        "resizing a tab must not resize the agent's terminal"
    );

    h.client.resize(&session.id, agent_chan, 90, 30).await;
    wait_for_host_size(&agent_host, "the agent", "90x30").await;
    assert_eq!(
        host_size(&tab_host),
        "111x33",
        "resizing the agent terminal must not resize a tab's terminal"
    );
}

/// The size a terminal host's runtime has in force, as "COLSxROWS": the
/// size of the latest resize record the host sent back.
fn host_size(host: &farhelm_supervisor::hosts::HostTerminal) -> String {
    let size = host.size();
    format!("{}x{}", size.cols, size.rows)
}

/// Wait until `host` (`what`, for the failure message) has `expected` in
/// force ([`host_size`]), failing the test after ten seconds. Woken by any
/// change of the runtime's status and otherwise re-checked every 100 ms,
/// since the size is the emulator's and not a status field of its own. A
/// runtime that has ended (its status channel closed) can change size no
/// more, so the wait stops there and fails at once if the size is not the
/// one expected.
async fn wait_for_host_size(
    host: &farhelm_supervisor::hosts::HostTerminal,
    what: &str,
    expected: &str,
) {
    let mut status = host.subscribe();
    let reached = tokio::time::timeout(Duration::from_secs(10), async {
        while host_size(host) != expected {
            if let Ok(Err(_)) =
                tokio::time::timeout(Duration::from_millis(100), status.changed()).await
            {
                return;
            }
        }
    })
    .await;
    assert!(
        reached.is_ok() && host_size(host) == expected,
        "{what}'s terminal never reached {expected} (last: {})",
        host_size(host)
    );
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
        .send_keys(agent_chan, b"for-the-agent-only\r".to_vec())
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
/// Every terminal is its own host with its own stream, so
/// the claim is that the supervisor's per-attachment flow control and stall
/// handling stay per terminal: a wedged tab viewer must not hold up the
/// agent's stream, the terminal the user is actually looking at. The agent
/// is round-tripped repeatedly for longer than the stall window that could
/// interfere, so a check that finished early cannot miss the interaction.
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
        .send_keys(
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
            .send_keys(agent_chan, format!("{marker}\r").into_bytes())
            .await;
        wait_for(&mut agent_rx, &mut agent_seen, &marker, 10).await;
        // sleep-ok: spread repeated input round-trips across the intentional stalled-tab window
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
    /// The closing marker must arrive before parsing: opening markers alone
    /// allow a split value to look truncated, or even like an unknown shell.
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
            &format!("printf 'EN%s[%s]%s\\n' V \"${RC_MARKER_VAR}\" VNE"),
            "]VNE",
            20,
        )
        .await;
        h.client.detach(chan).await;
        let text = String::from_utf8_lossy(&seen).into_owned();
        text.split("ENV[")
            .nth(1)
            .and_then(|rest| rest.split_once("]VNE").map(|(value, _)| value))
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
