//! Host terminals across a supervisor restart: an orderly stop that
//! checkpoints each host, a crash that leaves the host its last snapshot and
//! the output since, a launch interrupted before its confirmation, and a
//! host the restarted supervisor cannot read.
//!
//! Every supervisor here is the e2e suite's in-process one with the
//! internal switch on (`host_creates::host_seams`), so creates start their
//! agents in hosts started from the built `farhelm`, never inside this test
//! process. A "restart" drops one supervisor and constructs the next on the
//! same state directory, as `create_idempotency`'s handoff does, once the
//! first has released the state directory's claim: the hosts keep running
//! in between, as they do across a real supervisor's restart. An orderly
//! stop is the supervisor's own (`Supervisor::stop_in_order_for_test`, what
//! `farhelm supervisor run` does on SIGTERM); a crash is the supervisor
//! dropped without it, which closes every host connection the way a dying
//! process does and leaves no checkpoint behind.
//!
//! Each test ends the hosts it leaves through the last supervisor's runtime
//! before its state directory drops, so that runtime's connection does not
//! fight the directory's own cleanup.

use crate::harness::*;
use crate::host_creates::{end_runtime, host_seams};
use crate::host_terminals::{WAIT_SECS, host_process, wait_for_screen};
use farhelm_supervisor::hosts::{CheckpointOutcome, ClientDeadlines, HostTerminal, Liveness};
use std::path::Path;

// ---- Fixtures -------------------------------------------------------------------

/// One test's state across its supervisors: the state directory (whose
/// drop ends any host left in it), its private tmux server, and the
/// session's working directory. The tmux guard is declared before the state
/// directory so the server stops before its socket's directory goes.
pub(crate) struct Stack {
    _tmux: TmuxServerGuard,
    pub(crate) state: farhelm_teststate::TestDir,
    pub(crate) work: farhelm_teststate::TestDir,
    _slot: tokio::sync::SemaphorePermit<'static>,
}

impl Stack {
    pub(crate) async fn new() -> Self {
        let slot = SLOTS.acquire().await.expect("semaphore is never closed");
        let state = farhelm_teststate::tempdir().expect("state");
        Self {
            _tmux: TmuxServerGuard::new(state.path().join("tmux.sock")),
            state,
            work: farhelm_teststate::tempdir().expect("workdir"),
            _slot: slot,
        }
    }

    /// A supervisor on this stack's state directory with `seams`, which must
    /// hold the directory's claim (a supervisor without it reconciles
    /// nothing and finds no host, which would make a restart test pass for
    /// the wrong reason), and a client of it.
    pub(crate) async fn supervisor(
        &self,
        seams: SupervisorSeams,
    ) -> (Arc<Supervisor>, Arc<SupervisorClient>) {
        let sup = Supervisor::new_with_seams(
            self.state.path(),
            farhelm_bin().into(),
            suite_timeouts(),
            seams,
        )
        .await
        .expect("supervisor");
        assert!(
            sup.owns_state_dir(),
            "premise: the supervisor holds its state directory"
        );
        let client = connect_client(&sup).await;
        (sup, client)
    }

    /// Create a session running `command` in this stack's working directory.
    pub(crate) async fn create(&self, client: &SupervisorClient, command: &str) -> SessionInfo {
        client
            .create_session(&self.work.path().to_string_lossy(), command, None, 80, 24)
            .await
            .expect("create")
    }
}

/// Drop `sup` and its client, and wait until it has released the state
/// directory's claim, so that the next supervisor owns the directory. Its
/// host runtimes end with it, their connections closed and their hosts left
/// running, as at a crash.
pub(crate) async fn retire(state: &Path, sup: Arc<Supervisor>, client: Arc<SupervisorClient>) {
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(WAIT_SECS);
    while Arc::strong_count(&sup) > 1 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the old supervisor's connection tasks never released it"
        );
        // sleep-ok: observe the connection tasks' references drain before dropping the owner.
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);
    crate::create_idempotency::wait_for_state_dir_claim_release(state, deadline).await;
}

/// The published runtime of `session_id`'s agent host.
pub(crate) async fn agent_host(sup: &Supervisor, session_id: &str) -> HostTerminal {
    sup.agent_host_for_test(session_id)
        .await
        .expect("the session's agent terminal is a host")
}

/// Wait until `host`'s runtime has processed everything its host had
/// journaled when it was adopted.
pub(crate) async fn wait_until_caught_up(host: &HostTerminal) {
    let mut status = host.subscribe();
    let waited = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while !host.status().caught_up {
            if status.changed().await.is_err() {
                return host.status().caught_up;
            }
        }
        true
    })
    .await;
    assert_eq!(
        waited,
        Ok(true),
        "the runtime never caught up: {:?}",
        host.status()
    );
}

/// The session's durable row, read from the store the supervisors write.
pub(crate) async fn stored_row(state: &Path, session_id: &str) -> StoredSession {
    SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the store")
        .session(session_id)
        .await
        .expect("read the row")
        .expect("the row exists")
}

/// The session as `client`'s supervisor lists it.
pub(crate) async fn listed(client: &SupervisorClient, session_id: &str) -> SessionInfo {
    client
        .list_sessions()
        .await
        .expect("list")
        .sessions
        .into_iter()
        .find(|info| info.id == session_id)
        .expect("the session is listed")
}

/// Require that `session_id` survived its supervisor's restart as a running
/// session: after an observation pass (which records any exit it sees), its
/// row is still `Running`, and it lists as `Unknown`, the provisional status
/// every reloaded session has until the ticker samples its screen (these
/// harnesses run no ticker), where an exited one would list as exited.
async fn assert_reloaded_as_running(
    sup: &Supervisor,
    client: &SupervisorClient,
    state: &Path,
    session_id: &str,
) {
    sup.reconcile_for_test().await;
    assert_eq!(
        stored_row(state, session_id).await.outcome,
        LastOutcome::Running
    );
    assert_eq!(
        listed(client, session_id).await.status,
        SessionStatus::Unknown
    );
}

/// Send `line` and Enter to the program in `host`, and require that the
/// host wrote it.
pub(crate) async fn type_line(host: &HostTerminal, line: &str) {
    let receipt = host
        .send_input(format!("{line}\r").as_bytes())
        .await
        .expect("the runtime takes input");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(WAIT_SECS), receipt.fate())
            .await
            .expect("the input's fate in time"),
        farhelm_supervisor::hosts::InputFate::Written
    );
}

/// Wait until `host`'s runtime has ended: its task, which owns the host's
/// connection, is gone, so its status channel closes.
async fn wait_until_runtime_ended(host: &HostTerminal) {
    let mut status = host.subscribe();
    let ended = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while status.changed().await.is_ok() {}
    })
    .await;
    assert!(ended.is_ok(), "the runtime still runs: {:?}", host.status());
}

/// Wait until `sup` publishes, for `session_id`, a host terminal its runtime
/// drives (running, or exited), as re-adoption puts one in place of an
/// unavailable one, and return it.
async fn wait_for_driven_host(sup: &Supervisor, session_id: &str, secs: u64) -> HostTerminal {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(host) = sup.agent_host_for_test(session_id).await
            && matches!(
                host.liveness(),
                Liveness::Running { .. } | Liveness::Exited { .. }
            )
        {
            return host;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no host terminal driven for the session within {secs}s: {:?}",
            sup.agent_host_for_test(session_id)
                .await
                .map(|host| host.liveness())
        );
        // sleep-ok: polling interval for the supervisor's own re-adoption task, which publishes no event.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// The socket a session's launch record names for its first launch's host:
/// the host an interrupted create started, before anything confirms it.
fn recorded_socket(state: &Path, session_id: &str) -> std::path::PathBuf {
    let record = std::fs::read_to_string(state.join("launch").join(format!("{session_id}.0.host")))
        .expect("the launch record");
    let name = record
        .lines()
        .find_map(|line| line.strip_prefix("socket "))
        .expect("the record names its socket");
    state.join("hosts").join(name)
}

/// A stopped host process, continued when this is dropped: a test that
/// stops a host (SIGSTOP) holds one, so that a failing assertion never
/// leaves the host stopped, which the state directory's cleanup could not
/// end through the protocol.
pub(crate) struct Stopped(u32);

impl Stopped {
    /// Stop process `pid`, the test's own host.
    pub(crate) fn stop(pid: u32) -> Self {
        // SAFETY: `kill` has no memory-safety preconditions; the pid is a
        // host this test's supervisor started, found by its socket.
        assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGSTOP) }, 0);
        Stopped(pid)
    }
}

impl Drop for Stopped {
    fn drop(&mut self) {
        // SAFETY: as in `stop`.
        unsafe { libc::kill(self.0 as i32, libc::SIGCONT) };
    }
}

// ---- Tests ----------------------------------------------------------------------

/// After an orderly stop and a restart, a host agent's session is still
/// running in the same host, with the same screen, and the new supervisor
/// drives it.
///
/// Why: the plan's PR 17 and the inventory's top risk 2. Without reload
/// finding the host again, every supervisor restart (each update, each
/// quit of the Mac app) would record a running host agent as exited and
/// leave its host unreachable; and without the stop's checkpoint, the next
/// supervisor would replay the whole journal since the last one. Spec: a
/// created host agent that echoed a line is stopped in order: the stop
/// leaves the host a snapshot at a position its runtime reached during the
/// stop (at least where the terminal was when the stop began), and the
/// host process and the agent keep running past the supervisor. A
/// supervisor constructed next on the same state directory publishes the
/// same host for the session (same socket, same host process, same child),
/// recovered from that snapshot (its last known snapshot is the stop's),
/// showing exactly the screen the stopped one showed once caught up; the
/// row is still `Running` with the socket's name in `pane`, also after an
/// observation pass, the session lists as `Unknown` (any reloaded session's
/// status until its screen is sampled, which these harnesses never do; an
/// exit would list as exited). The stop also closed its runtime (its task,
/// which owns the connection, ended). Once the new supervisor serves,
/// which reloads the sessions a second time, the session still holds the
/// very runtime the first reload adopted, still driving its host, and a
/// line typed through it is echoed.
#[farhelm_testtrace::test]
async fn an_orderly_restart_keeps_a_host_agent_running_with_its_screen() {
    let stack = Stack::new().await;
    let (sup, client) = stack.supervisor(host_seams()).await;
    let session = stack
        .create(&client, &fixture_cmd("fake-agent --script basic"))
        .await;
    let host = agent_host(&sup, &session.id).await;
    wait_for_screen(&host, AGENT_READY_MARKER).await;
    type_line(&host, "before-restart").await;
    wait_for_screen(&host, "echo:before-restart").await;
    let socket = host.socket().to_path_buf();
    let host_pid = host_process(&socket).expect("the host's process serves its socket");
    let child = host.status().child_pid;
    let screen = host.screen_tail(4096);
    let before = host.status().position;

    sup.stop_in_order_for_test().await;
    let stopped = host.status();
    // The agent's echo can still be arriving (the rest of its line after the
    // text the screen wait saw) while the stop runs, and the runtime reads
    // it until the stop closes it, so the snapshot lies between where the
    // terminal was when the stop began and where the runtime ended, not
    // necessarily at the latter.
    let checkpoint = stopped
        .checkpoint
        .expect("the stop left the host a snapshot");
    assert!(
        before.offset <= checkpoint.offset
            && checkpoint.offset <= stopped.position.offset
            && checkpoint.resizes == stopped.position.resizes,
        "the stop's snapshot {checkpoint:?} covers where the terminal was when the stop began \
         ({before:?}) and no more than the runtime read ({:?})",
        stopped.position
    );
    wait_until_runtime_ended(&host).await;
    drop(host);
    retire(stack.state.path(), sup, client).await;
    assert!(
        !process_is_gone(host_pid) && !process_is_gone(child),
        "premise: the host and its agent outlive the supervisor"
    );

    let (sup, client) = stack.supervisor(host_seams()).await;
    let host = agent_host(&sup, &session.id).await;
    assert_eq!(host.socket(), socket, "the same host");
    assert_eq!(host_process(&socket), Some(host_pid));
    assert_eq!(host.status().child_pid, child);
    assert_eq!(
        host.status().checkpoint,
        stopped.checkpoint,
        "recovered from the stop's snapshot"
    );
    wait_until_caught_up(&host).await;
    assert_eq!(host.screen_tail(4096), screen, "the same screen");
    let row = stored_row(stack.state.path(), &session.id).await;
    assert_eq!(row.outcome, LastOutcome::Running);
    assert_eq!(std::ffi::OsStr::new(&row.pane), socket.file_name().unwrap());
    assert_reloaded_as_running(&sup, &client, stack.state.path(), &session.id).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, stack.state.path()).await;
    let served = agent_host(&sup, &session.id).await;
    assert!(served == host, "serve's reload kept the adopted runtime");
    assert!(
        matches!(served.liveness(), Liveness::Running { child: Some(_) }),
        "{:?}",
        served.liveness()
    );
    type_line(&host, "after-restart").await;
    wait_for_screen(&host, "echo:after-restart").await;
    end_runtime(&host, host_pid).await;
    serving.stop().await;
}

/// After a crash, a restarted supervisor rebuilds a host agent's terminal
/// from the host's snapshot and the output after it: output an earlier
/// supervisor saw, and output written while none was attached.
///
/// Why: a crash (a kill, an OOM) gives no chance to checkpoint, and the
/// host has dropped everything before its last snapshot, so the snapshot
/// plus the tail is all there is to rebuild from (the plan's PR 12 replay,
/// reached through PR 17's reload). Spec: an agent prints a first line,
/// which a checkpoint then snapshots (premise: accepted), prints a second
/// line the supervisor sees, and the supervisor is dropped without an
/// orderly stop; with no supervisor attached the agent prints a third line
/// (premise: it says so in a file it writes afterwards). The next
/// supervisor publishes the same host, recovered from that snapshot, and
/// once caught up its screen shows all three lines; the row is still
/// `Running` after an observation pass and the session lists as `Unknown`
/// until sampled; and output after the recovery reaches the screen live.
#[farhelm_testtrace::test]
async fn a_crash_recovers_a_host_terminal_from_its_snapshot_and_tail() {
    let stack = Stack::new().await;
    // Each step waits for a file the test creates in the working directory,
    // which is the agent's working directory too.
    let agent = stack.work.path().join("agent.sh");
    std::fs::write(
        &agent,
        "printf 'IN-THE-SNAPSHOT\\r\\n'\n\
         while [ ! -e seen ]; do sleep 0.05; done\n\
         printf 'SEEN-BEFORE-THE-CRASH\\r\\n'\n\
         while [ ! -e away ]; do sleep 0.05; done\n\
         printf 'WRITTEN-WHILE-AWAY\\r\\n'\n\
         : > away-written\n\
         while [ ! -e live ]; do sleep 0.05; done\n\
         printf 'LIVE-AFTER-RECOVERY\\r\\n'\n\
         exec sleep 3600\n",
    )
    .expect("write the agent script");
    let command = format!(
        "/bin/sh {}",
        shell_words::quote(agent.to_str().expect("a UTF-8 path"))
    );
    let (sup, client) = stack.supervisor(host_seams()).await;
    let session = stack.create(&client, &command).await;
    let host = agent_host(&sup, &session.id).await;
    wait_for_screen(&host, "IN-THE-SNAPSHOT").await;
    let checkpoint = host
        .checkpoint_now(tokio::time::Instant::now() + Duration::from_secs(WAIT_SECS))
        .await
        .expect("the runtime runs");
    let CheckpointOutcome::Accepted(snapshot) = checkpoint else {
        panic!("premise: the host accepted a snapshot: {checkpoint:?}");
    };
    std::fs::write(stack.work.path().join("seen"), b"").unwrap();
    wait_for_screen(&host, "SEEN-BEFORE-THE-CRASH").await;
    let socket = host.socket().to_path_buf();
    let host_pid = host_process(&socket).expect("the host's process serves its socket");

    drop(host);
    retire(stack.state.path(), sup, client).await;
    std::fs::write(stack.work.path().join("away"), b"").unwrap();
    let written = stack.work.path().join("away-written");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(WAIT_SECS);
    while !written.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "premise: the agent wrote while no supervisor was attached"
        );
        // sleep-ok: polling interval for a file the session's own program writes.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let (sup, client) = stack.supervisor(host_seams()).await;
    let host = agent_host(&sup, &session.id).await;
    assert_eq!(host.socket(), socket, "the same host");
    assert_eq!(
        host.status().checkpoint,
        Some(snapshot),
        "recovered from the host's snapshot"
    );
    wait_until_caught_up(&host).await;
    for line in [
        "IN-THE-SNAPSHOT",
        "SEEN-BEFORE-THE-CRASH",
        "WRITTEN-WHILE-AWAY",
    ] {
        wait_for_screen(&host, line).await;
    }
    assert_eq!(
        stored_row(stack.state.path(), &session.id).await.outcome,
        LastOutcome::Running
    );
    assert_reloaded_as_running(&sup, &client, stack.state.path(), &session.id).await;
    std::fs::write(stack.work.path().join("live"), b"").unwrap();
    wait_for_screen(&host, "LIVE-AFTER-RECOVERY").await;
    end_runtime(&host, host_pid).await;
}

/// A create interrupted after its host started and before its launch was
/// confirmed is found again by the host's tag, and confirmed, by the next
/// supervisor.
///
/// Why: the plan's PR 17, "a host row with no terminal but a live host of
/// its launch is recovered". The row's `pane` is empty until the launch is
/// confirmed, so only the host's tag (session and launch generation) ties
/// the running agent to its session; without it, the session would wait
/// for a retry that may never come, its agent running unlisted. Spec: a
/// create of the fake agent interrupted during its launch leaves a
/// launching row with an empty `pane` and a host process serving the socket
/// its launch record names (premise). The next supervisor publishes that
/// host for the
/// session, showing the agent's ready marker, and records the launch: the
/// row is `Running` with the host's socket name in `pane`, also after an
/// observation pass, and the session lists as `Unknown` until sampled.
#[farhelm_testtrace::test]
async fn an_unconfirmed_host_launch_is_found_by_its_tag() {
    let stack = Stack::new().await;
    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            faults: FaultHooks {
                create_crash: Some(crate::boot_id_durable_outcome::crash_at(
                    CreateStage::DuringLaunch,
                )),
                ..FaultHooks::default()
            },
            ..host_seams()
        })
        .await;
    client
        .create_session(
            &stack.work.path().to_string_lossy(),
            &fixture_cmd("fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect_err("the create is interrupted before its confirmation");
    let row = SessionStore::open(&stack.state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .load_all()
        .await
        .unwrap()
        .pop()
        .expect("the launching row");
    assert_eq!(row.outcome, LastOutcome::Launching, "premise");
    assert!(row.pane.is_empty(), "premise: the launch was not confirmed");
    let socket = recorded_socket(stack.state.path(), &row.id);
    let host_pid = host_process(&socket).expect("premise: the interrupted launch's host runs");
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(host_seams()).await;
    let host = agent_host(&sup, &row.id).await;
    assert_eq!(host.socket(), socket, "the launch's own host");
    wait_for_screen(&host, AGENT_READY_MARKER).await;
    let recorded = stored_row(stack.state.path(), &row.id).await;
    assert_eq!(recorded.outcome, LastOutcome::Running);
    assert_eq!(
        std::ffi::OsStr::new(&recorded.pane),
        host.socket().file_name().unwrap()
    );
    assert_reloaded_as_running(&sup, &client, stack.state.path(), &row.id).await;
    end_runtime(&host, host_pid).await;
}

/// A host the restarted supervisor cannot read is left as it is: the
/// session lists as unknown, nothing is recorded, Stop refuses, and the
/// host and its agent keep running; once the host can be read again, the
/// same supervisor adopts it.
///
/// Why: the plan's D-16. A host may be unreadable for a reason that passes
/// (stopped, wedged, a loaded machine), or because a newer supervisor owns
/// it after a downgrade; treating it as gone would record a running agent
/// as exited and, once Delete and the startup reclaim end hosts, kill it.
/// And one that was only briefly unreadable must not stay unusable until
/// the supervisor's next restart, which can be weeks away. Spec: a created
/// host agent's host is stopped (SIGSTOP) while no supervisor runs, so it
/// cannot answer. The next supervisor, with short connection deadlines,
/// publishes a host terminal for the session that is unavailable and names
/// the host's socket; the session lists as unknown; after an observation
/// pass the row is still `Running`; Stop is refused for the unavailable
/// host; and the host process and its agent still exist. Once the host is
/// continued, the same supervisor publishes a runtime driving it, with the
/// agent's screen, within 60 s.
#[farhelm_testtrace::test]
async fn an_unreadable_host_is_left_alone_and_adopted_once_readable() {
    let stack = Stack::new().await;
    let (sup, client) = stack.supervisor(host_seams()).await;
    let session = stack
        .create(&client, &fixture_cmd("fake-agent --script basic"))
        .await;
    let host = agent_host(&sup, &session.id).await;
    wait_for_screen(&host, AGENT_READY_MARKER).await;
    let socket = host.socket().to_path_buf();
    let host_pid = host_process(&socket).expect("the host's process serves its socket");
    let child = host.status().child_pid;
    drop(host);
    retire(stack.state.path(), sup, client).await;
    let stopped = Stopped::stop(host_pid);

    let quick = ClientDeadlines {
        connect: Duration::from_secs(2),
        reply: Duration::from_secs(2),
        ..ClientDeadlines::default()
    };
    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            faults: FaultHooks {
                host_terminal_options: Some(Arc::new(move |options| options.deadlines = quick)),
                ..FaultHooks::default()
            },
            ..host_seams()
        })
        .await;
    let host = agent_host(&sup, &session.id).await;
    assert_eq!(host.socket(), socket, "the handle names the host's socket");
    assert!(
        matches!(host.liveness(), Liveness::Unavailable { .. }),
        "{:?}",
        host.liveness()
    );
    assert_eq!(
        listed(&client, &session.id).await.status,
        SessionStatus::Unknown
    );
    sup.reconcile_for_test().await;
    assert_eq!(
        stored_row(stack.state.path(), &session.id).await.outcome,
        LastOutcome::Running,
        "nothing is recorded for an unreadable host"
    );
    let refused = client
        .stop_session(&session.id)
        .await
        .expect_err("Stop refuses a host it cannot read");
    assert!(
        format!("{refused:#}").contains("unavailable"),
        "refused for the unavailable host: {refused:#}"
    );
    assert!(
        !process_is_gone(host_pid) && !process_is_gone(child),
        "the host and its agent are left alone"
    );
    drop(host);

    drop(stopped);
    let host = wait_for_driven_host(&sup, &session.id, 60).await;
    assert_eq!(host.socket(), socket, "the same host, adopted again");
    wait_for_screen(&host, AGENT_READY_MARKER).await;
    assert_eq!(
        stored_row(stack.state.path(), &session.id).await.outcome,
        LastOutcome::Running
    );
    end_runtime(&host, host_pid).await;
}

/// An agent that exits while no supervisor is attached has its exit, with
/// its code, recorded by the next supervisor once its runtime has replayed
/// the host's exit record.
///
/// Why: the host reports the exit after the agent's last output, and a
/// restarted supervisor learns of it only by replaying the journal; reload
/// does not wait for that, so the session is first taken for running, and
/// the exit must still be recorded, with the code the host kept, not as an
/// exit of unknown code. Spec: an agent that waits for a file and then
/// exits with code 7 is created; its supervisor is dropped; the file is
/// created and the agent exits (premise: its process is gone) while no
/// supervisor runs. The next supervisor publishes the host for the session,
/// whose runtime reports the exit once caught up, with the agent's last
/// line on screen; an observation pass then records `Exited` with code 7,
/// and the host keeps running with the final screen.
#[farhelm_testtrace::test]
async fn an_exit_while_no_supervisor_was_attached_is_recorded_with_its_code() {
    let stack = Stack::new().await;
    let agent = stack.work.path().join("agent.sh");
    std::fs::write(
        &agent,
        "printf 'AGENT-RUNNING\\r\\n'\n\
         while [ ! -e finish ]; do sleep 0.05; done\n\
         printf 'AGENT-FINISHING\\r\\n'\n\
         exit 7\n",
    )
    .expect("write the agent script");
    let command = format!(
        "/bin/sh {}",
        shell_words::quote(agent.to_str().expect("a UTF-8 path"))
    );
    let (sup, client) = stack.supervisor(host_seams()).await;
    let session = stack.create(&client, &command).await;
    let host = agent_host(&sup, &session.id).await;
    wait_for_screen(&host, "AGENT-RUNNING").await;
    let socket = host.socket().to_path_buf();
    let host_pid = host_process(&socket).expect("the host's process serves its socket");
    let child = host.status().child_pid;
    drop(host);
    retire(stack.state.path(), sup, client).await;
    std::fs::write(stack.work.path().join("finish"), b"").unwrap();
    wait_until_pid_gone(child, WAIT_SECS).await;

    let (sup, _client) = stack.supervisor(host_seams()).await;
    let host = agent_host(&sup, &session.id).await;
    assert_eq!(host.socket(), socket, "the same host");
    crate::host_terminals::wait_for_liveness(&host, "the exit record's replay", |liveness| {
        matches!(liveness, Liveness::Exited { .. })
    })
    .await;
    assert!(host.screen_tail(4096).contains("AGENT-FINISHING"));
    sup.reconcile_for_test().await;
    assert!(
        matches!(
            stored_row(stack.state.path(), &session.id).await.outcome,
            LastOutcome::Exited {
                exit_code: Some(7),
                ..
            }
        ),
        "{:?}",
        stored_row(stack.state.path(), &session.id).await.outcome
    );
    assert!(
        !process_is_gone(host_pid),
        "the host keeps the final screen"
    );
    end_runtime(&host, host_pid).await;
}

/// A host that was killed while no supervisor was attached is absent to the
/// next supervisor: its running session is recorded as exited, with no
/// code, and holds no terminal.
///
/// Why: this is the only way reload records an exit for a host row, and
/// the evidence must be proof, not a guess: a socket nothing serves (a
/// killed host leaves its socket file behind) whose name was drawn for
/// this launch alone. Spec: a created host agent's host process is killed
/// (SIGKILL) while no supervisor runs (premise: the process is gone, its
/// socket file stays). The next supervisor publishes no host for the
/// session, and the row is `Exited` with no code and no annotation.
#[farhelm_testtrace::test]
async fn a_killed_host_is_absent_and_its_session_recorded_as_exited() {
    let stack = Stack::new().await;
    let (sup, client) = stack.supervisor(host_seams()).await;
    let session = stack
        .create(&client, &fixture_cmd("fake-agent --script basic"))
        .await;
    let host = agent_host(&sup, &session.id).await;
    wait_for_screen(&host, AGENT_READY_MARKER).await;
    let socket = host.socket().to_path_buf();
    let host_pid = host_process(&socket).expect("the host's process serves its socket");
    drop(host);
    retire(stack.state.path(), sup, client).await;
    // SAFETY: `kill` has no memory-safety preconditions; the pid is the
    // host this test's supervisor started, found by its socket.
    assert_eq!(unsafe { libc::kill(host_pid as i32, libc::SIGKILL) }, 0);
    wait_until_pid_gone(host_pid, WAIT_SECS).await;
    assert!(
        socket.exists(),
        "premise: the killed host left its socket file"
    );

    let (sup, _client) = stack.supervisor(host_seams()).await;
    assert!(
        sup.agent_host_for_test(&session.id).await.is_none(),
        "no host for the session"
    );
    assert_eq!(
        stored_row(stack.state.path(), &session.id).await.outcome,
        LastOutcome::Exited {
            exit_code: None,
            annotation: None,
        }
    );
}

/// After an orderly stop whose checkpoint did not finish in its budget, the
/// next supervisor recovers the terminal from the host's previous snapshot
/// and the output after it.
///
/// Why: the bounded stop gives up on an encode that runs past its deadline
/// (it cancels it, and the host keeps what it had), and its promise is
/// that this costs a longer replay, never the terminal. Spec: an agent
/// prints a first line, which a checkpoint snapshots (premise: accepted);
/// it then prints a second line; every later encode is held at a gate, and
/// the stop's checkpoints, with a 500 ms budget, report the host left
/// without a fresh snapshot: the held encode was released by its cancel
/// and the runtime ended, its last known snapshot still the first one. The
/// next supervisor publishes the same host, recovered from that snapshot,
/// and once caught up shows both lines; output after the recovery reaches
/// the screen live.
#[farhelm_testtrace::test]
async fn a_stop_that_ran_out_of_time_leaves_the_previous_snapshot_and_tail() {
    use farhelm_supervisor::hosts::terminal::EncodeGate;
    let stack = Stack::new().await;
    let agent = stack.work.path().join("agent.sh");
    std::fs::write(
        &agent,
        "printf 'IN-THE-SNAPSHOT\\r\\n'\n\
         while [ ! -e tail ]; do sleep 0.05; done\n\
         printf 'IN-THE-TAIL\\r\\n'\n\
         while [ ! -e live ]; do sleep 0.05; done\n\
         printf 'LIVE-AFTER-RECOVERY\\r\\n'\n\
         exec sleep 3600\n",
    )
    .expect("write the agent script");
    let command = format!(
        "/bin/sh {}",
        shell_words::quote(agent.to_str().expect("a UTF-8 path"))
    );
    let gate = EncodeGate::closed();
    gate.open();
    let held = gate.clone();
    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            faults: FaultHooks {
                host_terminal_options: Some(Arc::new(move |options| {
                    options.encode_gate = Some(held.clone());
                })),
                ..FaultHooks::default()
            },
            ..host_seams()
        })
        .await;
    let session = stack.create(&client, &command).await;
    let host = agent_host(&sup, &session.id).await;
    wait_for_screen(&host, "IN-THE-SNAPSHOT").await;
    let checkpoint = host
        .checkpoint_now(tokio::time::Instant::now() + Duration::from_secs(WAIT_SECS))
        .await
        .expect("the runtime runs");
    let CheckpointOutcome::Accepted(snapshot) = checkpoint else {
        panic!("premise: the host accepted a snapshot: {checkpoint:?}");
    };
    std::fs::write(stack.work.path().join("tail"), b"").unwrap();
    wait_for_screen(&host, "IN-THE-TAIL").await;
    let socket = host.socket().to_path_buf();
    let host_pid = host_process(&socket).expect("the host's process serves its socket");

    gate.close();
    let current = sup
        .checkpoint_hosts_for_test(Duration::from_millis(500))
        .await;
    assert!(!current, "the held encode left no fresh snapshot");
    assert!(
        gate.cancelled(),
        "the held encode was released by its cancel"
    );
    assert!(!gate.expired());
    wait_until_runtime_ended(&host).await;
    assert_eq!(host.status().checkpoint, Some(snapshot));
    drop(host);
    retire(stack.state.path(), sup, client).await;

    let (sup, client) = stack.supervisor(host_seams()).await;
    let host = agent_host(&sup, &session.id).await;
    assert_eq!(host.socket(), socket, "the same host");
    assert_eq!(
        host.status().checkpoint,
        Some(snapshot),
        "recovered from the previous snapshot"
    );
    wait_until_caught_up(&host).await;
    for line in ["IN-THE-SNAPSHOT", "IN-THE-TAIL"] {
        wait_for_screen(&host, line).await;
    }
    assert_reloaded_as_running(&sup, &client, stack.state.path(), &session.id).await;
    std::fs::write(stack.work.path().join("live"), b"").unwrap();
    wait_for_screen(&host, "LIVE-AFTER-RECOVERY").await;
    end_runtime(&host, host_pid).await;
}

/// A create interrupted before its confirmation, whose agent never started
/// (its login shell failed before the launch step read its spec), ends as a
/// launch error once the next supervisor has replayed the host's exit.
///
/// Why: for a tmux pane reload classifies a dead pane's launch itself; for
/// a host, reload confirms the launch it finds by the host's tag before the
/// exit record is replayed, and the never-started classification then
/// falls to the observer. Without it the session would read as an
/// ordinary exit of an agent that never ran. Spec: with a login shell that
/// exits at once without running anything, a create interrupted during its
/// launch leaves a launching row with an empty `pane` and a host serving
/// the socket its launch record names (premise), whose child exits while no
/// supervisor runs. The next supervisor publishes that host, whose runtime
/// reports the exit once replayed; after an observation pass the row is an
/// `Error` whose detail says the agent was never started (the
/// never-started classification, not some other error).
#[farhelm_testtrace::test]
async fn an_unconfirmed_launch_that_never_started_ends_as_an_error() {
    let stack = Stack::new().await;
    let (sup, client) = stack
        .supervisor(SupervisorSeams {
            launch_shell: Some("/bin/false".to_string()),
            faults: FaultHooks {
                create_crash: Some(crate::boot_id_durable_outcome::crash_at(
                    CreateStage::DuringLaunch,
                )),
                ..FaultHooks::default()
            },
            ..host_seams()
        })
        .await;
    client
        .create_session(
            &stack.work.path().to_string_lossy(),
            &fixture_cmd("fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect_err("the create is interrupted before its confirmation");
    let row = SessionStore::open(&stack.state.path().join("supervisor.db"), false)
        .await
        .unwrap()
        .load_all()
        .await
        .unwrap()
        .pop()
        .expect("the launching row");
    assert_eq!(row.outcome, LastOutcome::Launching, "premise");
    assert!(row.pane.is_empty(), "premise: the launch was not confirmed");
    let socket = recorded_socket(stack.state.path(), &row.id);
    let host_pid = host_process(&socket).expect("premise: the interrupted launch's host runs");
    retire(stack.state.path(), sup, client).await;

    let (sup, _client) = stack.supervisor(host_seams()).await;
    let host = agent_host(&sup, &row.id).await;
    assert_eq!(host.socket(), socket, "the launch's own host");
    crate::host_terminals::wait_for_liveness(&host, "the exit record's replay", |liveness| {
        matches!(liveness, Liveness::Exited { .. })
    })
    .await;
    sup.reconcile_for_test().await;
    let outcome = stored_row(stack.state.path(), &row.id).await.outcome;
    assert!(
        matches!(&outcome, LastOutcome::Error { detail } if detail.contains("the agent was never started")),
        "the launch that never started is classified so: {outcome:?}"
    );
    end_runtime(&host, host_pid).await;
}

/// A create that publishes its host after an orderly stop has begun
/// gathering the host terminals it closes publishes a placeholder instead:
/// its runtime is stopped (cancelling its held encode), its launch stays
/// recorded, and the tokio runtime that owns the supervisor drops at once.
///
/// Why: the stop closes and cancels only the runtimes it gathers, and a
/// runtime published after that would keep its host's connection and could
/// run an encode nothing cancels, holding up the supervisor's exit and the
/// release of its state directory (the review's finding on creates at a
/// stop). The host is left to the next supervisor, which adopts it by its
/// tag. Spec: on a runtime the test owns, a create whose new host's every
/// encode is held at a gate (a checkpoint is due after any output) is held
/// before its publication once an encode has reached the gate (premise);
/// the stop's checkpoints then run (with nothing published, nothing to
/// checkpoint), and the create is released: it completes, the session's
/// entry holds a placeholder, the held encode was released by its cancel
/// (not opened, not run out), the row is `Running`, and the host process
/// still runs. Dropping the runtime then takes under 2 s (the gate's own
/// limit is 30 s).
#[farhelm_testtrace::test]
fn a_create_published_after_the_stop_began_is_closed_not_installed() {
    use farhelm_supervisor::hosts::terminal::EncodeGate;
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let gate = EncodeGate::closed();
    let (stack, socket) = runtime.block_on(async {
        let stack = Stack::new().await;
        let (held_tx, held_rx) = std::sync::mpsc::channel::<()>();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let held_tx = std::sync::Mutex::new(held_tx);
        let release_rx = std::sync::Mutex::new(release_rx);
        let encoding = gate.clone();
        let seam: CreateCrashSeam = Arc::new(move |stage| {
            if stage != CreateStage::DuringLaunch {
                return Ok(());
            }
            tokio::task::block_in_place(|| {
                anyhow::ensure!(
                    encoding.wait_entered(Duration::from_secs(WAIT_SECS)),
                    "premise: no encode reached the gate"
                );
                let _ = held_tx.lock().unwrap().send(());
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(WAIT_SECS))?;
                Ok(())
            })
        });
        let held = gate.clone();
        let (sup, client) = stack
            .supervisor(SupervisorSeams {
                faults: FaultHooks {
                    create_crash: Some(seam),
                    host_terminal_options: Some(Arc::new(move |options| {
                        options.encode_gate = Some(held.clone());
                        options.checkpoints = farhelm_supervisor::hosts::CheckpointPolicy {
                            min_bytes: 1,
                            max_bytes: 1,
                        };
                    })),
                    ..FaultHooks::default()
                },
                ..host_seams()
            })
            .await;
        let creating = {
            let client = Arc::clone(&client);
            let work = stack.work.path().to_string_lossy().into_owned();
            tokio::spawn(async move {
                client
                    .create_session(
                        &work,
                        "printf 'output for a checkpoint\\n' && exec sleep 600",
                        None,
                        80,
                        24,
                    )
                    .await
            })
        };
        tokio::task::spawn_blocking(move || {
            held_rx.recv_timeout(Duration::from_secs(WAIT_SECS * 2))
        })
        .await
        .expect("the wait")
        .expect("premise: the create is held before publication, its encode held");

        assert!(
            sup.checkpoint_hosts_for_test(Duration::from_millis(500))
                .await,
            "nothing was published to checkpoint"
        );
        release_tx.send(()).expect("release the create");
        let session = creating
            .await
            .expect("the create's task")
            .expect("the create completes");
        let host = agent_host(&sup, &session.id).await;
        assert!(
            host.is_placeholder(),
            "no runtime was installed after the stop's gather"
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(WAIT_SECS);
        while !gate.passed() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the held encode was never released"
            );
            // sleep-ok: polling interval for the encode's thread, which publishes no event.
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(gate.cancelled(), "released by the runtime's cancel");
        assert!(!gate.expired());
        assert_eq!(
            stored_row(stack.state.path(), &session.id).await.outcome,
            LastOutcome::Running,
            "the launch stays recorded"
        );
        let socket = host.socket().to_path_buf();
        assert!(
            host_process(&socket).is_some(),
            "the host is left running for the next supervisor"
        );
        drop(host);
        retire(stack.state.path(), sup, client).await;
        (stack, socket)
    });
    let dropping = std::time::Instant::now();
    drop(runtime);
    assert!(
        dropping.elapsed() < Duration::from_secs(2),
        "no encode outlived the stop: the runtime took {:?} to drop",
        dropping.elapsed()
    );
    let cleanup = tokio::runtime::Runtime::new().expect("a runtime");
    let ended = cleanup.block_on(farhelm_supervisor::hosts::end_host_at(
        &socket,
        farhelm_supervisor::hosts::StopLimits::default(),
    ));
    assert!(
        matches!(ended, farhelm_supervisor::hosts::HostOutcome::Stopped(_)),
        "the test's host ended: {ended:?}"
    );
    drop(stack);
}
