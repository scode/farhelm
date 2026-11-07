//! The supervisor's terminal switch points with a host terminal behind a
//! session, against a stand-in host in this test process
//! (`hosts::terminal::test_host`): status and exit recording, the
//! never-started excerpt, an unreadable host, the ticker's wake on a host
//! exit, the hook anchor, the hold wait, prompt delivery, and Stop of a
//! child whose host has not yet reported its exit. Re-adoption tests keep
//! retry timing separate from the action: they assert the real due time,
//! then force only the test-owned schedule due, so the reconnect window
//! needs no minute-long test sleep.
//!
//! The stand-in's child is a `sleep` the stand-in owns, so a verb that
//! kills from the terminal's root reaches only that. The full lifecycle
//! against real hosts, started from the real `farhelm` binary, is in the
//! e2e suite (`crates/farhelm/tests/e2e/host_terminals.rs`), since a host
//! forks and is never started inside a test process.

use super::*;
use crate::hosts::terminal::test_host::{
    TestHost, TestHostSpec, adopt_test_host, adopt_test_host_at, adopt_test_host_recovering,
    adopt_test_host_with, start_test_host, start_test_host_at, test_host_options,
};
use crate::hosts::{HostTerminal, Liveness};
use crate::procs::ProcessIdentity;
use farhelm_proto::io::parse_control;
use farhelm_proto::{FrameKind, TerminalSelector};

/// How long any wait in these tests may take before it fails.
const WAIT: Duration = Duration::from_secs(10);

/// A session entry `s1` whose agent terminal is a fresh test host, running.
async fn host_entry() -> (Arc<SessionEntry>, HostTerminal, TestHost) {
    let (host, test_host) = adopt_test_host(None).await;
    let entry = Arc::new(entry_with(
        Some(Terminal::Host(host.clone())),
        LastOutcome::Running,
    ));
    (entry, host, test_host)
}

/// Wait until `condition` holds for `host`, re-checking on every change of
/// its status.
async fn wait_for(host: &HostTerminal, what: &str, condition: impl Fn(&HostTerminal) -> bool) {
    let mut status = host.subscribe();
    let waited = tokio::time::timeout(WAIT, async {
        while !condition(host) {
            if status.changed().await.is_err() {
                return condition(host);
            }
        }
        true
    })
    .await;
    assert_eq!(
        waited,
        Ok(true),
        "{what} did not happen; status {:?}",
        host.status()
    );
}

/// The row of a session whose agent terminal is the host at `socket`, as
/// the store keeps it: kind `host`, and the socket's name in `pane`.
pub(crate) fn host_row(id: &str, socket: &Path) -> StoredSession {
    let now = now_unix();
    StoredSession {
        conversation_source: None,
        capture_ownership_version: 0,
        omp_reporter_asset: None,
        omp_launch_program: None,
        launch_hooked: false,
        launch_boot_ended: false,
        id: id.to_string(),
        parent: None,
        title: id.to_string(),
        created_at: now,
        last_activity_at: now,
        last_work_started_at: 0,
        creation_seq: 0,
        cwd: "/tmp".to_string(),
        launch: farhelm_proto::SessionLaunch::Legacy {
            invocation: "agent".to_string(),
            agent_kind: AgentKind::Generic,
            resume_template: None,
        },
        tmux_name: session_tmux_name(id),
        pane: socket.file_name().unwrap().to_str().unwrap().to_string(),
        terminal_kind: crate::store::TerminalKind::Host,
        outcome: LastOutcome::Running,
        canonical_cwd: None,
        captured_conversation: None,
        generation: 0,
        launch_scoped: false,
    }
}

/// A host terminal's status follows its runtime: running until the child's
/// exit record, then exited with the code the host reported.
///
/// Why: the ticker's observer commits what this evidence says, durably and
/// every tick; a host-backed agent read as absent would be recorded as
/// exited within a tick of its launch (the inventory's top risk 1). Spec:
/// while the child runs, the session lists as live and an observation
/// offers the store nothing; once the host reports exit status 7, the
/// session lists as exited with code 7 and the observation offers that
/// exit; neither needs a pane map.
#[farhelm_testtrace::test]
async fn a_host_terminals_status_follows_its_runtime_and_records_the_exit_code() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (entry, host, test_host) = host_entry().await;
    let no_panes = HashMap::new();
    let known = crate::service::status::KnownTmuxNames::default();

    let (status, _) = session_status(&entry, &no_panes, &known);
    assert!(
        status.is_live(),
        "a running child lists as live: {status:?}"
    );
    let observed = crate::service::status::observe_entry(&sup, &entry, &no_panes, &known)
        .await
        .expect("observe");
    assert_eq!(observed.transition, None, "nothing to record while it runs");

    test_host.output(b"done\r\n");
    test_host.exit(7);
    wait_for(&host, "the exit", |host| {
        matches!(host.liveness(), Liveness::Exited { .. })
    })
    .await;
    assert_eq!(
        session_status(&entry, &no_panes, &known).0,
        SessionStatus::Exited { exit_code: Some(7) }
    );
    let observed = crate::service::status::observe_entry(&sup, &entry, &no_panes, &known)
        .await
        .expect("observe");
    assert_eq!(
        observed.transition,
        Some(Transition::ObservedExit { exit_code: Some(7) })
    );
    assert_eq!(observed.sentinel, None);
}

/// A host launch that never reached the agent is classified as never
/// started, quoting the final screen from the emulator, with no tmux
/// banner.
///
/// Why: the never-started classification needs a terminal that exists and
/// is dead, which a host reports from its exit record; and the excerpt is
/// what tells the user why (a startup file that exits the shell). A host
/// has no "Pane is dead" line, so nothing in the excerpt may come from
/// tmux. Spec: with the launch's spec still unconsumed and the child
/// having printed a complaint and exited 1, the observation reports the
/// login shell's failure with exit code 1 and the complaint as the last
/// terminal output, and records it as an error.
#[farhelm_testtrace::test]
async fn a_never_started_host_launch_quotes_the_emulators_screen() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (entry, host, test_host) = host_entry().await;
    let spec = crate::launch::LaunchArtifacts::for_launch(state.path(), "s1", 0)
        .spec()
        .to_path_buf();
    std::fs::create_dir_all(spec.parent().unwrap()).expect("launch dir");
    std::fs::write(&spec, b"{}").expect("an unconsumed launch spec");

    test_host.output(b"bash: .bashrc: line 3: oops\r\n");
    test_host.exit(1);
    wait_for(&host, "the exit", |host| {
        matches!(host.liveness(), Liveness::Exited { .. })
    })
    .await;
    let observed = crate::service::status::observe_entry(
        &sup,
        &entry,
        &HashMap::new(),
        &crate::service::status::KnownTmuxNames::default(),
    )
    .await
    .expect("observe");
    let detail = observed.sentinel.expect("a never-started classification");
    assert!(
        detail.contains("your login shell exited (code 1)"),
        "{detail}"
    );
    assert!(
        detail.ends_with("; last terminal output: bash: .bashrc: line 3: oops"),
        "{detail}"
    );
    assert!(!detail.contains("Pane is dead"), "{detail}");
    assert!(matches!(
        observed.transition,
        Some(Transition::SentinelError { .. })
    ));
}

/// A host that cannot be read is left alone: no exit is concluded or
/// recorded, and the verbs refuse.
///
/// Why: the host may run a live agent that a newer supervisor, or a
/// recovering one, can still read; recording an exit, or killing nothing
/// and reporting it stopped, would be a lie the user acts on (the plan's
/// D-16). Spec: with the child alive and a valid sentinel for the current
/// launch on disk, a probe sees the child live; once the runtime finds its
/// host speaking a protocol it does not know, the root that probe would
/// authorize cannot be taken (an error, not "no root"), the session lists
/// as unknown, an observation offers nothing (no transition and no
/// sentinel), the process probe and the delete guard are errors, and a
/// prompt is refused for the host's unavailability, with nothing reaching
/// the host.
#[farhelm_testtrace::test]
async fn an_unreadable_host_records_nothing_and_its_verbs_refuse() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (host, test_host) = adopt_test_host(None).await;
    let mut entry = entry_with(Some(Terminal::Host(host.clone())), LastOutcome::Running);
    entry.snapshot.kind = AgentKind::Claude;
    entry.info.agent_kind = AgentKind::Claude;
    let entry = Arc::new(entry);
    sup.sessions
        .lock()
        .await
        .insert(entry.info.id.clone(), Arc::clone(&entry));

    let status = state.path().to_path_buf();
    let sentinel = crate::launch::LaunchArtifacts::for_launch(&status, "s1", 0)
        .status()
        .to_path_buf();
    std::fs::create_dir_all(sentinel.parent().unwrap()).expect("launch dir");
    std::fs::write(&sentinel, "exec failed: no such file").expect("a current sentinel");
    let terminal = entry.terminal.as_ref().unwrap();
    let crate::tmux::PaneProbe::Owned(live) = terminal.probe_process(&sup).await.unwrap() else {
        panic!("premise: the child is owned and live");
    };
    assert!(!live.dead, "premise: the child runs");

    test_host.become_unreadable();
    wait_for(&host, "the host becoming unreadable", |host| {
        matches!(host.liveness(), Liveness::Unavailable { .. })
    })
    .await;

    let no_panes = HashMap::new();
    let known = crate::service::status::KnownTmuxNames::default();
    assert_eq!(
        session_status(&entry, &no_panes, &known).0,
        SessionStatus::Unknown
    );
    assert!(
        terminal.live_root(&live).is_err(),
        "a root the earlier probe authorized cannot be taken from an unreadable host"
    );
    let observed = crate::service::status::observe_entry(&sup, &entry, &no_panes, &known)
        .await
        .expect("observe");
    assert_eq!(observed.transition, None, "not even the sentinel");
    assert_eq!(observed.sentinel, None);
    assert!(terminal.probe_process(&sup).await.is_err());
    for guard in [
        farhelm_proto::DeleteGuard::AgentEnded,
        farhelm_proto::DeleteGuard::NothingAlive,
    ] {
        assert!(
            sup.still_alive_for_delete(&entry, guard).await.is_err(),
            "{guard:?} cannot tell, so it refuses"
        );
    }
    let refused = sup
        .prompt_session("s1", "hello")
        .await
        .expect_err("an unreadable host takes no prompt");
    assert!(
        format!("{refused:#}").contains("the terminal host is unavailable"),
        "refused for the host, not for its status or paste mode: {refused:#}"
    );
    assert_eq!(test_host.received(), b"");
}

/// A nothing-alive approval must not end a tab that becomes unreadable
/// during the guard. Force the running-to-unavailable transition after
/// the initial uncertainty check, where a tmux query can yield, and require
/// a refusal rather than treating the now-uncountable tab as absent.
#[farhelm_testtrace::test]
async fn delete_refuses_a_tab_that_becomes_unreadable_between_its_checks() {
    let state = StateDir::new();
    let (host, peer) = adopt_test_host(None).await;
    let peer = Arc::new(peer);
    let mut seams = SupervisorSeams::default();
    let changing_host = host.clone();
    seams.faults.delete_tab_count_gate = Some(Arc::new(move || {
        let peer = Arc::clone(&peer);
        let host = changing_host.clone();
        Box::pin(async move {
            assert!(matches!(host.liveness(), Liveness::Running { .. }));
            peer.become_unreadable();
            wait_for(&host, "tab becoming unreadable", |host| {
                matches!(host.liveness(), Liveness::Unavailable { .. })
            })
            .await;
        })
    }));
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts::default(),
        seams,
    )
    .await
    .expect("supervisor");
    let entry = entry_with(
        None,
        LastOutcome::Exited {
            exit_code: Some(0),
            annotation: None,
        },
    );
    entry
        .session
        .host_tabs
        .lock()
        .unwrap()
        .push(super::super::host_tabs::HostTab {
            tab_id: "t1".into(),
            created_ns: 1,
            scoped: false,
            host,
            end_failed: false,
            failed_ends: 0,
            retry_at: None,
        });
    let (refusal, settled) = sup
        .still_alive_for_delete(&entry, farhelm_proto::DeleteGuard::NothingAlive)
        .await
        .expect("guard");
    assert!(refusal.is_some_and(|why| why.contains("cannot be read")));
    assert!(settled.is_none(), "a refusal must not authorize teardown");
}

/// A host agent's exit is recorded as soon as its runtime announces it,
/// without waiting for a tick.
///
/// Why: tmux's `pane-died` hook wakes the ticker for a pane; a host has no
/// hook, and without the liveness notifier's wake a host agent's exit
/// would wait for the next tick. Spec: with the tick interval far beyond
/// the wait, a running session whose host reports exit status 5 has
/// `Exited` with code 5 in its durable row well before any tick.
#[farhelm_testtrace::test]
async fn the_ticker_records_a_host_agents_exit_on_its_announcement() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            ticker_interval: Duration::from_secs(600),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let (host, test_host) = adopt_test_host(Some(sup.host_liveness.clone())).await;
    sup.store
        .insert_session(host_row("s1", test_host.socket()), None)
        .await
        .expect("insert the row the ticker must update");
    let entry = Arc::new(entry_with(
        Some(Terminal::Host(host.clone())),
        LastOutcome::Running,
    ));
    sup.sessions
        .lock()
        .await
        .insert("s1".to_string(), Arc::clone(&entry));
    let _ticker = start_ticker(&sup);

    test_host.exit(5);
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let row = sup.store.session("s1").await.expect("read").expect("row");
        if row.outcome
            == (LastOutcome::Exited {
                exit_code: Some(5),
                annotation: None,
            })
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the exit was not recorded; row {:?}, host {:?}",
            row.outcome,
            host.status()
        );
        // sleep-ok: polling the durable row, the oracle for the recorded exit, until the deadline.
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// A hook report from a host-backed session is anchored at the host's
/// child, with its start identity, and waits rather than being refused
/// while the session's entry is not published.
///
/// Why: the anchor is what ties a report to this launch; without it every
/// report from a host agent is refused, the conversation identity is never
/// captured, and Restart cannot offer resume (the inventory's top risk 10).
/// The row's `pane` holds the host's socket name, which must never be read
/// as a tmux pane. Spec: for a host row of generation 0 whose entry holds
/// that host, the anchor is the host's child with its start identity; with no entry, or an entry of another generation, the
/// anchor is a transient `Internal`; with an entry of the row's generation
/// holding a tmux terminal, a `Conflict`.
#[farhelm_testtrace::test]
async fn a_host_agents_hook_anchor_is_its_child() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (entry, _host, test_host) = host_entry().await;
    let row = host_row("s1", test_host.socket());
    let child = ProcessIdentity::read(test_host.child_pid()).expect("the host's child");

    let gap = sup.owned_pane_anchor(&row).await.expect_err("no entry yet");
    assert_eq!(gap.kind, ErrorKind::Internal, "{}", gap.message);

    sup.sessions
        .lock()
        .await
        .insert("s1".to_string(), Arc::clone(&entry));
    assert_eq!(
        sup.owned_pane_anchor(&row).await.expect("the anchor"),
        crate::procs::PaneAnchor {
            pid: child.pid,
            start: Some(child.start)
        }
    );

    let later = StoredSession {
        generation: 1,
        ..row.clone()
    };
    let other = sup
        .owned_pane_anchor(&later)
        .await
        .expect_err("another generation's entry");
    assert_eq!(other.kind, ErrorKind::Internal, "{}", other.message);

    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(
            Some(Terminal::tmux("fh-s1".into(), "%0".into())),
            LastOutcome::Running,
        )),
    );
    let mismatch = sup
        .owned_pane_anchor(&row)
        .await
        .expect_err("a tmux entry for a host row");
    assert_eq!(mismatch.kind, ErrorKind::Conflict, "{}", mismatch.message);
}

/// An orderly stop waits for a new host-backed session to hold its version,
/// by its child's pid, and not for one whose child has exited.
///
/// Why: a host-backed session's probe must not read as gone while its
/// child starts, or the wait ends early and an install can remove the
/// version the starting session needs. Spec: with the supervisor in the
/// versioned layout and a launch tracked against a running test host, the
/// wait runs out without a mark, runs out with a mark naming another
/// process, and settles with a mark naming the host's child; after the
/// child's exit, a launch with no mark settles at once.
#[farhelm_testtrace::test]
async fn the_hold_wait_waits_for_a_host_agents_child() {
    let state = StateDir::new();
    let program = state
        .path()
        .join("Farhelm.app/Contents/Versions/9.9.9/farhelm");
    let sup = Supervisor::new_with_exe(state.path(), program)
        .await
        .expect("supervisor");
    let (host, test_host) = adopt_test_host(None).await;
    let child = test_host.child_pid();
    let spec = crate::launch::spec_path_for_launch(state.path(), "s", 0);
    let held = crate::session_hold::held_mark_path(&spec);
    std::fs::create_dir_all(spec.parent().unwrap()).expect("launch dir");
    let short = Duration::from_millis(300);

    sup.track_pending_hold(&spec, Terminal::Host(host.clone()));
    assert!(
        !sup.wait_for_pending_holds(short).await,
        "no mark yet: the wait runs out"
    );
    std::fs::write(&held, format!("{}\n", child + 1)).expect("a stale mark");
    sup.track_pending_hold(&spec, Terminal::Host(host.clone()));
    assert!(
        !sup.wait_for_pending_holds(short).await,
        "a mark naming another process does not count"
    );
    std::fs::write(&held, format!("{child}\n")).expect("the child's mark");
    sup.track_pending_hold(&spec, Terminal::Host(host.clone()));
    assert!(
        sup.wait_for_pending_holds(PENDING_HOLD_WAIT).await,
        "the child's own mark settles it"
    );

    std::fs::remove_file(&held).expect("clear the mark");
    test_host.exit(0);
    wait_for(&host, "the exit", |host| host.child_root().is_none()).await;
    sup.track_pending_hold(&spec, Terminal::Host(host.clone()));
    assert!(
        sup.wait_for_pending_holds(PENDING_HOLD_WAIT).await,
        "an exited child needs no wait"
    );
}

/// A prompt to a host-backed agent is gated on the emulator's bracketed
/// paste mode and arrives as one bracketed paste followed by the submit.
///
/// Why: without paste mode the program would read the prompt's newlines as
/// submits; and the program reads its own terminal modes from the
/// supervisor's emulator now, not from tmux. Spec: with paste mode off, the
/// prompt is refused and nothing reaches the host; once the program has
/// turned paste mode on, the host receives exactly the opening marker, the
/// prompt, the closing marker and Claude's submit key, in that order, for a
/// prompt whose paste fits one input and for one at the prompt cap, whose
/// paste is sent in two parts with the submit only after both.
#[farhelm_testtrace::test]
async fn a_prompt_to_a_host_agent_is_gated_on_its_paste_mode() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (host, test_host) = adopt_test_host(None).await;
    let mut entry = entry_with(Some(Terminal::Host(host.clone())), LastOutcome::Running);
    entry.snapshot.kind = AgentKind::Claude;
    entry.info.agent_kind = AgentKind::Claude;
    entry.run.activity.lock().unwrap().reading.state =
        crate::agent_kind::screen_reader::ScreenState::Idle;
    sup.sessions
        .lock()
        .await
        .insert("s1".to_string(), Arc::new(entry));

    let refused = sup
        .prompt_session("s1", "first\nsecond")
        .await
        .expect_err("paste mode is off");
    assert!(
        format!("{refused:#}").contains("bracketed paste mode is off"),
        "{refused:#}"
    );
    assert_eq!(test_host.received(), b"");

    test_host.output(b"\x1b[?2004h");
    wait_for(&host, "paste mode", |host| host.modes().bracketed_paste).await;
    let mut expected = Vec::new();
    // One prompt whose paste fits one input, and one at the prompt cap,
    // whose paste (the markers included) takes two.
    let small = format!("first\n\t{}\nlast λ", "chunk ".repeat(100));
    let large = format!(
        "{}λ",
        "x".repeat(farhelm_proto::launcher::TEMPLATE_FIELDS_CAP - "λ".len())
    );
    assert!(
        large.len() + 12 > terminal_host::protocol::MAX_INPUT,
        "premise: the large paste does not fit one input"
    );
    for prompt in [small, large] {
        sup.prompt_session("s1", &prompt)
            .await
            .expect("the prompt is delivered");
        expected.extend_from_slice(b"\x1b[200~");
        expected.extend_from_slice(prompt.as_bytes());
        expected.extend_from_slice(b"\x1b[201~\r");
        assert_eq!(test_host.received(), expected);
    }
}

/// Send `message` through the control dispatcher and return its reply.
async fn control_reply(sup: &Arc<Supervisor>, message: ControlMsg) -> ControlMsg {
    let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
    let mut channels = ConnectionChannels::default();
    let mut tasks = tokio::task::JoinSet::new();
    handle_control(
        sup,
        message,
        ConnectionCtx {
            panics: &mpsc::channel(1).0,
            tx: &tx,
            priority: &tx,
            channels: &mut channels,
            tasks: &mut tasks,
        },
    )
    .await;
    let reply = tokio::time::timeout(WAIT, rx.recv())
        .await
        .expect("a reply in time")
        .expect("the reply channel is open");
    serde_json::from_slice(&reply.body).expect("a control reply")
}

/// A session `s1` whose agent terminal is a fresh test host, published and
/// stored as a host row, with its launch spec still unconsumed: the shape
/// of a launch whose login shell exited before it reached the agent.
async fn never_started_host_session(
    sup: &Arc<Supervisor>,
    state: &StateDir,
) -> (Arc<SessionEntry>, HostTerminal, TestHost) {
    let (entry, host, test_host) = host_entry().await;
    sup.store
        .insert_session(host_row("s1", test_host.socket()), None)
        .await
        .expect("insert the session's row");
    sup.sessions
        .lock()
        .await
        .insert("s1".to_string(), Arc::clone(&entry));
    let spec = crate::launch::LaunchArtifacts::for_launch(state.path(), "s1", 0)
        .spec()
        .to_path_buf();
    std::fs::create_dir_all(spec.parent().unwrap()).expect("launch dir");
    std::fs::write(&spec, b"{}").expect("an unconsumed launch spec");
    (entry, host, test_host)
}

/// A Stop that finds a host's child gone before the host has reported its
/// exit records a plain exit rather than a launch failure from a partial
/// screen, and the failure is classified once the record arrives.
///
/// Why: a host's probe says dead as soon as its child is reaped, while the
/// child's last output and its exit code arrive with the exit record. A
/// launch failure is durable and never revised, so one recorded from the
/// early probe would keep a screen missing the line that explains the
/// failure, and no code. Spec: with the child killed and nothing reported,
/// Stop succeeds after its bounded wait and stores a plain exit with no
/// code; once the host sends the complaint and exit status 1, an
/// observation offers the never-started error quoting the complaint and
/// the code, and the store takes it over the plain exit.
#[farhelm_testtrace::test]
async fn a_stop_before_the_exit_record_records_no_launch_failure() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (entry, host, test_host) = never_started_host_session(&sup, &state).await;
    test_host.kill_child();
    assert!(
        host.child_root().is_none(),
        "premise: the root is withdrawn"
    );
    assert_eq!(
        host.evidence(),
        crate::hosts::HostEvidence::Owned {
            dead: false,
            exit_code: None
        },
        "premise: no exit record yet"
    );

    let reply = control_reply(
        &sup,
        ControlMsg::StopSession {
            req_id: 1,
            session_id: "s1".to_string(),
        },
    )
    .await;
    assert!(
        matches!(reply, ControlMsg::SessionStopped { .. }),
        "{reply:?}"
    );
    let row = sup.store.session("s1").await.expect("read").expect("row");
    assert_eq!(
        row.outcome,
        LastOutcome::Exited {
            exit_code: None,
            annotation: None
        }
    );

    test_host.output(b"bash: .bashrc: line 3: oops\r\n");
    test_host.exit(1);
    wait_for(&host, "the exit record", |host| {
        matches!(host.liveness(), Liveness::Exited { .. })
    })
    .await;
    let observed = crate::service::status::observe_entry(
        &sup,
        &entry,
        &HashMap::new(),
        &crate::service::status::KnownTmuxNames::default(),
    )
    .await
    .expect("observe");
    let transition = observed.transition.expect("the launch failure");
    sup.record("s1", &entry, transition)
        .await
        .expect("record the failure");
    let row = sup.store.session("s1").await.expect("read").expect("row");
    let LastOutcome::Error { detail } = row.outcome else {
        panic!(
            "the launch failure replaces the plain exit: {:?}",
            row.outcome
        );
    };
    assert!(detail.contains("exited (code 1)"), "{detail}");
    assert!(detail.ends_with("bash: .bashrc: line 3: oops"), "{detail}");
}

/// A Stop that finds a host's child gone classifies the launch from the
/// final screen and code when the host's exit record arrives while it runs.
///
/// Why: when the record arrives in time, Stop must record the launch
/// failure itself, complete. Whether Stop's probe saw the child before or
/// after the record depends on scheduling; either way the outcome must be
/// the complete error. That the wait itself holds until the record arrives
/// is pinned by
/// `a_host_terminal_holds_its_final_state_only_once_its_exit_record_arrives`.
/// Spec: with the child killed and the host then sending the complaint and
/// exit status 1 while Stop runs, Stop stores the never-started error
/// quoting the complaint, with the code.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_stop_classifies_a_host_launch_from_its_exit_record() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (_entry, _host, test_host) = never_started_host_session(&sup, &state).await;
    test_host.kill_child();

    let stop = tokio::spawn({
        let sup = Arc::clone(&sup);
        async move {
            control_reply(
                &sup,
                ControlMsg::StopSession {
                    req_id: 1,
                    session_id: "s1".to_string(),
                },
            )
            .await
        }
    });
    test_host.output(b"bash: .bashrc: line 3: oops\r\n");
    test_host.exit(1);
    let reply = stop.await.expect("the stop task");
    assert!(
        matches!(reply, ControlMsg::SessionStopped { .. }),
        "{reply:?}"
    );
    let row = sup.store.session("s1").await.expect("read").expect("row");
    let LastOutcome::Error { detail } = row.outcome else {
        panic!("Stop records the launch failure: {:?}", row.outcome);
    };
    assert!(detail.contains("exited (code 1)"), "{detail}");
    assert!(detail.ends_with("bash: .bashrc: line 3: oops"), "{detail}");
}

/// Poll `future` once, saying whether it is still pending, so that a test
/// can show a wait is waiting without a delay to wait it out. The test
/// task's waker is registered, so awaiting the future later goes on from
/// there.
async fn pending_once<F: std::future::Future + Unpin>(future: &mut F) -> bool {
    std::future::poll_fn(|cx| {
        std::task::Poll::Ready(std::pin::Pin::new(&mut *future).poll(cx).is_pending())
    })
    .await
}

/// A host terminal holds its final state only once the host's exit record
/// has arrived, and a wait for it waits until then.
///
/// Why: Stop classifies a launch that never started only from a terminal
/// holding its final state (`Terminal::holds_final_state`); a host's probe
/// says dead as soon as the child is gone, before the output it left and
/// its exit code have arrived. Spec: with the child killed and nothing
/// reported, the probe says dead; a short wait ends without the final
/// state; a longer one is still pending, and completes holding it once the
/// host sends the child's last words and its exit status, which the screen
/// then shows.
#[farhelm_testtrace::test]
async fn a_host_terminal_holds_its_final_state_only_once_its_exit_record_arrives() {
    let (host, test_host) = adopt_test_host(None).await;
    test_host.kill_child();
    let terminal = Terminal::Host(host.clone());
    assert!(
        matches!(
            host.probe(),
            Ok(crate::tmux::PaneProbe::Owned(process)) if process.dead
        ),
        "premise: the probe says dead"
    );
    assert!(
        !terminal.holds_final_state(Duration::from_millis(200)).await,
        "no record: the bound passes without the final state"
    );

    let mut wait = Box::pin(terminal.holds_final_state(WAIT));
    assert!(pending_once(&mut wait).await, "waiting for the record");
    test_host.output(b"bash: .bashrc: line 3: oops\r\n");
    test_host.exit(1);
    assert!(
        tokio::time::timeout(WAIT, wait)
            .await
            .expect("the wait ends once the record arrives"),
        "the record gives the final state"
    );
    assert!(host.screen_tail(4096).contains("oops"));
}

/// A Stop that finds a host's child gone records the launch shim's own
/// failure report at once, without waiting for the host's exit record.
///
/// Why: the shim writes its report before the child ends and the report
/// quotes no screen, so nothing about it needs the record; and if the host
/// became unreadable while Stop waited, nothing would ever read the report
/// for this launch again (an unreadable host is not observed). Spec: with
/// the shim's report on disk, the child killed and no exit record, Stop
/// stores the launch's error with the report's text. (Not asserted, since
/// it is a timing on a shared machine: Stop reads the report before it
/// would wait for the record, so it returns without the wait.)
#[farhelm_testtrace::test]
async fn a_stop_before_the_exit_record_still_records_the_shims_report() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (_entry, host, test_host) = never_started_host_session(&sup, &state).await;
    let report = crate::launch::LaunchArtifacts::for_launch(state.path(), "s1", 0)
        .status()
        .to_path_buf();
    std::fs::write(&report, "exec failed: no such file").expect("the shim's report");
    test_host.kill_child();
    assert!(
        host.child_root().is_none(),
        "premise: the root is withdrawn"
    );
    assert_eq!(
        host.evidence(),
        crate::hosts::HostEvidence::Owned {
            dead: false,
            exit_code: None
        },
        "premise: no exit record yet"
    );

    let reply = control_reply(
        &sup,
        ControlMsg::StopSession {
            req_id: 1,
            session_id: "s1".to_string(),
        },
    )
    .await;
    assert!(
        matches!(reply, ControlMsg::SessionStopped { .. }),
        "{reply:?}"
    );
    let row = sup.store.session("s1").await.expect("read").expect("row");
    assert_eq!(
        row.outcome,
        LastOutcome::Error {
            detail: "exec failed: no such file".to_string()
        }
    );
}

/// A Restart of a session with no terminal whose row says its agent was
/// started in a host relaunches it in a new host, never in tmux.
///
/// Why: a relaunch keeps the row's kind, and a host row with no terminal
/// (its host was found gone, or a create was kept as a launching record)
/// relaunched in tmux would leave a host row whose `pane` holds a tmux pane
/// id, which every later reader looks for as a host's socket. Spec: with a
/// resumable session's row turned into a host row naming socket `ABCDEFGH`
/// and its entry holding no terminal, under a supervisor whose host program
/// does not exist, a consented Restart fails starting the session's
/// terminal host (not tmux) definitively: the row is still a host row with
/// `pane` `ABCDEFGH` and its earlier outcome, at the next generation, and
/// the failed launch left no launch spec or host record behind.
#[farhelm_testtrace::test]
async fn a_terminal_less_host_row_restarts_in_a_host_never_in_tmux() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            host_program: Some(PathBuf::from("/nonexistent/farhelm-host")),
            ..SupervisorSeams::tmux_terminals()
        },
    )
    .await
    .expect("supervisor");
    let created = create_resumable_session(&sup, "/tmp", "sleep 300").await;
    let id = created.id.clone();
    sup.store
        .set_terminal_for_test(&id, crate::store::TerminalKind::Host, "ABCDEFGH")
        .await
        .expect("make the row a host row");
    sup.forget_agent_terminal_for_test(&id)
        .await
        .expect("leave the session with no terminal");
    let before = sup.store.session(&id).await.expect("read").expect("row");

    let failed = sup
        .restart_session(&id, true, None, None)
        .await
        .expect_err("no host can start from a program that does not exist");
    let failed = format!("{failed:#}");
    assert!(failed.contains("terminal host"), "{failed}");
    assert!(!failed.contains("tmux"), "{failed}");
    let after = sup.store.session(&id).await.expect("read").expect("row");
    assert_eq!(
        (
            after.terminal_kind,
            after.pane.as_str(),
            &after.outcome,
            after.generation
        ),
        (
            crate::store::TerminalKind::Host,
            "ABCDEFGH",
            &before.outcome,
            before.generation + 1
        )
    );
    let artifacts = LaunchArtifacts::for_launch(state.path(), &id, after.generation);
    assert!(
        !artifacts.spec().exists(),
        "the failed launch's spec is gone"
    );
    assert!(!artifacts.host().exists(), "no host record is left");
}

/// A retried create whose launch kept and published its host, with no
/// other evidence of the launch, replays the session and starts nothing.
///
/// Why: both reviews' finding. A create whose confirmation failed and
/// whose host could not be confirmed ended keeps its launching row (with no
/// `pane`) and publishes that host; nothing else may show the launch (no
/// scope on a machine without a user manager, no sentinel, no report). A
/// retry that looked only at hosts it does not drive would find nothing
/// and relaunch beside the running agent. Spec: with a pending reservation
/// for `s1`, its launching host row with an empty `pane`, a published entry
/// holding a stand-in host, no user manager and no sentinel, a create with
/// the same key answers with `s1`; the reservation is settled as created;
/// the entry still holds the same host, and the host was sent nothing.
#[farhelm_testtrace::test]
async fn a_retry_finds_the_sessions_own_published_host() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            scopes: Arc::new(crate::scope::ScopeManager::disabled()),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let (host, test_host) = adopt_test_host(Some(sup.host_liveness.clone())).await;
    let claim = IntentClaim {
        intent_key: "kept".to_string(),
        fingerprint: command_fingerprint("/tmp", "agent", None),
        dedup_scope: DedupScope::SessionLifetime,
    };
    let mut row = host_row("s1", test_host.socket());
    row.pane = String::new();
    row.outcome = LastOutcome::Launching;
    sup.store
        .insert_session(row, Some(claim.clone()))
        .await
        .expect("the launching row and its reservation");
    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(
            Some(Terminal::Host(host.clone())),
            LastOutcome::Launching,
        )),
    );

    let replayed = sup
        .create_session_without_overrides("/tmp", "agent", None, 80, 24, Some(claim))
        .await
        .expect("the retry answers");
    assert_eq!(replayed.id, "s1");
    let reservation = sup
        .store
        .reservation("kept")
        .await
        .expect("read")
        .expect("the reservation");
    assert_eq!(reservation.outcome, ReservationOutcome::Created);
    let entry = sup.sessions.lock().await.get("s1").cloned().expect("entry");
    assert_eq!(entry.terminal, Some(Terminal::Host(host)));
    assert_eq!(test_host.received(), b"");
}

// ---- Delete with hosts it cannot read, cannot end, or that are not its own ----------

/// Ask `sup` to delete `s1` under `guard`, through the control dispatcher.
async fn delete_s1(sup: &Arc<Supervisor>, guard: farhelm_proto::DeleteGuard) -> ControlMsg {
    control_reply(
        sup,
        ControlMsg::DeleteSession {
            req_id: 1,
            session_id: "s1".to_string(),
            only_if_nothing_alive: guard.only_if_nothing_alive(),
            only_if_agent_ended: guard.only_if_agent_ended(),
            refuse_if_agent_request_in_flight: false,
        },
    )
    .await
}

/// The message of a refused reply, failing the test on anything else.
fn refusal(reply: ControlMsg, what: &str) -> String {
    match reply {
        ControlMsg::Error { message, .. } => message,
        other => panic!("{what}: expected a refusal, got {other:?}"),
    }
}

/// A socket path in `state`'s socket directory, where launch records name
/// hosts.
fn socket_in_state(state: &StateDir) -> std::path::PathBuf {
    let hosts = crate::hosts::ensure_hosts_dir(state.path()).expect("the socket directory");
    crate::hosts::HostName::random()
        .expect("a socket name")
        .socket_in(&hosts)
}

/// A tag naming `session_id`'s agent of launch generation 0.
fn agent_tag(session_id: &str) -> Vec<u8> {
    crate::hosts::HostTag {
        session_id: session_id.to_string(),
        created_ns: 1,
        role: crate::hosts::HostRole::Agent { generation: 0 },
    }
    .encode()
    .expect("encode the tag")
}

/// Write `s1`'s launch record for generation 0, naming `socket` and a first
/// stage that has already ended, as a create leaves it once its host
/// started.
fn record_s1(state: &StateDir, socket: &Path) {
    let mut ended = std::process::Command::new("true")
        .spawn()
        .expect("start a process to end");
    let pid = ended.id();
    ended.wait().expect("reap it");
    let record = super::super::host_launch::LaunchHostRecord {
        socket: crate::hosts::HostName::parse(socket.file_name().unwrap().to_str().unwrap())
            .expect("a socket name this supervisor makes"),
        starter: crate::hosts::HostProcess { pid, start: 0 },
    };
    let artifacts = crate::launch::LaunchArtifacts::for_launch(state.path(), "s1", 0);
    std::fs::create_dir_all(artifacts.host().parent().unwrap()).expect("launch dir");
    std::fs::write(artifacts.host(), record.encode()).expect("write the launch record");
}

/// Publish `s1` with no agent terminal, as a reload that has not found it
/// leaves a session, over a row whose agent ran in tmux, so that only its
/// launch records name a terminal host.
async fn terminal_less_tmux_s1(sup: &Arc<Supervisor>) {
    let mut row = host_row("s1", Path::new("/nonexistent/hosts/ABCDEFGH"));
    row.terminal_kind = crate::store::TerminalKind::Tmux;
    row.pane = "%1".to_string();
    sup.store
        .insert_session(row, None)
        .await
        .expect("insert the session's row");
    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(None, LastOutcome::Running)),
    );
}

/// Whether process `pid` runs: it exists and is not a zombie. A stopped
/// process runs, for this purpose. A stand-in host's child, killed, stays a
/// zombie until the stand-in reaps it, which is why a zombie must not count.
///
/// It asks the sweep's own process reads (`crate::procs`) rather than
/// reading `/proc/<pid>/stat`, because macOS has no `/proc` and these tests
/// run there too. A failed read fails the test rather than reading as "not
/// running", so that neither a premise nor an "it is gone" check passes on a
/// process it could not see. The one exception is Linux's: a `/proc` entry
/// hidden by `hidepid` (a pid reused by another user's process) reads as no
/// process. A pid reused by another process of this user is not detected: it
/// reads as the same process.
fn runs(pid: u32) -> bool {
    let process = crate::procs::read_process(pid)
        .unwrap_or_else(|error| panic!("fixture: reading process {pid}: {error}"));
    matches!(process, Some((_, _, crate::procs::ProcessState::Running)))
}

/// An unconditional Delete of a session whose agent host cannot be read
/// still kills the agent's child from the identity pinned at adoption, and
/// keeps the session when the host itself cannot be confirmed ended.
///
/// Why: an unreadable host says nothing about its child (the plan's D-16),
/// but Delete ends the terminal for good, and refusing outright would leave
/// a session nobody can delete whose host runs on. So Delete roots its
/// sweep at the pinned child, which needs nothing from the host, and ends
/// the host by the verified signal; and when even that cannot confirm the
/// host ended, it must not report the session deleted while the host may
/// still run. The stand-in's process is this test runner, which the
/// verified signal never signals (its command line names no host socket),
/// so this reaches the refusal deliberately. Spec: with the host made
/// unreadable and its child running (premises checked), an unconditional
/// Delete fails saying the session's terminal hosts could not be confirmed
/// ended and the session was kept; the child is gone; the row and the
/// session's entry remain.
#[farhelm_testtrace::test]
async fn delete_roots_an_unreadable_hosts_child_and_keeps_the_session_if_the_host_may_run() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (entry, host, test_host) = host_entry().await;
    sup.store
        .insert_session(host_row("s1", test_host.socket()), None)
        .await
        .expect("insert the session's row");
    sup.sessions
        .lock()
        .await
        .insert("s1".to_string(), Arc::clone(&entry));
    test_host.become_unreadable();
    wait_for(&host, "the host becoming unreadable", |host| {
        matches!(host.liveness(), Liveness::Unavailable { .. })
    })
    .await;
    let child = test_host.child_pid();
    assert!(runs(child), "premise: the host's child {child} runs");

    let message = refusal(
        delete_s1(&sup, farhelm_proto::DeleteGuard::Unconditional).await,
        "a host that may still run keeps the session",
    );
    assert!(
        message.contains("could not confirm that this session's terminal hosts ended"),
        "{message}"
    );
    assert!(message.contains("kept for a retry"), "{message}");
    assert!(
        !runs(child),
        "the child was killed from its pinned identity"
    );
    assert!(
        sup.store.session("s1").await.expect("read").is_some(),
        "the row is kept"
    );
    assert!(
        sup.sessions.lock().await.contains_key("s1"),
        "the session stays listed"
    );
}

/// A recorded host of the session that cannot be read makes a guarded
/// Delete refuse, and an unconditional one signal its process and keep the
/// session when that cannot confirm the end.
///
/// Why: a launch record names a host of the session that may still run its
/// agent; one that answers nothing usable cannot say whether it does (a
/// guarded Delete must not kill what nobody agreed to), and an
/// unconditional Delete must end it or keep the session. It ends the
/// process the connection named, with the verified signal, and never
/// whatever answers the socket on a new connection. The stand-in's process
/// is this test runner, which the verified signal never signals (its
/// command line names no host socket). Spec: with `s1` terminal-less over a
/// tmux row and a launch record naming a stand-in in the socket directory
/// that answers only with a protocol no supervisor speaks (premise: its
/// child runs), a Delete guarded on the agent having ended is refused
/// because the recorded host cannot be read, and the child still runs; an
/// unconditional Delete fails naming the socket and saying the process is
/// not the host there, having connected to the stand-in only to settle the
/// record (two connections in all, one per Delete), and the row is kept.
#[farhelm_testtrace::test]
async fn a_recorded_host_that_cannot_be_read_is_guarded_and_signalled() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    terminal_less_tmux_s1(&sup).await;
    let socket = socket_in_state(&state);
    let test_host = start_test_host_at(&socket, Vec::new(), true);
    record_s1(&state, &socket);
    let child = test_host.child_pid();
    assert!(runs(child), "premise: the host's child {child} runs");

    let refused = refusal(
        delete_s1(&sup, farhelm_proto::DeleteGuard::AgentEnded).await,
        "the guard cannot tell whether the recorded host's program runs",
    );
    assert!(refused.contains("cannot be read"), "{refused}");
    assert!(runs(child), "a refused delete kills nothing");

    let unconfirmed = refusal(
        delete_s1(&sup, farhelm_proto::DeleteGuard::Unconditional).await,
        "the host cannot be confirmed ended",
    );
    assert!(
        unconfirmed.contains(&socket.display().to_string()),
        "{unconfirmed}"
    );
    assert!(
        unconfirmed.contains("is not the terminal host"),
        "{unconfirmed}"
    );
    assert_eq!(
        test_host.connections(),
        2,
        "each Delete connected once, to settle the record; the signal reconnected to nothing"
    );
    assert!(
        sup.store.session("s1").await.expect("read").is_some(),
        "the row is kept"
    );
}

/// A launch record whose socket now serves another session's host makes
/// Delete refuse and touch nothing.
///
/// Why: Delete acts only on what the session records, and a record is only
/// as good as the socket still being the session's; a host whose tag names
/// another session is someone else's terminal, which no Delete of this
/// session may sweep, shut down or signal. Spec: with `s1` terminal-less
/// over a tmux row and its launch record naming a stand-in tagged as `s2`'s
/// agent (premise: its child runs), an unconditional Delete fails saying
/// the socket serves a host for another terminal; the stand-in's child
/// still runs, the stand-in was sent no input, and the row is kept.
#[farhelm_testtrace::test]
async fn a_record_naming_another_sessions_host_refuses_and_touches_nothing() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    terminal_less_tmux_s1(&sup).await;
    let socket = socket_in_state(&state);
    let other = start_test_host_at(&socket, agent_tag("s2"), false);
    record_s1(&state, &socket);
    let child = other.child_pid();
    assert!(runs(child), "premise: the other host's child {child} runs");

    let refused = refusal(
        delete_s1(&sup, farhelm_proto::DeleteGuard::Unconditional).await,
        "a record naming another terminal's host refuses",
    );
    assert!(
        refused.contains("serves a host for another terminal"),
        "{refused}"
    );
    assert!(runs(child), "the other session's program was not swept");
    assert_eq!(other.received(), b"", "nothing was written to it");
    assert!(
        sup.store.session("s1").await.expect("read").is_some(),
        "the row is kept"
    );
}

/// Ending a session's hosts is bounded as a whole: a host whose end does
/// not finish in time is reported, the session is kept, and the delete
/// lets go of everything it held.
///
/// Why: Delete holds the session's lifecycle claim and the supervisor-wide
/// directory admission while it ends hosts; without an overall deadline one
/// host whose end does not finish would hold every create and delete on the
/// machine. The gate holds each end before it starts, standing in for an
/// end that does not finish; it does not reproduce a process read stuck in
/// the kernel, which no test can arrange (`hosts::process`, where such a
/// read runs off the async workers behind the walk gate, says what then
/// happens). Spec: with `s1` terminal-less over a tmux row, a launch record
/// naming a stand-in tagged as `s1`'s agent, a gate that holds every host's
/// end for good and a budget of one second, an unconditional Delete returns
/// within ten seconds, failing with the host not confirmed ended within the
/// budget; the row is kept; and afterwards both the directory admission and
/// `s1`'s lifecycle claim can be taken at once.
#[farhelm_testtrace::test]
async fn deleting_a_sessions_hosts_gives_up_at_its_deadline_and_keeps_the_session() {
    let state = StateDir::new();
    let mut seams = SupervisorSeams::default();
    let held: SinkReservationGate = Arc::new(|| Box::pin(std::future::pending()));
    seams.faults.delete_host_end_gate = Some(held);
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts {
            delete_host_end: Duration::from_secs(1),
            ..SupervisorTimeouts::default()
        },
        seams,
    )
    .await
    .expect("supervisor");
    terminal_less_tmux_s1(&sup).await;
    let socket = socket_in_state(&state);
    let _test_host = start_test_host_at(&socket, agent_tag("s1"), false);
    record_s1(&state, &socket);

    let started = std::time::Instant::now();
    let message = refusal(
        tokio::time::timeout(
            Duration::from_secs(10),
            delete_s1(&sup, farhelm_proto::DeleteGuard::Unconditional),
        )
        .await
        .expect("the delete returns within its budget"),
        "a host whose end does not finish keeps the session",
    );
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(
        message.contains("was not confirmed ended within"),
        "{message}"
    );
    assert!(
        sup.store.session("s1").await.expect("read").is_some(),
        "the row is kept"
    );
    assert!(
        Arc::clone(&sup.working_copy_operations)
            .try_lock_owned()
            .is_ok(),
        "the directory admission was released"
    );
    tokio::time::timeout(Duration::from_secs(1), sup.lifecycle_locks.claim("s1"))
        .await
        .expect("the lifecycle claim was released");
}

/// Delete leaves the launch record of the host its agent runtime holds to
/// that runtime, and never connects to that host itself.
///
/// Why: every connection to a host takes it over, so one made to settle
/// the record of the host a runtime holds would cut that runtime off,
/// mid-shutdown at worst, and both would then fight for the host. A create
/// leaves exactly such a record for every host it starts. The gate holds
/// every end, so the only connections are the adoption's and any settling
/// would make. Spec: with `s1`'s agent a stand-in host adopted from the
/// state directory's socket directory, a launch record naming it, a gate
/// that holds every host's end and a one-second budget, an unconditional
/// Delete fails with the host not confirmed ended within the budget, and
/// the stand-in has accepted exactly one connection, the adoption's.
#[farhelm_testtrace::test]
async fn delete_leaves_the_record_of_the_runtimes_host_to_the_runtime() {
    let state = StateDir::new();
    let mut seams = SupervisorSeams::default();
    let held: SinkReservationGate = Arc::new(|| Box::pin(std::future::pending()));
    seams.faults.delete_host_end_gate = Some(held);
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts {
            delete_host_end: Duration::from_secs(1),
            ..SupervisorTimeouts::default()
        },
        seams,
    )
    .await
    .expect("supervisor");
    let socket = socket_in_state(&state);
    let (host, test_host) = adopt_test_host_at(&socket, agent_tag("s1"), None).await;
    sup.store
        .insert_session(host_row("s1", &socket), None)
        .await
        .expect("insert the session's row");
    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(Some(Terminal::Host(host)), LastOutcome::Running)),
    );
    record_s1(&state, &socket);
    assert_eq!(
        test_host.connections(),
        1,
        "premise: the adoption connected"
    );

    let message = refusal(
        delete_s1(&sup, farhelm_proto::DeleteGuard::Unconditional).await,
        "the held end keeps the session",
    );
    assert!(
        message.contains("was not confirmed ended within"),
        "{message}"
    );
    assert_eq!(
        test_host.connections(),
        1,
        "Delete connected to the host its runtime holds"
    );
}

/// Delete removes the socket file a host killed outright left behind, once
/// its record's socket is found gone.
///
/// Why: a host killed with SIGKILL leaves its socket file, and every later
/// look at that socket costs the refusal window before it reads as gone;
/// Delete is the last moment anything comes back for this session's host.
/// The removal judges the socket gone again first. Spec: with `s1`
/// terminal-less over a tmux row and its launch record naming a socket file
/// nothing listens on any more (premise: the file is there), an
/// unconditional Delete succeeds, the socket file is gone, and so is the
/// row.
#[farhelm_testtrace::test]
async fn delete_removes_the_socket_file_a_killed_recorded_host_left() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    terminal_less_tmux_s1(&sup).await;
    let socket = socket_in_state(&state);
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("bind, then leave the file"));
    record_s1(&state, &socket);
    assert!(
        socket.exists(),
        "premise: the dead host's socket file is there"
    );

    let reply = delete_s1(&sup, farhelm_proto::DeleteGuard::Unconditional).await;
    assert!(
        matches!(reply, ControlMsg::SessionDeleted { .. }),
        "{reply:?}"
    );
    assert!(!socket.exists(), "the leftover socket file was removed");
    assert!(
        sup.store.session("s1").await.expect("read").is_none(),
        "the row is gone"
    );
}

// ---- Across supervisor restarts ---------------------------------------------------

/// A prompt to a host agent whose runtime is still replaying its host's
/// journal after a restart is refused, even with paste mode on, and is
/// delivered once the runtime has caught up.
///
/// Why: the prompt's bracketed-paste check reads the emulator's modes, and
/// after a recovered adoption those are a past screen's until the replay
/// ends (PR 15's review, tracked for this change): a program that has
/// since turned paste mode off would read the prompt's newlines as submits.
/// The status sampler waits for the same. Spec: a stand-in host that
/// journaled paste mode on and more output before its recovered adoption
/// streams only the first part; the runtime shows paste mode on and is not
/// caught up; a prompt is refused as still being restored and nothing
/// reaches the host. Once the rest is streamed and the runtime has caught
/// up, the same prompt reaches the host whole, with Claude's submit key.
#[farhelm_testtrace::test]
async fn a_prompt_waits_for_a_recovered_host_terminal_to_catch_up() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let paste_on = b"\x1b[?2004h";
    let mut journaled = paste_on.to_vec();
    journaled.extend_from_slice(b"output from before the restart\r\n");
    let (host, test_host) = adopt_test_host_recovering(None, journaled.clone()).await;
    let mut entry = entry_with(Some(Terminal::Host(host.clone())), LastOutcome::Running);
    entry.snapshot.kind = AgentKind::Claude;
    entry.info.agent_kind = AgentKind::Claude;
    entry.run.activity.lock().unwrap().reading.state =
        crate::agent_kind::screen_reader::ScreenState::Idle;
    sup.sessions
        .lock()
        .await
        .insert("s1".to_string(), Arc::new(entry));

    test_host.replay_journaled(paste_on.len());
    wait_for(&host, "paste mode from the replay", |host| {
        host.modes().bracketed_paste
    })
    .await;
    assert!(
        !host.status().caught_up,
        "premise: the runtime has not caught up with the journal: {:?}",
        host.status()
    );
    let refused = sup
        .prompt_session("s1", "hello")
        .await
        .expect_err("a terminal still replaying takes no prompt");
    assert!(
        format!("{refused:#}").contains("still being restored"),
        "refused for the replay, not for its status or paste mode: {refused:#}"
    );
    assert_eq!(test_host.received(), b"");

    test_host.replay_journaled(journaled.len());
    wait_for(&host, "the runtime catching up", |host| {
        host.status().caught_up
    })
    .await;
    assert!(
        host.modes().bracketed_paste,
        "premise: paste mode is still on"
    );
    sup.prompt_session("s1", "hello")
        .await
        .expect("the prompt is delivered once caught up");
    assert_eq!(test_host.received(), b"\x1b[200~hello\x1b[201~\r");
}

/// `entry` with its agent terminal replaced by `terminal`, everything else
/// shared.
fn with_terminal(entry: &SessionEntry, terminal: Option<Terminal>) -> SessionEntry {
    SessionEntry {
        info: entry.info.clone(),
        terminal,
        run: entry.run.clone(),
        session: entry.session.clone(),
        snapshot: entry.snapshot.clone(),
        canonical_cwd: entry.canonical_cwd.clone(),
        generation: entry.generation,
        scope: entry.scope.clone(),
    }
}

/// The spare-checkout claims at startup protect a host session's clone
/// while its launch's host may still run, read from the host itself and
/// never from tmux.
///
/// Why: the inventory's top risk 3. A clone claim that reads as abandoned
/// is reclaimed by the repository-cache sweep at startup, deleting a
/// checkout an agent is still filling; a host row's `pane` names a socket,
/// which tmux's pane lookup reads as a pane that does not exist, so every
/// host session would read as abandoned. Spec: for a session whose clone is
/// in progress and whose row is a host row naming its host's socket, the
/// claim is protected while its runtime runs and while its runtime cannot
/// read the host (unavailable is never gone); for the same session with no
/// terminal on its entry, the claim is released only while no socket file
/// is at the path its row names, and protected once one is there; and a
/// host whose child has exited releases it, as a dead pane does.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn spare_claims_read_a_host_rows_own_host() {
    let state = StateDir::new();
    let root = state.path().join("checkouts");
    std::fs::create_dir(&root).unwrap();
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts::default(),
        scopeless_seams(),
    )
    .await
    .unwrap();
    let origin = fresh_create(&sup, &checkout_fixture(&root), None)
        .await
        .unwrap();
    let generation = sup
        .store
        .session(&origin.id)
        .await
        .unwrap()
        .unwrap()
        .generation;
    let plan = sup
        .store
        .preparation_origin(&origin.id, generation)
        .await
        .unwrap()
        .expect("the create's preparation");
    crate::launch::write_preparation_state(
        &crate::launch::preparation_state_path(state.path(), &plan.id),
        &plan.id,
        &crate::launch::PreparationState::CloneStarted,
        &crate::files::RealFs,
    )
    .unwrap();
    let claim = (origin.id.clone(), generation);
    let protected = || async { sup.spare_claims_from_panes(&HashMap::new()).await.protected };

    // The runtime of a host that runs: live, protected.
    let (_, test_host) = adopt_test_host(None).await;
    let host = sup
        .install_host_agent_for_test(&origin.id, test_host.socket())
        .await
        .expect("the session's agent moves into the stand-in host");
    let row = sup.store.session(&origin.id).await.unwrap().unwrap();
    assert_eq!(row.terminal_kind, crate::store::TerminalKind::Host);
    assert_eq!(
        row.pane, "h",
        "premise: the row names the stand-in's socket"
    );
    assert!(
        protected().await.contains(&claim),
        "a running host protects"
    );

    // No terminal on the entry: the socket the row names decides.
    let entry = sup.sessions.lock().await.get(&origin.id).cloned().unwrap();
    sup.sessions
        .lock()
        .await
        .insert(origin.id.clone(), Arc::new(with_terminal(&entry, None)));
    let named = crate::hosts::hosts_dir(state.path()).join(&row.pane);
    assert!(
        !named.exists(),
        "premise: nothing at the named socket's path"
    );
    assert!(
        !protected().await.contains(&claim),
        "no host at the row's socket releases the claim"
    );
    std::fs::create_dir_all(named.parent().unwrap()).unwrap();
    std::fs::write(&named, b"").unwrap();
    assert!(
        protected().await.contains(&claim),
        "something at the row's socket may be its host"
    );
    std::fs::remove_file(&named).unwrap();

    // A runtime that cannot read its host: unavailable, never gone.
    sup.sessions.lock().await.insert(
        origin.id.clone(),
        Arc::new(with_terminal(&entry, Some(Terminal::Host(host.clone())))),
    );
    test_host.become_unreadable();
    wait_for(&host, "the host becoming unreadable", |host| {
        matches!(host.liveness(), Liveness::Unavailable { .. })
    })
    .await;
    assert!(
        protected().await.contains(&claim),
        "an unreadable host protects"
    );

    // A host whose child exited: dead, as a dead pane.
    let (exited, exiting_host) = adopt_test_host(None).await;
    sup.sessions.lock().await.insert(
        origin.id.clone(),
        Arc::new(with_terminal(&entry, Some(Terminal::Host(exited.clone())))),
    );
    exiting_host.exit(0);
    wait_for(&exited, "the child's exit", |host| {
        matches!(host.liveness(), Liveness::Exited { .. })
    })
    .await;
    assert!(
        !protected().await.contains(&claim),
        "a host whose child exited releases the claim"
    );
}

/// A stand-in host for `id`'s agent, launch generation 0, bound in
/// `state`'s socket directory where a reload's scan finds it, with `spec`
/// otherwise; and the socket's name.
fn discoverable_host(state: &StateDir, id: &str, spec: TestHostSpec) -> (TestHost, String) {
    let dir = crate::hosts::ensure_hosts_dir(state.path()).expect("the socket directory");
    let name = crate::hosts::HostName::random().expect("a socket name");
    let tag = crate::hosts::HostTag {
        session_id: id.to_string(),
        created_ns: crate::hosts::tag::creation_stamp(),
        role: crate::hosts::HostRole::Agent { generation: 0 },
    }
    .encode()
    .expect("a tag");
    let host = start_test_host(TestHostSpec {
        socket: Some(name.socket_in(&dir)),
        tag: if spec.tag.is_empty() { tag } else { spec.tag },
        ..spec
    });
    (host, name.to_string())
}

/// End a stand-in host started in a state directory's socket directory and
/// remove its socket file, which a stand-in leaves behind, so that the
/// state directory's own cleanup does not wait out the refusal window for
/// it.
fn end_discoverable(host: TestHost) {
    let socket = host.socket().to_path_buf();
    drop(host);
    let _ = std::fs::remove_file(socket);
}

/// A durable Stop request survives a restart whose replay of the host's
/// journal has not reached the child's exit record yet, and becomes an exit
/// annotated as stopped once it has.
///
/// Why: a recovered host whose child the host already reaped reads as
/// running until its runtime replays the exit record, and reload does not
/// wait for the replay; if reload took that for proof that the stop's kill
/// missed, it would clear the stop intent, and the exit would then be
/// recorded as a plain exit, losing "stopped by user" (the review's
/// finding on provisional replay liveness). Spec: a host row `s1` at
/// `StopRequested` whose stand-in host reports its child reaped with
/// status 3 and holds its journal and exit record back: after a reload the
/// session's terminal is that host, running with no child to root, and
/// the row is still `StopRequested`, also after an observation pass; once
/// the journal and the exit record are streamed and processed, an
/// observation records `Exited` with code 3 and the stop annotation.
#[farhelm_testtrace::test]
async fn a_stop_request_survives_a_restart_whose_replay_has_not_reached_the_exit() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (test_host, name) = discoverable_host(
        &state,
        "s1",
        TestHostSpec {
            journaled: b"bye\r\n".to_vec(),
            exited: Some(3),
            ..TestHostSpec::default()
        },
    );
    let mut row = host_row("s1", test_host.socket());
    row.pane = name;
    row.outcome = LastOutcome::StopRequested;
    sup.store.insert_session(row, None).await.expect("the row");

    let seams = SupervisorSeams::default();
    let (mut sessions, _) =
        Supervisor::reload_sessions(state.path(), &sup.store, &sup.tmux, &seams, true)
            .await
            .expect("reload");
    let entry = sessions.remove("s1").expect("the session");
    let Some(Terminal::Host(host)) = entry.terminal.clone() else {
        panic!("the session's terminal is its host: {:?}", entry.terminal);
    };
    assert_eq!(host.liveness(), Liveness::Running { child: None });
    let stored = || async { sup.store.session("s1").await.unwrap().unwrap().outcome };
    assert_eq!(stored().await, LastOutcome::StopRequested);
    sup.sessions.lock().await.insert("s1".to_string(), entry);
    sup.reconcile_for_test().await;
    assert_eq!(stored().await, LastOutcome::StopRequested);

    test_host.replay_journaled(5);
    test_host.exit(3);
    wait_for(&host, "the exit record", |host| {
        matches!(host.liveness(), Liveness::Exited { .. })
    })
    .await;
    sup.reconcile_for_test().await;
    assert_eq!(
        stored().await,
        LastOutcome::Exited {
            exit_code: Some(3),
            annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
        }
    );
    drop(host);
    end_discoverable(test_host);
}

/// A host session whose launch record cannot be read, beside a host whose
/// tag cannot be read, keeps an unavailable terminal through reload and
/// later observations, which record nothing and remove nothing, its launch
/// sentinel included.
///
/// Why: the plan's D-16. The unreadable record may name a host that still
/// runs (here, perhaps the one with the unreadable tag), so the session
/// must not read as having no terminal: no terminal reads as gone, and an
/// observation would then record the sentinel's launch error and clean the
/// launch's artifacts up, the very inference reload refused to draw.
/// Spec: a launching host row `s1` with an empty `pane`, a launch record
/// that is not one, a launch sentinel, and a stand-in host in the socket
/// directory with a tag no build reads: after a reload the session holds a
/// host terminal that is unavailable; after an observation pass the row is
/// still `Launching`, the session lists as unknown, and the sentinel and
/// the launch record are still there.
#[farhelm_testtrace::test]
async fn an_unreadable_launch_record_leaves_its_session_unavailable() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (stranger, _) = discoverable_host(
        &state,
        "s1",
        TestHostSpec {
            tag: b"not a tag any build reads".to_vec(),
            ..TestHostSpec::default()
        },
    );
    let mut row = host_row("s1", stranger.socket());
    row.pane = String::new();
    row.outcome = LastOutcome::Launching;
    sup.store.insert_session(row, None).await.expect("the row");
    let artifacts = crate::launch::LaunchArtifacts::for_launch(state.path(), "s1", 0);
    std::fs::create_dir_all(artifacts.host().parent().unwrap()).unwrap();
    std::fs::write(artifacts.host(), b"not a launch record").unwrap();
    std::fs::write(artifacts.status(), "exec failed: no such file").unwrap();

    let seams = SupervisorSeams::default();
    let (mut sessions, _) =
        Supervisor::reload_sessions(state.path(), &sup.store, &sup.tmux, &seams, true)
            .await
            .expect("reload");
    let entry = sessions.remove("s1").expect("the session");
    let Some(Terminal::Host(host)) = entry.terminal.clone() else {
        panic!("the session holds a host terminal: {:?}", entry.terminal);
    };
    assert!(
        matches!(host.liveness(), Liveness::Unavailable { .. }),
        "{:?}",
        host.liveness()
    );
    sup.sessions
        .lock()
        .await
        .insert("s1".to_string(), Arc::clone(&entry));
    sup.reconcile_for_test().await;
    assert_eq!(
        sup.store.session("s1").await.unwrap().unwrap().outcome,
        LastOutcome::Launching
    );
    let no_panes = HashMap::new();
    let known = crate::service::status::KnownTmuxNames::default();
    assert_eq!(
        session_status(&entry, &no_panes, &known).0,
        SessionStatus::Unknown
    );
    assert!(artifacts.status().exists(), "the sentinel is left alone");
    assert!(artifacts.host().exists(), "the launch record is left alone");
    end_discoverable(stranger);
}

/// Checkpoints at a stop are admitted by tail size, even when the session
/// map offers them in exactly the opposite order, and a held encode keeps
/// the only slot until it settles.
///
/// Why: sorting before spawning is insufficient when the spawned tasks
/// race to acquire their permits. The map's actual order establishes a
/// deliberately wrong admission order here; each terminal has a distinct
/// tail and its own encode gate. The largest gate must be reached first,
/// while smaller runtimes still process new output with their gates
/// untouched. Opening it admits the next
/// largest, and then the smallest, before the stop succeeds. No output is
/// enough to trigger an automatic checkpoint, so every encode observed
/// belongs to the stop.
#[farhelm_testtrace::test]
async fn an_orderly_stop_admits_largest_tails_first_with_one_slot() {
    use crate::hosts::terminal::EncodeGate;

    /// Release every gate during an assertion unwind too, so a failed
    /// admission check does not leave blocking encoders held until their
    /// independent safety timeout.
    struct ReleaseEncodes(Vec<EncodeGate>);
    impl Drop for ReleaseEncodes {
        fn drop(&mut self) {
            for gate in &self.0 {
                gate.open();
            }
        }
    }

    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let mut fixtures = HashMap::new();
    let mut release = ReleaseEncodes(Vec::new());
    for id in ["s1", "s2", "s3"] {
        let gate = EncodeGate::closed();
        release.0.push(gate.clone());
        let (host, test_host) = adopt_test_host_with(
            TestHostSpec {
                options: Some(crate::hosts::HostTerminalOptions {
                    encode_gate: Some(gate.clone()),
                    // The ordering proof needs every tiny tail encoded;
                    // automatic checkpoints keep their ordinary threshold.
                    checkpoints: crate::hosts::CheckpointPolicy {
                        stop_min_bytes: 0,
                        ..Default::default()
                    },
                    ..test_host_options(None)
                }),
                ..TestHostSpec::default()
            },
            crate::hosts::Adoption::Fresh,
        )
        .await;
        sup.sessions.lock().await.insert(
            id.to_string(),
            Arc::new(entry_with(
                Some(Terminal::Host(host.clone())),
                LastOutcome::Running,
            )),
        );
        fixtures.insert(id.to_string(), (host, test_host, gate));
    }
    // No map mutation follows: the gather will see this same order. Give
    // its first entry the smallest tail and its last entry the largest.
    let map_order: Vec<_> = sup.sessions.lock().await.keys().cloned().collect();
    for (index, id) in map_order.iter().enumerate() {
        let (host, test_host, gate) = &fixtures[id];
        let output = vec![b'x'; (index + 1) * 128];
        test_host.output(&output);
        wait_for(host, "the complete tail before stopping", |host| {
            host.status().position.offset == output.len() as u64
        })
        .await;
        assert_eq!(host.status().checkpoint, None);
        assert!(!gate.wait_entered(Duration::ZERO), "no automatic encode");
    }

    let inspect = async {
        for (admitted, id) in map_order.iter().rev().enumerate() {
            let gate = fixtures[id].2.clone();
            assert!(
                tokio::task::spawn_blocking(move || gate.wait_entered(WAIT))
                    .await
                    .expect("wait for the admitted encode"),
                "the next-largest tail {id} reached its encode gate"
            );
            for queued in map_order.iter().take(map_order.len() - admitted - 1) {
                let (host, test_host, gate) = &fixtures[queued];
                let before = host.status().position.offset;
                test_host.output(b"y");
                wait_for(host, "a queued runtime still processing output", |host| {
                    host.status().position.offset > before
                })
                .await;
                assert!(
                    !gate.wait_entered(Duration::ZERO),
                    "smaller tail {queued} entered while {id} held the only slot"
                );
            }
            fixtures[id].2.open();
        }
    };
    let (current, ()) = tokio::join!(sup.checkpoint_hosts_for_exit_with_slots(WAIT, 1), inspect,);
    assert!(current, "every ordered checkpoint was accepted");
    for (_, (_, _, gate)) in fixtures {
        assert!(gate.passed());
        assert!(!gate.expired());
        assert!(!gate.cancelled());
    }
}

/// An orderly stop's checkpoints end within its budget even when an encode
/// would run far longer, and leave no encode behind on the blocking pool,
/// so the runtime that owns the supervisor drops at once and its state
/// directory's claim comes free.
///
/// Why: the review's finding on the bounded stop. An encode runs on the
/// blocking pool, which nothing interrupts from outside, and the runtime
/// `farhelm supervisor run` owns waits for it as it drops, holding the
/// state directory's claim: an encode left running past the stop's deadline
/// held up the exit, and a successor's start, for as long as it took.
/// Spec: on a runtime the test owns, a supervisor whose host terminal's
/// every encode is held by a closed gate (as a very long encode would hold
/// it) is checkpointed for exit with a 500 ms budget: that returns within
/// 1.5 s, reports a host left without a fresh snapshot, and the held encode
/// was released by its cancel, not by the gate's own time limit; dropping
/// the runtime then takes under 2 s (the gate's limit is 30 s), and the
/// state directory can be claimed again.
#[farhelm_testtrace::test]
fn an_orderly_stop_ends_its_encodes_before_its_runtime_drops() {
    let state = StateDir::new();
    let gate = crate::hosts::terminal::EncodeGate::closed();
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let budget = Duration::from_millis(500);
    runtime.block_on(async {
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (host, test_host) = adopt_test_host_with(
            TestHostSpec {
                options: Some(crate::hosts::HostTerminalOptions {
                    encode_gate: Some(gate.clone()),
                    // Force the stop to reach the cancellation gate even
                    // though the fixture writes only a few bytes.
                    checkpoints: crate::hosts::CheckpointPolicy {
                        stop_min_bytes: 0,
                        ..Default::default()
                    },
                    ..test_host_options(None)
                }),
                ..TestHostSpec::default()
            },
            crate::hosts::Adoption::Fresh,
        )
        .await;
        test_host.output(b"some output\r\n");
        wait_for(&host, "the output", |host| {
            host.screen_tail(4096).contains("some output")
        })
        .await;
        sup.sessions.lock().await.insert(
            "s1".to_string(),
            Arc::new(entry_with(
                Some(Terminal::Host(host.clone())),
                LastOutcome::Running,
            )),
        );
        let started = std::time::Instant::now();
        let current = sup.checkpoint_hosts_for_exit(budget).await;
        let took = started.elapsed();
        assert!(!current, "the held encode left the host no fresh snapshot");
        assert!(
            took < budget + Duration::from_secs(1),
            "the stop kept its budget: {took:?}"
        );
        assert!(gate.passed(), "premise: an encode reached the gate");
        assert!(gate.cancelled(), "the encode was released by its cancel");
        assert!(!gate.expired(), "not by the gate's own limit");
        drop(test_host);
    });
    let dropping = std::time::Instant::now();
    drop(runtime);
    assert!(
        dropping.elapsed() < Duration::from_secs(2),
        "no encode outlived the stop: the runtime took {:?} to drop",
        dropping.elapsed()
    );
    assert!(
        StateDirOwnership::claim(state.path())
            .expect("claim")
            .is_some(),
        "the state directory's claim came free"
    );
}

/// A placeholder in session `s1`'s entry for the stand-in host `test_host`
/// (a startup that timed out), with `s1`'s row naming its socket.
async fn placeholder_session(sup: &Supervisor, test_host: &TestHost, name: &str) -> HostTerminal {
    let placeholder = HostTerminal::unavailable(
        test_host.socket(),
        crate::hosts::Unavailability::Connect(crate::hosts::Unavailable::Timeout),
        None,
    );
    let mut row = host_row("s1", test_host.socket());
    row.pane = name.to_string();
    sup.store.insert_session(row, None).await.expect("the row");
    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(
            Some(Terminal::Host(placeholder.clone())),
            LastOutcome::Running,
        )),
    );
    placeholder
}

/// The terminal session `s1`'s entry holds.
async fn s1_terminal(sup: &Supervisor) -> Option<Terminal> {
    sup.sessions
        .lock()
        .await
        .get("s1")
        .and_then(|entry| entry.terminal.clone())
}

/// Wait until `host`'s runtime has ended (its status channel closes).
async fn wait_until_ended(host: &HostTerminal) {
    let mut status = host.subscribe();
    let ended = tokio::time::timeout(WAIT, async { while status.changed().await.is_ok() {} }).await;
    assert!(ended.is_ok(), "the runtime still runs: {:?}", host.status());
}

/// A re-adoption whose session's entry changed while it reached the host,
/// or during which an orderly stop began, installs nothing and stops the
/// runtime it made.
///
/// Why: the re-adoption fence. A retry connects to the host before it
/// swaps; a Restart, Delete or a newer handle that replaced the entry
/// meanwhile must win, and a runtime installed after the orderly stop
/// gathered its hosts would never be checkpointed, closed or have its
/// encodes cancelled, holding the process's exit (the review's finding on
/// re-adoption through a stop). Spec: with session `s1` holding a
/// placeholder for a stand-in host whose tag names its launch, a probe of
/// the placeholder adopts the host; with the entry then replaced by
/// another handle, installing the outcome is refused, the entry still holds
/// that other handle and the probe's runtime ends. With the placeholder put
/// back and probed again, an orderly stop that runs before the install
/// makes the install refuse as well: the entry keeps the placeholder, the
/// runtime ends, and a re-adoption pass then does nothing.
#[farhelm_testtrace::test]
async fn a_readoption_that_loses_its_fence_installs_nothing() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (test_host, name) = discoverable_host(&state, "s1", TestHostSpec::default());
    let placeholder = placeholder_session(&sup, &test_host, &name).await;
    // The session's claim keeps the supervisor's own re-adoption task away.
    let _claim = sup.lifecycle_locks.claim("s1").await;

    let outcome = sup.probe_placeholder("s1", 0, &placeholder).await;
    let RowHost::Adopted(adopted) = &outcome else {
        panic!("premise: the probe adopts the host");
    };
    let adopted = adopted.clone();
    let other =
        HostTerminal::unavailable(test_host.socket(), crate::hosts::Unavailability::Busy, None);
    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(
            Some(Terminal::Host(other.clone())),
            LastOutcome::Running,
        )),
    );
    assert!(
        !sup.install_readopted("s1", 0, &placeholder, outcome).await,
        "a changed entry refuses the swap"
    );
    assert_eq!(s1_terminal(&sup).await, Some(Terminal::Host(other)));
    wait_until_ended(&adopted).await;

    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(
            Some(Terminal::Host(placeholder.clone())),
            LastOutcome::Running,
        )),
    );
    let outcome = sup.probe_placeholder("s1", 0, &placeholder).await;
    let RowHost::Adopted(adopted) = &outcome else {
        panic!("premise: the probe adopts the host again");
    };
    let adopted = adopted.clone();
    sup.checkpoint_hosts_for_exit(Duration::from_secs(2)).await;
    assert!(
        !sup.install_readopted("s1", 0, &placeholder, outcome).await,
        "a stop that has begun refuses the swap"
    );
    assert_eq!(
        s1_terminal(&sup).await,
        Some(Terminal::Host(placeholder.clone()))
    );
    wait_until_ended(&adopted).await;
    let mut schedule = super::super::host_handover::ReadoptSchedule::default();
    assert_eq!(sup.readopt_pass(&mut schedule).await, None);
    end_discoverable(test_host);
}

/// Re-adoption backs off: a placeholder is first tried a second after it is
/// seen, every try schedules the next one later before it is made, and a
/// try that cannot be made (the session is busy) is not repeated before
/// then.
///
/// Why: the review found re-adoption spinning: a retry that produced no
/// outcome stayed due at once, so the task went round without pause,
/// rereading the store and taking locks for as long as the cause lasted.
/// Spec: with session `s1` holding a placeholder and its lifecycle claim
/// held by the test, a pass schedules it (no try) and says the next is
/// due in the future; a second pass at once makes no try; once that time
/// has passed, a pass makes one try (busy, so the entry keeps its
/// placeholder) and says the next is due later; a pass straight after
/// makes no further try.
#[farhelm_testtrace::test]
async fn readoption_backs_off_without_spinning() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (test_host, name) = discoverable_host(&state, "s1", TestHostSpec::default());
    let placeholder = placeholder_session(&sup, &test_host, &name).await;
    let _busy = sup.lifecycle_locks.claim("s1").await;
    let mut schedule = super::super::host_handover::ReadoptSchedule::default();

    let first = sup
        .readopt_pass(&mut schedule)
        .await
        .expect("the placeholder is scheduled");
    assert!(first > tokio::time::Instant::now(), "not due at once");
    assert_eq!(schedule.attempts("s1", 0), Some(0));
    assert_eq!(sup.readopt_pass(&mut schedule).await, Some(first));
    assert_eq!(
        schedule.attempts("s1", 0),
        Some(0),
        "no try before it is due"
    );

    // sleep-ok: waits out the first backoff, the boundary this test is about.
    tokio::time::sleep_until(first).await;
    let second = sup
        .readopt_pass(&mut schedule)
        .await
        .expect("still scheduled");
    assert_eq!(schedule.attempts("s1", 0), Some(1), "one try once due");
    assert!(
        second > tokio::time::Instant::now() + Duration::from_secs(1),
        "the next try is later than the first was"
    );
    assert_eq!(sup.readopt_pass(&mut schedule).await, Some(second));
    assert_eq!(schedule.attempts("s1", 0), Some(1), "no spin");
    assert_eq!(s1_terminal(&sup).await, Some(Terminal::Host(placeholder)));
    end_discoverable(test_host);
}

// ---- Recovery after a runtime ran ----------------------------------------------------

/// A failed reconnect must preserve the agent's or tab's root through Delete.
/// The unmarked child must be swept even though the unreadable stand-in's
/// peer is this test runner, whose identity deliberately refuses host teardown.
#[farhelm_testtrace::test]
async fn delete_roots_children_after_failed_agent_and_tab_reconnects() {
    for agent in [true, false] {
        let state = StateDir::new();
        let (sup, old, test_host) = if agent {
            let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
                .await
                .unwrap();
            let (host, test_host) = readoption_runtime(&sup, &state).await;
            (sup, host, test_host)
        } else {
            let (sup, test_host) = readoption_tab_fixture(&state).await;
            let host = readoption_tab(&sup).await.host;
            // The fixture's starter is its live child; Delete needs a finished starter.
            let record = crate::launch::tab_host_record_path(state.path(), "s1", "t1");
            std::fs::remove_file(record).unwrap();
            (sup, host, test_host)
        };
        record_s1(&state, old.socket());
        let child = test_host.child_pid();
        assert_eq!(old.status().child_pid, child);
        assert!(old.status().child_start.is_some());
        readoption_make_unstable(&old, &test_host).await;
        test_host.set_answering(false);
        let recovered = if agent {
            sup.readopt_one("s1", 0, &old).await
        } else {
            sup.readopt_tab_one("s1", "t1", &old).await
        };
        assert!(!recovered);
        assert!(if agent {
            matches!(s1_terminal(&sup).await, Some(Terminal::Host(host)) if host.is_placeholder())
        } else {
            readoption_tab(&sup).await.host.is_placeholder()
        });
        assert!(
            runs(child),
            "premise: failed recovery left the child running"
        );
        let message = refusal(
            delete_s1(&sup, farhelm_proto::DeleteGuard::Unconditional).await,
            "the stand-in peer cannot be ended",
        );
        assert!(
            message.contains("could not confirm that this session's terminal hosts ended"),
            "{message}"
        );
        assert!(!runs(child), "Delete lost the child root (agent={agent})");
        end_discoverable(test_host);
    }
}

/// Install an adopted agent at a socket the one-socket retry can discover.
/// An ordinary temporary socket would look absent to that scan and would
/// let a test pass through the wrong recovery path.
async fn readoption_runtime(sup: &Supervisor, state: &StateDir) -> (HostTerminal, TestHost) {
    let socket = socket_in_state(state);
    let (host, test_host) =
        adopt_test_host_at(&socket, agent_tag("s1"), Some(sup.host_liveness.clone())).await;
    sup.store
        .insert_session(host_row("s1", &socket), None)
        .await
        .expect("row");
    sup.sessions.lock().await.insert(
        "s1".into(),
        Arc::new(entry_with(
            Some(Terminal::Host(host.clone())),
            LastOutcome::Running,
        )),
    );
    assert!(matches!(
        host.liveness(),
        Liveness::Running { child: Some(_) }
    ));
    assert!(
        runs(test_host.child_pid()),
        "premise: the host's child runs"
    );
    (host, test_host)
}

/// End four distinct connections, waiting for each reconnect to finish.
/// Dropping twice before a reconnect lands would test fewer failures than
/// the reconnect limit requires; an accepted socket alone is not ready.
async fn readoption_make_unstable(host: &HostTerminal, test_host: &TestHost) {
    for reconnect in 1..=3 {
        test_host.drop_connection();
        let deadline = tokio::time::Instant::now() + WAIT;
        while host.reconnects() < reconnect {
            assert!(
                tokio::time::Instant::now() < deadline,
                "reconnect {reconnect} never landed: {:?}; connections {}",
                host.status(),
                test_host.connections()
            );
            // sleep-ok: the reconnect counter has no notification; poll until the new connection resumed.
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    test_host.drop_connection();
    wait_for(host, "the reconnect limit", |host| {
        matches!(
            host.liveness(),
            Liveness::Unavailable {
                reason: crate::hosts::Unavailability::Unstable,
                ..
            }
        )
    })
    .await;
    assert!(
        runs(test_host.child_pid()),
        "premise: unavailability did not kill the child"
    );
    assert!(
        !host.is_placeholder(),
        "premise: this runtime ran before it failed"
    );
}

/// A recovered runtime replaces the failed handle without resetting retry
/// history. Every later unstable episode still waits a full reconnect
/// window, even when the successful runtime failed before another pass.
/// This pins both the anti-fight policy and the absence of a healthy-key spin.
#[farhelm_testtrace::test]
async fn readoption_keeps_attempts_and_waits_out_each_unstable_episode() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (old, test_host) = readoption_runtime(&sup, &state).await;
    readoption_make_unstable(&old, &test_host).await;
    let mut schedule = super::super::host_handover::ReadoptSchedule::default();
    let began = tokio::time::Instant::now();
    let due = sup.readopt_pass(&mut schedule).await.expect("scheduled");
    assert!(due >= began + crate::hosts::terminal::RECONNECT_WINDOW);
    assert_eq!(schedule.attempts("s1", 0), Some(0));
    schedule.make_due("s1", 0);
    sup.readopt_pass(&mut schedule).await;
    let Some(Terminal::Host(recovered)) = s1_terminal(&sup).await else {
        panic!("recovered host")
    };
    assert_ne!(recovered, old);
    assert!(matches!(
        recovered.liveness(),
        Liveness::Running { child: Some(_) }
    ));
    assert_eq!(schedule.attempts("s1", 0), Some(1));
    assert!(
        runs(test_host.child_pid()),
        "re-adoption left the child running"
    );

    // No intervening healthy pass: the successful try itself must finish
    // the old episode, or this second loss would reuse its short due time.
    readoption_make_unstable(&recovered, &test_host).await;
    let began = tokio::time::Instant::now();
    let due = sup.readopt_pass(&mut schedule).await.expect("new episode");
    assert!(due >= began + crate::hosts::terminal::RECONNECT_WINDOW);
    assert_eq!(schedule.attempts("s1", 0), Some(1));
    schedule.make_due("s1", 0);
    sup.readopt_pass(&mut schedule).await;
    assert_eq!(schedule.attempts("s1", 0), Some(2));
    assert_eq!(
        sup.readopt_pass(&mut schedule).await,
        None,
        "healthy keys never keep the task spinning"
    );
    assert_eq!(
        schedule.attempts("s1", 0),
        Some(2),
        "success keeps the count"
    );
    end_discoverable(test_host);
}

/// A timeout after an established connection wakes recovery through the
/// ticker's liveness branch, without a reload or the ordinary tick. The
/// same child remains usable through a real grid attachment afterwards.
#[farhelm_testtrace::test]
async fn readoption_recovers_a_midlife_timeout_on_the_liveness_wake() {
    let state = StateDir::new();
    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            ticker_interval: Duration::from_secs(600),
            readoption_passes: Some(Arc::clone(&passes)),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let (old, test_host) = readoption_runtime(&sup, &state).await;
    // The constructor's pass must have finished before the failure: an
    // overdue initial pass could otherwise masquerade as the notifier wake.
    let deadline = tokio::time::Instant::now() + WAIT;
    while passes.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "initial readoption pass never parked; host {:?}",
            old.status()
        );
        // sleep-ok: the test-only pass counter is the retry task's parked-readiness oracle.
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(matches!(
        old.liveness(),
        Liveness::Running { child: Some(_) }
    ));
    let _ticker = start_ticker(&sup);
    test_host.set_answering(false);
    test_host.drop_connection();
    wait_for(&old, "a timed-out reconnect", |host| {
        matches!(
            host.liveness(),
            Liveness::Unavailable {
                reason: crate::hosts::Unavailability::Connect(crate::hosts::Unavailable::Timeout),
                ..
            }
        )
    })
    .await;
    assert!(
        runs(test_host.child_pid()),
        "premise: the child still runs at the timeout"
    );
    assert!(
        test_host.connections() >= 2,
        "premise: the reconnect reached the silent host"
    );
    test_host.set_answering(true);
    let deadline = tokio::time::Instant::now() + WAIT;
    let recovered = loop {
        if let Some(Terminal::Host(host)) = s1_terminal(&sup).await
            && host != old
            && matches!(host.liveness(), Liveness::Running { child: Some(_) })
        {
            break host;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no recovery before the tick; old {:?}, child alive {}, connections {}",
            old.status(),
            runs(test_host.child_pid()),
            test_host.connections()
        );
        // sleep-ok: polls the session's handle replacement, for which there is no test subscription.
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    let entry = sup.sessions.lock().await.get("s1").unwrap().clone();
    assert!(
        session_status(&entry, &HashMap::new(), &Default::default())
            .0
            .is_live()
    );
    let mut viewer = GridPeer::connect(&sup).await;
    assert!(matches!(
        viewer.attach(1, 1, "recovered-viewer", true).await,
        ControlMsg::Attached { .. }
    ));
    viewer
        .until("the recovered catch-up", |peer| peer.replayed.contains(&1))
        .await;
    viewer
        .writer
        .write_frame(&Frame::data(
            1,
            br#"{"t":"text","data":"recovered input"}"#.to_vec(),
        ))
        .await
        .expect("input");
    assert!(
        test_host.received_becomes(b"recovered input", WAIT).await,
        "recovered attachment delivers input"
    );
    assert!(recovered.status().caught_up);
    drop(viewer);
    end_discoverable(test_host);
}

/// An unsupported host stays final even though ordinary connection losses
/// now recover. A retry pass must neither connect nor replace its handle.
#[farhelm_testtrace::test]
async fn readoption_leaves_a_nonpassing_midlife_reason_alone() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (old, test_host) = readoption_runtime(&sup, &state).await;
    test_host.become_unreadable();
    wait_for(&old, "the permanent refusal", |host| {
        matches!(
            host.liveness(),
            Liveness::Unavailable {
                reason: crate::hosts::Unavailability::Connect(
                    crate::hosts::Unavailable::UnsupportedVersion(_)
                ),
                ..
            }
        )
    })
    .await;
    let connections = test_host.connections();
    let mut schedule = super::super::host_handover::ReadoptSchedule::default();
    assert_eq!(sup.readopt_pass(&mut schedule).await, None);
    assert_eq!(s1_terminal(&sup).await, Some(Terminal::Host(old)));
    assert_eq!(test_host.connections(), connections);
    assert!(
        runs(test_host.child_pid()),
        "the final refusal left the child alone"
    );
    end_discoverable(test_host);
}

/// A retry of a runtime that already reported exit rebuilds its final
/// evidence without recording another exit. Lost connections must not turn
/// an exited session back into a live one or duplicate its transition.
#[farhelm_testtrace::test]
async fn readoption_replays_an_exited_runtime_without_recording_twice() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (old, test_host) = readoption_runtime(&sup, &state).await;
    test_host.exit(7);
    wait_for(&old, "the first exit", |host| {
        matches!(host.liveness(), Liveness::Exited { .. })
    })
    .await;
    sup.reconcile_for_test().await;
    let recorded = sup.store.session("s1").await.unwrap().unwrap().outcome;
    assert!(matches!(
        recorded,
        LastOutcome::Exited {
            exit_code: Some(7),
            ..
        }
    ));
    readoption_make_unstable(&old, &test_host).await;
    assert!(sup.readopt_one("s1", 0, &old).await);
    let Some(Terminal::Host(recovered)) = s1_terminal(&sup).await else {
        panic!("recovered host")
    };
    // The stand-in streams its durable exit record on this cue, as a real
    // host's Read would. Info alone withdraws the root, never records exit.
    test_host.exit(7);
    wait_for(&recovered, "the replayed exit", |host| {
        matches!(host.liveness(), Liveness::Exited { .. })
    })
    .await;
    let entry = sup.sessions.lock().await.get("s1").unwrap().clone();
    let observed =
        crate::service::status::observe_entry(&sup, &entry, &HashMap::new(), &Default::default())
            .await
            .expect("observe");
    assert_eq!(observed.transition, None);
    assert_eq!(
        sup.store.session("s1").await.unwrap().unwrap().outcome,
        recorded
    );
    end_discoverable(test_host);
}

/// A runtime that failed after startup obeys the same final stop fence as
/// a placeholder. A retry that already adopted a replacement must abort it
/// if orderly shutdown gathered the old handle before installation.
#[farhelm_testtrace::test]
async fn readoption_of_a_midlife_runtime_cannot_install_after_a_stop() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let (old, test_host) = readoption_runtime(&sup, &state).await;
    readoption_make_unstable(&old, &test_host).await;
    let outcome = sup.probe_placeholder("s1", 0, &old).await;
    let RowHost::Adopted(adopted) = &outcome else {
        panic!("premise: the retry adopted a runtime")
    };
    let adopted = adopted.clone();
    sup.checkpoint_hosts_for_exit(Duration::from_secs(2)).await;
    assert!(!sup.install_readopted("s1", 0, &old, outcome).await);
    assert_eq!(s1_terminal(&sup).await, Some(Terminal::Host(old)));
    wait_until_ended(&adopted).await;
    assert!(
        runs(test_host.child_pid()),
        "refusing an install never stops the host's child"
    );
    end_discoverable(test_host);
}

// ---- Recovering terminal tabs --------------------------------------------------------

/// A tab with a discoverable host and a durable launch record. The
/// constructor's initial retry pass has parked while the tab is healthy,
/// so later manual passes cannot race a startup retry of the same handle.
async fn readoption_tab_fixture(state: &StateDir) -> (Arc<Supervisor>, TestHost) {
    use super::super::host_tabs::HostTab;
    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            readoption_passes: Some(Arc::clone(&passes)),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let socket = socket_in_state(state);
    let tag = crate::hosts::HostTag {
        session_id: "s1".into(),
        created_ns: 42,
        role: crate::hosts::HostRole::Tab {
            tab_id: "t1".into(),
            scoped: true,
        },
    }
    .encode()
    .expect("tab tag");
    let (host, test_host) = adopt_test_host_at(&socket, tag, None).await;
    let mut row = host_row("s1", &socket);
    row.pane.clear(); // No agent host: this fixture isolates tab attribution.
    sup.store.insert_session(row, None).await.expect("row");
    let entry = Arc::new(entry_with(None, LastOutcome::Running));
    entry.session.host_tabs.lock().unwrap().push(HostTab {
        tab_id: "t1".into(),
        created_ns: 42,
        scoped: true,
        host,
        end_failed: false,
        failed_ends: 0,
        retry_at: None,
    });
    sup.sessions.lock().await.insert("s1".into(), entry);
    let starter = ProcessIdentity::read(test_host.child_pid()).expect("owned child's identity");
    let record = super::super::host_launch::LaunchHostRecord {
        socket: crate::hosts::HostName::parse(socket.file_name().unwrap().to_str().unwrap())
            .expect("host name"),
        starter: crate::hosts::HostProcess {
            pid: starter.pid,
            start: starter.start,
        },
    };
    let path = crate::launch::tab_host_record_path(state.path(), "s1", "t1");
    std::fs::create_dir_all(path.parent().unwrap()).expect("launch directory");
    std::fs::write(path, record.encode()).expect("tab record");
    let deadline = tokio::time::Instant::now() + WAIT;
    while passes.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "initial retry pass never completed"
        );
        // sleep-ok: the pass counter establishes retry-task readiness before the test changes the tab.
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(runs(test_host.child_pid()), "premise: the tab's child runs");
    (sup, test_host)
}

/// Read the tab from its current session cell rather than a stale entry:
/// successful recovery must replace the cell's handle, not only make a
/// usable runtime elsewhere.
async fn readoption_tab(sup: &Supervisor) -> super::super::host_tabs::HostTab {
    let entry = sup
        .sessions
        .lock()
        .await
        .get("s1")
        .expect("session")
        .clone();
    super::super::host_tabs::find_host_tab(&entry.session.host_tabs, "t1").expect("tab")
}

/// A tab runtime recovers in place, keeping its identity, order, scope and
/// retry history. Restarting the agent must not reset that tab's backoff:
/// tabs belong to the session, rather than its agent's launch generation.
#[farhelm_testtrace::test]
async fn readoption_recovers_a_tab_without_resetting_its_metadata_or_count() {
    let state = StateDir::new();
    let (sup, test_host) = readoption_tab_fixture(&state).await;
    let old = readoption_tab(&sup).await;
    readoption_make_unstable(&old.host, &test_host).await;
    let mut schedule = super::super::host_handover::ReadoptSchedule::default();
    let now = tokio::time::Instant::now();
    assert!(
        sup.readopt_pass(&mut schedule).await.unwrap()
            >= now + crate::hosts::terminal::RECONNECT_WINDOW
    );
    schedule.make_tab_due("s1", "t1");
    sup.readopt_pass(&mut schedule).await;
    let recovered = readoption_tab(&sup).await;
    assert_ne!(recovered.host, old.host);
    assert!(matches!(
        recovered.host.liveness(),
        Liveness::Running { child: Some(_) }
    ));
    assert_eq!(
        (
            recovered.tab_id.as_str(),
            recovered.created_ns,
            recovered.scoped
        ),
        ("t1", 42, true)
    );
    assert_eq!(schedule.tab_attempts("s1", "t1"), Some(1));
    assert_eq!(sup.readopt_pass(&mut schedule).await, None);

    let current = sup.sessions.lock().await.get("s1").unwrap().clone();
    // Agent Restart temporarily hides the session entry but keeps its tab
    // cell in restarting_tabs. That phase must not erase the tab's history.
    sup.restarting_tabs
        .lock()
        .unwrap()
        .insert("s1".into(), Arc::clone(&current.session.host_tabs));
    sup.sessions.lock().await.remove("s1");
    assert_eq!(sup.readopt_pass(&mut schedule).await, None);
    assert_eq!(schedule.tab_attempts("s1", "t1"), Some(1));
    sup.sessions.lock().await.insert("s1".into(), current);
    sup.restarting_tabs.lock().unwrap().remove("s1");
    assert!(
        runs(test_host.child_pid()),
        "recovery left the same tab program running"
    );
    end_discoverable(test_host);
}

/// Startup placeholders learn ordering and scope from the recovered tag;
/// an ordinary runtime's failed retry must instead keep the metadata it
/// already knew. A permanent refusal stops the latter's retries.
#[farhelm_testtrace::test]
async fn readoption_learns_a_tab_placeholders_tag_and_keeps_known_metadata_on_failure() {
    let state = StateDir::new();
    let (sup, test_host) = readoption_tab_fixture(&state).await;
    let old = readoption_tab(&sup).await;
    old.host.abort();
    wait_until_ended(&old.host).await;
    let placeholder =
        HostTerminal::unavailable(test_host.socket(), crate::hosts::Unavailability::Busy, None);
    let entry = sup.sessions.lock().await.get("s1").unwrap().clone();
    {
        let mut tabs = entry.session.host_tabs.lock().unwrap();
        tabs[0].host = placeholder.clone();
        tabs[0].created_ns = u64::MAX;
        tabs[0].scoped = false;
    }
    assert!(sup.readopt_tab_one("s1", "t1", &placeholder).await);
    let recovered = readoption_tab(&sup).await;
    assert_eq!((recovered.created_ns, recovered.scoped), (42, true));
    readoption_make_unstable(&recovered.host, &test_host).await;
    test_host.become_unreadable();
    assert!(!sup.readopt_tab_one("s1", "t1", &recovered.host).await);
    let failed = readoption_tab(&sup).await;
    assert!(failed.host.is_placeholder());
    assert!(!failed.host.liveness().may_readopt());
    assert_eq!((failed.created_ns, failed.scoped), (42, true));
    let connections = test_host.connections();
    let mut schedule = super::super::host_handover::ReadoptSchedule::default();
    assert_eq!(sup.readopt_pass(&mut schedule).await, None);
    assert_eq!(test_host.connections(), connections);
    end_discoverable(test_host);
}

/// A failed close belongs to the reap, even if its frozen status would
/// otherwise qualify for recovery. Installing a new runtime would undo
/// the close's deliberate abort and restart a fight with shutdown.
#[farhelm_testtrace::test]
async fn readoption_leaves_an_end_failed_tab_to_the_reap() {
    let state = StateDir::new();
    let (sup, test_host) = readoption_tab_fixture(&state).await;
    let old = readoption_tab(&sup).await;
    readoption_make_unstable(&old.host, &test_host).await;
    let entry = sup.sessions.lock().await.get("s1").unwrap().clone();
    entry.session.host_tabs.lock().unwrap()[0].end_failed = true;
    let connections = test_host.connections();
    let mut schedule = super::super::host_handover::ReadoptSchedule::default();
    assert_eq!(sup.readopt_pass(&mut schedule).await, None);
    assert!(!sup.readopt_tab_one("s1", "t1", &old.host).await);
    assert_eq!(readoption_tab(&sup).await.host, old.host);
    assert_eq!(test_host.connections(), connections);
    end_discoverable(test_host);
}

/// A vanished placeholder becomes a finished tab that the normal close
/// can remove with its record. Merely forgetting it during the retry
/// would bypass that cleanup and leave the launch record behind.
#[farhelm_testtrace::test]
async fn readoption_of_a_gone_tab_leaves_its_record_for_the_reaps_close() {
    let state = StateDir::new();
    let (sup, test_host) = readoption_tab_fixture(&state).await;
    let old = readoption_tab(&sup).await;
    old.host.abort();
    wait_until_ended(&old.host).await;
    let placeholder =
        HostTerminal::unavailable(test_host.socket(), crate::hosts::Unavailability::Busy, None);
    let entry = sup.sessions.lock().await.get("s1").unwrap().clone();
    {
        let mut tabs = entry.session.host_tabs.lock().unwrap();
        tabs[0].host = placeholder.clone();
        tabs[0].created_ns = u64::MAX;
        tabs[0].scoped = false;
    }
    end_discoverable(test_host);
    let record = crate::launch::tab_host_record_path(state.path(), "s1", "t1");
    assert!(
        record.exists(),
        "premise: the tab still has its launch record"
    );
    assert!(!sup.readopt_tab_one("s1", "t1", &placeholder).await);
    let gone = readoption_tab(&sup).await;
    assert_eq!(gone.host.liveness(), Liveness::Gone);
    assert!(gone.reap_due());
    assert!(
        record.exists(),
        "re-adoption leaves record removal to close"
    );
    let lifecycle = sup.lifecycle_locks.claim("s1").await;
    sup.close_host_tab_claimed(lifecycle, &entry, "t1", gone.host, true)
        .await
        .expect("reap closes the gone tab");
    assert!(entry.session.host_tabs.lock().unwrap().is_empty());
    assert!(!record.exists(), "close removed the record");
}

/// Gone evidence from a retry must keep the process pinned by the old
/// runtime: removing a socket alone cannot authorize forgetting a host
/// whose process still exists. The stand-in's peer is this test process,
/// so this test checks the identity without trying to end that peer.
#[farhelm_testtrace::test]
async fn readoption_of_a_gone_tab_keeps_its_adopted_process() {
    let state = StateDir::new();
    let (sup, test_host) = readoption_tab_fixture(&state).await;
    let old = readoption_tab(&sup).await;
    readoption_make_unstable(&old.host, &test_host).await;
    let adopted = old
        .host
        .adopted_process()
        .expect("premise: runtime pinned its host");
    assert_eq!(adopted.pid, std::process::id());
    end_discoverable(test_host);
    assert!(!sup.readopt_tab_one("s1", "t1", &old.host).await);
    let gone = readoption_tab(&sup).await;
    assert_eq!(gone.host.liveness(), Liveness::Gone);
    assert_eq!(gone.host.adopted_process(), Some(adopted));
}

/// A tab probe cannot resurrect a cell replaced by another session entry,
/// or install after orderly stop. Both refusals must stop the runtime the
/// probe made, while leaving the tab's host and program alive.
#[farhelm_testtrace::test]
async fn readoption_of_a_tab_obeys_the_cell_and_stop_fences() {
    let state = StateDir::new();
    let (sup, test_host) = readoption_tab_fixture(&state).await;
    let old = readoption_tab(&sup).await;
    readoption_make_unstable(&old.host, &test_host).await;
    let entry = sup.sessions.lock().await.get("s1").unwrap().clone();
    let made = sup.probe_readopted_tab("s1", &old).await;
    assert!(matches!(made.host.liveness(), Liveness::Running { .. }));
    let made_host = made.host.clone();
    let replacement = Arc::new(entry_with(None, LastOutcome::Running));
    sup.sessions
        .lock()
        .await
        .insert("s1".into(), Arc::clone(&replacement));
    assert!(
        !sup.install_readopted_tab("s1", &entry.session.host_tabs, &old.host, made)
            .await
    );
    assert!(replacement.session.host_tabs.lock().unwrap().is_empty());
    wait_until_ended(&made_host).await;
    sup.sessions
        .lock()
        .await
        .insert("s1".into(), Arc::clone(&entry));
    let made = sup.probe_readopted_tab("s1", &old).await;
    let made_host = made.host.clone();
    sup.checkpoint_hosts_for_exit(Duration::from_secs(2)).await;
    assert!(
        !sup.install_readopted_tab("s1", &entry.session.host_tabs, &old.host, made)
            .await
    );
    assert_eq!(readoption_tab(&sup).await.host, old.host);
    wait_until_ended(&made_host).await;
    assert!(
        runs(test_host.child_pid()),
        "fence refusals leave the tab's child alone"
    );
    end_discoverable(test_host);
}

/// Restart republication must wake a retry task that already observed the
/// session off-map. No new liveness event follows publication, and the
/// task has no due retry while it cannot act on that session's tabs.
#[farhelm_testtrace::test]
async fn readoption_republication_wakes_a_tab_after_the_restart_window() {
    let state = StateDir::new();
    let (sup, test_host) = readoption_tab_fixture(&state).await;
    let old = readoption_tab(&sup).await;
    old.host.abort();
    wait_until_ended(&old.host).await;
    let placeholder =
        HostTerminal::unavailable(test_host.socket(), crate::hosts::Unavailability::Busy, None);
    let entry = sup.sessions.lock().await.get("s1").unwrap().clone();
    entry.session.host_tabs.lock().unwrap()[0].host = placeholder.clone();
    {
        let mut sessions = sup.sessions.lock().await;
        sup.restarting_tabs
            .lock()
            .unwrap()
            .insert("s1".into(), Arc::clone(&entry.session.host_tabs));
        sessions.remove("s1");
    }
    let passes = sup
        .seams
        .readoption_passes
        .as_ref()
        .expect("fixture pass counter");
    let before = passes.load(std::sync::atomic::Ordering::SeqCst);
    sup.readoption_wake.notify_one();
    let deadline = tokio::time::Instant::now() + WAIT;
    while passes.load(std::sync::atomic::Ordering::SeqCst) == before {
        assert!(
            tokio::time::Instant::now() < deadline,
            "retry task never observed the off-map window"
        );
        // sleep-ok: verifies the background retry pass finished while the session was hidden.
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(
        !sup.sessions.lock().await.contains_key("s1"),
        "premise: pass completed with the session hidden"
    );
    {
        let mut sessions = sup.sessions.lock().await;
        sessions.insert("s1".into(), entry.clone());
        sup.restarting_tabs.lock().unwrap().remove("s1");
        // Both successful and failed Restart publications use this fence
        // while holding the map. Its wake is the only event after the
        // retry task was shown to have parked above.
        sup.fence_republished_tabs(&entry.session.host_tabs);
    }
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let tab = readoption_tab(&sup).await;
        if tab.host != placeholder
            && matches!(tab.host.liveness(), Liveness::Running { child: Some(_) })
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no recovery after republication; status {:?}, child alive {}",
            tab.host.status(),
            runs(test_host.child_pid())
        );
        // sleep-ok: the session cell's replaced tab handle is the recovery oracle.
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        runs(test_host.child_pid()),
        "recovery preserved the tab program"
    );
    end_discoverable(test_host);
}

/// Failed retry placeholders must keep the previously adopted process
/// until a later gone outcome's cleanup can confirm it ended. Otherwise
/// a transient timeout followed by a missing socket can leak that host.
#[farhelm_testtrace::test]
async fn readoption_keeps_a_tabs_adopted_process_through_a_failed_retry_then_gone() {
    let state = StateDir::new();
    let (sup, test_host) = readoption_tab_fixture(&state).await;
    let old = readoption_tab(&sup).await;
    readoption_make_unstable(&old.host, &test_host).await;
    let adopted = old
        .host
        .adopted_process()
        .expect("premise: the running tab pinned its host");
    test_host.set_answering(false);
    assert!(!sup.readopt_tab_one("s1", "t1", &old.host).await);
    let failed = readoption_tab(&sup).await;
    assert!(
        matches!(
            failed.host.liveness(),
            Liveness::Unavailable {
                reason: crate::hosts::Unavailability::Connect(crate::hosts::Unavailable::Timeout),
                ..
            }
        ),
        "premise: a passing retry failure, got {:?}",
        failed.host.liveness()
    );
    assert!(failed.host.is_placeholder());
    assert_eq!(failed.host.adopted_process(), Some(adopted));
    end_discoverable(test_host);
    assert!(!sup.readopt_tab_one("s1", "t1", &failed.host).await);
    let gone = readoption_tab(&sup).await;
    assert_eq!(gone.host.liveness(), Liveness::Gone);
    assert_eq!(gone.host.adopted_process(), Some(adopted));
    // The peer is this test process, still alive: do not ask close to end
    // it. Keeping its identity is what makes close refuse to forget it.
    assert_eq!(adopted.pid, std::process::id());
}

/// Adoption can succeed and its runtime fail before the probe installs it.
/// That result is still the new runtime, with its own pinned host process,
/// rather than a failed-adoption placeholder inheriting the old peer.
/// Exercising the post-probe policy directly makes this race deterministic.
#[farhelm_testtrace::test]
async fn readoption_keeps_a_new_runtime_that_became_unavailable_before_install() {
    let state = StateDir::new();
    let (sup, test_host) = readoption_tab_fixture(&state).await;
    let adopted = readoption_tab(&sup).await;
    let peer = adopted
        .host
        .adopted_process()
        .expect("the new runtime pinned its host");
    readoption_make_unstable(&adopted.host, &test_host).await;
    let old = super::super::host_tabs::HostTab {
        host: HostTerminal::unavailable(
            test_host.socket(),
            crate::hosts::Unavailability::Busy,
            None,
        ),
        created_ns: u64::MAX,
        scoped: false,
        ..adopted.clone()
    };
    assert_eq!(
        old.host.adopted_process(),
        None,
        "premise: old startup placeholder pinned no process"
    );
    let result = adopted.clone().after_retry(&old);
    assert_eq!(
        result.host, adopted.host,
        "keep the exact successfully adopted runtime"
    );
    assert!(!result.host.is_placeholder());
    assert_eq!(result.host.adopted_process(), Some(peer));
    assert_eq!((result.created_ns, result.scoped), (42, true));
    end_discoverable(test_host);
}

/// A host runtime that nothing stops explicitly still cancels its encode
/// when the tokio runtime that owns it drops, so that drop finishes at once.
///
/// Why: a runtime no stop reaches (one a re-adoption made and had not yet
/// installed when the process stopped, or every runtime when the stop could
/// not take the session map's lock in time) is ended only by the process's
/// tokio runtime dropping its task, and that drop waits for the blocking
/// pool: an encode the runtime started would otherwise hold the exit, and
/// the state directory's claim, for as long as it takes. Spec: on a runtime
/// the test owns, a stand-in host's runtime whose every encode is held at a
/// closed gate, with a checkpoint due after any output, starts an encode
/// once it is sent output (premise: the encode reached the gate); with the
/// terminal's handle still held and no abort, dropping the tokio runtime
/// takes under 2 s (the gate's own limit is 30 s), and the encode left the
/// gate by its cancel.
#[farhelm_testtrace::test]
fn a_runtime_dropped_with_its_tokio_runtime_cancels_its_encode() {
    let gate = crate::hosts::terminal::EncodeGate::closed();
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let (host, test_host) = runtime.block_on(async {
        let (host, test_host) = adopt_test_host_with(
            TestHostSpec {
                options: Some(crate::hosts::HostTerminalOptions {
                    encode_gate: Some(gate.clone()),
                    checkpoints: crate::hosts::CheckpointPolicy {
                        min_bytes: 1,
                        max_bytes: 1,
                        stop_min_bytes: 0,
                    },
                    ..test_host_options(None)
                }),
                ..TestHostSpec::default()
            },
            crate::hosts::Adoption::Fresh,
        )
        .await;
        test_host.output(b"output for a checkpoint\r\n");
        let held = gate.clone();
        let entered = tokio::task::spawn_blocking(move || held.wait_entered(WAIT))
            .await
            .expect("the wait");
        assert!(entered, "premise: an encode reached the gate");
        (host, test_host)
    });
    let dropping = std::time::Instant::now();
    drop(runtime);
    assert!(
        dropping.elapsed() < Duration::from_secs(2),
        "the encode held the runtime's drop for {:?}",
        dropping.elapsed()
    );
    assert!(gate.cancelled(), "the encode left the gate by its cancel");
    assert!(!gate.expired());
    drop(host);
    drop(test_host);
}

/// Delete of a session whose agent terminal is a startup placeholder settles
/// the launch record naming its socket, as for a session with no runtime,
/// rather than taking the placeholder for a runtime that holds the host.
///
/// Why: a placeholder (a host a restarted supervisor could not adopt) holds
/// no connection and adopted no process. Delete leaves the record of a
/// socket a runtime holds to that runtime; doing so for a placeholder would
/// skip the record's settling, whose first-stage fence keeps a recorded
/// start from binding the socket after Delete looked, and would reach the
/// host over a new connection instead. Spec: with `s1`'s entry holding a
/// placeholder for a socket served by a stand-in tagged as `s1`'s agent, and
/// a launch record naming that socket whose recorded first stage is a live
/// process that is not a host (a `sleep`, which the verified signal never
/// signals), an unconditional Delete is refused for the recorded start
/// (naming its process) without having connected to the stand-in at all;
/// the recorded process still runs and the row is kept.
#[farhelm_testtrace::test]
async fn delete_settles_the_record_of_a_startup_placeholders_socket() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let socket = socket_in_state(&state);
    let test_host = start_test_host_at(&socket, agent_tag("s1"), false);
    let placeholder = HostTerminal::unavailable(
        &socket,
        crate::hosts::Unavailability::Connect(crate::hosts::Unavailable::Timeout),
        None,
    );
    sup.store
        .insert_session(host_row("s1", &socket), None)
        .await
        .expect("insert the session's row");
    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(
            Some(Terminal::Host(placeholder)),
            LastOutcome::Running,
        )),
    );
    let starter = OwnedChild(
        std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("start a stand-in first stage"),
    );
    let identity =
        ProcessIdentity::read(starter.0.id()).expect("the stand-in first stage's identity");
    assert!(
        runs(identity.pid),
        "premise: the stand-in first stage runs before Delete"
    );
    let record = super::super::host_launch::LaunchHostRecord {
        socket: crate::hosts::HostName::parse(socket.file_name().unwrap().to_str().unwrap())
            .expect("a socket name this supervisor makes"),
        starter: crate::hosts::HostProcess {
            pid: identity.pid,
            start: identity.start,
        },
    };
    let artifacts = crate::launch::LaunchArtifacts::for_launch(state.path(), "s1", 0);
    std::fs::create_dir_all(artifacts.host().parent().unwrap()).expect("launch dir");
    std::fs::write(artifacts.host(), record.encode()).expect("write the launch record");

    let refused = refusal(
        delete_s1(&sup, farhelm_proto::DeleteGuard::Unconditional).await,
        "a recorded start that cannot be fenced keeps the session",
    );
    assert!(
        refused.contains(&format!("process {}", identity.pid)),
        "refused for the recorded start: {refused}"
    );
    assert_eq!(
        test_host.connections(),
        0,
        "the placeholder's host was reached only through its record"
    );
    assert!(runs(identity.pid), "the recorded process was not signalled");
    assert!(
        sup.store.session("s1").await.expect("read").is_some(),
        "the row is kept"
    );
    drop(starter);
    drop(test_host);
    let _ = std::fs::remove_file(&socket);
}

// ---- Attaching a host terminal as a grid --------------------------------------------

/// A viewer connected to the supervisor the way the helm is, over the
/// supervisor's own connection handler, applying each grid channel's
/// messages to a test client of its own.
struct GridPeer {
    reader: farhelm_proto::io::FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
    writer: farhelm_proto::io::FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
    server: tokio::task::JoinHandle<anyhow::Result<()>>,
    /// Each grid channel's client.
    clients: HashMap<u32, crate::grid_output::test_client::TestClient>,
    /// Channels whose `ReplayComplete` arrived.
    replayed: std::collections::HashSet<u32>,
    /// Every `Detached` that arrived, by channel.
    detached: HashMap<u32, farhelm_proto::DetachCode>,
    /// Replies to requests, by request id.
    replies: HashMap<u64, ControlMsg>,
}

impl Drop for GridPeer {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl GridPeer {
    /// Connect to `sup` and complete the handshake.
    async fn connect(sup: &Arc<Supervisor>) -> GridPeer {
        let (client_side, server_side) = tokio::io::duplex(1 << 20);
        let server = tokio::spawn(crate::service::connection::handle_connection(
            Arc::clone(sup),
            server_side,
        ));
        let (read_half, write_half) = tokio::io::split(client_side);
        let mut peer = GridPeer {
            reader: farhelm_proto::io::FrameReader::new(read_half),
            writer: farhelm_proto::io::FrameWriter::new(write_half),
            server,
            clients: HashMap::new(),
            replayed: Default::default(),
            detached: HashMap::new(),
            replies: HashMap::new(),
        };
        tokio::time::timeout(
            WAIT,
            farhelm_proto::io::handshake(&mut peer.reader, &mut peer.writer, "helm"),
        )
        .await
        .expect("the supervisor answers the handshake")
        .expect("handshake");
        peer
    }

    async fn send(&mut self, message: &ControlMsg) {
        self.writer
            .write_control(message)
            .await
            .expect("write a control frame");
    }

    /// Attach `s1`'s agent on `channel` under `lease`, as a client that can
    /// (`grid`) or cannot show grids, and return the reply.
    async fn attach(&mut self, req_id: u64, channel: u32, lease: &str, grid: bool) -> ControlMsg {
        self.send(&ControlMsg::Attach {
            req_id,
            session_id: "s1".to_string(),
            channel,
            cols: 80,
            rows: 24,
            terminal: TerminalSelector::default(),
            lease: lease.to_string(),
            if_unowned: false,
            grid,
        })
        .await;
        self.until("the attach's reply", |peer| {
            peer.replies.contains_key(&req_id)
        })
        .await;
        let reply = self.replies.remove(&req_id).expect("the reply");
        if matches!(reply, ControlMsg::Attached { .. }) {
            self.clients
                .insert(channel, crate::grid_output::test_client::TestClient::new());
        }
        reply
    }

    /// Read and file one frame.
    async fn read_one(&mut self) {
        let frame = self
            .reader
            .read_frame()
            .await
            .expect("read a frame")
            .expect("the connection stays open");
        match frame.kind {
            FrameKind::Data => {
                self.clients
                    .get_mut(&frame.channel)
                    .expect("data only on an attached channel")
                    .apply(&frame.body);
            }
            FrameKind::Control => match parse_control(&frame).expect("a control frame") {
                ControlMsg::ReplayComplete { channel } => {
                    self.replayed.insert(channel);
                }
                ControlMsg::Detached { channel, code, .. } => {
                    self.detached.insert(channel, code);
                }
                ControlMsg::SessionsChanged => {}
                reply => {
                    let req_id = reply.reply_req_id().expect("a reply to a request");
                    self.replies.insert(req_id, reply);
                }
            },
        }
    }

    /// Read frames until `done` holds; fails after [`WAIT`].
    async fn until(&mut self, what: &str, done: impl Fn(&GridPeer) -> bool) {
        let reached = tokio::time::timeout(WAIT, async {
            while !done(self) {
                self.read_one().await;
            }
        })
        .await;
        assert!(
            reached.is_ok(),
            "{what} did not happen; replayed {:?}, detached {:?}",
            self.replayed,
            self.detached
        );
    }
}

/// A supervisor whose session `s1` has a fresh test host as its agent
/// terminal, and that host's handles.
async fn grid_fixture(
    timeouts: SupervisorTimeouts,
) -> (StateDir, Arc<Supervisor>, HostTerminal, TestHost) {
    let state = StateDir::new();
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        timeouts,
        SupervisorSeams::default(),
    )
    .await
    .expect("supervisor");
    let (entry, host, test_host) = host_entry().await;
    sup.sessions.lock().await.insert("s1".to_string(), entry);
    (state, sup, host, test_host)
}

/// A client that can show grids attaches a host agent as a grid stream,
/// sees its screen, and types into it; one that cannot is refused.
///
/// Why: this is the path a viewer of a host terminal takes end to end
/// through the supervisor's own connection handling: the attach's stream
/// kind, the attach sequence ending in `ReplayComplete`, and input routed
/// through the attachment's grid input side. Spec: a bytes-only attach is
/// refused as an invalid request with the permanent "needs a client that
/// can show grid terminals" message; a grid attach is answered `Attached`
/// with stream kind `grid`, then the client's screen shows `hello from the
/// host` and `ReplayComplete` arrives; a `text` input of `typed` then
/// reaches the host's child.
#[farhelm_testtrace::test]
async fn a_grid_client_attaches_a_host_agent_and_types_into_it() {
    let (_state, sup, host, test_host) = grid_fixture(SupervisorTimeouts::default()).await;
    test_host.output(b"hello from the host\r\n");
    let mut peer = GridPeer::connect(&sup).await;

    let refused = peer.attach(1, 1, "viewer", false).await;
    let ControlMsg::Error { kind, message, .. } = refused else {
        panic!("a bytes-only attach is refused, got {refused:?}");
    };
    assert_eq!(kind, ErrorKind::InvalidRequest);
    assert_eq!(message, crate::service::handlers::ATTACH_NEEDS_GRID_CLIENT);

    let attached = peer.attach(2, 2, "viewer", true).await;
    assert!(
        matches!(
            attached,
            ControlMsg::Attached {
                channel: 2,
                stream: farhelm_proto::StreamKind::Grid,
                ..
            }
        ),
        "{attached:?}"
    );
    peer.until("the screen and the end of the catch-up", |peer| {
        peer.replayed.contains(&2)
            && peer.clients[&2].screen.first().is_some_and(|row| {
                crate::grid_output::test_client::row_text(row) == "hello from the host"
            })
    })
    .await;

    peer.writer
        .write_frame(&Frame::data(2, br#"{"t":"text","data":"typed"}"#.to_vec()))
        .await
        .expect("write input");
    assert!(
        test_host.received_becomes(b"typed", WAIT).await,
        "the host received {:?}",
        test_host.received()
    );
    assert!(host.status().caught_up);
}

/// An attach to a startup placeholder is refused as unavailable before
/// anything is taken over, so the session's current viewer keeps its
/// attachment, and the same attach works once a runtime replaces the
/// placeholder.
///
/// Why: a placeholder has no runtime: a stream of it would show a blank
/// screen that is not the program's, and a refusal after the takeover would
/// cost the current viewer its attachment for nothing. A placeholder never
/// has an attachment of its own, so after re-adoption swaps in a runtime
/// the viewer attaches again. Spec: viewer `a` attaches `s1`'s running
/// host agent and catches up; with the entry's terminal then a placeholder
/// for that host (unavailable as busy, a reason that may pass), viewer
/// `b`'s grid attach is refused with `Unavailable` and a message saying the
/// supervisor keeps trying to adopt the host, while `a` is not detached
/// and still shows the host's next output; with the entry's terminal a
/// runtime again, as re-adoption leaves it, `b`'s attach is answered
/// `Attached` with stream kind `grid` and catches up, taking over from `a`.
#[farhelm_testtrace::test]
async fn a_grid_attach_to_a_placeholder_is_refused_until_a_runtime_replaces_it() {
    let (_state, sup, host, test_host) = grid_fixture(SupervisorTimeouts::default()).await;
    let mut incumbent = GridPeer::connect(&sup).await;
    assert!(matches!(
        incumbent.attach(1, 1, "a", true).await,
        ControlMsg::Attached { .. }
    ));
    incumbent
        .until("the incumbent's catch-up", |peer| {
            peer.replayed.contains(&1)
        })
        .await;

    // Put in by hand, the placeholder is not scheduled for re-adoption,
    // whose task waits for a reload's or a stop's wake: nothing replaces
    // it behind the test's back.
    let placeholder =
        HostTerminal::unavailable(test_host.socket(), crate::hosts::Unavailability::Busy, None);
    assert!(placeholder.is_placeholder(), "premise: a placeholder");
    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(
            Some(Terminal::Host(placeholder)),
            LastOutcome::Running,
        )),
    );
    let mut viewer = GridPeer::connect(&sup).await;
    let refused = viewer.attach(1, 1, "b", true).await;
    let ControlMsg::Error { kind, message, .. } = refused else {
        panic!("an attach to a placeholder is refused, got {refused:?}");
    };
    assert_eq!(kind, ErrorKind::Unavailable);
    assert!(message.contains("keeps trying"), "{message}");
    test_host.output(b"still attached\r\n");
    incumbent
        .until("the incumbent's next output", |peer| {
            peer.clients[&1]
                .screen
                .iter()
                .any(|row| crate::grid_output::test_client::row_text(row) == "still attached")
        })
        .await;
    assert!(
        incumbent.detached.is_empty(),
        "the incumbent kept its attachment"
    );

    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(Some(Terminal::Host(host)), LastOutcome::Running)),
    );
    let attached = viewer.attach(2, 1, "b", true).await;
    assert!(
        matches!(
            attached,
            ControlMsg::Attached {
                channel: 1,
                stream: farhelm_proto::StreamKind::Grid,
                ..
            }
        ),
        "{attached:?}"
    );
    viewer
        .until("the catch-up", |peer| peer.replayed.contains(&1))
        .await;
    incumbent
        .until("the takeover notice", |peer| peer.detached.contains_key(&1))
        .await;
}

/// A takeover displaces a host agent's grid attachment like any other.
///
/// Why: SPEC.md's one-attached-client rule holds per session, whatever
/// kind of terminal the session has; a second viewer of a host agent must
/// take it from the first, which is told so. Spec: a grid attach under
/// lease `a` and then one under lease `b` ends the first channel with a
/// takeover detach, while the second catches up as usual. (A session whose
/// tabs are hosts as well, which the takeover must displace together with
/// the agent, needs host tabs, a later change.)
#[farhelm_testtrace::test]
async fn a_takeover_displaces_a_host_agents_grid_attachment() {
    let (_state, sup, _host, _test_host) = grid_fixture(SupervisorTimeouts::default()).await;
    let mut first = GridPeer::connect(&sup).await;
    assert!(matches!(
        first.attach(1, 1, "a", true).await,
        ControlMsg::Attached { .. }
    ));
    first
        .until("the first catch-up", |peer| peer.replayed.contains(&1))
        .await;
    let mut second = GridPeer::connect(&sup).await;
    assert!(matches!(
        second.attach(1, 1, "b", true).await,
        ControlMsg::Attached { .. }
    ));
    first
        .until("the takeover notice", |peer| peer.detached.contains_key(&1))
        .await;
    assert_eq!(first.detached[&1], farhelm_proto::DetachCode::TakenOver);
    second
        .until("the second catch-up", |peer| peer.replayed.contains(&1))
        .await;
    assert!(second.detached.is_empty());
}

/// A grid viewer that stays paused past the stall timeout is detached as
/// stalled.
///
/// Why: the helm relies on the stall detach to end the attachment of a
/// browser that stops reading without closing its socket, for grid streams
/// as for byte streams. Spec: with a 300 ms stall timeout, a grid viewer
/// that pauses its channel after the catch-up is sent a `Detached` with the
/// stalled code.
#[farhelm_testtrace::test]
async fn a_stalled_grid_viewer_is_detached() {
    let (_state, sup, _host, test_host) = grid_fixture(SupervisorTimeouts {
        stall_detach: Duration::from_millis(300),
        ..SupervisorTimeouts::default()
    })
    .await;
    let mut peer = GridPeer::connect(&sup).await;
    assert!(matches!(
        peer.attach(1, 1, "viewer", true).await,
        ControlMsg::Attached { .. }
    ));
    peer.until("the catch-up", |peer| peer.replayed.contains(&1))
        .await;
    peer.send(&ControlMsg::PauseOutput { channel: 1 }).await;
    test_host.output(b"output nobody reads\r\n");
    peer.until("the stall detach", |peer| peer.detached.contains_key(&1))
        .await;
    assert_eq!(peer.detached[&1], farhelm_proto::DetachCode::Stalled);
}

/// A child process of this test that is killed and reaped when dropped, so
/// that a failed assertion (which unwinds past the test's own cleanup)
/// leaves no stray process behind.
struct OwnedChild(std::process::Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A live process whose command line ends with `--socket <socket>`, as a
/// host's does, while it serves nothing: a shell waiting on its input,
/// returned once its command line reads that way.
fn process_naming(socket: &Path) -> OwnedChild {
    let child = OwnedChild(
        std::process::Command::new("/bin/sh")
            // `read` waits on the piped input, which stays open until the
            // guard kills the shell.
            .args(["-c", "read line; exit 0"])
            .arg("--socket")
            .arg(socket)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("fixture: starting a process that names the socket"),
    );
    let deadline = std::time::Instant::now() + WAIT;
    loop {
        let argv = crate::procs::read_process_argv(child.0.id()).unwrap_or_default();
        if argv.last().map(Vec::as_slice)
            == Some(std::os::unix::ffi::OsStrExt::as_bytes(socket.as_os_str()))
        {
            return child;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "fixture: the process never ran its own command line"
        );
        // sleep-ok: polling interval for the shell's exec.
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Delete of a session whose startup placeholder no launch record names
/// refuses, and signals nothing, when the placeholder's socket is served by
/// a process that cannot be read as a host tagged for the session.
///
/// Why: with no record there is nothing to tie the socket to the session
/// but the host's own tag. A process that merely names the socket on its
/// command line (which is all the verified signal checks) may be anyone's,
/// renamed over the path, so ending it on the strength of the placeholder
/// would kill a process Delete has no claim to (the port review's finding
/// on unrecorded placeholders). Spec: with `s1`'s entry holding a
/// placeholder for a socket in the state's socket directory, no launch
/// record for `s1`, the socket file left by a listener that is gone, and a
/// live process whose command line ends with `--socket` and that socket's
/// path (premise: it runs), an unconditional Delete is refused saying the
/// host has no launch record; that process still runs, and the row is kept.
#[farhelm_testtrace::test]
async fn delete_never_signals_an_unrecorded_placeholders_unreadable_socket() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let socket = socket_in_state(&state);
    // A socket file nothing listens on: connecting to it is refused, and
    // the process naming it is the connection's only candidate peer.
    drop(std::os::unix::net::UnixListener::bind(&socket).expect("bind the socket file"));
    let foreign = process_naming(&socket);
    let pid = foreign.0.id();
    assert!(runs(pid), "premise: the process naming the socket runs");
    let placeholder = HostTerminal::unavailable(
        &socket,
        crate::hosts::Unavailability::Connect(crate::hosts::Unavailable::Timeout),
        None,
    );
    sup.store
        .insert_session(host_row("s1", &socket), None)
        .await
        .expect("insert the session's row");
    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(
            Some(Terminal::Host(placeholder)),
            LastOutcome::Running,
        )),
    );

    let refused = refusal(
        delete_s1(&sup, farhelm_proto::DeleteGuard::Unconditional).await,
        "an unrecorded placeholder over an unreadable socket keeps the session",
    );
    assert!(
        refused.contains("has no launch record"),
        "refused for the unproven host: {refused}"
    );
    assert!(runs(pid), "the process naming the socket was not signalled");
    assert!(
        sup.store.session("s1").await.expect("read").is_some(),
        "the row is kept"
    );
    drop(foreign);
    let _ = std::fs::remove_file(&socket);
}

// ---- Tabs in terminal hosts ---------------------------------------------------------

/// Closing a host tab whose runtime reads its host gone, while the process
/// the runtime adopted still runs, keeps the tab, its record and the mark
/// for the reap to try again, rather than forgetting a host that may run.
///
/// Why: a runtime judges its host gone from the socket alone (a missing
/// file), which is not proof the host process ended; a close that trusted
/// it would remove the record and forget the tab, leaving a running host
/// that nothing lists, closes, reaps or deletes (the round-1 review's
/// finding). The close must confirm the adopted process gone first, by the
/// verified signal, which here (the stand-in's process is the test runner,
/// whose command line names no socket) refuses. Spec: with `s1` holding a
/// host tab whose stand-in host was adopted (premise: its adopted process is
/// this test process) and whose launch record names its socket, the socket
/// file is removed and the connection dropped, so the runtime reads the
/// host gone; closing the tab fails saying the host could not be confirmed
/// ended; the tab is still in the session's cell, marked as a failed end
/// with a retry time; and its record is still there.
#[farhelm_testtrace::test]
async fn closing_a_tab_whose_runtime_reads_gone_confirms_its_process_first() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    terminal_less_tmux_s1(&sup).await;
    let tab_id = "7d2e9f40-1c3b-4a5d-8e6f-0a1b2c3d4e5f";
    let socket = socket_in_state(&state);
    let tag = crate::hosts::HostTag {
        session_id: "s1".to_string(),
        created_ns: 1,
        role: crate::hosts::HostRole::Tab {
            tab_id: tab_id.to_string(),
            scoped: false,
        },
    }
    .encode()
    .expect("encode the tag");
    let (host, test_host) = adopt_test_host_at(&socket, tag, None).await;
    let adopted = host
        .adopted_process()
        .expect("premise: the runtime knows the process it adopted");
    assert_eq!(
        adopted.pid,
        std::process::id(),
        "premise: the stand-in's process"
    );
    let mut ended = std::process::Command::new("true")
        .spawn()
        .expect("start a process to end");
    let starter = ended.id();
    ended.wait().expect("reap it");
    let record = crate::launch::tab_host_record_path(state.path(), "s1", tab_id);
    std::fs::write(
        &record,
        super::super::host_launch::LaunchHostRecord {
            socket: crate::hosts::HostName::parse(socket.file_name().unwrap().to_str().unwrap())
                .expect("a socket name this supervisor makes"),
            starter: crate::hosts::HostProcess {
                pid: starter,
                start: 0,
            },
        }
        .encode(),
    )
    .expect("write the tab's record");
    let entry = sup.sessions.lock().await.get("s1").cloned().expect("s1");
    entry.session.host_tabs.lock().unwrap().push(HostTab {
        tab_id: tab_id.to_string(),
        created_ns: 1,
        scoped: false,
        host: host.clone(),
        end_failed: false,
        failed_ends: 0,
        retry_at: None,
    });

    std::fs::remove_file(&socket).expect("remove the socket file");
    test_host.drop_connection();
    wait_for(&host, "the runtime reading its host gone", |host| {
        matches!(host.liveness(), Liveness::Gone)
    })
    .await;
    let refused = sup
        .close_tab("s1", tab_id)
        .await
        .expect_err("a host whose process may run is not taken for ended");
    assert!(
        refused.message.contains("could not be confirmed ended"),
        "{}",
        refused.message
    );
    let kept = super::super::host_tabs::host_tabs_of(&entry.session.host_tabs);
    assert!(
        matches!(kept.as_slice(), [tab] if tab.tab_id == tab_id && tab.end_failed && tab.retry_at.is_some()),
        "the tab is kept and marked for a retry: {kept:?}"
    );
    assert!(record.exists(), "the tab's record is kept");
}

// ---- Restart seeds -------------------------------------------------------------------

/// The text of `emulator`'s history (above its screen), oldest first.
fn history_text(emulator: &farhelm_terminal_emulator::Emulator) -> String {
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
}

/// A host terminal that has printed 60 numbered lines on a 24-row screen,
/// once its screen shows the last one.
async fn host_with_history() -> (HostTerminal, TestHost) {
    let (host, test_host) = adopt_test_host(None).await;
    let mut output = Vec::new();
    for n in 1..=60 {
        output.extend_from_slice(format!("seed line {n:02}\r\n").as_bytes());
    }
    test_host.output(&output);
    wait_for(&host, "the output", |host| {
        host.screen_tail(4096).contains("seed line 60")
    })
    .await;
    (host, test_host)
}

/// A Restart seed of a running host terminal holds its history, and the
/// runtime goes on driving the host afterwards.
///
/// Why: the seed is encoded on the blocking pool with the emulator taken
/// out of its slot (the reviews' finding against encoding under the
/// emulator's lock on an async worker); the emulator must come back, or
/// every later read and the runtime's own feed would find it away. Spec:
/// after 60 lines, `history_seed` gives a seed whose restored emulator's
/// history holds `seed line 01` and not `seed line 60` (on the screen); the
/// runtime is still running, a `read` answers, and output after the seed
/// reaches the screen.
#[farhelm_testtrace::test]
async fn a_restart_seed_holds_the_history_and_gives_the_emulator_back() {
    let (host, test_host) = host_with_history().await;
    let seed = host
        .history_seed(terminal_host::protocol::MAX_SNAPSHOT)
        .await
        .expect("a running runtime gives a seed");
    let restored = farhelm_terminal_emulator::Emulator::restore(&seed.blob, 1000)
        .expect("the seed restores")
        .emulator;
    let history = history_text(&restored);
    assert!(history.contains("seed line 01"), "{history}");
    assert!(!history.contains("seed line 60"), "{history}");
    assert!(restored.screen_text().trim().is_empty(), "a blank screen");
    assert!(matches!(host.liveness(), Liveness::Running { .. }));
    assert!(host.read(|_| ()).await.is_ok(), "the emulator came home");
    test_host.output(b"after the seed\r\n");
    wait_for(&host, "output after the seed", |host| {
        host.screen_tail(4096).contains("after the seed")
    })
    .await;
}

/// A tokio runtime with one blocking thread, for the tests that hold a seed's
/// encode between the emulator leaving its slot and the encode starting, by
/// keeping that one thread busy: the in-flight boundary at which a caller or
/// a runtime that goes away must not strand the emulator.
fn one_blocking_thread() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .expect("a test runtime")
}

/// Keep the runtime's one blocking thread busy until the returned sender is
/// used (or dropped), and return once it is.
async fn occupy_the_blocking_thread() -> (std::sync::mpsc::Sender<()>, tokio::task::JoinHandle<()>)
{
    let (release, released) = std::sync::mpsc::channel::<()>();
    let (busy_tx, busy) = tokio::sync::oneshot::channel();
    let occupier = tokio::task::spawn_blocking(move || {
        let _ = busy_tx.send(());
        let _ = released.recv_timeout(Duration::from_secs(60));
    });
    busy.await.expect("the blocking thread is taken");
    (release, occupier)
}

/// Wait until the emulator is away (`try_read` finds nothing) or `done`
/// holds, whichever comes first.
async fn wait_away_or(host: &HostTerminal, done: impl Fn() -> bool) {
    let waited = tokio::time::timeout(WAIT, async {
        while host.try_read(|_| ()).is_some() && !done() {
            // sleep-ok: polls the emulator's slot, which no event announces when it leaves.
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(
        waited.is_ok(),
        "the seed neither took the emulator nor answered"
    );
}

/// A seed whose runtime is stopped while its encode is in flight answers at
/// once, and the encode ends.
///
/// Why: the sol review's first reproduced stranding. An orderly stop aborts
/// every runtime, also one whose task is waiting for a seed's encode; the
/// Restart waiting for the seed must get its answer (and so the "supervisor
/// is stopping" refusal) rather than wait for an emulator nothing will bring
/// back, and the stop must not wait for the encode longer than its cancel
/// takes. Spec: on a runtime with one blocking thread, held busy, a seed is
/// asked for; once the emulator has left its slot (the encode queued), the
/// runtime is stopped (`abort`) and the thread released; the seed's request
/// returns within the test's bound, and the runtime's encodes end.
#[test]
fn a_seed_whose_runtime_is_stopped_mid_encode_answers_and_ends() {
    one_blocking_thread().block_on(async {
        let (host, _test_host) = host_with_history().await;
        let (release, occupier) = occupy_the_blocking_thread().await;
        let seeding = tokio::spawn({
            let host = host.clone();
            async move {
                let _ = host
                    .history_seed(terminal_host::protocol::MAX_SNAPSHOT)
                    .await;
            }
        });
        wait_away_or(&host, || seeding.is_finished()).await;
        assert!(!seeding.is_finished(), "premise: the seed is in flight");
        host.abort();
        let _ = release.send(());
        tokio::time::timeout(WAIT, seeding)
            .await
            .expect("the seed answers once its runtime is stopped")
            .expect("the seed task");
        tokio::time::timeout(WAIT, host.encodes_ended())
            .await
            .expect("the cancelled encode ends");
        occupier.await.expect("the occupier");
    });
}

/// A caller that stops waiting for a seed of a runtime that has ended leaves
/// the emulator readable.
///
/// Why: the sol review's second reproduced stranding: an encode the caller
/// owned (of a runtime already ended) lost the emulator when the caller was
/// cancelled, and every later read waited for good. Only a running runtime's
/// task encodes a seed now; an ended one gives none. Spec: on a runtime with
/// one blocking thread, for a stand-in host that went away (its runtime
/// ended), with the thread held busy, a seed is asked for and its caller is
/// cancelled once the emulator is away or the request answered; once the
/// thread is released and every encode ended, a `read` answers within the
/// bound.
#[test]
fn a_cancelled_seed_of_an_ended_runtime_leaves_the_emulator_readable() {
    one_blocking_thread().block_on(async {
        let (host, test_host) = host_with_history().await;
        drop(test_host);
        wait_for(&host, "the runtime's end", |host| {
            matches!(
                host.liveness(),
                Liveness::Gone | Liveness::Unavailable { .. }
            )
        })
        .await;
        let (release, occupier) = occupy_the_blocking_thread().await;
        let seeding = tokio::spawn({
            let host = host.clone();
            async move {
                let _ = host
                    .history_seed(terminal_host::protocol::MAX_SNAPSHOT)
                    .await;
            }
        });
        wait_away_or(&host, || seeding.is_finished()).await;
        seeding.abort();
        let _ = release.send(());
        occupier.await.expect("the occupier");
        tokio::time::timeout(WAIT, host.encodes_ended())
            .await
            .expect("no encode runs");
        assert!(
            tokio::time::timeout(WAIT, host.read(|_| ())).await.is_ok(),
            "the emulator is readable"
        );
    });
}

/// A caller that stops waiting for a seed of a running runtime leaves the
/// emulator to come back, and the runtime driving its host.
///
/// Why: a Restart's request handler can be cancelled while the seed is
/// encoded (its client went away); the runtime's task owns the encode, so
/// the emulator must come back and later reads and seeds work. Spec: with
/// the runtime's encodes held at an encode gate, a seed is asked for; once
/// the encode reached the gate its caller is cancelled and the gate opened;
/// a `read` answers within the bound, a second seed is given, and output
/// still reaches the screen.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_cancelled_seed_of_a_running_runtime_gives_the_emulator_back() {
    let gate = crate::hosts::terminal::EncodeGate::closed();
    let (host, test_host) = adopt_test_host_with(
        TestHostSpec {
            options: Some(crate::hosts::HostTerminalOptions {
                encode_gate: Some(gate.clone()),
                ..test_host_options(None)
            }),
            ..TestHostSpec::default()
        },
        crate::hosts::Adoption::Fresh,
    )
    .await;
    test_host.output(b"seed line 01\r\n");
    wait_for(&host, "the output", |host| {
        host.screen_tail(4096).contains("seed line 01")
    })
    .await;
    let seeding = tokio::spawn({
        let host = host.clone();
        async move {
            host.history_seed(terminal_host::protocol::MAX_SNAPSHOT)
                .await
        }
    });
    let entered = {
        let gate = gate.clone();
        tokio::task::spawn_blocking(move || gate.wait_entered(WAIT))
            .await
            .expect("the wait")
    };
    assert!(entered, "premise: the seed's encode reached the gate");
    seeding.abort();
    gate.open();
    assert!(
        tokio::time::timeout(WAIT, host.read(|_| ())).await.is_ok(),
        "the emulator came back"
    );
    assert!(
        host.history_seed(terminal_host::protocol::MAX_SNAPSHOT)
            .await
            .is_some(),
        "a later seed is given"
    );
    test_host.output(b"after the seed\r\n");
    wait_for(&host, "output after the seed", |host| {
        host.screen_tail(4096).contains("after the seed")
    })
    .await;
}

/// A supervisor with dummy launches, whose host program does not exist and
/// whose Restart can be held at its window by `window`, and a resumable
/// session of it made a host session: its agent terminal is a stand-in host,
/// adopted with `options`, whose child has exited (so a Restart stops
/// nothing and needs no consent). Returns the supervisor, the session's id,
/// the stand-in's runtime and the stand-in, and the state directory.
///
/// The session is created in tmux (`SupervisorSeams::tmux_terminals`) and
/// then given the stand-in: a unit test cannot start a real host for the
/// create (`tmux_supervisor`), and a Restart of a host agent starts a host
/// whichever kind new terminals are.
async fn host_session_with(
    options: crate::hosts::HostTerminalOptions,
    window: Option<RestartWindowGate>,
) -> (Arc<Supervisor>, String, HostTerminal, TestHost, StateDir) {
    let mut seams = SupervisorSeams {
        host_program: Some(PathBuf::from("/nonexistent/farhelm-host")),
        ..SupervisorSeams::tmux_terminals()
    };
    seams.faults.restart_window_gate = window;
    let state = StateDir::new();
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts::default(),
        seams,
    )
    .await
    .expect("supervisor");
    let created = create_resumable_session(&sup, "/tmp", "sleep 300").await;
    let id = created.id.clone();
    let (old, test_host) = adopt_test_host_with(
        TestHostSpec {
            options: Some(options),
            ..TestHostSpec::default()
        },
        crate::hosts::Adoption::Fresh,
    )
    .await;
    test_host.output(b"seed line 01\r\n");
    test_host.exit(0);
    wait_for(&old, "the stand-in's exit", |host| {
        matches!(host.liveness(), Liveness::Exited { .. })
    })
    .await;
    sup.store
        .set_terminal_for_test(
            &id,
            crate::store::TerminalKind::Host,
            Terminal::Host(old.clone()).recorded_pane(),
        )
        .await
        .expect("make the row a host row");
    {
        let mut sessions = sup.sessions.lock().await;
        let entry = sessions.get(&id).cloned().expect("the created session");
        sessions.insert(
            id.clone(),
            Arc::new(SessionEntry {
                terminal: Some(Terminal::Host(old.clone())),
                info: entry.info.clone(),
                run: entry.run.clone(),
                session: entry.session.clone(),
                snapshot: entry.snapshot.clone(),
                canonical_cwd: entry.canonical_cwd.clone(),
                generation: entry.generation,
                scope: entry.scope.clone(),
            }),
        );
    }
    (sup, id, old, test_host, state)
}

/// An orderly stop that begins while a Restart's seed is encoded refuses
/// the Restart promptly, with the generation unclaimed, and the stop does not
/// hang.
///
/// Why: the seed is read before the generation is claimed so that a stop
/// meanwhile leaves the session as it was; the stop cancels the encode as it
/// closes the old runtime (still on the session map then), and the Restart
/// must turn the missing seed into the "supervisor is stopping" refusal
/// rather than wait or start an empty terminal. Spec: with the old runtime's
/// encodes held at an encode gate, a consented Restart's seed reaches the
/// gate; the host side of an orderly stop (one-second budget) returns within
/// the test's bound; the Restart fails within it saying the supervisor is
/// stopping; the gate saw the encode cancelled; the runtime's encodes ended;
/// the row is still at generation 0.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_stop_during_a_restarts_seed_refuses_it_promptly() {
    let gate = crate::hosts::terminal::EncodeGate::closed();
    let (sup, id, old, _test_host, _state) = host_session_with(
        crate::hosts::HostTerminalOptions {
            encode_gate: Some(gate.clone()),
            ..test_host_options(None)
        },
        None,
    )
    .await;
    let restart = tokio::spawn({
        let sup = Arc::clone(&sup);
        let id = id.clone();
        async move { sup.restart_session(&id, true, None, None).await }
    });
    let entered = {
        let gate = gate.clone();
        tokio::task::spawn_blocking(move || gate.wait_entered(WAIT))
            .await
            .expect("the wait")
    };
    assert!(entered, "premise: the seed's encode reached the gate");
    tokio::time::timeout(WAIT, sup.checkpoint_hosts_for_test(Duration::from_secs(1)))
        .await
        .expect("the stop's host side does not hang");
    let refused = tokio::time::timeout(WAIT, restart)
        .await
        .expect("the restart answers")
        .expect("the restart task")
        .expect_err("a stop during the seed refuses the restart");
    assert!(
        format!("{refused:#}").contains("supervisor is stopping"),
        "{refused:#}"
    );
    assert!(gate.cancelled(), "the stop cancelled the seed's encode");
    tokio::time::timeout(WAIT, old.encodes_ended())
        .await
        .expect("the encode ended");
    let row = sup.store.session(&id).await.expect("read").expect("row");
    assert_eq!(row.generation, 0, "no generation was claimed");
}

/// A host restart that fails definitively after an orderly stop gathered the
/// host terminals puts back a placeholder, never the old runtime, whose
/// encode in progress the stop then cancels.
///
/// Why: the reviews' finding against the failure path's republication: the
/// old runtime is off the session map while the restart runs, so the stop's
/// gather misses it, and restoring it afterwards would leave a connected
/// runtime and its encodes running past the stop that should have closed
/// them. Spec: a host session (its stand-in's child has exited, so nothing
/// is stopped), restarted under a supervisor whose host program does not
/// exist, with the restart held at its window (the session off the map); a
/// checkpoint of the old runtime started there is held at its encode gate
/// (the emulator away); the host side of an orderly stop runs, and the
/// restart is released: it fails; the session's terminal is a host
/// placeholder; the old runtime has ended (its status channel closed), the
/// gate saw the held encode cancelled, and the runtime's encodes ended.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_failed_host_restart_after_a_stop_began_restores_a_placeholder() {
    let (reached_tx, reached_rx) = tokio::sync::oneshot::channel::<()>();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
    let reached = Arc::new(std::sync::Mutex::new(Some(reached_tx)));
    let release = Arc::new(tokio::sync::Mutex::new(Some(release_rx)));
    let window: RestartWindowGate = Arc::new(move || {
        let reached = Arc::clone(&reached);
        let release = Arc::clone(&release);
        Box::pin(async move {
            if let Some(tx) = reached.lock().expect("gate mutex").take() {
                let _ = tx.send(());
            }
            if let Some(rx) = release.lock().await.take() {
                let _ = rx.await;
            }
        })
    });
    let encodes = crate::hosts::terminal::EncodeGate::closed();
    encodes.open();
    let (sup, id, old, test_host, _state) = host_session_with(
        crate::hosts::HostTerminalOptions {
            encode_gate: Some(encodes.clone()),
            // This test deliberately leaves a stop checkpoint encoding
            // for the failed Restart to cancel; a skip would remove that premise.
            checkpoints: crate::hosts::CheckpointPolicy {
                stop_min_bytes: 0,
                ..Default::default()
            },
            ..test_host_options(None)
        },
        Some(window),
    )
    .await;

    let mut restart = tokio::spawn({
        let sup = Arc::clone(&sup);
        let id = id.clone();
        async move { sup.restart_session(&id, true, None, None).await }
    });
    tokio::time::timeout(WAIT, async {
        tokio::select! {
            reached = reached_rx => reached.expect("the gate reports arrival"),
            done = &mut restart => panic!("the restart ended before its window: {done:?}"),
        }
    })
    .await
    .expect("the restart reaches its window");
    // An encode the stop will have to cancel: a checkpoint of the old
    // runtime, of output it has not checkpointed, held at the gate.
    encodes.close();
    test_host.output(b"more output\r\n");
    wait_for(&old, "the new output", |host| {
        host.screen_tail(4096).contains("more output")
    })
    .await;
    let checkpoint = tokio::spawn({
        let old = old.clone();
        async move { old.checkpoint_now(tokio::time::Instant::now() + WAIT).await }
    });
    wait_away_or(&old, || checkpoint.is_finished()).await;
    assert!(
        old.try_read(|_| ()).is_none(),
        "premise: the checkpoint's encode holds the emulator"
    );
    sup.checkpoint_hosts_for_test(Duration::from_secs(1)).await;
    let _ = release_tx.send(());
    let restarted = tokio::time::timeout(WAIT, restart)
        .await
        .expect("the restart finishes")
        .expect("the restart task");
    restarted.expect_err("no host can start from a program that does not exist");

    let entry = sup
        .sessions
        .lock()
        .await
        .get(&id)
        .cloned()
        .expect("the entry");
    match &entry.terminal {
        Some(Terminal::Host(host)) => assert!(host.is_placeholder(), "a placeholder"),
        other => panic!("not a host placeholder: {other:?}"),
    }
    let mut status = old.subscribe();
    let ended = tokio::time::timeout(WAIT, async { while status.changed().await.is_ok() {} }).await;
    assert!(ended.is_ok(), "the old runtime was stopped");
    tokio::time::timeout(WAIT, old.encodes_ended())
        .await
        .expect("no encode of the old runtime runs");
    assert!(
        encodes.cancelled() && !encodes.expired(),
        "the held encode was cancelled, not let through"
    );
    drop(checkpoint);
}

/// A host agent session with an earlier host start outstanding, as
/// [`host_agent_with_an_earlier_start`] makes it.
struct HostAgentWithEarlierStart {
    id: String,
    /// The session's entry as installed, whose activity reading a test
    /// can change while a Restart runs.
    entry: Arc<SessionEntry>,
    /// The session's agent terminal: the stand-in host tagged as its
    /// generation-0 agent, its program running.
    current_host: TestHost,
    /// The stand-in host tagged as its generation-1 agent, named by a
    /// launch record, its program running.
    earlier_host: TestHost,
    _current: HostTerminal,
}

/// Make a resumable session of `sup` (on `state`) a host agent session
/// whose agent terminal is a stand-in host tagged as its generation-0 agent
/// with its program running and the session reading `reading`, beside a
/// second stand-in host tagged as its generation-1 agent with its program
/// running, which a launch record names with `starter` as its first stage:
/// a Restart has a launch to settle after the held host's, whose program it
/// would root into its stop.
async fn host_agent_with_an_earlier_start(
    sup: &Arc<Supervisor>,
    state: &StateDir,
    starter: crate::hosts::HostProcess,
    reading: crate::agent_kind::screen_reader::ScreenState,
) -> HostAgentWithEarlierStart {
    let created = create_resumable_session(sup, "/tmp", "sleep 300").await;
    let id = created.id.clone();
    let hosts = crate::hosts::ensure_hosts_dir(&std::path::absolute(state.path()).unwrap())
        .expect("the socket directory");
    let tag = |generation| {
        crate::hosts::HostTag {
            session_id: id.clone(),
            created_ns: 1,
            role: crate::hosts::HostRole::Agent { generation },
        }
        .encode()
        .expect("a tag")
    };
    let current_name = crate::hosts::free_host_name(state.path()).expect("a name");
    let (current, current_host) =
        adopt_test_host_at(&current_name.socket_in(&hosts), tag(0), None).await;
    let earlier_name = crate::hosts::free_host_name(state.path()).expect("a name");
    let earlier_host = start_test_host_at(&earlier_name.socket_in(&hosts), tag(1), false);
    let record = super::host_launch::LaunchHostRecord {
        socket: earlier_name,
        starter,
    };
    std::fs::write(
        LaunchArtifacts::for_launch(state.path(), &id, 1).host(),
        record.encode(),
    )
    .expect("write the launch record");
    sup.store
        .set_terminal_for_test(
            &id,
            crate::store::TerminalKind::Host,
            Terminal::Host(current.clone()).recorded_pane(),
        )
        .await
        .expect("make the row a host row");
    let installed = {
        let mut sessions = sup.sessions.lock().await;
        let entry = sessions.get(&id).cloned().expect("the created session");
        let installed = Arc::new(SessionEntry {
            terminal: Some(Terminal::Host(current.clone())),
            info: entry.info.clone(),
            run: entry.run.clone(),
            session: entry.session.clone(),
            snapshot: entry.snapshot.clone(),
            canonical_cwd: entry.canonical_cwd.clone(),
            generation: entry.generation,
            scope: entry.scope.clone(),
        });
        {
            let mut activity = installed.run.activity.lock().unwrap();
            activity.reading.state = reading;
            activity.startup_provisional = false;
        }
        sessions.insert(id.clone(), Arc::clone(&installed));
        installed
    };
    assert!(
        runs(current_host.child_pid()) && runs(earlier_host.child_pid()),
        "premise: both programs run"
    );
    HostAgentWithEarlierStart {
        id,
        entry: installed,
        current_host,
        earlier_host,
        _current: current,
    }
}

/// A Restart without consent of a working host agent is refused before
/// anything is touched, also a program an earlier host start of the session
/// left running.
///
/// Why: the sol review's second-round finding. Settling the session's
/// earlier host starts roots their running programs into a sweep that also
/// takes every process carrying the session's marker, so a sweep run before
/// the consent check stopped the working agent the check exists to protect,
/// and the refusal came after the damage. Spec: a session whose agent
/// terminal is a stand-in host tagged as its generation-0 agent, with its
/// program running and the session reading as working, and a second
/// stand-in host tagged as its generation-1 agent, named by a launch record
/// whose first stage has ended, with its program running: a Restart without
/// consent fails as a conflict, and both programs still run.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_restart_without_consent_touches_no_program_of_a_working_host_agent() {
    let state = StateDir::new();
    let sup = tmux_supervisor(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let mut ended = std::process::Command::new("true")
        .spawn()
        .expect("a process");
    let starter = crate::hosts::HostProcess {
        pid: ended.id(),
        start: 0,
    };
    ended.wait().expect("reap it");
    let session = host_agent_with_an_earlier_start(
        &sup,
        &state,
        starter,
        crate::agent_kind::screen_reader::ScreenState::Working,
    )
    .await;

    let refused = sup
        .restart_session(&session.id, false, None, None)
        .await
        .expect_err("a working agent is not restarted without consent");
    assert_eq!(error_kind(&refused), ErrorKind::Conflict, "{refused:#}");
    assert!(
        runs(session.current_host.child_pid()),
        "the working agent's program still runs"
    );
    assert!(
        runs(session.earlier_host.child_pid()),
        "the earlier start's program still runs"
    );
}

/// A Restart without consent asks again after it settled the session's
/// earlier host starts, and refuses an agent that started working while it
/// waited.
///
/// Why: the reviews' third-round finding. The consent check moved before
/// the settle so that a refusal touches nothing, but the settle can wait
/// seconds on an earlier start's first stage, and the agent's input is not
/// held by the lifecycle claim: an agent idle at the first reading and
/// working by the stop would be stopped without the consent SPEC.md
/// requires. Spec: the session of [`host_agent_with_an_earlier_start`],
/// reading idle, whose earlier launch record names a first stage that
/// still runs; while the settle waits for that first stage (its waiting
/// seam), the session's reading turns to working and the first stage ends;
/// the Restart without consent fails as a conflict, the seam fired, and
/// both programs still run.
#[farhelm_testtrace::test(flavor = "multi_thread")]
async fn a_restart_asks_again_for_consent_after_settling_earlier_starts() {
    use crate::agent_kind::screen_reader::ScreenState;
    let entry: Arc<std::sync::OnceLock<Arc<SessionEntry>>> = Arc::default();
    let first_stage = std::process::Command::new("sleep")
        .arg("300")
        .spawn()
        .expect("a stand-in first stage");
    let starter =
        crate::hosts::HostProcess::current(first_stage.id()).expect("the first stage runs");
    let first_stage = Arc::new(std::sync::Mutex::new(Some(first_stage)));
    let _first_stage_ends = EndsChildOnDrop(Arc::clone(&first_stage));
    let waited = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut seams = SupervisorSeams::tmux_terminals();
    seams.faults.launch_starter_waiting = Some(Arc::new({
        let entry = Arc::clone(&entry);
        let first_stage = Arc::clone(&first_stage);
        let waited = Arc::clone(&waited);
        move || {
            waited.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(entry) = entry.get() {
                entry.run.activity.lock().unwrap().reading.state = ScreenState::Working;
            }
            end_child(&first_stage);
        }
    }));
    let state = StateDir::new();
    let sup = Supervisor::new_with_seams(
        state.path(),
        dummy_exe(),
        SupervisorTimeouts::default(),
        seams,
    )
    .await
    .expect("supervisor");
    let session = host_agent_with_an_earlier_start(&sup, &state, starter, ScreenState::Idle).await;
    let _ = entry.set(Arc::clone(&session.entry));
    assert_eq!(
        crate::service::status::live_status(&session.entry),
        SessionStatus::Idle,
        "premise: the agent reads idle at the first check"
    );

    let refused = sup
        .restart_session(&session.id, false, None, None)
        .await
        .expect_err("an agent working by the stop is not restarted without consent");
    assert!(
        waited.load(std::sync::atomic::Ordering::SeqCst),
        "premise: the settle waited for the first stage"
    );
    assert_eq!(error_kind(&refused), ErrorKind::Conflict, "{refused:#}");
    assert!(
        runs(session.current_host.child_pid()),
        "the working agent's program still runs"
    );
    assert!(
        runs(session.earlier_host.child_pid()),
        "the earlier start's program still runs"
    );
}

/// Kill and reap the child in `slot`, if it is still there.
fn end_child(slot: &std::sync::Mutex<Option<std::process::Child>>) {
    if let Some(mut child) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Ends the stand-in child in its slot when dropped, so a test that fails
/// before its seam ended the child leaves no process behind.
struct EndsChildOnDrop(Arc<std::sync::Mutex<Option<std::process::Child>>>);

impl Drop for EndsChildOnDrop {
    fn drop(&mut self) {
        end_child(&self.0);
    }
}
/// A short tail takes no stop slot and its runtime closes while a host at
/// the threshold still occupies the only slot. Leaving the old snapshot
/// and tail deliberately counts as success, rather than a timed-out save.
/// The held large encode makes this boundary observable before final close.
#[farhelm_testtrace::test]
async fn an_orderly_stop_skips_small_tails_before_waiting_for_a_slot() {
    use crate::hosts::terminal::EncodeGate;

    /// Keep an assertion failure from holding a blocking encoder until its
    /// independent safety timeout.
    struct ReleaseEncode(EncodeGate);
    impl Drop for ReleaseEncode {
        fn drop(&mut self) {
            self.0.open();
        }
    }

    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let small_gate = EncodeGate::closed();
    let large_gate = EncodeGate::closed();
    let _release = (
        ReleaseEncode(large_gate.clone()),
        ReleaseEncode(small_gate.clone()),
    );
    let mut hosts = Vec::new();
    let mut peers = Vec::new();
    for (id, tail, gate) in [
        ("small", 127, small_gate.clone()),
        ("threshold", 128, large_gate.clone()),
    ] {
        let mut options = test_host_options(None);
        options.checkpoints.stop_min_bytes = 128;
        options.encode_gate = Some(gate);
        assert!(
            (tail as u64)
                < options
                    .checkpoints
                    .min_bytes
                    .min(options.checkpoints.max_bytes),
            "no automatic checkpoint is due"
        );
        let (host, peer) = adopt_test_host_with(
            TestHostSpec {
                options: Some(options),
                ..TestHostSpec::default()
            },
            crate::hosts::Adoption::Fresh,
        )
        .await;
        peer.output(&vec![b'x'; tail]);
        wait_for(&host, "the fully processed tail", |host| {
            host.status().position.offset == tail as u64
        })
        .await;
        assert_eq!(host.status().checkpoint, None);
        sup.sessions.lock().await.insert(
            id.to_string(),
            Arc::new(entry_with(
                Some(Terminal::Host(host.clone())),
                LastOutcome::Running,
            )),
        );
        hosts.push(host);
        peers.push(peer);
    }
    let inspect = async {
        let gate = large_gate.clone();
        assert!(
            tokio::task::spawn_blocking(move || gate.wait_entered(WAIT))
                .await
                .expect("wait for the threshold encode")
        );
        let mut status = hosts[0].subscribe();
        tokio::time::timeout(WAIT, async { while status.changed().await.is_ok() {} })
            .await
            .expect("the skipped runtime closes before the held checkpoint finishes");
        assert!(!small_gate.passed(), "the short tail was never encoded");
        assert_eq!(
            hosts[0].status().checkpoint,
            None,
            "the previous snapshot is kept"
        );
        assert!(
            hosts[1].status().checkpoint.is_none(),
            "the threshold encode is still held"
        );
        large_gate.open();
    };
    let (current, ()) = tokio::join!(sup.checkpoint_hosts_for_exit_with_slots(WAIT, 1), inspect);
    assert!(current, "a deliberate skip is a successful stop");
    assert!(
        hosts[1].status().checkpoint.is_some(),
        "the threshold tail was saved"
    );
    assert!(!small_gate.passed());
    assert!(!large_gate.expired());
    drop(peers);
}
