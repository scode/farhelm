//! Terminal tabs in terminal hosts: opening one beside a host agent, the tab
//! strip's order, attaching and closing, the dead-at-open refusal, a failed
//! open leaving nothing, the reap driven by the host's exit, Delete ending
//! tab hosts, a restarted supervisor keeping them, and viewer ownership.
//!
//! Every supervisor here is the e2e suite's in-process one, starting hosts
//! from the built `farhelm` (`host_creates::host_seams`), never inside this
//! test process.
//!
//! Tab shells are `/bin/sh` with a private `HOME`, so no startup file of the
//! user running the tests is involved. Each test ends with a Delete of every
//! session it made, which ends their hosts through the supervisor's own
//! runtimes, so that no runtime's connection is left to fight the state
//! directory's cleanup.

use crate::harness::*;
use crate::host_creates::host_seams;
use crate::host_restarts::{Stack, agent_host, retire, type_line, wait_until_caught_up};
use crate::host_terminals::{WAIT_SECS, host_process, wait_for_liveness, wait_for_screen};
use crate::terminal_tabs::{listed_tabs, notifying_gate};
use farhelm_supervisor::hosts::{HostTerminal, Liveness};
use std::path::{Path, PathBuf};

// ---- Fixtures -------------------------------------------------------------------

/// The size every session here is created at, which is not the 80x24 a tab
/// host would get without the agent's size, so a tab started at the agent's
/// size is told apart from one started at the default.
const COLS: u16 = 100;
const ROWS: u16 = 30;

/// [`host_seams`] with `/bin/sh` as every launch's shell and `home` as its
/// `HOME`, so tab shells are quick to start and read none of the test
/// user's startup files.
pub(crate) fn tab_seams(home: &Path) -> SupervisorSeams {
    SupervisorSeams {
        launch_shell: Some("/bin/sh".into()),
        launch_env: vec![("HOME".into(), home.to_string_lossy().into_owned())],
        ..host_seams()
    }
}

/// [`tab_seams`] with every orderly stop checkpointing its hosts however
/// little output they have since their last snapshot.
///
/// The shipped policy skips a stop's checkpoint for a tail under its
/// minimum (the next supervisor replays the tail instead), so a tab that
/// echoed one short line would get no snapshot at all; a test about
/// recovering a tab from its stop-time snapshot needs this, as the agent
/// restart tests do (`host_restarts.rs`, `checkpointing_host_seams`).
fn checkpointing_tab_seams(home: &Path) -> SupervisorSeams {
    let mut seams = tab_seams(home);
    seams.faults.host_terminal_options = Some(Arc::new(|options| {
        options.checkpoints.stop_min_bytes = 0;
    }));
    seams
}

/// Create a session running the fake agent in `work`, at [`COLS`] x
/// [`ROWS`].
pub(crate) async fn create(client: &SupervisorClient, work: &Path) -> SessionInfo {
    client
        .create_session(
            &work.to_string_lossy(),
            &fixture_cmd("fake-agent --script basic"),
            None,
            COLS,
            ROWS,
        )
        .await
        .expect("create")
}

/// The runtime of `session_id`'s host tab `tab_id`.
pub(crate) async fn tab_host(sup: &Supervisor, session_id: &str, tab_id: &str) -> HostTerminal {
    sup.tab_host_for_test(session_id, tab_id)
        .await
        .expect("the tab runs in a terminal host")
}

/// The launch record a tab host's start writes.
pub(crate) fn tab_record(state: &Path, session_id: &str, tab_id: &str) -> PathBuf {
    state
        .join("launch")
        .join(format!("{session_id}.tab-{tab_id}.host"))
}

/// The socket name a launch record names.
fn recorded_socket_name(record: &Path) -> String {
    std::fs::read_to_string(record)
        .expect("the launch record")
        .lines()
        .find_map(|line| line.strip_prefix("socket "))
        .expect("the record names its socket")
        .to_string()
}

/// The sockets in `state`'s host socket directory, by name.
pub(crate) fn host_socket_names(state: &Path) -> Vec<String> {
    use std::os::unix::fs::FileTypeExt as _;
    let Ok(entries) = std::fs::read_dir(state.join("hosts")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .file_type()
                .ok()?
                .is_socket()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    names
}

/// Every live process whose environment marks it as one of `session_id`'s
/// tabs (`FARHELM_SESSION_ID` and a `FARHELM_TAB_ID`): what a tab leaves
/// running.
fn tab_marked_processes(session_id: &str) -> Vec<u32> {
    let session = format!("FARHELM_SESSION_ID={session_id}");
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            std::fs::read(format!("/proc/{pid}/environ")).is_ok_and(|raw| {
                let vars: Vec<&[u8]> = raw.split(|byte| *byte == 0).collect();
                vars.contains(&session.as_bytes())
                    && vars.iter().any(|var| var.starts_with(b"FARHELM_TAB_ID="))
            })
        })
        .filter(|pid| !process_is_gone(*pid))
        .collect()
}

/// Attach `session_id`'s `terminal` as a client that can show grids, under
/// `lease`, and require a grid stream.
async fn attach_grid(
    client: &SupervisorClient,
    session_id: &str,
    terminal: TerminalSelector,
    lease: &str,
) -> TermStream {
    let (_channel, stream) = client
        .attach_terminal(session_id, COLS, ROWS, terminal, lease)
        .await
        .expect("a grid attach");
    stream
}

/// Read a grid stream until a screen row it sends shows `text` and its
/// catch-up is complete.
async fn wait_for_grid_text(stream: &mut TermStream, text: &str) {
    let mut shown = false;
    let mut replay_complete = false;
    let waited = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while !(shown && replay_complete) {
            match stream.recv().await {
                Some(TermEvent::Data(bytes)) => {
                    let message =
                        farhelm_proto::grid::GridOutput::decode(&bytes).expect("a grid message");
                    if let farhelm_proto::grid::GridOutput::Rows { rows, .. } = &message {
                        shown |= rows.iter().any(|row| {
                            let line: String =
                                row.row.runs.iter().map(|run| run.text.as_str()).collect();
                            line.contains(text)
                        });
                    }
                }
                Some(TermEvent::ReplayComplete) => replay_complete = true,
                Some(TermEvent::Detached(detach)) => panic!("detached: {detach:?}"),
                None => panic!("the attachment ended"),
            }
        }
    })
    .await;
    assert!(
        waited.is_ok(),
        "{text:?} shown {shown}, replay complete {replay_complete}"
    );
}

/// Wait until `condition` answers true, failing with `what` and the last
/// answer's `detail` after [`WAIT_SECS`]: the one poll for state no event
/// announces (the process table, files, a listing).
pub(crate) async fn wait_for_condition<F, Fut>(what: &str, mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(WAIT_SECS);
    loop {
        let Err(detail) = condition().await else {
            return;
        };
        assert!(tokio::time::Instant::now() < deadline, "{what}: {detail}");
        // sleep-ok: polling interval for process-table, filesystem and listing state no event announces.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The host processes serving sockets in `state`'s socket directory, by
/// their command lines (never by connecting, which would take one over).
fn host_pids(state: &Path) -> Vec<u32> {
    let mut pids = crate::host_creates::host_processes_in(state);
    pids.sort_unstable();
    pids
}

/// The tab records in `state`'s launch directory, by name.
fn tab_records(state: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(state.join("launch"))
        .expect("the launch directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tab-"))
        .collect();
    names.sort();
    names
}

/// Write `body` as the launch shell at `path`: an agent's launch
/// (`shell -l -i -c <command>`) runs its command under `/bin/sh` as before,
/// and a tab's (`shell -l -i`) runs `body`.
fn launch_shell(path: &Path, body: &str) -> PathBuf {
    crate::host_creates::script(
        path,
        &format!("case \"$3\" in -c) exec /bin/sh -c \"$4\";; esac\n{body}"),
    )
}

/// Whether process `pid` ignores SIGHUP, from `/proc/<pid>/status`.
pub(crate) fn ignores_hangup(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("SigIgn:"))
                .and_then(|mask| u64::from_str_radix(mask.trim(), 16).ok())
        })
        .is_some_and(|mask| mask & (1 << (libc::SIGHUP - 1)) != 0)
}

/// Whether process `pid`'s environment carries any `FARHELM_` variable.
fn carries_markers(pid: u32) -> bool {
    std::fs::read(format!("/proc/{pid}/environ")).is_ok_and(|raw| {
        raw.split(|byte| *byte == 0)
            .any(|var| var.starts_with(b"FARHELM_"))
    })
}

// ---- Tests ----------------------------------------------------------------------

/// A tab beside a host agent opens in a host of its own at the agent's
/// size, lists, attaches as a grid, and closes leaving nothing; a takeover
/// displaces the previous client's agent and tab attachments together.
///
/// Why: this is the switch's whole effect on tabs (the plan's PR 18), and a
/// host agent's session has no tmux session at all, so its only tabs are
/// hosts. The size keeps a tab's first attach from repainting a torn frame
/// (BUGS_BURNDOWN.md issue 4, which the tmux tab's sizing exists for); the
/// record is how Delete and a restarted supervisor find the host; and
/// SPEC.md's one attached client per session must hold across terminals of
/// every kind. Spec: with the switch on, opening a tab of a created host
/// agent's session succeeds and the session lists that one tab; its runtime
/// is not the agent's, its host is at the agent's size, and the tab's
/// launch record names its socket; a line typed into it is echoed. A grid
/// client "a" attaches the agent and the tab; a grid client "b" attaching
/// the tab displaces both of "a"'s attachments as taken over. Closing the
/// tab detaches "b" as tab-closed; the tab's host process, its socket, its
/// record and its shell's processes are gone, the session lists no tab, and
/// the agent's host runs on.
#[farhelm_testtrace::test]
async fn a_host_tab_beside_a_host_agent_opens_attaches_and_closes() {
    let home = farhelm_teststate::tempdir().expect("home");
    let h = harness_with_seams(SupervisorTimeouts::default(), tab_seams(home.path())).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create(&h.client, work.path()).await;
    let agent = h
        .sup
        .agent_host_for_test(&session.id)
        .await
        .expect("premise: the agent runs in a host");
    wait_for_screen(&agent, AGENT_READY_MARKER).await;

    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    assert_eq!(
        listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()]
    );
    let host = tab_host(&h.sup, &session.id, &tab.id).await;
    assert!(host != agent, "the tab has a host of its own");
    assert_eq!(
        host.size(),
        agent.size(),
        "the tab starts at the agent's size"
    );
    assert_eq!(
        (host.size().cols, host.size().rows),
        (COLS, ROWS),
        "premise: the agent's size is not the default"
    );
    let record = tab_record(h.state.path(), &session.id, &tab.id);
    assert_eq!(
        std::ffi::OsStr::new(&recorded_socket_name(&record)),
        host.socket().file_name().unwrap(),
        "the tab's record names its host's socket"
    );
    let host_pid = host_process(host.socket()).expect("the tab's host process serves its socket");
    type_line(&host, "echo TAB-SAYS-$((40+2))").await;
    wait_for_screen(&host, "TAB-SAYS-42").await;
    assert!(
        !tab_marked_processes(&session.id).is_empty(),
        "premise: the tab's shell carries the tab's markers"
    );

    let first = h.second_client().await;
    let mut agent_stream = attach_grid(&first, &session.id, TerminalSelector::Agent, "a").await;
    let mut first_tab_stream = attach_grid(
        &first,
        &session.id,
        TerminalSelector::Tab { id: tab.id.clone() },
        "a",
    )
    .await;
    wait_for_grid_text(&mut first_tab_stream, "TAB-SAYS-42").await;
    let mut tab_stream = attach_grid(
        &h.client,
        &session.id,
        TerminalSelector::Tab { id: tab.id.clone() },
        "b",
    )
    .await;
    for (which, stream) in [("agent", &mut agent_stream), ("tab", &mut first_tab_stream)] {
        let detach = expect_detached(stream, WAIT_SECS).await;
        assert!(
            detach.code == farhelm_proto::DetachCode::TakenOver,
            "the first client's {which} attachment was taken over: {detach}"
        );
    }

    h.client
        .close_tab(&session.id, &tab.id)
        .await
        .expect("close the tab");
    let detach = expect_detached(&mut tab_stream, WAIT_SECS).await;
    assert!(
        detach.code == farhelm_proto::DetachCode::TabClosed,
        "the tab's viewer learns it closed: {detach}"
    );
    assert!(
        process_is_gone(host_pid),
        "the tab's host is gone at close's return"
    );
    assert!(!host.socket().exists(), "the tab's socket is gone");
    assert!(!record.exists(), "the tab's record is gone");
    assert!(
        tab_marked_processes(&session.id).is_empty(),
        "nothing of the tab's shell runs"
    );
    assert!(listed_tabs(&h.client, &session.id).await.is_empty());
    assert!(
        matches!(agent.liveness(), Liveness::Running { child: Some(_) }),
        "the agent runs on: {:?}",
        agent.liveness()
    );
    h.client.delete_session(&session.id).await.expect("delete");
}

/// A tab whose shell is dead by the time the open would reply is refused
/// with the shell's last words from the emulator, and leaves nothing.
///
/// Why: SPEC.md's every-failed-operation rule, as for a tmux tab: a broken
/// login shell must read as a refused open with its own complaint, never a
/// "successful" tab holding a corpse, and the refusal must not leave a host
/// (which outlives its program) or a record behind. The settle gate makes
/// the death certain before the open looks, as the tmux test's does. Spec:
/// with a launch shell that, for a tab (no `-c`), records its pid, prints
/// `SHELL-REFUSED-TO-START` and exits 9: the open, released once that
/// shell's pid is gone, fails saying the shell was already dead and quoting
/// that line; the session lists no tab; the socket directory holds only the
/// agent's socket, and no tab record is left.
#[farhelm_testtrace::test]
async fn a_tab_shell_dead_at_open_is_refused_with_its_last_words_and_leaves_nothing() {
    let home = farhelm_teststate::tempdir().expect("home");
    let pid_file = home.path().join("tab-shell.pid");
    let shell = home.path().join("dying-shell");
    std::fs::write(
        &shell,
        format!(
            "#!/bin/sh\n\
             case \"$3\" in -c) exec /bin/sh -c \"$4\";; esac\n\
             echo $$ > {}\n\
             echo SHELL-REFUSED-TO-START\n\
             exit 9\n",
            shell_words::quote(pid_file.to_str().unwrap())
        ),
    )
    .expect("write the shell");
    std::fs::set_permissions(
        &shell,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .expect("make the shell executable");
    let (settle_gate, settle_entered, settle_release) = notifying_gate();
    let mut seams = tab_seams(home.path());
    seams.launch_shell = Some(shell.to_string_lossy().into_owned());
    seams.faults.tab_settle_gate = Some(settle_gate);
    let h = harness_with_seams(SupervisorTimeouts::default(), seams).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create(&h.client, work.path()).await;
    let agent = h
        .sup
        .agent_host_for_test(&session.id)
        .await
        .expect("premise: the agent runs in a host");
    wait_for_screen(&agent, AGENT_READY_MARKER).await;
    let sockets_before = host_socket_names(h.state.path());
    let hosts_before = host_pids(h.state.path());

    let (opened, ()) = tokio::join!(h.client.open_tab(&session.id), async {
        tokio::time::timeout(Duration::from_secs(60), settle_entered.notified())
            .await
            .expect("the open never reached the settle; it failed before the gate");
        let pid = wait_for_pid_file(&pid_file, WAIT_SECS).await;
        wait_until_pid_gone(pid, WAIT_SECS).await;
        settle_release.notify_one();
    });
    let refused = format!("{:#}", opened.expect_err("a dead shell refuses the open"));
    assert!(refused.contains("already dead"), "{refused}");
    assert!(refused.contains("SHELL-REFUSED-TO-START"), "{refused}");
    assert!(listed_tabs(&h.client, &session.id).await.is_empty());
    assert_eq!(
        host_socket_names(h.state.path()),
        sockets_before,
        "only the agent's host is left"
    );
    assert_eq!(
        host_pids(h.state.path()),
        hosts_before,
        "no tab host process is left"
    );
    assert!(
        tab_records(h.state.path()).is_empty(),
        "no tab record is left"
    );
    h.client.delete_session(&session.id).await.expect("delete");
}

/// An open that fails after its shell started ends the tab's host and its
/// shell and removes its record.
///
/// Why: a tab host outlives its shell and carries no marker, so a failed
/// open that did not end it would leave a process nothing lists, closes or
/// deletes but the launch record; the seam stands for any failure between
/// the shell starting and the tab being published, which no other input
/// reaches. Spec: with the open failed just before publishing, the open
/// fails with the injected reason saying the tab was removed again; when it
/// returns, the session lists no tab, the socket directory holds only the
/// agent's socket and only the agent's host process runs, no tab record is
/// left, and no process carries the session's tab markers.
#[farhelm_testtrace::test]
async fn an_open_that_fails_after_its_shell_started_leaves_nothing() {
    let home = farhelm_teststate::tempdir().expect("home");
    let mut seams = tab_seams(home.path());
    seams.faults.tab_open_fault = Some(Arc::new(|stage| match stage {
        farhelm_supervisor::service::TabOpenStage::BeforePublishing => {
            Err(anyhow::anyhow!("injected failure before publishing"))
        }
        _ => Ok(()),
    }));
    let h = harness_with_seams(SupervisorTimeouts::default(), seams).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create(&h.client, work.path()).await;
    let agent = h
        .sup
        .agent_host_for_test(&session.id)
        .await
        .expect("premise: the agent runs in a host");
    wait_for_screen(&agent, AGENT_READY_MARKER).await;
    let sockets_before = host_socket_names(h.state.path());
    let hosts_before = host_pids(h.state.path());

    let refused = format!(
        "{:#}",
        h.client
            .open_tab(&session.id)
            .await
            .expect_err("the injected failure refuses the open")
    );
    assert!(
        refused.contains("injected failure before publishing") && refused.contains("removed again"),
        "{refused}"
    );
    assert!(
        tab_marked_processes(&session.id).is_empty(),
        "nothing of the tab's shell runs when the open returns"
    );
    assert_eq!(host_pids(h.state.path()), hosts_before);
    assert_eq!(host_socket_names(h.state.path()), sockets_before);
    assert!(listed_tabs(&h.client, &session.id).await.is_empty());
    assert!(
        tab_records(h.state.path()).is_empty(),
        "no tab record is left"
    );
    h.client.delete_session(&session.id).await.expect("delete");
}

/// A host tab whose shell exits is reaped promptly, driven by its host's
/// exit, with the ticker's own interval far away.
///
/// Why: SPEC.md reaps a tab whose process exits; for a tmux tab tmux's
/// `pane-died` hook wakes the reap, and a host tab must not wait for a tick
/// either (the goal: the exit event drives the reap, no polling). Spec: on
/// a served supervisor whose ticker interval is ten minutes, a host tab whose
/// shell is told to exit disappears from the listing, its host process ends
/// and its socket and record are removed, all well within the wait bound.
#[farhelm_testtrace::test]
async fn an_exited_host_tab_is_reaped_on_its_hosts_exit() {
    let home = farhelm_teststate::tempdir().expect("home");
    let mut seams = tab_seams(home.path());
    seams.ticker_interval = Duration::from_secs(600);
    let h = harness_with_seams(SupervisorTimeouts::default(), seams).await;
    let serving = crate::hook_identity::ServeTask::spawn(&h.sup, h.state.path()).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create(&h.client, work.path()).await;
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let host = tab_host(&h.sup, &session.id, &tab.id).await;
    let host_pid = host_process(host.socket()).expect("the tab's host runs");
    let record = tab_record(h.state.path(), &session.id, &tab.id);
    assert!(record.exists(), "premise: the tab's record exists");
    type_line(&host, "exit").await;

    wait_for_condition("the exited tab was reaped", || async {
        let listed = listed_tabs(&h.client, &session.id).await;
        if listed.is_empty()
            && process_is_gone(host_pid)
            && !record.exists()
            && !host.socket().exists()
        {
            Ok(())
        } else {
            Err(format!(
                "listed {listed:?}, host gone {}, record {}, liveness {:?}",
                process_is_gone(host_pid),
                record.exists(),
                host.liveness()
            ))
        }
    })
    .await;
    h.client.delete_session(&session.id).await.expect("delete");
    serving.stop().await;
}

/// After an orderly stop and a restart, a host tab is listed again and
/// attaches with the screen it had, and keeps working.
///
/// Why: tabs survive supervisor restarts (SPEC.md: only a reboot erases
/// them), and a host tab has no window marker to be rediscovered by: the
/// restarted supervisor must find it by its record and tag, recover its
/// terminal, and drive it, or every update would strand open tab shells.
/// Spec: a host tab of a host agent's session that echoed a line is stopped
/// in order with its supervisor (its host keeps running); the next
/// supervisor on the same state directory lists the tab, holds a runtime for
/// it at the same socket, whose screen once caught up shows the line, and a
/// grid attach to the tab shows the line too; a line typed into it after the
/// restart is echoed. The stop checkpoints every tail
/// ([`checkpointing_tab_seams`]), so the recovery starts from the stop's
/// snapshot rather than replaying the short tail from the host's start.
#[farhelm_testtrace::test]
async fn a_restarted_supervisor_keeps_a_host_tab_attachable_with_its_screen() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(checkpointing_tab_seams(home.path())).await;
    let session = create(&client, stack.work.path()).await;
    let tab = client.open_tab(&session.id).await.expect("open a tab");
    let host = tab_host(&sup, &session.id, &tab.id).await;
    type_line(&host, "echo BEFORE-RESTART-$((1+1))").await;
    wait_for_screen(&host, "BEFORE-RESTART-2").await;
    let socket = host.socket().to_path_buf();
    let host_pid = host_process(&socket).expect("the tab's host runs");

    sup.stop_in_order_for_test().await;
    assert!(
        host.status().checkpoint.is_some(),
        "the stop left the tab's host a snapshot"
    );
    drop(host);
    retire(stack.state.path(), sup, client).await;
    assert!(
        !process_is_gone(host_pid),
        "premise: the tab's host outlives the supervisor"
    );

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    assert_eq!(
        listed_tabs(&client, &session.id).await,
        vec![tab.id.clone()]
    );
    let host = tab_host(&sup, &session.id, &tab.id).await;
    assert_eq!(host.socket(), socket, "the same host");
    wait_until_caught_up(&host).await;
    assert!(
        host.screen_tail(4096).contains("BEFORE-RESTART-2"),
        "{:?}",
        host.screen_tail(4096)
    );
    let mut stream = attach_grid(
        &client,
        &session.id,
        TerminalSelector::Tab { id: tab.id.clone() },
        "",
    )
    .await;
    wait_for_grid_text(&mut stream, "BEFORE-RESTART-2").await;
    type_line(&host, "echo AFTER-RESTART-$((2+2))").await;
    wait_for_screen(&host, "AFTER-RESTART-4").await;
    drop(stream);
    client.delete_session(&session.id).await.expect("delete");
    drop(host);
    retire(stack.state.path(), sup, client).await;
}

/// Ends a held first stage on drop, by its pinned identity, and waits for
/// it to be gone: a failing assertion must not leave the test's held start
/// waiting for good on a gate in a fixture directory that is about to be
/// removed (writing the gate instead would race that removal, since nothing
/// says when the stage has seen it). Declared after the fixture directory,
/// so it runs first. The pid's start time is checked again before the
/// signal, as `PidKillGuard` does, so a reused pid is never signalled; a
/// stage already gone (or a zombie, once it finished) needs nothing.
struct EndHeldStage {
    pid: u32,
    start: Option<u64>,
}

impl EndHeldStage {
    fn arm(pid: u32) -> Self {
        Self {
            pid,
            start: proc_starttime(pid),
        }
    }
}

impl Drop for EndHeldStage {
    fn drop(&mut self) {
        if self.start.is_none() || proc_starttime(self.pid) != self.start {
            return;
        }
        // SAFETY: `kill` has no memory-safety preconditions; the identity
        // was checked just above.
        unsafe { libc::kill(self.pid as libc::pid_t, libc::SIGKILL) };
        let deadline = std::time::Instant::now() + Duration::from_secs(WAIT_SECS);
        while !process_is_gone(self.pid) && std::time::Instant::now() < deadline {
            // sleep-ok: bounded wait in a drop for a killed process to leave the process table.
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// A tab host start interrupted with its first stage holding the spec is
/// settled by the next supervisor's reload, which adopts the host that
/// start brings up rather than leaving it running unseen.
///
/// Why: the launch record exists for exactly this gap: a supervisor that
/// dies after giving the first stage its spec leaves a process that can
/// still bind the socket and start the shell. A reload that read the missing
/// socket as a finished tab would drop the tab while its host comes up
/// later, with nothing listing, closing or reaping it. Spec: with a tab open
/// interrupted after its first stage took the spec (held there by a
/// stand-in host program), the open fails, the tab's record names a first
/// stage that runs, and no tab socket exists. A supervisor constructed next
/// waits for that first stage (the test releases it from the wait's own
/// hook, which must have fired once construction returns), lists the tab,
/// and drives its host; one tab host process runs beside the agent's. Delete
/// then ends both hosts and removes the tab's record.
#[farhelm_testtrace::test]
async fn an_interrupted_tab_start_is_settled_and_adopted_at_restart() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let arm = fixture.path().join("arm");
    let gate = fixture.path().join("gate");
    // Held only once armed, after the agent's own start: it takes the whole
    // spec, then waits for the gate before running the real first stage on
    // it (as the same process, so the recorded identity stays its own).
    let program = crate::host_creates::script(
        &fixture.path().join("farhelm"),
        &format!(
            "spec={spec}.$$\n\
             /bin/cat > \"$spec\"\n\
             if [ -e {arm} ] && /bin/mkdir {held} 2>/dev/null; then\n\
             \x20 while [ ! -e {gate} ]; do /bin/sleep 0.05; done\n\
             fi\n\
             exec {farhelm} \"$@\" < \"$spec\"",
            spec = shell_words::quote(&fixture.path().join("spec").to_string_lossy()),
            arm = shell_words::quote(&arm.to_string_lossy()),
            held = shell_words::quote(&fixture.path().join("held").to_string_lossy()),
            gate = shell_words::quote(&gate.to_string_lossy()),
            farhelm = shell_words::quote(farhelm_bin()),
        ),
    );
    let interrupt_arm = arm.clone();
    let interrupted = Arc::new(AtomicBool::new(false));
    let mut seams = tab_seams(home.path());
    seams.host_program = Some(program.clone());
    seams.faults.host_start_interrupt = Some(Arc::new(move || {
        if interrupt_arm.exists() && !interrupted.swap(true, Ordering::SeqCst) {
            anyhow::bail!("interrupted with the tab's first stage holding its spec");
        }
        Ok(())
    }));
    let (sup, client) = stack.supervisor(seams).await;
    let session = create(&client, stack.work.path()).await;
    let agent = agent_host(&sup, &session.id).await;
    wait_for_screen(&agent, AGENT_READY_MARKER).await;
    let sockets_before = host_socket_names(stack.state.path());
    std::fs::write(&arm, b"").expect("arm the held start");
    client
        .open_tab(&session.id)
        .await
        .expect_err("the interrupted open fails");
    let [record] = tab_records(stack.state.path())
        .try_into()
        .unwrap_or_else(|records| panic!("one tab record: {records:?}"));
    let starter: u32 = std::fs::read_to_string(stack.state.path().join("launch").join(&record))
        .expect("the tab's record")
        .lines()
        .find_map(|line| line.strip_prefix("starter "))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|pid| pid.parse().ok())
        .expect("the record names its first stage");
    let _held = EndHeldStage::arm(starter);
    assert!(
        !process_is_gone(starter),
        "premise: the first stage is held"
    );
    assert_eq!(
        host_socket_names(stack.state.path()),
        sockets_before,
        "premise: no tab socket yet"
    );
    assert!(!gate.exists(), "premise: not released yet");
    drop(agent);
    retire(stack.state.path(), sup, client).await;

    let released = gate.clone();
    let mut seams = tab_seams(home.path());
    seams.host_program = Some(program);
    seams.faults.launch_starter_waiting = Some(Arc::new(move || {
        std::fs::write(&released, b"").expect("release the first stage");
    }));
    let (sup, client) = stack.supervisor(seams).await;
    assert!(gate.exists(), "the reload waited for the held first stage");
    let tab_id = record
        .strip_prefix(&format!("{}.tab-", session.id))
        .and_then(|rest| rest.strip_suffix(".host"))
        .expect("a tab record name")
        .to_string();
    assert_eq!(
        listed_tabs(&client, &session.id).await,
        vec![tab_id.clone()]
    );
    let host = tab_host(&sup, &session.id, &tab_id).await;
    wait_for_liveness(&host, "the tab host's adoption", |liveness| {
        matches!(liveness, Liveness::Running { .. })
    })
    .await;
    let hosts = host_pids(stack.state.path());
    assert_eq!(hosts.len(), 2, "the agent's host and the tab's: {hosts:?}");

    client.delete_session(&session.id).await.expect("delete");
    for pid in hosts {
        assert!(
            process_is_gone(pid),
            "every host is gone at Delete's return"
        );
    }
    assert!(tab_records(stack.state.path()).is_empty());
    drop(host);
    retire(stack.state.path(), sup, client).await;
}

/// A tab open that fails after its host started and before its runtime
/// adopted it still sweeps what the tab's shell started, in the tab's scope
/// and by its marker.
///
/// Why: with no runtime there is no pinned child to root a sweep at, and
/// ending the host only hangs its shell up, which a startup file's
/// background job can ignore; such a job (in the tab's scope, carrying the
/// tab's marker) would outlive a refusal that said the tab was never
/// opened. Spec: with the user manager (else `SKIPPED`) and a tab shell
/// that starts a background job ignoring SIGHUP and records its pid
/// (premise: in a tab scope, ignoring SIGHUP), and the open failed by the
/// seam once that pid is recorded, after the host reported ready and before
/// its adoption: when the open's refusal returns, the job is gone, the tab's
/// scope unit no longer exists, no tab host process or record is left.
#[farhelm_testtrace::test]
async fn a_tab_open_failed_before_adoption_sweeps_its_shells_jobs() {
    let Some(scopes) =
        probed_scope_manager("a_tab_open_failed_before_adoption_sweeps_its_shells_jobs").await
    else {
        return;
    };
    let home = farhelm_teststate::tempdir().expect("home");
    let job_pid = home.path().join("job.pid");
    let shell = launch_shell(
        &home.path().join("job-shell"),
        &format!(
            "/bin/sh -c 'trap \"\" HUP; echo $$ > {pid}.tmp; mv {pid}.tmp {pid}; exec sleep 1000' &\n\
             exec /bin/sh -i",
            pid = shell_words::quote(job_pid.to_str().unwrap()),
        ),
    );
    let unit = Arc::new(std::sync::Mutex::new(None::<(u32, String)>));
    let seen = Arc::clone(&unit);
    let fault_pid = job_pid.clone();
    let mut seams = tab_seams(home.path());
    seams.scopes = scopes.clone();
    seams.launch_shell = Some(shell.to_string_lossy().into_owned());
    seams.faults.tab_open_fault = Some(Arc::new(move |stage| match stage {
        farhelm_supervisor::service::TabOpenStage::BeforeAdoption => {
            // The seam is synchronous: wait here, on the open's own task,
            // for the job's pid, with a bound.
            let deadline = std::time::Instant::now() + Duration::from_secs(WAIT_SECS);
            let pid = loop {
                if let Some(pid) = std::fs::read_to_string(&fault_pid)
                    .ok()
                    .and_then(|text| text.trim().parse::<u32>().ok())
                {
                    break pid;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the job never started"
                );
                // sleep-ok: polling for a pid file the tab's own shell writes, inside a synchronous seam.
                std::thread::sleep(Duration::from_millis(20));
            };
            let cgroup = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap_or_default();
            let scope = cgroup
                .lines()
                .filter_map(|line| line.rsplit('/').next())
                .find(|name| name.ends_with(".scope"))
                .unwrap_or_default()
                .to_string();
            *seen.lock().unwrap() = Some((pid, scope));
            Err(anyhow::anyhow!("injected failure before adoption"))
        }
        _ => Ok(()),
    }));
    let h = harness_with_seams(SupervisorTimeouts::default(), seams).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create(&h.client, work.path()).await;
    let agent = h
        .sup
        .agent_host_for_test(&session.id)
        .await
        .expect("premise: the agent runs in a host");
    wait_for_screen(&agent, AGENT_READY_MARKER).await;
    let hosts_before = host_pids(h.state.path());

    let refused = format!(
        "{:#}",
        h.client
            .open_tab(&session.id)
            .await
            .expect_err("the injected failure refuses the open")
    );
    let (job, scope) = unit.lock().unwrap().clone().expect("the seam saw the job");
    assert!(
        refused.contains("injected failure before adoption"),
        "{refused}"
    );
    assert!(
        scope.contains("-tab-"),
        "premise: the job ran in a tab scope, not {scope:?}"
    );
    assert!(
        process_is_gone(job),
        "the job is gone when the open returns"
    );
    assert_eq!(
        scopes.exists(&scope).await.ok(),
        Some(false),
        "the tab's scope is gone when the open returns"
    );
    assert_eq!(host_pids(h.state.path()), hosts_before);
    assert!(tab_records(h.state.path()).is_empty());
    h.client.delete_session(&session.id).await.expect("delete");
}

/// `serve`'s reload keeps the tab runtimes the constructor's reload adopted
/// and never contacts their hosts, also when it scans for agent hosts.
///
/// Why: every connection to a host takes it over, so a scan that reached a
/// held tab host would cut its runtime off once and spend one of its few
/// reconnects, dropping input in flight. `serve`'s reload scans the socket
/// directory whenever a host row has no runtime (its host was gone at the
/// constructor's reload, say). Spec: one session with a host tab and one
/// whose agent host is killed while no supervisor runs; after a restart,
/// the tab's runtime has reconnected 0 times; once the restarted supervisor
/// serves (premise: the other session has no agent terminal, so its row is
/// scanned for again), the session holds the very same tab runtime, a line
/// typed into it is echoed, and it has still reconnected 0 times.
#[farhelm_testtrace::test]
async fn serve_keeps_held_tab_runtimes_without_contacting_their_hosts() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let session = create(&client, stack.work.path()).await;
    let tab = client.open_tab(&session.id).await.expect("open a tab");
    let other_work = farhelm_teststate::tempdir().expect("workdir");
    let other = create(&client, other_work.path()).await;
    let other_host = agent_host(&sup, &other.id).await;
    let other_pid = host_process(other_host.socket()).expect("the other agent's host runs");
    drop(other_host);
    sup.stop_in_order_for_test().await;
    retire(stack.state.path(), sup, client).await;
    // SAFETY: ending the test's own host process.
    assert_eq!(unsafe { libc::kill(other_pid as i32, libc::SIGKILL) }, 0);
    wait_until_pid_gone(other_pid, WAIT_SECS).await;

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let held = tab_host(&sup, &session.id, &tab.id).await;
    assert_eq!(held.reconnects(), 0);
    assert!(
        sup.agent_host_for_test(&other.id).await.is_none(),
        "premise: the other session's agent terminal is gone, so serve scans for it"
    );
    let serving = crate::hook_identity::ServeTask::spawn(&sup, stack.state.path()).await;
    let served = tab_host(&sup, &session.id, &tab.id).await;
    assert!(
        served == held,
        "serve's reload kept the adopted tab runtime"
    );
    type_line(&served, "echo AFTER-SERVE-$((3+4))").await;
    wait_for_screen(&served, "AFTER-SERVE-7").await;
    assert_eq!(
        served.reconnects(),
        0,
        "nothing took the tab host over from its runtime"
    );
    client.delete_session(&session.id).await.expect("delete");
    client
        .delete_session(&other.id)
        .await
        .expect("delete the other");
    drop((held, served));
    serving.stop().await;
    retire(stack.state.path(), sup, client).await;
}

/// Closing a host tab, Stop and Delete each kill a process under
/// the tab's shell that ignores SIGHUP and carries no marker.
///
/// Why: only a sweep rooted at the tab's child reaches such a process: the
/// host's end only hangs the shell up, the marker sweep cannot see it, and
/// with no user manager there is no scope to kill (the inventory's top risk
/// 7, for tabs; the earlier form of PR 19's mixed-session test had this
/// premise). Spec: with no user manager and a tab shell that starts a
/// background job under `env -i` that ignores SIGHUP and records its pid
/// per tab (premise: running, ignoring SIGHUP, no `FARHELM_` variable),
/// two tabs open; closing the first leaves its job gone when the close
/// returns, Stop leaves the second's gone, and an unconditional Delete
/// leaves a tab opened after Stop's job gone when it returns.
#[farhelm_testtrace::test]
async fn close_stop_and_delete_kill_a_hangup_immune_unmarked_job_of_a_tab() {
    let home = farhelm_teststate::tempdir().expect("home");
    let jobs = home.path().join("jobs");
    std::fs::create_dir(&jobs).expect("the jobs directory");
    let shell = launch_shell(
        &home.path().join("immune-shell"),
        &format!(
            "tab=\"$FARHELM_TAB_ID\"\n\
             /usr/bin/env -i /bin/sh -c \"trap '' HUP; echo \\$\\$ > {dir}/$tab.tmp; mv {dir}/$tab.tmp {dir}/$tab.pid; exec /bin/sleep 1000\" &\n\
             exec /bin/sh -i",
            dir = shell_words::quote(jobs.to_str().unwrap()),
        ),
    );
    let mut seams = tab_seams(home.path());
    seams.launch_shell = Some(shell.to_string_lossy().into_owned());
    let h = harness_with_seams(SupervisorTimeouts::default(), seams).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create(&h.client, work.path()).await;
    let first = h.client.open_tab(&session.id).await.expect("open a tab");
    let second = h.client.open_tab(&session.id).await.expect("open another");
    let job = |tab: &str| {
        let path = jobs.join(format!("{tab}.pid"));
        async move { wait_for_pid_file(&path, WAIT_SECS).await }
    };
    let first_job = job(&first.id).await;
    let second_job = job(&second.id).await;
    for pid in [first_job, second_job] {
        assert!(
            !process_is_gone(pid) && ignores_hangup(pid) && !carries_markers(pid),
            "premise: job {pid} runs, ignores SIGHUP and carries no marker"
        );
    }

    h.client
        .close_tab(&session.id, &first.id)
        .await
        .expect("close the first tab");
    assert!(process_is_gone(first_job), "the closed tab's job is gone");
    assert!(!process_is_gone(second_job), "the other tab's job runs on");
    h.client.stop_session(&session.id).await.expect("stop");
    assert!(
        process_is_gone(second_job),
        "Stop killed the other tab's job"
    );
    let third = h
        .client
        .open_tab(&session.id)
        .await
        .expect("a tab after Stop");
    let third_job = job(&third.id).await;
    assert!(
        !process_is_gone(third_job) && ignores_hangup(third_job) && !carries_markers(third_job),
        "premise: the post-Stop tab's job runs, ignores SIGHUP and carries no marker"
    );
    h.client.delete_session(&session.id).await.expect("delete");
    assert!(
        process_is_gone(third_job),
        "Delete killed the post-Stop tab's job"
    );
}

/// A tab host that cannot be read at restart is kept as a placeholder:
/// listed, its close refused, and ended by an unconditional Delete through
/// its record.
///
/// Why: a host the restarted supervisor cannot read may still run, so it
/// must neither vanish from the strip (a running shell nobody can see) nor
/// be closed blind; Delete is the one verb that ends it, from what the
/// session records (the decision for unreadable tabs, after PR 17's
/// placeholder rules). Spec: a host tab's host is stopped (SIGSTOP) across
/// a restart; the next supervisor lists the tab and holds a placeholder for
/// it; closing it is refused as a conflict saying its host cannot be read;
/// once the agent is stopped, a Delete guarded on nothing alive is refused
/// for the same reason; an unconditional Delete succeeds with the tab's host
/// process gone and its record removed.
#[farhelm_testtrace::test]
async fn an_unreadable_tab_host_at_restart_is_listed_refused_and_deleted() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let session = create(&client, stack.work.path()).await;
    let tab = client.open_tab(&session.id).await.expect("open a tab");
    let host = tab_host(&sup, &session.id, &tab.id).await;
    let tab_pid = host_process(host.socket()).expect("the tab's host runs");
    drop(host);
    sup.stop_in_order_for_test().await;
    retire(stack.state.path(), sup, client).await;
    let stopped = crate::host_restarts::Stopped::stop(tab_pid);

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    assert_eq!(
        listed_tabs(&client, &session.id).await,
        vec![tab.id.clone()]
    );
    let placeholder = tab_host(&sup, &session.id, &tab.id).await;
    assert!(placeholder.is_placeholder(), "{:?}", placeholder.liveness());
    let refused = client
        .close_tab(&session.id, &tab.id)
        .await
        .expect_err("an unreadable tab is not closed");
    assert!(
        format!("{refused:#}").contains("cannot be read"),
        "{refused:#}"
    );
    assert_eq!(
        refused
            .downcast_ref::<SupervisorError>()
            .map(SupervisorError::kind),
        Some(ErrorKind::Conflict)
    );
    // Stop leaves only the unreadable tab, reports it, and still closes a
    // healthy sibling. One host's refusal must not short-circuit the loop.
    let healthy = client
        .open_tab(&session.id)
        .await
        .expect("a healthy sibling");
    let healthy_host = tab_host(&sup, &session.id, &healthy.id).await;
    let healthy_pid = host_process(healthy_host.socket()).expect("the healthy host runs");
    let stopped_outcome = client
        .stop_session_guarded(&session.id, false, false)
        .await
        .expect("stop the agent and readable tabs");
    let notice = stopped_outcome
        .notice
        .expect("the unreadable tab must be reported");
    assert!(
        notice.contains(&tab.id) && notice.contains("cannot be read"),
        "{notice}"
    );
    assert!(
        process_is_gone(healthy_pid),
        "the healthy sibling closed despite the refusal"
    );
    assert!(
        !process_is_gone(tab_pid),
        "the unreadable tab is left alone"
    );
    assert_eq!(
        listed_tabs(&client, &session.id).await,
        vec![tab.id.clone()]
    );
    let guarded = client
        .delete_session_with(&session.id, farhelm_proto::DeleteGuard::NothingAlive)
        .await
        .expect_err("whether the tab runs cannot be told");
    assert!(
        format!("{guarded:#}").contains("cannot be read"),
        "{guarded:#}"
    );
    client.delete_session(&session.id).await.expect("delete");
    assert!(
        process_is_gone(tab_pid),
        "the unreadable tab's host is gone"
    );
    assert!(tab_records(stack.state.path()).is_empty());
    drop((stopped, placeholder));
    retire(stack.state.path(), sup, client).await;
}

/// A tab whose runtime can no longer read its host is "cannot tell" to a
/// guarded Delete, refuses its close, and is ended by an unconditional
/// Delete.
///
/// Why: a runtime that becomes unreadable says nothing current about its
/// shell (the plan's D-16); a guard that counted it as closed would let an
/// unconfirmed Delete kill a live shell, and a close that went ahead would
/// act blind. Spec: a host tab's host is taken over from its runtime four
/// times (each time it takes it back, until its reconnect limit makes it
/// unavailable); then closing the tab is refused as a conflict, and once
/// the agent is stopped, a Delete guarded on nothing alive is refused saying
/// the tab's host cannot be read, and an unconditional Delete succeeds with
/// the tab's host process gone.
#[farhelm_testtrace::test]
async fn a_tab_runtime_that_becomes_unreadable_is_untold_and_deleted() {
    let home = farhelm_teststate::tempdir().expect("home");
    let h = harness_with_seams(SupervisorTimeouts::default(), tab_seams(home.path())).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create(&h.client, work.path()).await;
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let host = tab_host(&h.sup, &session.id, &tab.id).await;
    let tab_pid = host_process(host.socket()).expect("the tab's host runs");
    for taken in 1..=4u64 {
        let connection = farhelm_supervisor::hosts::HostConnection::connect(host.socket())
            .await
            .expect("take the tab's host over");
        if taken < 4 {
            // Counted only once the runtime has connected again and
            // resumed, so its connection has taken the host back from this
            // one before the next takeover.
            wait_for_condition("the runtime takes its host back", || async {
                if host.reconnects() >= taken {
                    Ok(())
                } else {
                    Err(format!("{} reconnects", host.reconnects()))
                }
            })
            .await;
        }
        drop(connection);
    }
    wait_for_liveness(&host, "the runtime giving up", |liveness| {
        matches!(liveness, Liveness::Unavailable { .. })
    })
    .await;

    let refused = h
        .client
        .close_tab(&session.id, &tab.id)
        .await
        .expect_err("an unreadable tab is not closed");
    assert_eq!(
        refused
            .downcast_ref::<SupervisorError>()
            .map(SupervisorError::kind),
        Some(ErrorKind::Conflict),
        "{refused:#}"
    );
    let stopped = h
        .client
        .stop_session_guarded(&session.id, false, false)
        .await
        .expect("stop");
    let notice = stopped.notice.expect("Stop must report the unreadable tab");
    assert!(
        notice.contains(&tab.id) && notice.contains("cannot be read"),
        "{notice}"
    );
    let guarded = h
        .client
        .delete_session_with(&session.id, farhelm_proto::DeleteGuard::NothingAlive)
        .await
        .expect_err("whether the tab runs cannot be told");
    assert!(
        format!("{guarded:#}").contains(&format!("terminal tab {}'s host cannot be read", tab.id)),
        "{guarded:#}"
    );
    h.client.delete_session(&session.id).await.expect("delete");
    assert!(
        process_is_gone(tab_pid),
        "the tab's host is gone at Delete's return"
    );
}

/// A tab whose host is killed under it is reaped: the tab leaves the strip
/// and its socket and record go.
///
/// Why: a killed host announces no exit, only its end, and leaves its
/// socket file and record behind; a reap that waited for an exit would keep
/// listing a tab with nothing behind it, and its record would name a dead
/// socket for good. Spec: on a served supervisor whose ticker interval is
/// ten minutes, a host tab's host is killed (SIGKILL); the tab disappears
/// from the listing and its socket and record are removed within the wait
/// bound.
#[farhelm_testtrace::test]
async fn a_tab_whose_host_is_killed_is_reaped() {
    let home = farhelm_teststate::tempdir().expect("home");
    let mut seams = tab_seams(home.path());
    seams.ticker_interval = Duration::from_secs(600);
    let h = harness_with_seams(SupervisorTimeouts::default(), seams).await;
    let serving = crate::hook_identity::ServeTask::spawn(&h.sup, h.state.path()).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create(&h.client, work.path()).await;
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let host = tab_host(&h.sup, &session.id, &tab.id).await;
    let host_pid = host_process(host.socket()).expect("the tab's host runs");
    let record = tab_record(h.state.path(), &session.id, &tab.id);
    // SAFETY: killing the test's own tab host.
    assert_eq!(unsafe { libc::kill(host_pid as i32, libc::SIGKILL) }, 0);

    wait_for_condition("the killed tab was reaped", || async {
        let listed = listed_tabs(&h.client, &session.id).await;
        if listed.is_empty() && !record.exists() && !host.socket().exists() {
            Ok(())
        } else {
            Err(format!(
                "listed {listed:?}, record {}, socket {}, liveness {:?}",
                record.exists(),
                host.socket().exists(),
                host.liveness()
            ))
        }
    })
    .await;
    h.client.delete_session(&session.id).await.expect("delete");
    serving.stop().await;
}

/// A close whose end of the tab's host fails leaves the tab for the reap,
/// which ends it on a later pass, no sooner than a second after the failure.
///
/// Why: once a close could not confirm its host ended, the tab's runtime is
/// stopped and will announce nothing more, so only the reap can finish the
/// job; and an end that keeps failing must not be retried on every pass,
/// where it would hold its session's lifecycle claim and a reap slot almost
/// all the time. Spec: on a served supervisor ticking every 100 ms, with
/// the first end of a tab's host made to fail (a seam): the user's close
/// fails with that reason, and the session still holds the tab (its shell,
/// swept by the close, is gone, so the strip hides it), its record is still
/// there and its host still runs; then the reap ends the host, removes the
/// record and forgets the tab, at least 900 ms after the close returned
/// (the first retry waits a second, where ticks come every 100 ms), with the
/// seam asked exactly twice.
#[farhelm_testtrace::test]
async fn a_failed_tab_end_is_retried_by_the_reap_after_its_backoff() {
    let home = farhelm_teststate::tempdir().expect("home");
    let ends = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&ends);
    let mut seams = tab_seams(home.path());
    seams.ticker_interval = Duration::from_millis(100);
    seams.faults.tab_host_end_fault = Some(Arc::new(move || {
        if counted.fetch_add(1, Ordering::SeqCst) == 0 {
            Err("injected end failure".to_string())
        } else {
            Ok(())
        }
    }));
    let h = harness_with_seams(SupervisorTimeouts::default(), seams).await;
    let serving = crate::hook_identity::ServeTask::spawn(&h.sup, h.state.path()).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = create(&h.client, work.path()).await;
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let host = tab_host(&h.sup, &session.id, &tab.id).await;
    let host_pid = host_process(host.socket()).expect("the tab's host runs");
    let record = tab_record(h.state.path(), &session.id, &tab.id);

    let refused = h
        .client
        .close_tab(&session.id, &tab.id)
        .await
        .expect_err("the injected end failure fails the close");
    let failed_at = tokio::time::Instant::now();
    assert!(
        format!("{refused:#}").contains("injected end failure"),
        "{refused:#}"
    );
    // The close's sweep ended the shell, so the strip hides the tab; the
    // tab itself, its record and its host remain for the retry.
    assert!(
        h.sup
            .tab_host_for_test(&session.id, &tab.id)
            .await
            .is_some(),
        "the session still holds the tab"
    );
    assert!(record.exists(), "the record stays for the retry");
    assert!(!process_is_gone(host_pid), "the host still runs");

    wait_for_condition("the reap ended the tab", || async {
        let held = h
            .sup
            .tab_host_for_test(&session.id, &tab.id)
            .await
            .is_some();
        if !held && process_is_gone(host_pid) && !record.exists() {
            Ok(())
        } else {
            Err(format!(
                "held {held}, host gone {}, record {}",
                process_is_gone(host_pid),
                record.exists()
            ))
        }
    })
    .await;
    assert!(listed_tabs(&h.client, &session.id).await.is_empty());
    assert!(
        failed_at.elapsed() >= Duration::from_millis(900),
        "the retry came {:?} after the failure, before its backoff",
        failed_at.elapsed()
    );
    assert_eq!(ends.load(Ordering::SeqCst), 2, "one failure, one retry");
    h.client.delete_session(&session.id).await.expect("delete");
    serving.stop().await;
}
