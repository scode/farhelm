//! Restart of an agent that runs in a terminal host: the new launch starts
//! in a new host seeded with the old terminal's history, the old host is
//! ended once the new launch is confirmed, a replacement that fails is
//! unwound, a restart interrupted by a crash converges at the next start
//! (the superseded-host rule), and a restart that loses to Delete, or a
//! Delete after an interrupted restart, leaves no host behind.
//!
//! Every supervisor here is the e2e suite's in-process one with the
//! internal switch on (`host_creates::host_seams`), so creates and restarts
//! start their agents in hosts started from the built `farhelm` (or a
//! wrapper around it), never inside this test process. A crash is simulated
//! with the restart's fault seam (`FaultHooks::host_restart_fault`), which
//! makes the restart return at once with no cleanup, and then the
//! supervisor is dropped and the next one constructed on the same state
//! directory (`host_restarts::Stack`), as a real crash and restart would:
//! the hosts keep running in between.
//!
//! The agent is a script that prints sixty numbered history lines, sets
//! bracketed paste, hides the cursor, sets a title and prints a last line
//! on its first run, and on every later run (a Restart's resume) prints
//! only `RESUMED RUN`: so what the new host holds tells the seed's history
//! from the new program's output, and the old screen's last line tells
//! whether the previous visible frame was painted back.
//!
//! Each test ends the hosts it leaves through the last supervisor's runtime
//! before its state directory drops, so that runtime's connection does not
//! fight the directory's own cleanup.

use crate::harness::*;
use crate::host_creates::{
    end_runtime, host_processes_in, host_seams, script, silent_host_program,
};
use crate::host_restarts::{Stack, Stopped, agent_host, listed, retire, stored_row, type_line};
use crate::host_terminals::{WAIT_SECS, host_process, wait_for_liveness, wait_for_screen};
use farhelm_supervisor::hosts::{HostTerminal, Liveness};
use farhelm_supervisor::service::{HostRestartFault, HostRestartStage};
use farhelm_supervisor::store::TerminalKind;
use std::path::{Path, PathBuf};

// ---- Fixtures -------------------------------------------------------------------

/// The last line the agent's first run prints, which stays on its visible
/// screen: never in the restarted terminal.
pub(crate) const OLD_SCREEN_MARK: &str = "OLD SCREEN MARK";

/// What every later run of the agent prints.
pub(crate) const RESUMED: &str = "RESUMED RUN";

/// Write the history agent (module docs) into `dir` and return the command
/// that runs it. Its first run leaves a marker beside it, which is how
/// later runs know they are resumes; every run appends a line to
/// `dir/runs`, which counts the agents started. With `last_words`, the
/// first run, once stopped (SIGTERM or SIGHUP), prints 30 numbered
/// `last words` lines before it exits, enough that the first ones scroll
/// into history: the old program's last output.
pub(crate) fn history_agent_with(dir: &Path, last_words: bool) -> String {
    let marker = dir.join("agent-ran");
    let runs = dir.join("runs");
    let wait = if last_words {
        "trap 'i=1; while [ \"$i\" -le 30 ]; do printf \"last words %02d\\n\" \"$i\"; \
         i=$((i + 1)); done; exit 0' TERM HUP\n/bin/sleep 600 &\nwait"
    } else {
        "exec /bin/sleep 600"
    };
    let program = script(
        &dir.join("history-agent"),
        &format!(
            "printf 'run\\n' >> {runs}\n\
             if [ -e {marker} ]; then printf '{RESUMED}\\n'; exec /bin/sleep 600; fi\n\
             : > {marker}\n\
             i=1\n\
             while [ \"$i\" -le 60 ]; do printf 'history line %02d\\n' \"$i\"; i=$((i + 1)); done\n\
             printf '\\033]2;old title\\007\\033[?2004h\\033[?25l{OLD_SCREEN_MARK}\\n'\n\
             {wait}",
            marker = shell_words::quote(&marker.to_string_lossy()),
            runs = shell_words::quote(&runs.to_string_lossy()),
        ),
    );
    shell_words::quote(&program.to_string_lossy()).into_owned()
}

/// Create the history agent's session through `client` on `sup`, in
/// `work`, and wait until its first run has set its modes and printed its
/// last line: the premise every restart here starts from.
async fn history_session(
    client: &SupervisorClient,
    sup: &Supervisor,
    work: &Path,
) -> (SessionInfo, HostTerminal) {
    history_session_with(client, sup, work, false).await
}

/// [`history_session`] with the agent's last words as `last_words` says
/// ([`history_agent_with`]).
async fn history_session_with(
    client: &SupervisorClient,
    sup: &Supervisor,
    work: &Path,
    last_words: bool,
) -> (SessionInfo, HostTerminal) {
    let session = create_resumable_session_on(
        client,
        sup,
        &work.to_string_lossy(),
        &history_agent_with(work, last_words),
        80,
        24,
    )
    .await;
    let host = agent_host(sup, &session.id).await;
    wait_for_screen(&host, OLD_SCREEN_MARK).await;
    let mut status = host.subscribe();
    let premise = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while !(host.modes().bracketed_paste && host.title() == "old title") {
            if status.changed().await.is_err() {
                break;
            }
        }
    })
    .await;
    assert!(
        premise.is_ok() && host.modes().bracketed_paste && !host.modes().cursor_visible,
        "premise: the first run set its modes and title: {:?}, {:?}",
        host.modes(),
        host.title()
    );
    (session, host)
}

/// The lines in `host`'s history (above its screen), oldest first, as text.
pub(crate) async fn history_text(host: &HostTerminal) -> String {
    host.read(|emulator| {
        use farhelm_terminal_emulator::alacritty_terminal::grid::Dimensions as _;
        use farhelm_terminal_emulator::alacritty_terminal::index::{Column, Line};
        let grid = emulator.term().grid();
        let mut text = String::new();
        for back in (1..=grid.history_size()).rev() {
            let row = &grid[Line(-(back as i32))];
            for column in 0..grid.columns() {
                text.push(row[Column(column)].c);
            }
            text.push('\n');
        }
        text
    })
    .await
    .expect("the emulator can be read")
}

/// The socket names in `state`'s host socket directory, sorted.
pub(crate) fn host_sockets_in(state: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(state.join("hosts")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            use std::os::unix::fs::FileTypeExt as _;
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

/// The launch host records in `state`'s launch directory, by file name.
pub(crate) fn host_records_in(state: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(state.join("launch")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().to_string_lossy().into_owned();
            name.ends_with(".host").then_some(name)
        })
        .collect();
    names.sort();
    names
}

/// The launch record of `session_id`'s launch `generation`.
pub(crate) fn record_path(state: &Path, session_id: &str, generation: i64) -> PathBuf {
    state
        .join("launch")
        .join(format!("{session_id}.{generation}.host"))
}

/// A socket's name, for comparing with a row's `pane`.
pub(crate) fn socket_name(socket: &Path) -> String {
    socket
        .file_name()
        .expect("a socket path names a file")
        .to_string_lossy()
        .into_owned()
}

/// The host program for a supervisor whose host starts can be made to fail:
/// the built `farhelm` while `flag` does not exist, and while it does, a
/// first stage that reads its spec and exits 3 (a refusal), or, with
/// `unusable`, the stand-in whose "host" binds the socket and never answers
/// (`host_creates::silent_host_program`), writing its listener's pid to
/// `listener_pid`.
pub(crate) fn failing_host_program(dir: &Path, flag: &Path, unusable: Option<&Path>) -> PathBuf {
    let failing = match unusable {
        Some(listener_pid) => {
            let silent_dir = dir.join("silent");
            std::fs::create_dir_all(&silent_dir).expect("the stand-in's directory");
            let silent = silent_host_program(&silent_dir, listener_pid, true, false);
            format!(
                "exec {} \"$@\"",
                shell_words::quote(&silent.to_string_lossy())
            )
        }
        None => "/bin/cat > /dev/null; exit 3".to_string(),
    };
    script(
        &dir.join("farhelm"),
        &format!(
            "if [ -e {flag} ]; then {failing}; fi\nexec {farhelm} \"$@\"",
            flag = shell_words::quote(&flag.to_string_lossy()),
            farhelm = shell_words::quote(farhelm_bin()),
        ),
    )
}

/// A crash seam that fires once, at `stage`.
pub(crate) fn crash_once_at(stage: HostRestartStage) -> (HostRestartFault, Arc<AtomicBool>) {
    let fired = Arc::new(AtomicBool::new(false));
    let seam: HostRestartFault = {
        let fired = Arc::clone(&fired);
        Arc::new(move |at| {
            if at == stage && !fired.swap(true, Ordering::SeqCst) {
                anyhow::bail!("the supervisor crashed at {at:?}");
            }
            Ok(())
        })
    };
    (seam, fired)
}

/// [`host_seams`] with `crash` installed.
fn crashing_seams(crash: HostRestartFault) -> SupervisorSeams {
    SupervisorSeams {
        faults: FaultHooks {
            host_restart_fault: Some(crash),
            ..FaultHooks::default()
        },
        ..host_seams()
    }
}

/// Require that the host process `pid` ended and its socket file at
/// `socket` went with it.
pub(crate) async fn assert_host_ended(pid: u32, socket: &Path) {
    wait_until_pid_gone(pid, WAIT_SECS).await;
    assert!(
        std::fs::symlink_metadata(socket).is_err(),
        "the ended host's socket file is gone: {}",
        socket.display()
    );
}

// ---- A restart -------------------------------------------------------------------

/// A Restart of a host agent keeps the session's tabs in terminal hosts:
/// the tab, its host, its shell and its launch record survive, and the tab
/// still answers afterwards.
///
/// Why: a tab is not the agent's terminal, and SPEC.md's Restart touches
/// the agent terminal only. A host agent's Restart is where the two meet:
/// it lists the session's launch records (a tab's among them) to settle
/// its agent's earlier starts,
/// sweeps the agent's tree by the session's marker (a tab's shell carries
/// it too), and ends the old agent host, so each of those must leave the
/// tab's host and shell alone. Spec: a host agent session (the history
/// agent, with `/bin/sh` as the launch shell) opens a tab whose shell
/// echoes a line; after a consented Restart, which succeeds with a new
/// agent host showing `RESUMED RUN` and the old agent host gone, the
/// session still lists that one tab, held by the same host at the same
/// socket, whose process still runs, with the same shell; the tab's launch
/// record is still there, and a line typed into the tab is echoed.
#[farhelm_testtrace::test]
async fn a_restart_of_a_host_agent_keeps_its_host_tabs() {
    let home = farhelm_teststate::tempdir().expect("home");
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_shell: Some("/bin/sh".into()),
            launch_env: vec![("HOME".into(), home.path().to_string_lossy().into_owned())],
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session(&h.client, &h.sup, work.path()).await;
    let old_pid = host_process(old.socket()).expect("the old agent host's process");
    let old_socket = old.socket().to_path_buf();
    drop(old);
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let tab_host = h
        .sup
        .tab_host_for_test(&session.id, &tab.id)
        .await
        .expect("premise: the tab runs in a terminal host");
    let tab_socket = tab_host.socket().to_path_buf();
    let tab_pid = host_process(&tab_socket).expect("the tab's host process serves its socket");
    let tab_record = h
        .state
        .path()
        .join("launch")
        .join(format!("{}.tab-{}.host", session.id, tab.id));
    assert!(tab_record.exists(), "premise: the tab's launch record");
    type_line(&tab_host, "echo TAB-BEFORE-$((40+2))").await;
    wait_for_screen(&tab_host, "TAB-BEFORE-42").await;
    let shell_status = tab_host.status();
    let tab_shell = (shell_status.child_pid, shell_status.child_start);
    assert!(
        tab_shell.1.is_some() && !process_is_gone(tab_shell.0),
        "premise: the tab's shell runs, pinned to its start identity"
    );
    drop(tab_host);

    h.client
        .restart_session(&session.id, true)
        .await
        .expect("the restart succeeds");
    let new = agent_host(&h.sup, &session.id).await;
    assert_ne!(new.socket(), old_socket, "a new agent host");
    wait_for_screen(&new, RESUMED).await;
    assert_host_ended(old_pid, &old_socket).await;

    assert_eq!(
        crate::terminal_tabs::listed_tabs(&h.client, &session.id).await,
        vec![tab.id.clone()],
        "the tab is still listed"
    );
    let tab_host = h
        .sup
        .tab_host_for_test(&session.id, &tab.id)
        .await
        .expect("the tab is still held");
    assert_eq!(tab_host.socket(), tab_socket, "by the same host");
    assert!(!process_is_gone(tab_pid), "the tab's host runs on");
    let shell_status = tab_host.status();
    assert_eq!(
        (shell_status.child_pid, shell_status.child_start),
        tab_shell,
        "with the same shell"
    );
    assert!(
        !process_is_gone(tab_shell.0),
        "the tab's shell was not stopped"
    );
    assert!(tab_record.exists(), "the tab's launch record is kept");
    type_line(&tab_host, "echo TAB-AFTER-$((40+3))").await;
    wait_for_screen(&tab_host, "TAB-AFTER-43").await;
    drop((tab_host, new));
    h.client
        .delete_session(&session.id)
        .await
        .expect("the delete ends the hosts");
}

/// Run a host agent's Restart that an orderly stop meets in its window
/// (held at the restart window's gate, with the session off the session
/// map), and require that the stop closes the session's host tab runtime
/// although the map does not hold it, and that the Restart, released after
/// the stop, puts no tab runtime back connected. With `fail`, the Restart's
/// replacement refuses to start, so the session comes back through the
/// failed Restart's restore rather than its publication.
async fn stop_during_a_restart_closes_its_host_tab(fail: bool) {
    let home = farhelm_teststate::tempdir().expect("home");
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let refuse = fixture.path().join("refuse");
    let (gate, entered, release) = crate::terminal_tabs::notifying_gate();
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            launch_shell: Some("/bin/sh".into()),
            launch_env: vec![("HOME".into(), home.path().to_string_lossy().into_owned())],
            host_program: Some(failing_host_program(fixture.path(), &refuse, None)),
            faults: FaultHooks {
                restart_window_gate: Some(gate),
                ..FaultHooks::default()
            },
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session(&h.client, &h.sup, work.path()).await;
    drop(old);
    let tab = h.client.open_tab(&session.id).await.expect("open a tab");
    let tab_host = h
        .sup
        .tab_host_for_test(&session.id, &tab.id)
        .await
        .expect("premise: the tab runs in a terminal host");
    let tab_pid = host_process(tab_host.socket()).expect("the tab's host process");
    type_line(&tab_host, "echo TAB-BEFORE-$((40+2))").await;
    wait_for_screen(&tab_host, "TAB-BEFORE-42").await;
    if fail {
        std::fs::write(&refuse, b"").expect("make the replacement refuse");
    }

    let restart = tokio::spawn({
        let client = Arc::clone(&h.client);
        let id = session.id.clone();
        async move { client.restart_session(&id, true).await }
    });
    tokio::time::timeout(Duration::from_secs(WAIT_SECS), entered.notified())
        .await
        .expect("the restart reaches its window, its session off the map");
    h.sup.stop_in_order_for_test().await;
    let mut status = tab_host.subscribe();
    let closed = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while status.changed().await.is_ok() {}
    })
    .await;
    // Released before asserting, so a failure leaves no Restart held.
    release.notify_one();
    let restarted = tokio::time::timeout(Duration::from_secs(WAIT_SECS), restart)
        .await
        .expect("the restart finishes")
        .expect("the restart task");
    assert!(
        closed.is_ok(),
        "the stop closed the restarting session's tab runtime"
    );
    assert_eq!(
        restarted.is_err(),
        fail,
        "the restart's outcome: {restarted:?}"
    );
    assert!(
        !process_is_gone(tab_pid),
        "the stop leaves the tab's host running"
    );
    let back = h
        .sup
        .tab_host_for_test(&session.id, &tab.id)
        .await
        .expect("the tab is back with its session");
    let mut back_status = back.subscribe();
    let back_closed = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while back_status.changed().await.is_ok() {}
    })
    .await;
    assert!(back_closed.is_ok(), "no tab runtime came back connected");
    drop((tab_host, back));
    h.client
        .delete_session(&session.id)
        .await
        .expect("the delete ends the hosts");
}

/// An orderly stop during a host agent's Restart closes the session's host
/// tab runtime, and the Restart's publication after it brings none back
/// connected.
///
/// Why: the sol review's finding against the port onto tabs. A Restart takes
/// its session off the session map for its window, the stop gathers the
/// runtimes it checkpoints and closes from that map, and the tabs ride in
/// the session's cells: missed by the stop and brought back by the
/// publication, their connections and encodes outlived the stop, contrary
/// to its contract. Spec: see [`stop_during_a_restart_closes_its_host_tab`],
/// for a Restart that succeeds.
#[farhelm_testtrace::test]
async fn a_stop_during_a_restart_closes_its_host_tab() {
    stop_during_a_restart_closes_its_host_tab(false).await;
}

/// The same as [`a_stop_during_a_restart_closes_its_host_tab`], for a
/// Restart whose replacement refuses to start, so the session comes back
/// through the failed Restart's restore.
///
/// Why: the restore is the second way a restarting session's tab cell
/// returns to the map, with its own fence. Spec: see
/// [`stop_during_a_restart_closes_its_host_tab`], with the replacement's
/// first stage refusing (the Restart fails).
#[farhelm_testtrace::test]
async fn a_stop_during_a_failed_restart_closes_its_host_tab() {
    stop_during_a_restart_closes_its_host_tab(true).await;
}

/// A Restart of a host agent starts the new launch in a new host that keeps
/// the old terminal's history above a blank screen in default modes, and
/// ends the old host.
///
/// Why: a host runs one child, so a Restart cannot reuse it; SPEC.md's
/// Restart promise (the scrollback stays, the previous run's visible frame
/// is not painted back) then holds only if the new host is seeded with the
/// old history and nothing else, and the old host, which no longer belongs
/// to any launch, must not keep running (the goal's "A restart is a new
/// host, and keeps the history"). Spec: after a consented Restart of the
/// history agent (its first run set bracketed paste, hid the cursor and set
/// a title), the session's agent terminal is a new host at a new socket,
/// the same size, showing `RESUMED RUN`; its history holds the first run's
/// early lines; neither its history nor its screen holds the first run's
/// last screen (`OLD SCREEN MARK`, `history line 60`); bracketed paste is
/// off, the cursor shows and the old title is gone. The row is a running host
/// row at generation 1 naming the new socket; the old host's process and
/// socket file are gone, its launch record removed, and the new launch's
/// record kept. One host socket remains.
#[farhelm_testtrace::test]
async fn a_restart_starts_a_new_host_that_keeps_the_history_on_a_blank_screen() {
    let h = harness_with_seams(SupervisorTimeouts::default(), host_seams()).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session(&h.client, &h.sup, work.path()).await;
    let old_pid = host_process(old.socket()).expect("the old host's process");
    let old_socket = old.socket().to_path_buf();
    let old_size = old.size();

    h.client
        .restart_session(&session.id, true)
        .await
        .expect("the restart succeeds");

    let new = agent_host(&h.sup, &session.id).await;
    assert_ne!(new.socket(), old_socket, "a new host");
    wait_for_screen(&new, RESUMED).await;
    assert_eq!(
        new.size(),
        old_size,
        "the new terminal has the old one's size"
    );
    let history = history_text(&new).await;
    assert!(history.contains("history line 01"), "{history}");
    assert!(history.contains("history line 20"), "{history}");
    let screen = new.screen_tail(8192);
    for frame in [OLD_SCREEN_MARK, "history line 60"] {
        assert!(!history.contains(frame), "{frame:?} in history: {history}");
        assert!(!screen.contains(frame), "{frame:?} on screen: {screen}");
    }
    let modes = new.modes();
    assert!(
        !modes.bracketed_paste && modes.cursor_visible,
        "default modes: {modes:?}"
    );
    assert_ne!(new.title(), "old title", "no title carried over");

    let row = stored_row(h.state.path(), &session.id).await;
    assert_eq!(
        (
            row.generation,
            row.terminal_kind,
            &row.outcome,
            row.terminal_address.as_str()
        ),
        (
            1,
            TerminalKind::Host,
            &LastOutcome::Running,
            socket_name(new.socket()).as_str()
        )
    );
    assert_host_ended(old_pid, &old_socket).await;
    assert!(!record_path(h.state.path(), &session.id, 0).exists());
    assert!(record_path(h.state.path(), &session.id, 1).exists());
    assert_eq!(
        host_sockets_in(h.state.path()),
        vec![socket_name(new.socket())]
    );
    let new_pid = host_process(new.socket()).expect("the new host's process");
    end_runtime(&new, new_pid).await;
}

// ---- A failed replacement --------------------------------------------------------

/// A restart whose new host's first stage refuses is unwound definitively:
/// the previous host stays the session's terminal, also for the next
/// supervisor, and a later restart moves on from it.
///
/// Why: the inventory's top risk 5 (restart half). A failed replacement
/// must leave the old generation's state as it was (its record, its host,
/// its history), clean up everything of the replacement, and not strand the
/// session: and since the generation has moved on while the old host's tag
/// names the earlier one, a restarted supervisor must still find that host
/// through the row's `pane` rather than call it another terminal's. Spec:
/// with the host program refusing (exit 3 after reading its spec), a
/// consented Restart fails naming the terminal host; the row is a host row
/// at generation 1 still naming the old socket and not launching, listing
/// as stopped by the user; the session's terminal is still the old host
/// (same socket, its process running, its screen still showing the old
/// mark); the only host socket is the old one and generation 1 left no
/// launch record. A supervisor started next on the same state directory
/// adopts that very host as the session's terminal (not a placeholder).
/// With the program working again, a Restart there succeeds: the new host
/// keeps the first run's history and the old host and its record are gone.
#[farhelm_testtrace::test]
async fn a_refused_replacement_keeps_the_previous_host_for_this_and_the_next_supervisor() {
    let stack = Stack::new().await;
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let flag = fixture.path().join("refuse");
    let program = failing_host_program(fixture.path(), &flag, None);
    let seams = || SupervisorSeams {
        host_program: Some(program.clone()),
        ..host_seams()
    };
    let (sup, client) = stack.supervisor(seams()).await;
    let (session, old) = history_session(&client, &sup, stack.work.path()).await;
    let old_socket = old.socket().to_path_buf();
    let old_pid = host_process(&old_socket).expect("the old host's process");

    std::fs::write(&flag, b"").expect("make host starts refuse");
    let refused = client
        .restart_session(&session.id, true)
        .await
        .expect_err("the replacement host refuses to start");
    std::fs::remove_file(&flag).expect("let host starts work again");
    assert!(
        format!("{refused:#}").contains("terminal host"),
        "{refused:#}"
    );

    let row = stored_row(stack.state.path(), &session.id).await;
    assert_eq!(
        (
            row.generation,
            row.terminal_kind,
            row.terminal_address.as_str()
        ),
        (1, TerminalKind::Host, socket_name(&old_socket).as_str())
    );
    assert_ne!(row.outcome, LastOutcome::Launching);
    assert_eq!(
        listed(&client, &session.id).await.annotation.as_deref(),
        Some("stopped by user")
    );
    let kept = agent_host(&sup, &session.id).await;
    assert_eq!(kept.socket(), old_socket);
    assert!(!process_is_gone(old_pid), "the old host still runs");
    assert!(kept.screen_tail(4096).contains(OLD_SCREEN_MARK));
    assert_eq!(
        host_sockets_in(stack.state.path()),
        vec![socket_name(&old_socket)]
    );
    assert!(!record_path(stack.state.path(), &session.id, 1).exists());
    drop(kept);
    drop(old);
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(seams()).await;
    let adopted = agent_host(&sup, &session.id).await;
    assert_eq!(adopted.socket(), old_socket);
    assert!(
        !adopted.is_placeholder(),
        "the old host is adopted as the session's terminal, not left unavailable"
    );
    // A recovered runtime reads as running until it has replayed the exit
    // record of the child the restart stopped.
    wait_for_liveness(&adopted, "the stopped agent's exit", |liveness| {
        matches!(liveness, Liveness::Exited { .. })
    })
    .await;
    drop(adopted);

    client
        .restart_session(&session.id, true)
        .await
        .expect("the restart succeeds once hosts start again");
    let new = agent_host(&sup, &session.id).await;
    assert_ne!(new.socket(), old_socket);
    wait_for_screen(&new, RESUMED).await;
    let history = history_text(&new).await;
    assert!(history.contains("history line 01"), "{history}");
    assert_eq!(
        stored_row(stack.state.path(), &session.id).await.generation,
        2
    );
    assert_host_ended(old_pid, &old_socket).await;
    assert!(!record_path(stack.state.path(), &session.id, 0).exists());
    let new_pid = host_process(new.socket()).expect("the new host's process");
    end_runtime(&new, new_pid).await;
    drop(client);
    drop(sup);
}

/// A restart whose new host starts but cannot be used ends that host and is
/// unwound definitively.
///
/// Why: a replacement host that reported ready and then answers nothing
/// (its `Info` never comes) is still this launch's own; leaving it would
/// strand a host no row names, and keeping the launching record for a host
/// that was confirmed ended would show a session that never ran. Spec: with
/// the host program switched to a stand-in whose listener binds the socket
/// and never answers (its command line ending with that socket), a
/// consented Restart fails connecting to the new host; the listener's
/// process is gone; the row still names the old socket at generation 1,
/// not launching; the old host is still the session's terminal and runs;
/// and generation 1 left no launch record.
#[farhelm_testtrace::test]
async fn an_unusable_replacement_host_is_ended_and_the_restart_unwound() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let flag = fixture.path().join("unusable");
    let listener_pid = fixture.path().join("listener.pid");
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            host_program: Some(failing_host_program(
                fixture.path(),
                &flag,
                Some(&listener_pid),
            )),
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session(&h.client, &h.sup, work.path()).await;
    let old_socket = old.socket().to_path_buf();
    let old_pid = host_process(&old_socket).expect("the old host's process");

    std::fs::write(&flag, b"").expect("make the next host unusable");
    let failed = h
        .client
        .restart_session(&session.id, true)
        .await
        .expect_err("the replacement host cannot be used");
    assert!(
        format!("{failed:#}").contains("connecting to the session's new terminal host"),
        "{failed:#}"
    );
    let listener = wait_for_pid_file(&listener_pid, WAIT_SECS).await;
    wait_until_pid_gone(listener, WAIT_SECS).await;
    let row = stored_row(h.state.path(), &session.id).await;
    assert_eq!(
        (row.generation, row.terminal_address.as_str()),
        (1, socket_name(&old_socket).as_str())
    );
    assert_ne!(row.outcome, LastOutcome::Launching);
    assert_eq!(agent_host(&h.sup, &session.id).await.socket(), old_socket);
    assert!(!process_is_gone(old_pid), "the old host still runs");
    assert!(!record_path(h.state.path(), &session.id, 1).exists());
    end_runtime(&old, old_pid).await;
}

// ---- Interruptions ----------------------------------------------------------------

/// What [`interrupted_restart`] leaves for a test to check: the stack, the
/// supervisor started after the crash and its client, the session, the old
/// host's socket and process, and the new host's socket.
struct Interrupted {
    stack: Stack,
    sup: Arc<Supervisor>,
    client: Arc<SupervisorClient>,
    session: SessionInfo,
    old_socket: PathBuf,
    old_pid: u32,
    new_socket: PathBuf,
}

/// Run a restart that crashes at `stage`, check the state the crash left
/// (both hosts running, the row as `stage` says, both launch records), drop
/// the supervisor and start the next one on the same state directory: what
/// the interruption tests share.
async fn interrupted_restart(stage: HostRestartStage) -> Interrupted {
    let stack = Stack::new().await;
    let (crash, fired) = crash_once_at(stage);
    let (sup, client) = stack.supervisor(crashing_seams(crash)).await;
    let (session, old) = history_session(&client, &sup, stack.work.path()).await;
    let old_socket = old.socket().to_path_buf();
    let old_pid = host_process(&old_socket).expect("the old host's process");
    drop(old);

    client
        .restart_session(&session.id, true)
        .await
        .expect_err("the restart is interrupted");
    assert!(
        fired.load(Ordering::SeqCst),
        "premise: the crash seam fired"
    );
    let record = std::fs::read_to_string(record_path(stack.state.path(), &session.id, 1))
        .expect("the new launch's record");
    let new_name = record
        .lines()
        .find_map(|line| line.strip_prefix("socket "))
        .expect("the record names its socket")
        .to_string();
    let new_socket = stack.state.path().join("hosts").join(&new_name);
    let row = stored_row(stack.state.path(), &session.id).await;
    match stage {
        HostRestartStage::ReplacementStarted => {
            assert_eq!((row.generation, &row.outcome), (1, &LastOutcome::Launching));
            assert!(
                row.terminal_address.is_empty(),
                "nothing confirmed the new launch"
            );
        }
        HostRestartStage::BeforeOldHostEnded => {
            assert_eq!(
                (row.generation, &row.outcome, row.terminal_address.as_str()),
                (1, &LastOutcome::Running, new_name.as_str())
            );
        }
        HostRestartStage::GenerationClaimed
        | HostRestartStage::Confirmation
        | HostRestartStage::ReplacementUnwound => {
            unreachable!("only crashes after the new host started are run here")
        }
    }
    let new_pid = host_process(&new_socket).expect("premise: the new host runs");
    assert!(
        !process_is_gone(old_pid),
        "premise: the old host still runs"
    );
    assert!(record_path(stack.state.path(), &session.id, 0).exists());
    retire(stack.state.path(), sup, client).await;
    assert!(
        !process_is_gone(old_pid) && !process_is_gone(new_pid),
        "premise: both hosts outlived the supervisor"
    );

    let (sup, client) = stack.supervisor(host_seams()).await;
    Interrupted {
        stack,
        sup,
        client,
        session,
        old_socket,
        old_pid,
        new_socket,
    }
}

/// What the next supervisor must have made of an interrupted restart: the
/// new host is the session's running terminal, keeping the first run's
/// history, and the old host and its record are gone. Ends the new host
/// through its runtime afterwards.
async fn assert_converged(interrupted: Interrupted) {
    let Interrupted {
        stack,
        sup,
        client,
        session,
        old_socket,
        old_pid,
        new_socket,
    } = interrupted;
    let row = stored_row(stack.state.path(), &session.id).await;
    assert_eq!(
        (row.generation, &row.outcome, row.terminal_address.as_str()),
        (1, &LastOutcome::Running, socket_name(&new_socket).as_str())
    );
    let new = agent_host(&sup, &session.id).await;
    assert_eq!(new.socket(), new_socket);
    crate::host_restarts::wait_until_caught_up(&new).await;
    assert!(history_text(&new).await.contains("history line 01"));
    assert_host_ended(old_pid, &old_socket).await;
    assert!(!record_path(stack.state.path(), &session.id, 0).exists());
    assert!(record_path(stack.state.path(), &session.id, 1).exists());
    let new_pid = host_process(new.socket()).expect("the new host's process");
    end_runtime(&new, new_pid).await;
    drop(client);
}

/// A restart interrupted after its new host started, before the new launch
/// was confirmed, converges at the next start: the new host is the session's
/// and the old one is ended.
///
/// Why: the plan's interruption at replacement. A crash there leaves both
/// hosts running, the row launching with no `pane`, and nothing in memory;
/// the next supervisor must adopt the new host by its tag rather than
/// relaunch or call the session gone, and then end the old host, which is
/// no orphan (its session exists) and would otherwise run for good (D-25).
/// Spec: with the crash seam firing at `ReplacementStarted`, the restart
/// fails; the row is launching at generation 1 with no `pane`, both hosts
/// run and both launch records exist, also once the supervisor is dropped.
/// The next supervisor's row is running at generation 1 naming the new
/// socket; its agent terminal is that host, whose history holds the first
/// run's lines; the old host's process and socket file are gone and its
/// launch record is removed.
#[farhelm_testtrace::test]
async fn a_restart_interrupted_after_its_new_host_started_converges_at_the_next_start() {
    assert_converged(interrupted_restart(HostRestartStage::ReplacementStarted).await).await;
}

/// A restart interrupted after its new launch was confirmed, before the old
/// host was ended, converges at the next start.
///
/// Why: the plan's interruption at old-host shutdown. The session already
/// names the new host; only the old host is left over, and the next
/// supervisor ends it as superseded, never the new one. Spec: with the
/// crash seam firing at `BeforeOldHostEnded`, the restart fails; the row is
/// running at generation 1 naming the new socket and both hosts run, also
/// once the supervisor is dropped. The next supervisor keeps that row and
/// that host (with the first run's history) and ends the old host, whose
/// socket file and launch record go.
#[farhelm_testtrace::test]
async fn a_restart_interrupted_before_the_old_host_ended_converges_at_the_next_start() {
    assert_converged(interrupted_restart(HostRestartStage::BeforeOldHostEnded).await).await;
}

/// The superseded-host rule leaves an earlier host it cannot read alone,
/// and ends it at a later start once it can.
///
/// Why: a host a supervisor cannot read may belong to a newer build, which
/// must never be shut down on a guess; the rule acts only on hosts whose
/// tags it read (the plan's "an unavailable host is never reclaimed").
/// Spec: after a restart interrupted before the old host was ended, with
/// the old host stopped (SIGSTOP) while the next supervisor starts, that
/// supervisor adopts the new host as the session's terminal and leaves the
/// old host's process, socket file and launch record in place. Once the
/// old host is continued, the supervisor after that ends it and removes
/// its socket file and record, and keeps the new host.
#[farhelm_testtrace::test]
async fn the_superseded_rule_leaves_an_earlier_host_it_cannot_read_alone() {
    let stack = Stack::new().await;
    let (crash, fired) = crash_once_at(HostRestartStage::BeforeOldHostEnded);
    let (sup, client) = stack.supervisor(crashing_seams(crash)).await;
    let (session, old) = history_session(&client, &sup, stack.work.path()).await;
    let old_socket = old.socket().to_path_buf();
    let old_pid = host_process(&old_socket).expect("the old host's process");
    drop(old);
    client
        .restart_session(&session.id, true)
        .await
        .expect_err("the restart is interrupted");
    assert!(
        fired.load(Ordering::SeqCst),
        "premise: the crash seam fired"
    );
    let new_socket = agent_host(&sup, &session.id).await.socket().to_path_buf();
    retire(stack.state.path(), sup, client).await;

    let stopped = Stopped::stop(old_pid);
    let (sup, client) = stack.supervisor(host_seams()).await;
    assert_eq!(agent_host(&sup, &session.id).await.socket(), new_socket);
    assert!(
        !process_is_gone(old_pid),
        "the unread old host was left alone"
    );
    assert!(std::fs::symlink_metadata(&old_socket).is_ok());
    assert!(record_path(stack.state.path(), &session.id, 0).exists());
    drop(stopped);
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(host_seams()).await;
    assert_host_ended(old_pid, &old_socket).await;
    assert!(!record_path(stack.state.path(), &session.id, 0).exists());
    let new = agent_host(&sup, &session.id).await;
    assert_eq!(new.socket(), new_socket);
    let new_pid = host_process(new.socket()).expect("the new host still runs");
    end_runtime(&new, new_pid).await;
    drop(client);
}

// ---- Delete ---------------------------------------------------------------------

/// A restart whose session's row is deleted between its new host's start
/// and its confirmation ends both hosts.
///
/// Why: once the row is gone nothing would ever find either host again;
/// a teardown that swept only the new agent would leave the new host and
/// the old one (with its final screen) running for good. A real Delete
/// queues behind the restart's lifecycle claim, so the row is removed
/// through the store, as the tmux twin of this test does: the only way to
/// reach the confirmation that finds no row. Spec: with the fault seam
/// deleting the session's row (through the store) at `ReplacementStarted` and
/// letting the restart go on, the restart fails as a conflict saying the
/// session was deleted while it was being relaunched and the new agent was
/// torn back down; afterwards no host process runs in the state directory,
/// no host socket and no launch host record is left, and no row exists.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_restart_that_loses_to_delete_ends_both_hosts() {
    let stack = Stack::new().await;
    let session_id: Arc<std::sync::Mutex<Option<String>>> = Arc::default();
    let deleted = Arc::new(AtomicBool::new(false));
    let seam: HostRestartFault = {
        let session_id = Arc::clone(&session_id);
        let deleted = Arc::clone(&deleted);
        let db = stack.state.path().join("supervisor.db");
        Arc::new(move |stage| {
            if stage != HostRestartStage::ReplacementStarted {
                return Ok(());
            }
            let id = session_id
                .lock()
                .unwrap()
                .clone()
                .expect("the session was created");
            let db = db.clone();
            tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(async move {
                    SessionStore::open(&db, false)
                        .await?
                        .delete_session_settling_reservations(&id)
                        .await
                })
            })?;
            deleted.store(true, Ordering::SeqCst);
            Ok(())
        })
    };
    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            faults: FaultHooks {
                host_restart_fault: Some(seam),
                ..FaultHooks::default()
            },
            ..host_seams()
        })
        .await;
    let (session, old) = history_session(&client, &sup, stack.work.path()).await;
    drop(old);
    *session_id.lock().unwrap() = Some(session.id.clone());

    let error = client
        .restart_session(&session.id, true)
        .await
        .expect_err("a restart whose session was deleted mid-launch fails");
    assert!(
        deleted.load(Ordering::SeqCst),
        "premise: the delete happened"
    );
    let error = format!("{error:#}");
    assert!(
        error.contains("deleted while it was being relaunched") && error.contains("torn back down"),
        "{error}"
    );
    assert!(
        host_processes_in(stack.state.path()).is_empty(),
        "host processes left: {:?}",
        host_processes_in(stack.state.path())
    );
    assert!(host_sockets_in(stack.state.path()).is_empty());
    assert!(host_records_in(stack.state.path()).is_empty());
    assert!(
        SessionStore::open(&stack.state.path().join("supervisor.db"), false)
            .await
            .unwrap()
            .session(&session.id)
            .await
            .unwrap()
            .is_none()
    );
    drop(client);
    drop(sup);
}

/// Deleting a session after a restart was interrupted ends the hosts of
/// both launch generations.
///
/// Why: an interrupted restart leaves two hosts that no runtime holds, the
/// old generation's and the new one's, each named only by its launch record;
/// Delete acts on the session's records, never on a scan, so both records
/// being the session's own is what makes Delete end both (the plan's PR 19
/// loop). Spec: after a restart that crashed at `ReplacementStarted`
/// (premise: two host processes run), an unconditional Delete succeeds, and
/// afterwards no host process runs in the state directory and no host
/// socket or launch host record is left.
#[farhelm_testtrace::test]
async fn delete_after_an_interrupted_restart_ends_both_generations_hosts() {
    let (crash, fired) = crash_once_at(HostRestartStage::ReplacementStarted);
    let h = harness_with_seams(SupervisorTimeouts::default(), crashing_seams(crash)).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session(&h.client, &h.sup, work.path()).await;
    drop(old);
    h.client
        .restart_session(&session.id, true)
        .await
        .expect_err("the restart is interrupted");
    assert!(
        fired.load(Ordering::SeqCst),
        "premise: the crash seam fired"
    );
    assert_eq!(
        host_processes_in(h.state.path()).len(),
        2,
        "premise: both generations' hosts run"
    );

    h.client
        .delete_session(&session.id)
        .await
        .expect("the delete ends both hosts");
    assert!(
        host_processes_in(h.state.path()).is_empty(),
        "host processes left: {:?}",
        host_processes_in(h.state.path())
    );
    assert!(host_sockets_in(h.state.path()).is_empty());
    assert!(host_records_in(h.state.path()).is_empty());
}

// ---- Round two: outstanding starts, the claim window, failures, viewers ---------

/// The lines of `dir/runs`: how many times the history agent started.
fn agent_runs(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join("runs"))
        .map(|runs| runs.lines().count())
        .unwrap_or(0)
}

/// A host program whose first stage, once `armed` exists, reads its spec
/// and then waits (the first such start only) until `gate` exists before it
/// starts the host: a start that holds the whole spec with nothing bound
/// yet, as one whose supervisor died after giving it the spec. Every first
/// stage keeps the spec it read in `dir/spec.<its pid>`, and the held one,
/// once it has the whole spec, marks itself with `dir/held/<its pid>`.
/// Every other start is the built `farhelm`'s own.
fn held_host_program(dir: &Path, armed: &Path, gate: &Path) -> PathBuf {
    script(
        &dir.join("farhelm"),
        &format!(
            "spec={spec}.$$\n\
             /bin/cat > \"$spec\"\n\
             if [ -e {armed} ] && /bin/mkdir {held} 2>/dev/null; then\n\
             \x20 : > {held}/$$\n\
             \x20 while [ ! -e {gate} ]; do /bin/sleep 0.05; done\n\
             fi\n\
             exec {farhelm} \"$@\" < \"$spec\"",
            spec = shell_words::quote(&dir.join("spec").to_string_lossy()),
            armed = shell_words::quote(&armed.to_string_lossy()),
            held = shell_words::quote(&dir.join("held").to_string_lossy()),
            gate = shell_words::quote(&gate.to_string_lossy()),
            farhelm = shell_words::quote(farhelm_bin()),
        ),
    )
}

/// The first stage a session's launch record for `generation` names.
fn recorded_starter(state: &Path, session_id: &str, generation: i64) -> u32 {
    std::fs::read_to_string(record_path(state, session_id, generation))
        .expect("the launch record")
        .lines()
        .find_map(|line| line.strip_prefix("starter "))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|pid| pid.parse().ok())
        .expect("the record names its first stage")
}

/// A Restart after one whose host start was interrupted with its first
/// stage holding the spec fences that first stage before it launches, so
/// two agents can never run.
///
/// Why: the sol review's P1. The held first stage carries no marker and no
/// scope the restart's sweep could find, and the launch shim does not check
/// the generation: released after the next launch started, it would start a
/// second agent on the same conversation. Spec: with the first restart's host
/// start interrupted once its first stage (held) has the spec, the restart
/// fails, and the record of generation 1 names a first stage that marks
/// itself held once it has read the whole spec (waited for, bounded), with
/// its spec file non-empty. A second Restart succeeds, and when it returns
/// that first stage is gone.
/// Releasing it afterwards starts nothing: once released, two host
/// processes run (the created one and the second restart's) and the agent
/// has started twice in all (the create and the second restart). A guard
/// ends the held first stage should the test fail first.
#[farhelm_testtrace::test]
async fn a_restart_fences_an_earlier_restarts_held_start_first() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let armed = fixture.path().join("armed");
    let gate = fixture.path().join("gate");
    let interrupt = Arc::new(AtomicBool::new(false));
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            host_program: Some(held_host_program(fixture.path(), &armed, &gate)),
            faults: FaultHooks {
                host_start_interrupt: Some({
                    let interrupt = Arc::clone(&interrupt);
                    Arc::new(move || {
                        if interrupt.swap(false, Ordering::SeqCst) {
                            anyhow::bail!("interrupted with the first stage holding its spec");
                        }
                        Ok(())
                    })
                }),
                ..FaultHooks::default()
            },
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session(&h.client, &h.sup, work.path()).await;
    drop(old);
    assert_eq!(agent_runs(work.path()), 1, "premise: one agent started");

    std::fs::write(&armed, b"").expect("hold the next host start");
    interrupt.store(true, Ordering::SeqCst);
    h.client
        .restart_session(&session.id, true)
        .await
        .expect_err("the first restart is interrupted");
    let starter = recorded_starter(h.state.path(), &session.id, 1);
    // Ends the held first stage if the test fails before it is fenced or
    // released: it would otherwise wait for a gate nothing writes, and could
    // start a host after the state directory's cleanup.
    let _held = PidKillGuard::arm(starter);
    // The interrupted restart returns once it has written the spec into the
    // first stage's pipe, which says nothing about whether the first stage
    // has read it yet; its own marker, written only after it read the whole
    // spec, does. Bounded by the outer timeout, which ends before the shared
    // poller's own deadline so that a timeout reports whether the first
    // stage still runs and whether the gate was written.
    let marker = fixture.path().join("held").join(starter.to_string());
    if tokio::time::timeout(
        Duration::from_secs(WAIT_SECS),
        wait_for_file(&marker, 2 * WAIT_SECS),
    )
    .await
    .is_err()
    {
        panic!(
            "premise: the first stage {starter} never marked itself holding its spec \
             (still running: {}, gate written: {})",
            !process_is_gone(starter),
            gate.exists()
        );
    }
    assert!(!gate.exists(), "premise: not released");
    assert!(
        std::fs::metadata(fixture.path().join(format!("spec.{starter}")))
            .is_ok_and(|metadata| metadata.len() > 0),
        "premise: the held first stage has this start's spec"
    );

    h.client
        .restart_session(&session.id, true)
        .await
        .expect("the second restart succeeds");
    assert!(
        process_is_gone(starter),
        "the held first stage was ended before the second restart launched"
    );
    std::fs::write(&gate, b"").expect("release the held first stage");
    wait_until_pid_gone(starter, WAIT_SECS).await;
    let new = agent_host(&h.sup, &session.id).await;
    wait_for_screen(&new, RESUMED).await;
    assert_eq!(
        host_processes_in(h.state.path()).len(),
        2,
        "the created host and the second restart's, nothing of the held start"
    );
    assert_eq!(agent_runs(work.path()), 2, "two agents started in all");
    drop(new);
    h.client
        .delete_session(&session.id)
        .await
        .expect("the delete ends the hosts");
}

/// Run a restart that a crash interrupts at `stage` inside the claim
/// window (with `refuse`, its replacement's first stage refusing first),
/// start the next supervisor, and require that it took the old host back as
/// the session's terminal with its history, and that a Restart there keeps
/// that history and ends the old host.
async fn claim_window_crash_recovers_the_old_terminal(stage: HostRestartStage, refuse: bool) {
    let stack = Stack::new().await;
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let flag = fixture.path().join("refuse");
    let program = failing_host_program(fixture.path(), &flag, None);
    let seams = |fault: Option<HostRestartFault>| SupervisorSeams {
        host_program: Some(program.clone()),
        faults: FaultHooks {
            host_restart_fault: fault,
            ..FaultHooks::default()
        },
        ..host_seams()
    };
    let (crash, fired) = crash_once_at(stage);
    let (sup, client) = stack.supervisor(seams(Some(crash))).await;
    let (session, old) = history_session(&client, &sup, stack.work.path()).await;
    let old_socket = old.socket().to_path_buf();
    let old_pid = host_process(&old_socket).expect("the old host's process");
    drop(old);
    if refuse {
        std::fs::write(&flag, b"").expect("make the replacement refuse");
    }
    client
        .restart_session(&session.id, true)
        .await
        .expect_err("the restart is interrupted");
    let _ = std::fs::remove_file(&flag);
    assert!(
        fired.load(Ordering::SeqCst),
        "premise: the crash seam fired"
    );
    let row = stored_row(stack.state.path(), &session.id).await;
    assert_eq!(
        (row.generation, &row.outcome, row.terminal_address.as_str()),
        (1, &LastOutcome::Launching, ""),
        "premise: the generation is claimed and nothing confirmed"
    );
    assert!(
        !record_path(stack.state.path(), &session.id, 1).exists(),
        "premise: the replacement has no launch record"
    );
    retire(stack.state.path(), sup, client).await;
    assert!(
        !process_is_gone(old_pid),
        "premise: the old host outlived it"
    );

    let (sup, client) = stack.supervisor(seams(None)).await;
    let back = agent_host(&sup, &session.id).await;
    assert_eq!(
        back.socket(),
        old_socket,
        "the old host is the terminal again"
    );
    assert!(!back.is_placeholder());
    crate::host_restarts::wait_until_caught_up(&back).await;
    assert!(history_text(&back).await.contains("history line 01"));
    let row = stored_row(stack.state.path(), &session.id).await;
    assert_eq!(
        (row.generation, row.terminal_address.as_str()),
        (1, socket_name(&old_socket).as_str()),
        "the old host's socket is recorded as the row's"
    );
    drop(back);

    client
        .restart_session(&session.id, true)
        .await
        .expect("the next restart succeeds");
    let new = agent_host(&sup, &session.id).await;
    wait_for_screen(&new, RESUMED).await;
    let history = history_text(&new).await;
    assert!(history.contains("history line 01"), "{history}");
    assert_host_ended(old_pid, &old_socket).await;
    assert!(!record_path(stack.state.path(), &session.id, 0).exists());
    let new_pid = host_process(new.socket()).expect("the new host's process");
    end_runtime(&new, new_pid).await;
    drop(client);
}

/// A crash right after a Restart claimed its generation keeps the session's
/// terminal and history at the next start.
///
/// Why: both reviews' claim-window finding. Claiming the generation empties
/// the row's `pane` before the replacement has a record, so a supervisor that
/// dies there (a crash, or an update's stop) left a session with no terminal
/// while the old host, holding the whole history, ran unclaimed, and the next
/// Restart started empty. Spec: with the fault seam crashing at
/// `GenerationClaimed`, the row is launching at generation 1 with no `pane`
/// and no generation-1 record; the next supervisor publishes the old host
/// (no placeholder) as the session's terminal with the first run's history
/// and records its socket in the row's `pane`; a Restart there keeps that
/// history in its new host and ends the old host and its record.
#[farhelm_testtrace::test]
async fn a_crash_right_after_the_generation_claim_keeps_the_old_terminal() {
    claim_window_crash_recovers_the_old_terminal(HostRestartStage::GenerationClaimed, false).await;
}

/// A crash after a failed replacement was cleaned up, before the previous
/// record was restored, keeps the session's terminal and history at the next
/// start.
///
/// Why: the sol review's second claim-window case: the unwind removed the
/// replacement's record and spec, and the restore of the row never ran.
/// Spec: as for a crash right after the claim, with the replacement's first
/// stage refusing and the fault seam crashing at `ReplacementUnwound`.
#[farhelm_testtrace::test]
async fn a_crash_after_a_failed_replacement_was_cleaned_up_keeps_the_old_terminal() {
    claim_window_crash_recovers_the_old_terminal(HostRestartStage::ReplacementUnwound, true).await;
}

/// A restart whose confirmation write fails publishes the new host as
/// launching and still ends the old one.
///
/// Why: the opus review's finding 5. The new agent runs and holds the old
/// history in its seed; leaving the old host to the next start would leave
/// it running for good if the new host ended first. Spec: with the fault
/// seam failing the confirmation, the restart fails saying the confirmation
/// in the database failed; the session's terminal is the new host, showing
/// `RESUMED RUN`; the row is launching at generation 1 with no `pane`; the
/// old host's process and socket file are gone and its record removed.
#[farhelm_testtrace::test]
async fn a_failed_confirmation_write_still_ends_the_old_host() {
    let fault: HostRestartFault = Arc::new(|stage| {
        if stage == HostRestartStage::Confirmation {
            anyhow::bail!("the database refused the write");
        }
        Ok(())
    });
    let h = harness_with_seams(SupervisorTimeouts::default(), crashing_seams(fault)).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session(&h.client, &h.sup, work.path()).await;
    let old_socket = old.socket().to_path_buf();
    let old_pid = host_process(&old_socket).expect("the old host's process");
    drop(old);

    let failed = h
        .client
        .restart_session(&session.id, true)
        .await
        .expect_err("the confirmation write fails");
    assert!(
        format!("{failed:#}").contains("confirming the relaunch in the database"),
        "{failed:#}"
    );
    let new = agent_host(&h.sup, &session.id).await;
    assert_ne!(new.socket(), old_socket);
    wait_for_screen(&new, RESUMED).await;
    let row = stored_row(h.state.path(), &session.id).await;
    assert_eq!(
        (row.generation, &row.outcome, row.terminal_address.as_str()),
        (1, &LastOutcome::Launching, "")
    );
    assert_host_ended(old_pid, &old_socket).await;
    assert!(!record_path(h.state.path(), &session.id, 0).exists());
    let new_pid = host_process(new.socket()).expect("the new host's process");
    end_runtime(&new, new_pid).await;
}

/// A replacement host that can be neither used nor confirmed ended keeps
/// the launching record, leaves the session with no terminal, and Delete
/// then ends both generations' hosts.
///
/// Why: the restart's ambiguous unwind (the opus review's test gap): a host
/// that may still run must stay reachable, and the old host's runtime, which
/// the entry no longer holds, must not stay connected and fight whatever
/// settles the host next. Spec: with the next host program a stand-in whose
/// listener binds the socket and never answers and whose command line does
/// not end with its socket (so the verified signal refuses it), the restart
/// fails saying a terminal host may still be running; the row is launching
/// at generation 1; the session holds no terminal; the old runtime has
/// ended while the old host's process runs; the records of both generations
/// exist. With the listener then killed, an unconditional Delete succeeds
/// and no host process runs in the state directory.
#[farhelm_testtrace::test]
async fn an_unconfirmed_replacement_keeps_the_launching_record() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let flag = fixture.path().join("unusable");
    let listener_pid = fixture.path().join("listener.pid");
    let silent_dir = fixture.path().join("silent");
    std::fs::create_dir_all(&silent_dir).expect("the stand-in's directory");
    let silent = silent_host_program(&silent_dir, &listener_pid, false, false);
    let program = script(
        &fixture.path().join("farhelm"),
        &format!(
            "if [ -e {flag} ]; then exec {silent} \"$@\"; fi\nexec {farhelm} \"$@\"",
            flag = shell_words::quote(&flag.to_string_lossy()),
            silent = shell_words::quote(&silent.to_string_lossy()),
            farhelm = shell_words::quote(farhelm_bin()),
        ),
    );
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            host_program: Some(program),
            ..host_seams()
        },
    )
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session(&h.client, &h.sup, work.path()).await;
    let old_pid = host_process(old.socket()).expect("the old host's process");

    std::fs::write(&flag, b"").expect("make the next host unusable");
    let failed = h
        .client
        .restart_session(&session.id, true)
        .await
        .expect_err("the replacement cannot be ended");
    let _ = std::fs::remove_file(&flag);
    assert!(
        format!("{failed:#}").contains("may still be running"),
        "{failed:#}"
    );
    let row = stored_row(h.state.path(), &session.id).await;
    assert_eq!((row.generation, &row.outcome), (1, &LastOutcome::Launching));
    assert!(h.sup.agent_host_for_test(&session.id).await.is_none());
    let mut status = old.subscribe();
    let ended = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while status.changed().await.is_ok() {}
    })
    .await;
    assert!(ended.is_ok(), "the old runtime was stopped");
    assert!(!process_is_gone(old_pid), "the old host still runs");
    assert!(record_path(h.state.path(), &session.id, 0).exists());
    assert!(record_path(h.state.path(), &session.id, 1).exists());
    drop(old);

    let listener = wait_for_pid_file(&listener_pid, WAIT_SECS).await;
    // SAFETY: ending the test's own stand-in listener.
    unsafe { libc::kill(listener as libc::pid_t, libc::SIGKILL) };
    wait_until_pid_gone(listener, WAIT_SECS).await;
    h.client
        .delete_session(&session.id)
        .await
        .expect("the delete ends what is left");
    assert!(
        host_processes_in(h.state.path()).is_empty(),
        "host processes left: {:?}",
        host_processes_in(h.state.path())
    );
}

/// Read grid messages from `stream` until `ReplayComplete`, and return the
/// history rows' text and the screen rows' text, one line each.
async fn grid_until_caught_up(stream: &mut TermStream) -> (String, String) {
    let mut history = String::new();
    let mut screen = String::new();
    let waited = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        loop {
            match stream.recv().await {
                Some(TermEvent::Data(bytes)) => {
                    match farhelm_proto::grid::GridOutput::decode(&bytes).expect("a grid message") {
                        farhelm_proto::grid::GridOutput::History { rows, .. } => {
                            for row in rows {
                                history.extend(row.runs.iter().map(|run| run.text.as_str()));
                                history.push('\n');
                            }
                        }
                        farhelm_proto::grid::GridOutput::Rows { rows, .. } => {
                            for row in rows {
                                screen.extend(row.row.runs.iter().map(|run| run.text.as_str()));
                                screen.push('\n');
                            }
                        }
                        _ => {}
                    }
                }
                Some(TermEvent::ReplayComplete) => return,
                Some(TermEvent::Detached(detach)) => panic!("detached: {detach:?}"),
                None => panic!("the attachment ended"),
            }
        }
    })
    .await;
    assert!(waited.is_ok(), "the attach did not catch up: {history}");
    (history, screen)
}

/// A grid viewer of the old host is told the session restarted, and a new
/// attach streams the new host with the kept history.
///
/// Why: the restart replaces the terminal under the viewer; the brief's
/// decision is that the viewer is detached as for tmux and attaches again,
/// and nothing streams the old host afterwards. Spec: a grid attach of the
/// history agent's terminal catches up; after a consented Restart that
/// attachment is detached with the reason `session restarted`; a new grid
/// attach, made once the session's terminal is a new host showing
/// `RESUMED RUN`, catches up with history holding `history line 01` and a
/// screen showing `RESUMED RUN` and not the old screen's last line.
#[farhelm_testtrace::test]
async fn a_viewer_is_detached_and_reattaches_to_the_new_host() {
    let h = harness_with_seams(SupervisorTimeouts::default(), host_seams()).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session(&h.client, &h.sup, work.path()).await;
    drop(old);
    let attach = || {
        h.client.attach_terminal(
            &session.id,
            80,
            24,
            farhelm_proto::TerminalSelector::default(),
            "",
            farhelm_helm::ViewerStreams::BytesOrGrid,
        )
    };
    let (_channel, mut stream) = attach().await.expect("a grid attach");
    let old_socket = agent_host(&h.sup, &session.id).await.socket().to_path_buf();
    grid_until_caught_up(&mut stream).await;

    h.client
        .restart_session(&session.id, true)
        .await
        .expect("the restart succeeds");
    let detached = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        loop {
            match stream.recv().await {
                Some(TermEvent::Detached(detach)) => return Some(detach),
                Some(_) => {}
                None => return None,
            }
        }
    })
    .await
    .expect("the viewer is told in time")
    .expect("a detach, not a silent end");
    assert_eq!(detached.reason, "session restarted");

    let new = agent_host(&h.sup, &session.id).await;
    assert_ne!(
        new.socket(),
        old_socket,
        "the session's terminal is a new host"
    );
    wait_for_screen(&new, RESUMED).await;
    let (_channel, mut stream) = attach().await.expect("a grid attach of the new host");
    let (history, screen) = grid_until_caught_up(&mut stream).await;
    assert!(history.contains("history line 01"), "{history}");
    assert!(
        screen.contains(RESUMED) && !screen.contains(OLD_SCREEN_MARK),
        "the stream is the new host's: {screen}"
    );
    drop(stream);
    let new_pid = host_process(new.socket()).expect("the new host's process");
    end_runtime(&new, new_pid).await;
}

/// The seed can hold what the old program printed as it was stopped.
///
/// Why: a restarted terminal that dropped the old program's last words
/// would lose exactly the output a user restarting after a problem wants to
/// read. The restart reads the seed once the old host's exit record (sent
/// after the program's drained output) is processed; this test shows the
/// words survive, not that the wait is what keeps them, since whether the
/// runtime had fed them anyway by the time the seed was asked for depends on
/// timing nothing here controls. Spec: with an agent that prints 30 numbered
/// `last words` lines when stopped, after a consented Restart the new host's
/// history holds `last words 01`.
#[farhelm_testtrace::test]
async fn the_seed_can_hold_what_the_old_program_printed_as_it_stopped() {
    let h = harness_with_seams(SupervisorTimeouts::default(), host_seams()).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session_with(&h.client, &h.sup, work.path(), true).await;
    drop(old);
    h.client
        .restart_session(&session.id, true)
        .await
        .expect("the restart succeeds");
    let new = agent_host(&h.sup, &session.id).await;
    wait_for_screen(&new, RESUMED).await;
    let history = history_text(&new).await;
    assert!(history.contains("last words 01"), "{history}");
    let new_pid = host_process(new.socket()).expect("the new host's process");
    end_runtime(&new, new_pid).await;
}

// ---- Round three: earlier hosts a confirmed restart replaced ---------------------

/// A Restart is not held up by an earlier, already-replaced host that cannot
/// be read.
///
/// Why: the opus review's second-round finding. A host a confirmed later
/// launch replaced is one whose program the replacing Restart stopped; when
/// its end failed it lingers until Delete or a later start, and a wedged one
/// must not refuse every later Restart of the session (only launches after
/// the current host's are settled before a Restart). Spec: after a restart
/// interrupted once its new launch was published (the generation-0 host left
/// running, its record kept), with that generation-0 host stopped (SIGSTOP),
/// a second Restart in the same supervisor succeeds and starts a new host;
/// the generation-0 host is left as it was (still there, its record kept).
#[farhelm_testtrace::test]
async fn a_wedged_replaced_host_does_not_hold_up_a_restart() {
    let (crash, fired) = crash_once_at(HostRestartStage::BeforeOldHostEnded);
    let h = harness_with_seams(SupervisorTimeouts::default(), crashing_seams(crash)).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let (session, old) = history_session(&h.client, &h.sup, work.path()).await;
    let old_pid = host_process(old.socket()).expect("the old host's process");
    drop(old);
    h.client
        .restart_session(&session.id, true)
        .await
        .expect_err("the first restart is interrupted after its publication");
    assert!(
        fired.load(Ordering::SeqCst),
        "premise: the crash seam fired"
    );
    assert!(
        record_path(h.state.path(), &session.id, 0).exists(),
        "premise: the replaced host's record is kept"
    );
    let first = agent_host(&h.sup, &session.id).await;
    let stopped = Stopped::stop(old_pid);

    tokio::time::timeout(
        Duration::from_secs(WAIT_SECS),
        h.client.restart_session(&session.id, true),
    )
    .await
    .expect("the restart answers in time")
    .expect("the restart is not refused over the wedged replaced host");
    let second = agent_host(&h.sup, &session.id).await;
    assert_ne!(second.socket(), first.socket(), "a new host");
    wait_for_screen(&second, RESUMED).await;
    assert!(
        !process_is_gone(old_pid),
        "the wedged host was left as it was"
    );
    assert!(record_path(h.state.path(), &session.id, 0).exists());
    drop(stopped);
    drop(first);
    h.client
        .delete_session(&session.id)
        .await
        .expect("the delete ends the hosts");
}

/// A session left by a crash in a Restart's claim window, with two earlier
/// hosts still running: [`claim_window_crash`].
struct ClaimWindow {
    session: SessionInfo,
    gen0_socket: PathBuf,
    gen0_pid: u32,
    gen1_socket: PathBuf,
    gen1_pid: u32,
}

/// In one supervisor on `stack`, a history session whose first restart is
/// interrupted after its publication (the generation-0 host left running
/// beside the generation-1 one) and whose second restart crashes right after
/// it claimed generation 2; the supervisor is then dropped. The row is left
/// a launching host row at generation 2 with no `pane`, and both earlier
/// hosts run with their launch records: what the claim-window fallback of
/// the next start has to choose from.
async fn claim_window_crash(stack: &Stack) -> ClaimWindow {
    let stages = Arc::new(std::sync::Mutex::new(vec![
        HostRestartStage::BeforeOldHostEnded,
        HostRestartStage::GenerationClaimed,
    ]));
    let fault: HostRestartFault = {
        let stages = Arc::clone(&stages);
        Arc::new(move |stage| {
            let mut stages = stages.lock().unwrap();
            if stages.first() == Some(&stage) {
                stages.remove(0);
                anyhow::bail!("the supervisor crashed at {stage:?}");
            }
            Ok(())
        })
    };
    let (sup, client) = stack.supervisor(crashing_seams(fault)).await;
    let (session, gen0) = history_session(&client, &sup, stack.work.path()).await;
    let gen0_socket = gen0.socket().to_path_buf();
    let gen0_pid = host_process(&gen0_socket).expect("the generation-0 host");
    drop(gen0);
    client
        .restart_session(&session.id, true)
        .await
        .expect_err("the first restart is interrupted after its publication");
    let gen1 = agent_host(&sup, &session.id).await;
    let gen1_socket = gen1.socket().to_path_buf();
    let gen1_pid = host_process(&gen1_socket).expect("the generation-1 host");
    wait_for_screen(&gen1, RESUMED).await;
    drop(gen1);
    client
        .restart_session(&session.id, true)
        .await
        .expect_err("the second restart crashes right after its claim");
    assert!(
        stages.lock().unwrap().is_empty(),
        "premise: both crashes fired"
    );
    let row = stored_row(stack.state.path(), &session.id).await;
    assert_eq!(
        (row.generation, &row.outcome, row.terminal_address.as_str()),
        (2, &LastOutcome::Launching, "")
    );
    retire(stack.state.path(), sup, client).await;
    ClaimWindow {
        session,
        gen0_socket,
        gen0_pid,
        gen1_socket,
        gen1_pid,
    }
}

/// A crash in a Restart's claim window, with the newest earlier host
/// unreadable at the next start, adopts no older host instead, and a later
/// start recovers the newest one.
///
/// Why: the opus review's second-round finding 4. Picking the newest host
/// the scan could read would take an older one (with older history) as the
/// session's terminal, and the next start would then end the newer one as
/// superseded, keeping the stale history and destroying the latest. Spec:
/// after [`claim_window_crash`], with the generation-1 host stopped
/// (SIGSTOP), the next supervisor publishes no host as the session's
/// terminal, and the generation-0 and generation-1 hosts both still run
/// with their records. Continued, and with that supervisor dropped, the one
/// after takes the generation-1 host as the terminal with the first run's
/// history and ends the generation-0 host as superseded.
#[farhelm_testtrace::test]
async fn the_claim_window_fallback_takes_only_the_newest_recorded_host() {
    let stack = Stack::new().await;
    let ClaimWindow {
        session,
        gen0_socket,
        gen0_pid,
        gen1_socket,
        gen1_pid,
    } = claim_window_crash(&stack).await;

    let stopped = Stopped::stop(gen1_pid);
    let (sup, client) = stack.supervisor(host_seams()).await;
    assert!(
        sup.agent_host_for_test(&session.id).await.is_none(),
        "no host, the older generation-0 one included, is taken as the terminal"
    );
    assert!(!process_is_gone(gen0_pid) && !process_is_gone(gen1_pid));
    assert!(record_path(stack.state.path(), &session.id, 0).exists());
    assert!(record_path(stack.state.path(), &session.id, 1).exists());
    drop(stopped);
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(host_seams()).await;
    let back = agent_host(&sup, &session.id).await;
    assert_eq!(
        back.socket(),
        gen1_socket,
        "the newest recorded host is back"
    );
    crate::host_restarts::wait_until_caught_up(&back).await;
    assert!(history_text(&back).await.contains("history line 01"));
    assert_host_ended(gen0_pid, &gen0_socket).await;
    assert!(!record_path(stack.state.path(), &session.id, 0).exists());
    end_runtime(&back, gen1_pid).await;
    drop(client);
}

/// A crash in a Restart's claim window whose newest earlier host has since
/// died adopts the next older host that runs, with its history.
///
/// Why: the opus review's third-round finding 2. The fallback holds back
/// from an older host only because the newer one might be read later and
/// then superseded; a host that died leaves nothing to read or supersede,
/// so holding back there would leave the session with no terminal at every
/// start and throw away the only history that still exists. Spec: after
/// [`claim_window_crash`], with the generation-1 host killed (SIGKILL,
/// leaving its socket file, which the scan finds gone), the next
/// supervisor publishes the generation-0 host as the session's terminal,
/// holding the first run's history.
#[farhelm_testtrace::test]
async fn the_claim_window_fallback_passes_over_a_newest_host_that_died() {
    let stack = Stack::new().await;
    let ClaimWindow {
        session,
        gen0_socket,
        gen0_pid,
        gen1_socket,
        gen1_pid,
    } = claim_window_crash(&stack).await;
    // SAFETY: `kill` has no memory-safety preconditions; the pid is the
    // generation-1 host this test's supervisor started, found by its socket.
    assert_eq!(unsafe { libc::kill(gen1_pid as i32, libc::SIGKILL) }, 0);
    wait_until_pid_gone(gen1_pid, WAIT_SECS).await;
    assert!(
        gen1_socket.exists(),
        "premise: the killed host left its socket file"
    );

    let (sup, client) = stack.supervisor(host_seams()).await;
    let back = agent_host(&sup, &session.id).await;
    assert_eq!(
        back.socket(),
        gen0_socket,
        "the older host that runs is taken"
    );
    crate::host_restarts::wait_until_caught_up(&back).await;
    assert!(history_text(&back).await.contains("history line 01"));
    end_runtime(&back, gen0_pid).await;
    drop(client);
}
