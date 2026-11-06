//! Moving an old session to a new terminal: a Restart of a session whose
//! agent runs in tmux, under the supervisor's internal switch, starts the
//! agent in a terminal host seeded with the tmux pane's history, removes the
//! old agent window, and leaves the session's tmux tabs where they are (the
//! goal's "Old to new", `service/core/host_restart.rs`, "From a tmux pane").
//! Also what such a move leaves when it fails, is interrupted, races Delete
//! or meets an orderly stop; the mixed session it makes; and Clone and
//! Replace of an old session.
//!
//! An old session comes from a supervisor restart, as it would from an
//! update: a first in-process supervisor with the switch off creates the
//! session in tmux (with the real private tmux server under the state
//! directory), and is dropped; the next one, constructed on the same state
//! directory with the switch on (`host_creates::host_seams`), restarts it.
//! Hosts are started from the built `farhelm`, never inside this test
//! process.
//!
//! The agent ([`migration_agent`]) has distinct start and resume commands,
//! and records every launch, with the conversation a resume was given, in a
//! `runs` file beside it: its start prints sixty numbered history lines,
//! modes, a title and a last screen line, and its resume prints only
//! `RESUMED RUN` and the conversation. So what the new host holds tells the
//! tmux pane's history from the resumed program's output and from the
//! previous visible frame, and the `runs` file tells a resume of the
//! recorded conversation from a fresh start or another conversation. Every
//! launch runs `/bin/sh` as its shell with a private `HOME`, so no startup
//! file of the user running the tests is involved.
//!
//! Each test ends with a Delete of the sessions it made, which ends their
//! hosts through the supervisor's runtimes, so that no runtime's connection
//! is left to fight the state directory's cleanup.

use crate::harness::*;
use crate::host_creates::{host_processes_in, host_seams, script};
use crate::host_relaunches::{
    OLD_SCREEN_MARK, assert_host_ended, crash_once_at, failing_host_program, history_text,
    host_records_in, host_sockets_in, record_path, socket_name,
};
use crate::host_restarts::{Stack, agent_host, listed, retire, stored_row};
use crate::host_terminals::{WAIT_SECS, host_process, wait_for_liveness, wait_for_screen};
use crate::terminal_tabs::{listed_tabs, notifying_gate};
use farhelm_supervisor::hosts::Liveness;
use farhelm_supervisor::service::{
    HostRestartFault, HostRestartStage, NewTerminals, TmuxSeedFault,
};
use farhelm_supervisor::store::TerminalKind;
use std::path::{Path, PathBuf};

// ---- Fixtures -------------------------------------------------------------------

/// What the agent's resume prints, with the conversation it was given:
/// [`RESUMABLE_TEST_CONVERSATION`], the one the fixture records.
const RESUMED_CONVERSATION: &str = "RESUMED RUN conv-test";

/// The `runs` file after the first start and one resume of the recorded
/// conversation.
const STARTED_AND_RESUMED: [&str; 2] = ["start", "resume conv-test"];

/// Seams with the switch on (creates, tabs and Restarts in hosts), `/bin/sh`
/// as every launch's shell and `home` as its `HOME`.
fn new_seams(home: &Path) -> SupervisorSeams {
    SupervisorSeams {
        launch_shell: Some("/bin/sh".into()),
        launch_env: vec![("HOME".into(), home.to_string_lossy().into_owned())],
        ..host_seams()
    }
}

/// [`new_seams`] with the switch off: everything in tmux, as before
/// terminal hosts.
fn old_seams(home: &Path) -> SupervisorSeams {
    SupervisorSeams {
        new_terminals: NewTerminals::Tmux,
        ..new_seams(home)
    }
}

/// Write the migration agent into `dir` and return its path. `start`
/// appends `start` to `dir/runs`, prints `history line 01` to
/// `history line 60`, sets a title, bracketed paste and a hidden cursor,
/// and prints [`OLD_SCREEN_MARK`]; `resume <conversation>` appends
/// `resume <conversation>` and prints `RESUMED RUN <conversation>`; anything
/// else appends `unknown` and its arguments and exits 9. Both then wait.
fn migration_agent(dir: &Path) -> PathBuf {
    let runs = shell_words::quote(&dir.join("runs").to_string_lossy()).into_owned();
    script(
        &dir.join("migration-agent"),
        &format!(
            "case \"$1\" in\n\
             start)\n\
             printf 'start\\n' >> {runs}\n\
             i=1\n\
             while [ \"$i\" -le 60 ]; do printf 'history line %02d\\n' \"$i\"; i=$((i + 1)); done\n\
             printf '\\033]2;old title\\007\\033[?2004h\\033[?25l{OLD_SCREEN_MARK}\\n'\n\
             ;;\n\
             resume)\n\
             printf 'resume %s\\n' \"$2\" >> {runs}\n\
             printf 'RESUMED RUN %s\\n' \"$2\"\n\
             ;;\n\
             *)\n\
             printf 'unknown %s\\n' \"$*\" >> {runs}\n\
             exit 9\n\
             ;;\n\
             esac\n\
             exec /bin/sleep 600"
        ),
    )
}

/// The launches the migration agent in `dir` recorded, in order.
fn runs(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("runs"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// One pane of a session's tmux session, as tmux lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pane {
    id: String,
    /// The window's tab marker, empty for none.
    tab: String,
    /// The window's agent marker, empty for none.
    agent: String,
    dead: bool,
}

/// The panes of `session_id`'s tmux session in the private server at
/// `sock`, or none when that tmux session does not exist.
async fn panes(sock: &Path, session_id: &str) -> Vec<Pane> {
    let out = tmux_query(
        sock,
        &[
            "list-panes",
            "-s",
            "-t",
            &format!("=fh-{session_id}"),
            "-F",
            "#{pane_id}|#{@farhelm-tab}|#{@farhelm-agent}|#{pane_dead}",
        ],
    )
    .await;
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('|');
            Some(Pane {
                id: fields.next()?.to_string(),
                tab: fields.next()?.to_string(),
                agent: fields.next()?.to_string(),
                dead: fields.next()? == "1",
            })
        })
        .collect()
}

/// The agent-marked panes of `session_id`'s tmux session.
async fn agent_panes(sock: &Path, session_id: &str) -> Vec<Pane> {
    panes(sock, session_id)
        .await
        .into_iter()
        .filter(|pane| pane.agent == session_id)
        .collect()
}

/// What the tmux pane `pane` holds, history included, as text.
async fn pane_text(sock: &Path, pane: &str) -> String {
    let out = tmux_query(sock, &["capture-pane", "-p", "-t", pane, "-S", "-"]).await;
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Wait until the tmux pane `pane` holds `text` (history included).
async fn wait_for_pane_text(sock: &Path, pane: &str, text: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(WAIT_SECS);
    loop {
        let shown = pane_text(sock, pane).await;
        if shown.contains(text) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the pane {pane} never showed {text:?}: {shown}"
        );
        // sleep-ok: polling interval for tmux's grid, which announces nothing.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// How many lines of history the tmux pane `pane` holds.
async fn tmux_history_size(sock: &Path, pane: &str) -> u64 {
    let out = tmux_query(
        sock,
        &["display-message", "-p", "-t", pane, "#{history_size}"],
    )
    .await;
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("tmux reports a history size: {out:?}"))
}

/// An old session: the migration agent's session made in tmux by a
/// supervisor with the switch off, and its agent pane.
struct OldSession {
    session: SessionInfo,
    pane: String,
}

/// Create the migration agent's session on a supervisor with the switch off
/// (`sup`, `client`), at 80x24 in `stack`'s working directory, as a command
/// launch declaring Goose with distinct start and resume commands (the
/// resume passes the conversation to the agent), record
/// [`RESUMABLE_TEST_CONVERSATION`] as its conversation, and wait until its
/// start has printed its last screen line, with history in the tmux pane:
/// the premise every move here starts from.
async fn old_session(stack: &Stack, sup: &Supervisor, client: &SupervisorClient) -> OldSession {
    let agent = migration_agent(stack.work.path());
    let agent = agent.to_string_lossy();
    let start = shell_words::quote(&format!("exec {agent} start")).into_owned();
    let resume = shell_words::quote(&format!("exec {agent} resume \"$1\"")).into_owned();
    let session = client
        .create_session_with_extras(
            &stack.work.path().to_string_lossy(),
            declared_command(
                &format!("sh -c {start} {{farhelm_args}}"),
                farhelm_proto::LaunchHarness::Goose,
                Some(&format!(
                    "sh -c {resume} farhelm-test-resume {{conversation}} {{farhelm_args}}"
                )),
            ),
            None,
            80,
            24,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect("create the old session");
    sup.record_conversation_for_test(&session.id, RESUMABLE_TEST_CONVERSATION)
        .await;
    assert!(
        sup.agent_host_for_test(&session.id).await.is_none(),
        "premise: the agent runs in tmux"
    );
    let sock = stack.state.path().join("tmux.sock");
    let agents = agent_panes(&sock, &session.id).await;
    let [agent] = &agents[..] else {
        panic!("premise: one agent pane: {agents:?}");
    };
    wait_for_pane_text(&sock, &agent.id, OLD_SCREEN_MARK).await;
    assert!(
        tmux_history_size(&sock, &agent.id).await > 0,
        "premise: the tmux pane has history"
    );
    assert_eq!(runs(stack.work.path()), ["start"], "premise: one start");
    OldSession {
        session,
        pane: agent.id.clone(),
    }
}

/// Require that `host`, the new host of a session moved from tmux, holds the
/// tmux pane's history (its first lines) and nothing of the previous visible
/// frame (its last lines, the mark on its screen, tmux's dead-pane banner),
/// above a screen showing the resumed run of the recorded conversation, in
/// default modes with no title, at the tmux pane's 80x24.
async fn assert_seeded_from_tmux(host: &farhelm_supervisor::hosts::HostTerminal) {
    wait_for_screen(host, RESUMED_CONVERSATION).await;
    assert_eq!(
        (host.size().cols, host.size().rows),
        (80, 24),
        "the new terminal has the tmux pane's size"
    );
    let history = history_text(host).await;
    assert!(history.contains("history line 01"), "{history}");
    assert!(history.contains("history line 20"), "{history}");
    let screen = host.screen_tail(8192);
    for frame in [OLD_SCREEN_MARK, "history line 60", "Pane is dead"] {
        assert!(!history.contains(frame), "{frame:?} in history: {history}");
        assert!(!screen.contains(frame), "{frame:?} on screen: {screen}");
    }
    let modes = host.modes();
    assert!(
        !modes.bracketed_paste && modes.cursor_visible,
        "default modes: {modes:?}"
    );
    assert_ne!(host.title(), "old title", "no title carried over");
}

/// Attach `session_id`'s `terminal` through `client` under `lease`, as a
/// client that can show both kinds of stream, returning its channel and
/// stream.
async fn attach(
    client: &SupervisorClient,
    session_id: &str,
    terminal: TerminalSelector,
    lease: &str,
) -> (u32, TermStream) {
    client
        .attach_terminal(
            session_id,
            80,
            24,
            terminal,
            lease,
            ViewerStreams::BytesOrGrid,
        )
        .await
        .expect("attach")
}

// ---- The move -------------------------------------------------------------------

/// A Restart of an old session under the switch moves its agent into a
/// terminal host that keeps the tmux pane's history, removes the old agent
/// window, and leaves the session's tmux tab running, listed first; new tabs
/// are hosts, and a takeover displaces every attachment across both kinds.
///
/// Why: this is how every existing session meets terminal hosts (the goal's
/// "Old to new"): its agent must resume in a host with the scrollback SPEC.md
/// promises Restart keeps, never the previous visible frame; the row must
/// become a host row naming the socket so every later reader looks for a
/// host; the old window must go, since nothing would name it again; and a
/// live tab shell cannot move, so the session becomes mixed, which listing,
/// opening and viewer ownership must all handle. Spec: an old session with a
/// tmux tab, restarted by the next supervisor with the switch on, succeeds;
/// its agent terminal is a host seeded from the tmux pane
/// ([`assert_seeded_from_tmux`]); its row is a running host row at
/// generation 1 whose `pane` names the one host socket; no agent-marked pane
/// is left in its tmux session, whose tab pane is still alive; the session
/// lists the tmux tab; a new tab is a host, listed after it; the agent was
/// launched once more, by its resume command with the recorded
/// conversation; a client holding the agent, the tmux tab and the host tab
/// gets the echo of a line it types into the tmux tab through its
/// attachment, and loses all three attachments as taken over when another
/// client attaches the host tab. Delete then ends the hosts and the tmux
/// session.
#[farhelm_testtrace::test]
async fn an_old_session_restarts_into_a_host_with_its_history_and_tabs() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let sock = stack.state.path().join("tmux.sock");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    let tmux_tab = client.open_tab(&id).await.expect("a tmux tab");
    assert!(
        sup.tab_host_for_test(&id, &tmux_tab.id).await.is_none(),
        "premise: the tab is a tmux window"
    );
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(new_seams(home.path())).await;
    client
        .restart_session(&id, true)
        .await
        .expect("the restart moves the session into a host");
    let host = agent_host(&sup, &id).await;
    assert_seeded_from_tmux(&host).await;
    assert_eq!(runs(stack.work.path()), STARTED_AND_RESUMED);
    let row = stored_row(stack.state.path(), &id).await;
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
            socket_name(host.socket()).as_str()
        )
    );
    assert_eq!(
        host_sockets_in(stack.state.path()),
        vec![socket_name(host.socket())]
    );
    let left = panes(&sock, &id).await;
    assert!(
        left.iter()
            .all(|pane| pane.agent.is_empty() && pane.id != old.pane),
        "the old agent window is gone: {left:?}"
    );
    let [tab_pane] = &left[..] else {
        panic!("only the tab's pane is left: {left:?}");
    };
    assert!(
        tab_pane.tab == tmux_tab.id && !tab_pane.dead,
        "{tab_pane:?}"
    );

    let host_tab = client.open_tab(&id).await.expect("a host tab");
    assert!(
        sup.tab_host_for_test(&id, &host_tab.id).await.is_some(),
        "a new tab of a moved session is a host"
    );
    assert_eq!(
        listed_tabs(&client, &id).await,
        vec![tmux_tab.id.clone(), host_tab.id.clone()],
        "the tmux tab first, then the host tab"
    );
    let holder = connect_client(&sup).await;
    let mut held = Vec::new();
    for (which, terminal) in [
        ("host agent", TerminalSelector::Agent),
        (
            "tmux tab",
            TerminalSelector::Tab {
                id: tmux_tab.id.clone(),
            },
        ),
        (
            "host tab",
            TerminalSelector::Tab {
                id: host_tab.id.clone(),
            },
        ),
    ] {
        held.push((which, attach(&holder, &id, terminal, "holder").await));
    }
    let (tab_channel, tab_stream) = &mut held[1].1;
    holder
        .send_keys(*tab_channel, b"echo TMUX-TAB-$((40+3))\r".to_vec())
        .await;
    let mut seen = Vec::new();
    wait_for(tab_stream, &mut seen, "TMUX-TAB-43", WAIT_SECS).await;
    let taker = attach(
        &client,
        &id,
        TerminalSelector::Tab {
            id: host_tab.id.clone(),
        },
        "taker",
    )
    .await;
    for (which, (_, stream)) in &mut held {
        let detach = expect_detached(stream, WAIT_SECS).await;
        assert!(
            detach.code == farhelm_proto::DetachCode::TakenOver,
            "the holder's {which} attachment was taken over: {detach}"
        );
    }

    let pid = host_process(host.socket()).expect("the agent's host runs");
    client.delete_session(&id).await.expect("delete");
    assert_host_ended(pid, host.socket()).await;
    assert!(
        panes(&sock, &id).await.is_empty(),
        "Delete ended the tmux session"
    );
    drop((host, held, taker, holder));
    retire(stack.state.path(), sup, client).await;
}

/// With the switch off, a Restart of an old session stays in tmux.
///
/// Why: the switch is what moves sessions; until it is on, tmux is exactly
/// as it is (the goal's "What stays as it is"), and a Restart that moved a
/// session anyway would change every user's terminal behind the switch.
/// Spec: an old session restarted by a supervisor with the switch off
/// succeeds; its row is a tmux row at generation 1 naming a tmux pane, the
/// session holds no host, and the state directory has no host socket.
#[farhelm_testtrace::test]
async fn with_the_switch_off_a_restart_stays_in_tmux() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    client
        .restart_session(&id, true)
        .await
        .expect("the restart succeeds in tmux");
    let row = stored_row(stack.state.path(), &id).await;
    assert_eq!((row.generation, row.terminal_kind), (1, TerminalKind::Tmux));
    assert!(
        row.terminal_address.starts_with('%'),
        "a tmux pane: {:?}",
        row.terminal_address
    );
    assert!(sup.agent_host_for_test(&id).await.is_none());
    assert!(host_sockets_in(stack.state.path()).is_empty());
    client.delete_session(&id).await.expect("delete");
    retire(stack.state.path(), sup, client).await;
}

// ---- A failed move --------------------------------------------------------------

/// A move whose new host refuses to start leaves the session the tmux
/// session it was, history and all, and a later Restart moves it.
///
/// Why: the goal's "the new launch failing after the old agent stopped",
/// the case most worth getting right. The agent is already stopped when the
/// host start fails, so the session must stay recoverable: the row must be a
/// tmux row naming its pane again (a host row would read that pane as a
/// socket), the old window with its history must stay (it is the only copy),
/// and a Restart must be able to try again. Spec: with the host program
/// refusing (exit 3 after reading its spec), a consented Restart of an old
/// session fails naming the terminal host; the row is a tmux row at
/// generation 1, not launching, naming the old pane, listed as stopped by
/// the user; the session holds no host; the old agent pane is still there
/// (dead) with the first history line; no host socket or record for
/// generation 1 is left, and the agent was launched no more. With the
/// program working again, a Restart succeeds into a host seeded from that
/// same tmux pane, at generation 2, resuming the recorded conversation (one
/// resume in all), and the old window is gone.
#[farhelm_testtrace::test]
async fn a_move_whose_host_refuses_keeps_the_tmux_session_and_a_retry_moves_it() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let sock = stack.state.path().join("tmux.sock");
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let flag = fixture.path().join("refuse");
    let program = failing_host_program(fixture.path(), &flag, None);
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            host_program: Some(program.clone()),
            ..new_seams(home.path())
        })
        .await;
    std::fs::write(&flag, b"").expect("make host starts refuse");
    let refused = client
        .restart_session(&id, true)
        .await
        .expect_err("the new host refuses to start");
    std::fs::remove_file(&flag).expect("let host starts work again");
    assert!(
        format!("{refused:#}").contains("terminal host"),
        "{refused:#}"
    );
    let row = stored_row(stack.state.path(), &id).await;
    assert_eq!(
        (
            row.generation,
            row.terminal_kind,
            row.terminal_address.as_str()
        ),
        (1, TerminalKind::Tmux, old.pane.as_str())
    );
    assert_ne!(row.outcome, LastOutcome::Launching);
    assert_eq!(
        listed(&client, &id).await.annotation.as_deref(),
        Some("stopped by user")
    );
    assert!(sup.agent_host_for_test(&id).await.is_none());
    let kept = agent_panes(&sock, &id).await;
    assert!(
        matches!(&kept[..], [pane] if pane.id == old.pane && pane.dead),
        "the stopped agent's window is kept: {kept:?}"
    );
    assert!(
        pane_text(&sock, &old.pane)
            .await
            .contains("history line 01")
    );
    assert!(host_sockets_in(stack.state.path()).is_empty());
    assert!(!record_path(stack.state.path(), &id, 1).exists());
    assert_eq!(runs(stack.work.path()), ["start"], "nothing was launched");

    client
        .restart_session(&id, true)
        .await
        .expect("the restart moves the session once hosts start again");
    let host = agent_host(&sup, &id).await;
    assert_seeded_from_tmux(&host).await;
    assert_eq!(runs(stack.work.path()), STARTED_AND_RESUMED);
    let row = stored_row(stack.state.path(), &id).await;
    assert_eq!((row.generation, row.terminal_kind), (2, TerminalKind::Host));
    assert!(agent_panes(&sock, &id).await.is_empty());
    client.delete_session(&id).await.expect("delete");
    drop(host);
    retire(stack.state.path(), sup, client).await;
}

/// A move whose read of the tmux pane's history fails is refused before
/// anything but the stop, keeping the tmux session with its history, and
/// the next supervisor moves it with that history.
///
/// Why: the tmux pane is the only copy of the history and the move removes
/// it, so a read that fails must not start the new terminal empty and then
/// destroy the history. The failure is injected in place of the read
/// itself (`TmuxSeedFault::Fails`), so it takes the same handling as a real
/// failed read. The second half is a refusal followed by an orderly
/// shutdown and the next supervisor's Restart, not a crash; nothing durable
/// happens between the stop and the claim but the stop's record, so it
/// leaves the state a crash between them would. Spec: with every history
/// read failing, a consented Restart of an old session is refused, saying
/// the history could not be read and that Clone or Replace also move the
/// session; the row is the tmux row at generation 0 naming the old pane,
/// not launching, and the session lists as stopped by the user; the old
/// agent pane is still there, dead, with the first history line; no host
/// socket or launch record exists and the agent was not launched again.
/// After that supervisor is dropped, the next one (no fault) moves the
/// session into a host seeded from that pane, resuming the recorded
/// conversation, at generation 1.
#[farhelm_testtrace::test]
async fn a_move_whose_history_cannot_be_read_is_refused_and_the_next_supervisor_moves_it() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let sock = stack.state.path().join("tmux.sock");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            faults: FaultHooks {
                tmux_seed_fault: Some(Arc::new(|| Err(TmuxSeedFault::Fails))),
                ..FaultHooks::default()
            },
            ..new_seams(home.path())
        })
        .await;
    let refused = client
        .restart_session(&id, true)
        .await
        .expect_err("the move is refused");
    let refused = format!("{refused:#}");
    assert!(
        refused.contains("history could not be read") && refused.contains("Clone or Replace"),
        "{refused}"
    );
    let row = stored_row(stack.state.path(), &id).await;
    assert_eq!(
        (
            row.generation,
            row.terminal_kind,
            row.terminal_address.as_str()
        ),
        (0, TerminalKind::Tmux, old.pane.as_str())
    );
    assert_ne!(row.outcome, LastOutcome::Launching);
    assert_eq!(
        listed(&client, &id).await.annotation.as_deref(),
        Some("stopped by user")
    );
    let kept = agent_panes(&sock, &id).await;
    assert!(
        matches!(&kept[..], [pane] if pane.id == old.pane && pane.dead),
        "the stopped agent's window is kept: {kept:?}"
    );
    assert!(
        pane_text(&sock, &old.pane)
            .await
            .contains("history line 01")
    );
    assert!(host_sockets_in(stack.state.path()).is_empty());
    assert!(host_records_in(stack.state.path()).is_empty());
    assert_eq!(runs(stack.work.path()), ["start"]);
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(new_seams(home.path())).await;
    client
        .restart_session(&id, true)
        .await
        .expect("the next supervisor moves the session");
    let host = agent_host(&sup, &id).await;
    assert_seeded_from_tmux(&host).await;
    assert_eq!(runs(stack.work.path()), STARTED_AND_RESUMED);
    assert_eq!(stored_row(stack.state.path(), &id).await.generation, 1);
    client.delete_session(&id).await.expect("delete");
    drop(host);
    retire(stack.state.path(), sup, client).await;
}

/// A move whose capture runs out of time is tried again for the newest rows
/// and goes on; one that runs out of time again is refused, saying what else
/// moves the session.
///
/// Why: the time a capture takes is not bounded by the rows a seed holds
/// (a dense true-color history at a wide pane can take longer than the
/// budget on a slow machine every time), and Restart takes the same path on
/// every try, so a refusal alone could leave a session no Restart moves.
/// The timeouts are injected in place of the reads (`TmuxSeedFault::
/// TimesOut`), so they take the same handling as real ones. Spec: with the
/// first three reads timing out and the rest reading the pane, a consented
/// Restart of an old session is refused after two reads, saying the history
/// could not be read and that Clone or Replace also move the session, the
/// row still the tmux row at generation 0; the next Restart, whose first
/// read times out and whose second reads the pane, moves the session into a
/// host seeded from it, resuming the recorded conversation, after four
/// reads in all.
#[farhelm_testtrace::test]
async fn a_move_whose_capture_runs_out_of_time_tries_fewer_rows_then_refuses() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    retire(stack.state.path(), sup, client).await;

    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let fault = {
        let reads = Arc::clone(&reads);
        Arc::new(move || {
            if reads.fetch_add(1, Ordering::SeqCst) < 3 {
                Err(TmuxSeedFault::TimesOut)
            } else {
                Ok(())
            }
        })
    };
    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            faults: FaultHooks {
                tmux_seed_fault: Some(fault),
                ..FaultHooks::default()
            },
            ..new_seams(home.path())
        })
        .await;
    let refused = client
        .restart_session(&id, true)
        .await
        .expect_err("two timeouts refuse the move");
    let refused = format!("{refused:#}");
    assert!(
        refused.contains("history could not be read") && refused.contains("Clone or Replace"),
        "{refused}"
    );
    assert_eq!(
        reads.load(Ordering::SeqCst),
        2,
        "a timeout is tried once more"
    );
    let row = stored_row(stack.state.path(), &id).await;
    assert_eq!((row.generation, row.terminal_kind), (0, TerminalKind::Tmux));

    client
        .restart_session(&id, true)
        .await
        .expect("a retried capture that finishes moves the session");
    assert_eq!(reads.load(Ordering::SeqCst), 4);
    let host = agent_host(&sup, &id).await;
    assert_seeded_from_tmux(&host).await;
    assert_eq!(runs(stack.work.path()), STARTED_AND_RESUMED);
    client.delete_session(&id).await.expect("delete");
    drop(host);
    retire(stack.state.path(), sup, client).await;
}

/// A move from a pane too large for a seed to hold any history starts the
/// new terminal empty at the default size and keeps the old window, the
/// only copy of that history.
///
/// Why: a seed that exists but holds no history (the pane's screens leave
/// no room for a step of history within the restore budget) must not count
/// as having kept the history, or the move would destroy the only copy of
/// it. Spec: an old session whose agent window is resized to 10000x400
/// (premise: its history is still there) is moved by a consented Restart,
/// which succeeds into a host at 80x24 holding no history line, showing the
/// resumed recorded conversation; the old agent pane is still there, dead,
/// holding `history line 01`; Delete ends the host and the tmux session.
#[farhelm_testtrace::test]
async fn a_move_from_a_pane_too_large_to_seed_keeps_the_old_window() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let sock = stack.state.path().join("tmux.sock");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    let resized = tmux_query(
        &sock,
        &["resize-window", "-t", &old.pane, "-x", "10000", "-y", "400"],
    )
    .await;
    assert!(resized.status.success(), "{resized:?}");
    assert!(
        pane_text(&sock, &old.pane)
            .await
            .contains("history line 01"),
        "premise: the resized pane keeps its history"
    );
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(new_seams(home.path())).await;
    client
        .restart_session(&id, true)
        .await
        .expect("the move succeeds without the history");
    let host = agent_host(&sup, &id).await;
    wait_for_screen(&host, RESUMED_CONVERSATION).await;
    assert_eq!((host.size().cols, host.size().rows), (80, 24));
    assert!(
        !history_text(&host).await.contains("history line"),
        "no history in the new terminal"
    );
    let kept = agent_panes(&sock, &id).await;
    assert!(
        matches!(&kept[..], [pane] if pane.id == old.pane && pane.dead),
        "the old window is kept: {kept:?}"
    );
    assert!(
        pane_text(&sock, &old.pane)
            .await
            .contains("history line 01")
    );
    let pid = host_process(host.socket()).expect("the host runs");
    client.delete_session(&id).await.expect("delete");
    assert_host_ended(pid, host.socket()).await;
    assert!(
        panes(&sock, &id).await.is_empty(),
        "Delete ended the tmux session"
    );
    drop(host);
    retire(stack.state.path(), sup, client).await;
}

/// A move whose removal of the old agent window fails still succeeds, and
/// the window, which holds no program, goes with the session's Delete.
///
/// Why: the new agent is running and confirmed by then, so failing the
/// Restart over the window would invite a second Restart of a running
/// agent; the window is dead and shows nowhere, so leaving it to Delete
/// costs nothing. Spec: with the removal failing (the restart's fault seam at
/// `OldWindowRemoval`), a consented Restart of an old session succeeds into a
/// host seeded from the tmux pane, the row a running host row; the old agent
/// pane is still there, dead, and the session lists no tab; Delete ends the
/// host and the tmux session.
#[farhelm_testtrace::test]
async fn a_move_whose_old_window_cannot_be_removed_still_succeeds() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let sock = stack.state.path().join("tmux.sock");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    retire(stack.state.path(), sup, client).await;

    let failed: HostRestartFault = Arc::new(|stage| {
        if stage == HostRestartStage::OldWindowRemoval {
            anyhow::bail!("the old window could not be removed (a test fault)");
        }
        Ok(())
    });
    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            faults: FaultHooks {
                host_restart_fault: Some(failed),
                ..FaultHooks::default()
            },
            ..new_seams(home.path())
        })
        .await;
    client
        .restart_session(&id, true)
        .await
        .expect("the move succeeds");
    let host = agent_host(&sup, &id).await;
    assert_seeded_from_tmux(&host).await;
    let row = stored_row(stack.state.path(), &id).await;
    assert_eq!(
        (row.terminal_kind, &row.outcome),
        (TerminalKind::Host, &LastOutcome::Running)
    );
    let kept = agent_panes(&sock, &id).await;
    assert!(
        matches!(&kept[..], [pane] if pane.id == old.pane && pane.dead),
        "the old window is left, dead: {kept:?}"
    );
    assert!(listed_tabs(&client, &id).await.is_empty());
    let pid = host_process(host.socket()).expect("the host runs");
    client.delete_session(&id).await.expect("delete");
    assert_host_ended(pid, host.socket()).await;
    assert!(
        panes(&sock, &id).await.is_empty(),
        "Delete ended the tmux session"
    );
    drop(host);
    retire(stack.state.path(), sup, client).await;
}

// ---- Interruptions ----------------------------------------------------------------

/// An old session whose move crashed at `stage` (the restart's fault seam),
/// and the supervisor started after the crash on the same state directory.
struct Interrupted {
    stack: Stack,
    _home: farhelm_teststate::TestDir,
    sup: Arc<Supervisor>,
    client: Arc<SupervisorClient>,
    old: OldSession,
}

/// Make an old session, crash its move at `stage`, drop that supervisor and
/// start the next one (switch on, no seam): what the interruption tests
/// share.
async fn interrupted_move(stage: HostRestartStage) -> Interrupted {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    retire(stack.state.path(), sup, client).await;

    let (crash, fired) = crash_once_at(stage);
    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            faults: FaultHooks {
                host_restart_fault: Some(crash),
                ..FaultHooks::default()
            },
            ..new_seams(home.path())
        })
        .await;
    client
        .restart_session(&old.session.id, true)
        .await
        .expect_err("the move is interrupted");
    assert!(
        fired.load(Ordering::SeqCst),
        "premise: the crash seam fired"
    );
    let row = stored_row(stack.state.path(), &old.session.id).await;
    assert_eq!(
        (row.generation, row.terminal_kind),
        (1, TerminalKind::Host),
        "premise: the claim made the row a host row"
    );
    retire(stack.state.path(), sup, client).await;
    let (sup, client) = stack.supervisor(new_seams(home.path())).await;
    Interrupted {
        stack,
        _home: home,
        sup,
        client,
        old,
    }
}

/// A move interrupted right after its generation claim (before the new
/// host's launch record) leaves a host row with no terminal, which the next
/// Restart starts in an empty host; the dead old window stays until Delete.
///
/// Why: the claim writes the host kind (from then on a host may start for
/// the launch), so a crash there leaves a host row with nothing to find:
/// the session must not read as exited or be lost, the next Restart must
/// work, and nothing the move left may outlive the session's Delete. Spec:
/// after a crash at `GenerationClaimed`, the next supervisor holds no
/// terminal for the session, whose row is a launching host row at
/// generation 1 with no `pane`, and no host or record exists; a Restart
/// then succeeds into a host showing the resumed recorded conversation (its
/// history not seeded), at generation 2, the agent launched once more by its
/// resume command; the old agent pane, dead, is still in the tmux session;
/// Delete ends the host and the tmux session.
#[farhelm_testtrace::test]
async fn a_move_interrupted_after_its_claim_restarts_empty_and_delete_ends_the_old_window() {
    let Interrupted {
        stack,
        _home,
        sup,
        client,
        old,
    } = interrupted_move(HostRestartStage::GenerationClaimed).await;
    let id = old.session.id.clone();
    let sock = stack.state.path().join("tmux.sock");
    assert!(sup.agent_host_for_test(&id).await.is_none());
    let row = stored_row(stack.state.path(), &id).await;
    assert_eq!(
        (
            row.generation,
            row.terminal_kind,
            &row.outcome,
            row.terminal_address.as_str()
        ),
        (1, TerminalKind::Host, &LastOutcome::Launching, "")
    );
    assert!(host_sockets_in(stack.state.path()).is_empty());
    assert!(host_records_in(stack.state.path()).is_empty());

    client
        .restart_session(&id, true)
        .await
        .expect("a later restart starts the agent in a host");
    let host = agent_host(&sup, &id).await;
    wait_for_screen(&host, RESUMED_CONVERSATION).await;
    assert_eq!(runs(stack.work.path()), STARTED_AND_RESUMED);
    assert!(
        !history_text(&host).await.contains("history line"),
        "no history is seeded once the pane is no longer the session's terminal"
    );
    assert_eq!(stored_row(stack.state.path(), &id).await.generation, 2);
    let kept = agent_panes(&sock, &id).await;
    assert!(
        matches!(&kept[..], [pane] if pane.id == old.pane && pane.dead),
        "the old window is left, dead: {kept:?}"
    );
    let pid = host_process(host.socket()).expect("the host runs");
    client.delete_session(&id).await.expect("delete");
    assert_host_ended(pid, host.socket()).await;
    assert!(
        panes(&sock, &id).await.is_empty(),
        "Delete ended the tmux session"
    );
    drop(host);
    retire(stack.state.path(), sup, client).await;
}

/// What the next supervisor must make of a move interrupted after its new
/// host started: the new host adopted as the session's running terminal,
/// with the tmux history and the resumed recorded conversation, the agent
/// launched once by its resume command and not again by the recovery, the
/// old window left dead until Delete, which ends both.
async fn assert_move_converged(interrupted: Interrupted) {
    let Interrupted {
        stack,
        _home,
        sup,
        client,
        old,
    } = interrupted;
    let id = old.session.id.clone();
    let sock = stack.state.path().join("tmux.sock");
    let host = agent_host(&sup, &id).await;
    crate::host_restarts::wait_until_caught_up(&host).await;
    assert!(history_text(&host).await.contains("history line 01"));
    wait_for_screen(&host, RESUMED_CONVERSATION).await;
    assert_eq!(runs(stack.work.path()), STARTED_AND_RESUMED);
    let row = stored_row(stack.state.path(), &id).await;
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
            socket_name(host.socket()).as_str()
        )
    );
    let kept = agent_panes(&sock, &id).await;
    assert!(
        matches!(&kept[..], [pane] if pane.id == old.pane && pane.dead),
        "the old window is left, dead, with nothing naming it: {kept:?}"
    );
    assert!(
        listed_tabs(&client, &id).await.is_empty(),
        "the old window is no tab"
    );
    let pid = host_process(host.socket()).expect("the host runs");
    client.delete_session(&id).await.expect("delete");
    assert_host_ended(pid, host.socket()).await;
    assert!(
        panes(&sock, &id).await.is_empty(),
        "Delete ended the tmux session"
    );
    drop(host);
    retire(stack.state.path(), sup, client).await;
}

/// A move interrupted after its new host started, before its confirmation,
/// converges at the next start: the new host is adopted by its tag and
/// confirmed.
///
/// Why: the plan's interruption at replacement, for a session leaving tmux:
/// the row says host from the claim on, so the next supervisor must find the
/// launch by its tag rather than look for a tmux pane, read the session as
/// exited, or start a second agent; the old window, whose pane the claim
/// stopped recording, must at least not show up as a tab and must go with
/// Delete. Spec: after a crash at `ReplacementStarted`, the next supervisor
/// holds the new host as the session's terminal with the tmux history and
/// the resumed recorded conversation, launched once; the row is a running
/// host row at generation 1 naming its socket; the old
/// agent pane is still there, dead, and the session lists no tab; Delete
/// ends the host and the tmux session.
#[farhelm_testtrace::test]
async fn a_move_interrupted_after_its_host_started_converges_at_the_next_start() {
    assert_move_converged(interrupted_move(HostRestartStage::ReplacementStarted).await).await;
}

/// A move interrupted after its confirmation, before the old window was
/// removed, converges at the next start the same way.
///
/// Why: the plan's interruption at the old terminal's end; the confirmed
/// row already names the host, and nothing must undo that. Spec: as
/// [`a_move_interrupted_after_its_host_started_converges_at_the_next_start`],
/// after a crash at `BeforeOldHostEnded`.
#[farhelm_testtrace::test]
async fn a_move_interrupted_before_the_old_window_went_converges_at_the_next_start() {
    assert_move_converged(interrupted_move(HostRestartStage::BeforeOldHostEnded).await).await;
}

/// A move whose session's row is deleted before its confirmation ends the
/// new host, leaving the tmux side to Delete.
///
/// Why: once the row is gone nothing would find the new host again. A real
/// Delete queues behind the restart's lifecycle claim and runs its own tmux
/// teardown, so the row is removed through the store, as the host twin of
/// this test does. Spec: with the fault seam deleting the row at
/// `ReplacementStarted` and letting the move go on, the restart fails as a
/// conflict saying the session was deleted while it was being relaunched and
/// the new agent was torn back down; no host process, socket or record is
/// left, and no row exists.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_move_that_loses_to_delete_ends_its_new_host() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    retire(stack.state.path(), sup, client).await;

    let deleted = Arc::new(AtomicBool::new(false));
    let seam: HostRestartFault = {
        let deleted = Arc::clone(&deleted);
        let db = stack.state.path().join("supervisor.db");
        let id = id.clone();
        Arc::new(move |stage| {
            if stage != HostRestartStage::ReplacementStarted {
                return Ok(());
            }
            let (db, id) = (db.clone(), id.clone());
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
            ..new_seams(home.path())
        })
        .await;
    let error = client
        .restart_session(&id, true)
        .await
        .expect_err("a move whose session was deleted mid-launch fails");
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
            .session(&id)
            .await
            .unwrap()
            .is_none()
    );
    retire(stack.state.path(), sup, client).await;
}

/// An orderly stop that begins while a move reads the tmux pane's history
/// refuses the Restart at once, leaving the stopped tmux session as it was.
///
/// Why: the seed is made on the blocking pool with no runtime the stop would
/// cancel, and the stop must not wait for it (its exit is bounded); nor may
/// the move go on to claim a generation and start a host that no runtime of
/// this supervisor will hold. Spec: with the seed held at its gate (premise:
/// the gate is reached), an orderly stop makes the Restart fail within the
/// test's wait, saying the supervisor is stopping, while the gate is still
/// held; the row is the tmux row at generation 0 naming the old pane, not
/// launching; no host socket or launch record exists.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn an_orderly_stop_during_a_moves_seed_refuses_it() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    retire(stack.state.path(), sup, client).await;

    let (gate, entered, release) = notifying_gate();
    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            faults: FaultHooks {
                tmux_seed_gate: Some(gate),
                ..FaultHooks::default()
            },
            ..new_seams(home.path())
        })
        .await;
    let restart = {
        let client = Arc::clone(&client);
        let id = id.clone();
        tokio::spawn(async move { client.restart_session(&id, true).await })
    };
    tokio::time::timeout(Duration::from_secs(WAIT_SECS), entered.notified())
        .await
        .expect("premise: the move reached its seed");
    sup.stop_in_order_for_test().await;
    let refused = tokio::time::timeout(Duration::from_secs(WAIT_SECS), restart)
        .await
        .expect("the stop refuses the move without waiting for the seed")
        .expect("the restart task")
        .expect_err("the move is refused");
    release.notify_one();
    assert!(
        format!("{refused:#}").contains("supervisor is stopping"),
        "{refused:#}"
    );
    let row = stored_row(stack.state.path(), &id).await;
    assert_eq!(
        (
            row.generation,
            row.terminal_kind,
            row.terminal_address.as_str()
        ),
        (0, TerminalKind::Tmux, old.pane.as_str())
    );
    assert_ne!(row.outcome, LastOutcome::Launching);
    assert!(host_sockets_in(stack.state.path()).is_empty());
    assert!(host_records_in(stack.state.path()).is_empty());
    retire(stack.state.path(), sup, client).await;
}

// ---- The mixed session ------------------------------------------------------------

/// A moved session's agent outlives its tmux session: when the last old tab
/// closes and tmux ends the session, the agent still runs, is still
/// observed as running, survives a supervisor restart, and stops and
/// restarts through its host alone.
///
/// Why: the goal's deliberate subtlety. A moved session keeps the tmux
/// session its row names while it has old tabs, and tmux ends it with its
/// last window, as an ordinary event; no "tmux session missing means the
/// agent is gone" path may then fire for the agent, at the next observation,
/// the next supervisor's reload, a Stop or a Restart. Spec: an old session
/// with one tmux tab, moved into a host by a Restart (premise: its tmux
/// session exists, holding only the tab), has a host tab opened and its
/// tmux tab attached (premise: the tmux session's output client runs);
/// closing the tmux tab detaches its viewer and leaves no tmux session, and
/// once that output client has been reaped, an observation pass finds the
/// row running and the agent's host running its program; the next
/// supervisor adopts the same host as running and lists only the host tab;
/// a Stop records the stop with the host still running; a Restart then
/// succeeds into a new host that keeps the tmux history and shows the
/// resumed recorded conversation, the agent having been resumed twice in
/// all.
#[farhelm_testtrace::test]
async fn a_moved_agent_outlives_its_tmux_session() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let sock = stack.state.path().join("tmux.sock");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    let tmux_tab = client.open_tab(&id).await.expect("a tmux tab");
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(new_seams(home.path())).await;
    client
        .restart_session(&id, true)
        .await
        .expect("the restart moves the session into a host");
    let host = agent_host(&sup, &id).await;
    wait_for_screen(&host, RESUMED_CONVERSATION).await;
    let left = panes(&sock, &id).await;
    assert!(
        matches!(&left[..], [pane] if pane.tab == tmux_tab.id),
        "premise: the tmux session holds only the tab: {left:?}"
    );
    let host_tab = client.open_tab(&id).await.expect("a host tab");
    let tmux_name = format!("fh-{id}");
    let (_channel, mut tab_stream) = attach(
        &client,
        &id,
        TerminalSelector::Tab {
            id: tmux_tab.id.clone(),
        },
        "viewer",
    )
    .await;
    assert!(
        sup.session_sink_pid(&tmux_name).is_some(),
        "premise: the tmux session's output client runs"
    );
    client
        .close_tab(&id, &tmux_tab.id)
        .await
        .expect("close the last tmux tab");
    expect_detached(&mut tab_stream, WAIT_SECS).await;
    assert!(
        panes(&sock, &id).await.is_empty(),
        "tmux ended the session with its last window"
    );
    // Everything tmux ending the session sets off has run once its output
    // client is gone.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(WAIT_SECS);
    while sup.session_sink_pid(&tmux_name).is_some() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the tmux session's output client was never reaped"
        );
        // sleep-ok: polling interval for the sink's reap, which announces nothing to a test.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    sup.reconcile_for_test().await;
    assert_eq!(
        stored_row(stack.state.path(), &id).await.outcome,
        LastOutcome::Running
    );
    assert!(
        matches!(host.liveness(), Liveness::Running { child: Some(_) }),
        "{:?}",
        host.liveness()
    );
    let socket = host.socket().to_path_buf();
    drop(host);
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(new_seams(home.path())).await;
    let host = agent_host(&sup, &id).await;
    assert_eq!(host.socket(), socket, "the same host, adopted");
    wait_for_liveness(&host, "the adopted agent", |liveness| {
        matches!(liveness, Liveness::Running { child: Some(_) })
    })
    .await;
    sup.reconcile_for_test().await;
    assert_eq!(
        stored_row(stack.state.path(), &id).await.outcome,
        LastOutcome::Running
    );
    assert_eq!(listed_tabs(&client, &id).await, vec![host_tab.id.clone()]);
    let pid = host_process(&socket).expect("the agent's host runs");
    client.stop_session(&id).await.expect("stop");
    assert_eq!(
        listed(&client, &id).await.annotation.as_deref(),
        Some("stopped by user")
    );
    assert!(!process_is_gone(pid), "Stop leaves the host running");
    drop(host);

    client
        .restart_session(&id, true)
        .await
        .expect("a moved session restarts from its host alone");
    let new = agent_host(&sup, &id).await;
    assert_ne!(new.socket(), socket);
    wait_for_screen(&new, RESUMED_CONVERSATION).await;
    assert!(history_text(&new).await.contains("history line 01"));
    assert_eq!(
        runs(stack.work.path()),
        ["start", "resume conv-test", "resume conv-test"]
    );
    assert_host_ended(pid, &socket).await;
    client.delete_session(&id).await.expect("delete");
    drop(new);
    retire(stack.state.path(), sup, client).await;
}

// ---- Clone and Replace ------------------------------------------------------------

/// Clone and Replace of an old session create their sessions in hosts once
/// the switch is on, leaving (Clone) or deleting (Replace) the tmux source.
///
/// Why: the goal's "everything new is new": Clone and Replace are creates
/// carrying the source's settings (the helm sends the source's launch and
/// directory as an ordinary create, then, for Replace, deletes the source),
/// so they must take the switched create path whatever the source's kind,
/// and a Replace's Delete of a tmux source must end its tmux session while
/// the new host runs on. Spec: with the switch on, a create with an old
/// session's own stored launch and directory, as the helm's Clone sends it,
/// is a host row whose agent runs in a host (its start's last screen line
/// shown there), while the source is still a tmux row whose agent pane is
/// alive; a second such create followed by a Delete of the source (a
/// Replace) leaves the new session running in its host and no row or tmux
/// session for the source.
#[farhelm_testtrace::test]
async fn clone_and_replace_of_an_old_session_create_host_sessions() {
    let stack = Stack::new().await;
    let home = farhelm_teststate::tempdir().expect("home");
    let sock = stack.state.path().join("tmux.sock");
    let (sup, client) = stack.supervisor(old_seams(home.path())).await;
    let old = old_session(&stack, &sup, &client).await;
    let id = old.session.id.clone();
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(new_seams(home.path())).await;
    let cwd = old.session.cwd.clone();
    let create_like_source = || {
        client.create_session_with_extras(
            &cwd,
            old.session.launch.clone(),
            None,
            80,
            24,
            farhelm_helm::CreateExtras::default(),
        )
    };
    let clone = create_like_source().await.expect("a clone");
    let clone_host = agent_host(&sup, &clone.id).await;
    wait_for_screen(&clone_host, OLD_SCREEN_MARK).await;
    assert_eq!(
        stored_row(stack.state.path(), &clone.id)
            .await
            .terminal_kind,
        TerminalKind::Host
    );
    let source = stored_row(stack.state.path(), &id).await;
    assert_eq!(
        (source.terminal_kind, source.terminal_address.as_str()),
        (TerminalKind::Tmux, old.pane.as_str()),
        "the source is untouched"
    );
    assert!(
        matches!(&agent_panes(&sock, &id).await[..], [pane] if !pane.dead),
        "the source's agent still runs in tmux"
    );

    let replacement = create_like_source().await.expect("a replacement");
    let replacement_host = agent_host(&sup, &replacement.id).await;
    wait_for_screen(&replacement_host, OLD_SCREEN_MARK).await;
    client.delete_session(&id).await.expect("delete the source");
    assert!(
        SessionStore::open(&stack.state.path().join("supervisor.db"), false)
            .await
            .unwrap()
            .session(&id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        panes(&sock, &id).await.is_empty(),
        "the source's tmux session is gone"
    );
    assert!(
        matches!(
            replacement_host.liveness(),
            Liveness::Running { child: Some(_) }
        ),
        "the replacement runs on: {:?}",
        replacement_host.liveness()
    );
    for session in [&clone.id, &replacement.id] {
        client.delete_session(session).await.expect("delete");
    }
    drop((clone_host, replacement_host));
    retire(stack.state.path(), sup, client).await;
}
