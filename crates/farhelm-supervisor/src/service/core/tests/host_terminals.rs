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
use crate::hosts::terminal::test_host::{TestHost, adopt_test_host};
use crate::hosts::{HostTerminal, Liveness};
use crate::procs::ProcessIdentity;

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
