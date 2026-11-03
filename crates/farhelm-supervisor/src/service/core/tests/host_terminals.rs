//! The supervisor's terminal switch points with a host terminal behind a
//! session, against a stand-in host in this test process
//! (`hosts::terminal::test_host`): status and exit recording, the
//! never-started excerpt, an unreadable host, the ticker's wake on a host
//! exit, the hook anchor, the hold wait, prompt delivery, and Stop of a
//! child whose host has not yet reported its exit.
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

/// A Restart of a session with no terminal is refused, before anything
/// changes, when its row says the agent was started in a host.
///
/// Why: a create under the terminal-host switch can be kept as a launching
/// record with no terminal (its host could not be confirmed ended), and
/// every relaunch target is tmux's while a relaunch keeps the row's kind:
/// relaunching it would leave a host row whose `pane` holds a tmux pane id,
/// which a later reader looks for as a host's socket. Restart into a new
/// host comes with a later change. Spec: with a host row for `s1` and its
/// published entry holding no terminal, a consented Restart fails with the
/// host-agent refusal, and the row's generation, outcome, kind and `pane`
/// are as they were.
#[farhelm_testtrace::test]
async fn restart_refuses_a_terminal_less_session_whose_row_is_a_host_row() {
    let state = StateDir::new();
    let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
        .await
        .expect("supervisor");
    let row = host_row("s1", Path::new("/nonexistent/hosts/ABCDEFGH"));
    sup.store
        .insert_session(row.clone(), None)
        .await
        .expect("insert the host row");
    sup.sessions.lock().await.insert(
        "s1".to_string(),
        Arc::new(entry_with(None, LastOutcome::Running)),
    );

    let refused = sup
        .restart_session("s1", true, None, None)
        .await
        .expect_err("a terminal-less host agent is not relaunched into tmux");
    assert!(
        format!("{refused:#}").contains(HOST_RESTART_UNAVAILABLE),
        "{refused:#}"
    );
    let after = sup.store.session("s1").await.expect("read").expect("row");
    assert_eq!(
        (
            after.generation,
            after.outcome,
            after.terminal_kind,
            after.pane
        ),
        (row.generation, row.outcome, row.terminal_kind, row.pane)
    );
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

/// Whether the stand-in's child still runs (not a zombie, not gone).
fn runs(pid: u32) -> bool {
    matches!(
        crate::procs::read_process(pid),
        Ok(Some((_, _, crate::procs::ProcessState::Running)))
    )
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
