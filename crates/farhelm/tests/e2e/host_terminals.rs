//! The supervisor's terminal switch points with a real terminal host behind
//! a session's agent: exit recording, Stop, Restart's stop first, Delete (its
//! guard, its process roots and ending the session's hosts) and attach. What
//! Restart's new host keeps, and how a failed or interrupted one is unwound,
//! is in `host_relaunches.rs`.
//!
//! Each test here wants a host child of its own choosing rather than an
//! agent's launch chain, so it creates an ordinary session (in a host, as
//! every harness's sessions are) and gives its agent a host it started
//! itself (`Supervisor::install_host_agent_for_test`, which adopts the
//! host, records the row as a host row naming it and shuts the created
//! host down); what a create puts in its host is tested in
//! `host_creates.rs`, which also uses the helpers shared from here. The
//! host is started from the real `farhelm` binary by the supervisor's own
//! spawn (`farhelm_supervisor::hosts::spawn_host`), which runs the binary
//! first, so the host's own fork never happens inside this test process. The
//! host lives in the harness's state directory, whose drop ends it if the
//! test did not; each test also ends its host through the runtime once it
//! is done, so the supervisor's connection to it is not left to fight the
//! cleanup's.
//!
//! The host's child is a shell script with no session markers in its
//! environment, which is what makes the root tests mean something: Stop's
//! and Delete's marker sweep cannot find its processes, so only a sweep
//! rooted at the host's child reaches them.
//!
//! Delete's tests also start hosts that no runtime holds, tagged as a
//! session's agent of some launch generation and named by a launch record
//! the test writes, the way a host left behind by a crash looks to Delete,
//! which acts only on what the session records. Tab hosts come with the
//! change that opens tabs in hosts.
//!
//! What needs a host to behave on cue (status, the never-started excerpt,
//! Stop before the host's exit record, an unreadable host, the ticker's
//! wake and a failed tmux probe, the hook anchor, the hold wait, prompts) is
//! tested against a stand-in host in the supervisor's own unit tests
//! (`service::core::tests::host_terminals`).

use crate::harness::*;
use farhelm_supervisor::hosts::{
    ChildSpec, HostConnection, HostRole, HostTag, HostTerminal, InputFate, Liveness, spawn_host,
};
use std::path::{Path, PathBuf};
use terminal_host::protocol::Size;

/// How long a test waits for a process to go or a host to report a change.
pub(crate) const WAIT_SECS: u64 = 15;

/// A script that records its own pid and a background grandchild's in
/// `dir`, says it is ready, and then waits for the grandchild: a child with
/// a process tree under it and no session markers anywhere in it.
fn tree_script(dir: &Path) -> String {
    format!(
        "echo $$ > {child}; sleep 1000 & echo $! > {grand}; printf 'host child ready\\n'; wait",
        child = shell_words::quote(dir.join("child.pid").to_str().unwrap()),
        grand = shell_words::quote(dir.join("grand.pid").to_str().unwrap()),
    )
}

/// A host this test started and made a session's agent terminal: the
/// supervisor's runtime for it, and the host's own process, found
/// independently of the runtime.
struct Installed {
    host: HostTerminal,
    /// The host's pid, from the process table: the process serving the
    /// host's socket by its command line.
    pid: u32,
    socket: PathBuf,
}

/// A host this test started that no runtime holds: its socket, and its
/// process, found by its command line.
struct Started {
    socket: PathBuf,
    pid: u32,
}

/// Start a host in `h`'s state directory running `script` under `/bin/sh`,
/// with no session markers in its child's environment, tagged as
/// `session_id`'s `role`.
async fn start_host(h: &Harness, session_id: &str, role: HostRole, script: &str) -> Started {
    let child = ChildSpec {
        argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
        cwd: "/".into(),
        env: vec![("PATH".into(), "/usr/bin:/bin".into())],
        size: Size { cols: 80, rows: 24 },
        journal_cap: 1024 * 1024,
        initial_snapshot: None,
    };
    let tag = HostTag {
        session_id: session_id.to_string(),
        created_ns: farhelm_supervisor::hosts::tag::creation_stamp(),
        role,
    };
    let spawned = spawn_host(Path::new(farhelm_bin()), h.state.path(), child, &tag)
        .await
        .expect("the host starts");
    let pid = host_process(&spawned.socket).expect("the host's process serves its socket");
    Started {
        socket: spawned.socket,
        pid,
    }
}

/// Start a host running `script`, tagged as `session_id`'s agent, and make
/// it the session's agent terminal.
async fn install_host(h: &Harness, session_id: &str, script: &str) -> Installed {
    let Started { socket, pid } =
        start_host(h, session_id, HostRole::Agent { generation: 0 }, script).await;
    let host = h
        .sup
        .install_host_agent_for_test(session_id, &socket)
        .await
        .expect("the host becomes the session's agent terminal");
    Installed { host, pid, socket }
}

/// The process whose command line carries `--socket <socket>`, as a host's
/// own does, if one runs: an observation of the host that does not go
/// through the supervisor's runtime for it.
pub(crate) fn host_process(socket: &Path) -> Option<u32> {
    let socket = socket.as_os_str().as_encoded_bytes();
    std::fs::read_dir("/proc")
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
        .find(|pid| {
            std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|raw| {
                let args: Vec<&[u8]> = raw.split(|byte| *byte == 0).collect();
                args.windows(2)
                    .any(|pair| pair[0] == b"--socket" && pair[1] == socket)
            })
        })
}

/// Wait until `host`'s screen shows `text`.
pub(crate) async fn wait_for_screen(host: &HostTerminal, text: &str) {
    let mut status = host.subscribe();
    let waited = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while !host.screen_tail(4096).contains(text) {
            if status.changed().await.is_err() {
                return host.screen_tail(4096).contains(text);
            }
        }
        true
    })
    .await;
    assert_eq!(
        waited,
        Ok(true),
        "the screen never showed {text:?}: {:?}",
        host.screen_tail(4096)
    );
}

/// The session's durable row, read from the store the supervisor writes.
pub(crate) async fn stored_outcome(h: &Harness, session_id: &str) -> LastOutcome {
    SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open the store")
        .session(session_id)
        .await
        .expect("read the row")
        .expect("the row exists")
        .outcome
}

/// The pids [`tree_script`] recorded: its own, then its grandchild's.
async fn tree_pids(dir: &Path) -> (u32, u32) {
    let child = wait_for_pid_file(&dir.join("child.pid"), WAIT_SECS).await;
    let grand = wait_for_pid_file(&dir.join("grand.pid"), WAIT_SECS).await;
    assert!(
        !process_is_gone(child) && !process_is_gone(grand),
        "premise: the host's child {child} and its grandchild {grand} run"
    );
    (child, grand)
}

/// Wait until `host`'s liveness satisfies `condition`.
pub(crate) async fn wait_for_liveness(
    host: &HostTerminal,
    what: &str,
    condition: impl Fn(&Liveness) -> bool,
) {
    let mut status = host.subscribe();
    let waited = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while !condition(&host.liveness()) {
            if status.changed().await.is_err() {
                return condition(&host.liveness());
            }
        }
        true
    })
    .await;
    assert_eq!(
        waited,
        Ok(true),
        "{what} did not happen; liveness {:?}",
        host.liveness()
    );
}

/// The session as the list reports it.
pub(crate) async fn listed_info(h: &Harness, session_id: &str) -> SessionInfo {
    h.client
        .list_sessions()
        .await
        .expect("list")
        .sessions
        .into_iter()
        .find(|info| info.id == session_id)
        .expect("the session is listed")
}

/// End the test's host through its runtime, so that the harness's cleanup
/// does not race the runtime for the host's connection. The host must still
/// be running here (every test leaves it so), and must be gone after.
async fn end_host(installed: &Installed) {
    assert!(
        !process_is_gone(installed.pid),
        "premise: the host {} still runs",
        installed.pid
    );
    let report = tokio::time::timeout(Duration::from_secs(WAIT_SECS), installed.host.shutdown())
        .await
        .expect("the host's shutdown finishes in time")
        .expect("the runtime was still running the host");
    assert!(report.ended, "the host ended: {report:?}");
    wait_until_pid_gone(installed.pid, WAIT_SECS).await;
}

/// A host agent's exit is recorded only once its child exits, with the code
/// it exited with.
///
/// Why: the observer commits what host evidence says on every pass; a host
/// agent read like an absent pane would be durably recorded as exited
/// within one pass of its launch (the inventory's top risk 1). Spec: while
/// the host's child waits for a line, a reconciliation pass leaves the
/// stored outcome running and the session live; once the child has read it
/// and exited 7, the next pass stores the exit with code 7, and the session
/// lists so.
#[farhelm_testtrace::test]
async fn a_host_agents_exit_is_recorded_once_its_child_exits() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let installed = install_host(&h, &session.id, "printf 'waiting\\n'; read line; exit 7").await;
    let host = &installed.host;
    wait_for_screen(host, "waiting").await;

    h.sup.reconcile_for_test().await;
    assert_eq!(stored_outcome(&h, &session.id).await, LastOutcome::Running);
    let info = listed_info(&h, &session.id).await;
    assert!(info.status.is_live(), "running: {:?}", info.status);

    let fate = host
        .send_input(b"go\n")
        .await
        .expect("the runtime takes input")
        .fate()
        .await;
    assert_eq!(fate, InputFate::Written);
    wait_for_liveness(host, "the child's exit", |liveness| {
        matches!(liveness, Liveness::Exited { .. })
    })
    .await;
    h.sup.reconcile_for_test().await;
    assert_eq!(
        stored_outcome(&h, &session.id).await,
        LastOutcome::Exited {
            exit_code: Some(7),
            annotation: None
        }
    );
    assert_eq!(
        listed_info(&h, &session.id).await.status,
        SessionStatus::Exited { exit_code: Some(7) }
    );
    end_host(&installed).await;
}

/// Stop kills a host agent's whole process tree from its child, records a
/// stop, and leaves the host showing the final screen.
///
/// Why: a stop that read a host agent as already gone would record a plain
/// exit and kill only what carries the session's markers (the inventory's
/// top risk 9), and a stop that ended the host would take the final screen
/// with it (the plan's D-17). Spec: with the host's child and an unmarked
/// grandchild running and the child's marker on screen, Stop succeeds; both
/// processes go; the host's process still runs and the runtime reports the
/// child's exit rather than the host's end; the session lists as exited and
/// stopped by the user; and the screen still shows what the child printed.
#[farhelm_testtrace::test]
async fn stop_kills_a_host_agents_tree_and_leaves_the_host_showing_its_screen() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let installed = install_host(&h, &session.id, &tree_script(work.path())).await;
    let host = &installed.host;
    let (child, grand) = tree_pids(work.path()).await;
    wait_for_screen(host, "host child ready").await;

    h.client.stop_session(&session.id).await.expect("stop");
    wait_until_pid_gone(child, WAIT_SECS).await;
    wait_until_pid_gone(grand, WAIT_SECS).await;
    // The list reads the host's evidence, which says exited only once the
    // exit record (sent after the child's drained output) is processed.
    wait_for_liveness(host, "the host reporting the exit", |liveness| {
        matches!(liveness, Liveness::Exited { .. })
    })
    .await;
    assert!(
        !process_is_gone(installed.pid),
        "the host itself runs on after the stop"
    );
    let info = listed_info(&h, &session.id).await;
    assert!(
        matches!(info.status, SessionStatus::Exited { .. }),
        "{:?}",
        info.status
    );
    assert_eq!(info.annotation.as_deref(), Some("stopped by user"));
    assert!(
        host.screen_tail(4096).contains("host child ready"),
        "the final screen is kept: {:?}",
        host.screen_tail(4096)
    );
    assert!(
        matches!(host.liveness(), Liveness::Exited { .. }),
        "the host was not ended: {:?}",
        host.liveness()
    );
    end_host(&installed).await;
}

/// Restart of a running host agent stops the agent's whole tree before the
/// new host starts.
///
/// Why: a restart that read a host agent as gone, or started the new host
/// before the stop, would run two agents on one conversation, however
/// briefly (the inventory's top risk 6); and the agent's processes carry no
/// marker here, so only a stop rooted at the host's child reaches its
/// grandchild, which ignores the hangup that ending the old host gives. The
/// order is observed where it matters: the new host's start records, as it
/// begins, whether the old processes still run. Spec: with the host's child
/// and an unmarked grandchild that ignores SIGHUP running (checked first), a
/// consented Restart (under a supervisor that starts hosts through a wrapper
/// around the built `farhelm`) succeeds; the wrapper recorded, when the new
/// host's start began, both processes gone; the session's agent terminal is
/// a new host at another socket showing the resumed agent's ready marker;
/// the old host's process is gone; the row is running.
#[farhelm_testtrace::test]
async fn restart_of_a_host_agent_stops_its_tree_before_a_new_host_starts() {
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let at_start = fixture.path().join("at-start");
    let program = crate::host_creates::script(
        &fixture.path().join("farhelm"),
        &format!(
            "state=''\n\
             for file in {child} {grand}; do\n\
             \x20 if kill -0 \"$(cat \"$file\")\" 2>/dev/null; then state=\"$state alive\"; \
             else state=\"$state gone\"; fi\n\
             done\n\
             printf '%s\\n' \"$state\" >> {at_start}\n\
             exec {farhelm} \"$@\"",
            child = shell_words::quote(work.path().join("child.pid").to_str().unwrap()),
            grand = shell_words::quote(work.path().join("grand.pid").to_str().unwrap()),
            at_start = shell_words::quote(at_start.to_str().unwrap()),
            farhelm = shell_words::quote(farhelm_bin()),
        ),
    );
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            host_program: Some(program),
            ..SupervisorSeams::default()
        },
    )
    .await;
    let session = create_resumable_session(
        &h,
        &work.path().to_string_lossy(),
        &fixture_cmd("fake-agent --script basic"),
        80,
        24,
    )
    .await;
    wait_for_agent_ready(&h, &session.id).await;
    let installed = install_host(&h, &session.id, &hangup_immune_tree_script(work.path())).await;
    let tree = hangup_immune_tree(work.path()).await;
    // The create's own host started through the same wrapper and recorded a
    // line of its own; only the Restart's start is under test.
    std::fs::remove_file(&at_start).expect("the create's host start recorded its line");

    h.client
        .restart_session(&session.id, true)
        .await
        .expect("the restart succeeds");
    assert_eq!(
        std::fs::read_to_string(&at_start).expect("the new host's start recorded the old tree"),
        " gone gone\n",
        "the old child and grandchild were gone when the new host's start began"
    );
    assert!(process_is_gone(tree.child) && process_is_gone(tree.grand));
    let new = h
        .sup
        .agent_host_for_test(&session.id)
        .await
        .expect("the session's agent terminal is a host");
    assert_ne!(new.socket(), installed.socket, "a new host");
    wait_for_screen(&new, AGENT_READY_MARKER).await;
    wait_until_pid_gone(installed.pid, WAIT_SECS).await;
    assert_eq!(stored_outcome(&h, &session.id).await, LastOutcome::Running);
    let new_pid = host_process(new.socket()).expect("the new host's process");
    let report = tokio::time::timeout(Duration::from_secs(WAIT_SECS), new.shutdown())
        .await
        .expect("the new host's shutdown finishes in time")
        .expect("the runtime was still running the host");
    assert!(report.ended, "the new host ended: {report:?}");
    wait_until_pid_gone(new_pid, WAIT_SECS).await;
}

// ---- Delete --------------------------------------------------------------------------

/// A script like [`tree_script`] whose grandchild ignores the terminal's
/// hangup: what makes a Delete test tell a sweep rooted at the host's
/// child from the hangup that ending the host gives its pty's processes.
/// Ending a host closes its pty, and an ordinary background `sleep` dies of
/// that hangup with or without any root; this one dies only of a sweep
/// that reached it from the child. The grandchild publishes its pid only
/// once its trap is set, so a test that has read the pid can rely on the
/// disposition.
fn hangup_immune_tree_script(dir: &Path) -> String {
    format!(
        "echo $$ > {child}; \
         /bin/sh -c 'trap \"\" HUP; echo $$ > \"$1\"; exec sleep 1000' sh {grand} & \
         printf 'host child ready\\n'; wait",
        child = shell_words::quote(dir.join("child.pid").to_str().unwrap()),
        grand = shell_words::quote(dir.join("grand.pid").to_str().unwrap()),
    )
}

/// The processes [`hangup_immune_tree_script`] started, with a guard that
/// kills the grandchild should the test fail before something else does
/// (it outlives its host's hangup by design).
struct ImmuneTree {
    child: u32,
    grand: u32,
    _grand_guard: PidKillGuard,
}

/// Wait for [`hangup_immune_tree_script`]'s processes and check the
/// premise the Delete tests rest on: both run, the grandchild's parent is
/// the child, and the grandchild ignores `SIGHUP`.
async fn hangup_immune_tree(dir: &Path) -> ImmuneTree {
    let (child, grand) = tree_pids(dir).await;
    let guard = PidKillGuard::arm(grand);
    let status = std::fs::read_to_string(format!("/proc/{grand}/status"))
        .expect("read the grandchild's status");
    let field = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .map(str::trim)
            .unwrap_or_else(|| panic!("no {name} in the grandchild's status"))
            .to_string()
    };
    assert_eq!(
        field("PPid:"),
        child.to_string(),
        "premise: the grandchild is the host child's own child"
    );
    let ignored = u64::from_str_radix(&field("SigIgn:"), 16).expect("a signal mask");
    assert_ne!(
        ignored & (1 << (libc::SIGHUP - 1)),
        0,
        "premise: the grandchild ignores SIGHUP"
    );
    ImmuneTree {
        child,
        grand,
        _grand_guard: guard,
    }
}

/// Write `session_id`'s launch record for `generation`, naming `socket` and
/// a first stage that has already ended, as a create leaves it once its
/// host started (`service::core::host_launch`'s record format, version 1).
/// What makes a host started by the test the session's to Delete, which
/// acts only on what the session records.
fn write_launch_record(h: &Harness, session_id: &str, generation: i64, socket: &Path) {
    let mut ended = std::process::Command::new("true")
        .spawn()
        .expect("start a process to end");
    let pid = ended.id();
    ended.wait().expect("reap it");
    let launch = h.state.path().join("launch");
    std::fs::create_dir_all(&launch).expect("the launch directory");
    std::fs::write(
        launch.join(format!("{session_id}.{generation}.host")),
        format!(
            "farhelm launch host 1\nsocket {}\nstarter {pid} 0\n",
            socket.file_name().unwrap().to_str().unwrap()
        ),
    )
    .expect("write the launch record");
}

/// Stop `pid` with `SIGSTOP` and wait until it is stopped.
async fn stop_process(pid: u32) {
    let stopped = std::process::Command::new("kill")
        .args(["-STOP", &pid.to_string()])
        .status()
        .expect("run kill");
    assert!(stopped.success(), "send SIGSTOP to {pid}");
    wait_for_process_state(pid, 'T', WAIT_SECS).await;
}

/// Delete's guard refuses while a host agent runs, and Delete kills the
/// agent's whole tree from the host's child and then ends the host.
///
/// Why: a guard that read a running host agent as ended would let a guarded
/// delete kill it without the consent prompt (the inventory's top risk 8),
/// a delete without the child as its root would leave an unmarked
/// descendant running, and one that left the host running would leak it
/// for good, since nothing else ends a host whose session is gone (top
/// risk 7). The grandchild ignores the hangup ending the host gives it, so
/// only the sweep's root reaches it. Spec: with the host's child and an
/// unmarked grandchild that ignores SIGHUP running (premises checked), and
/// a launch record naming the host the runtime holds, a
/// delete guarded on the agent having ended is refused because the agent
/// is still running, and nothing dies; an unconditional delete succeeds,
/// both processes go, the host's process is gone when it returns and its
/// socket file with it, its runtime saw it end, and the session is no
/// longer listed.
#[farhelm_testtrace::test]
async fn delete_guards_roots_and_ends_a_host_agent() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let installed = install_host(&h, &session.id, &hangup_immune_tree_script(work.path())).await;
    // As a create leaves it: a record naming the very host the runtime
    // holds, which Delete must leave to the runtime rather than connect to.
    write_launch_record(&h, &session.id, 0, &installed.socket);
    let tree = hangup_immune_tree(work.path()).await;

    let refused = h
        .client
        .delete_session_with(&session.id, farhelm_proto::DeleteGuard::AgentEnded)
        .await
        .expect_err("the agent still runs");
    assert!(
        format!("{refused:#}").contains("agent is still running"),
        "{refused:#}"
    );
    assert!(
        !process_is_gone(tree.child)
            && !process_is_gone(tree.grand)
            && !process_is_gone(installed.pid),
        "a refused delete kills nothing"
    );

    h.client.delete_session(&session.id).await.expect("delete");
    assert!(
        process_is_gone(installed.pid),
        "the host is gone when Delete returns"
    );
    wait_until_pid_gone(tree.child, WAIT_SECS).await;
    wait_until_pid_gone(tree.grand, WAIT_SECS).await;
    assert!(
        !installed.socket.exists(),
        "the host was shut down, removing its socket"
    );
    wait_for_liveness(
        &installed.host,
        "the runtime seeing its host end",
        |liveness| matches!(liveness, Liveness::Gone),
    )
    .await;
    assert!(!is_listed(&h, &session.id).await, "the session is deleted");
}

/// Whether the list still reports `session_id`.
async fn is_listed(h: &Harness, session_id: &str) -> bool {
    h.client
        .list_sessions()
        .await
        .expect("list")
        .sessions
        .iter()
        .any(|info| info.id == session_id)
}

/// End a host no runtime holds through its protocol, after checking that it
/// still answers as the `session_id` host it was started as, and wait for
/// its process to go: how a test cleans up a host it expects Delete to have
/// left alone.
async fn end_unheld_host(started: &Started, session_id: &str) {
    let mut connection = HostConnection::connect(&started.socket)
        .await
        .expect("the host still answers");
    let info = connection.info().await.expect("the host reports its state");
    let tag = HostTag::decode(&info.tag).expect("the host's tag reads");
    assert_eq!(tag.session_id, session_id, "the host is the one started");
    connection
        .shutdown()
        .await
        .expect("the host takes Shutdown");
    wait_until_pid_gone(started.pid, WAIT_SECS).await;
}

/// Delete finds the host of a session that has no agent terminal by the
/// session's id, guards on it, and ends it with its whole tree.
///
/// Why: a session can be without a terminal while its agent's host runs on
/// (a restart gap, or a reload that has not found it yet); the host is then
/// held by no runtime, and only the session's launch record finds it. A
/// delete that missed it would leave a live agent and its host running with
/// no session to show them. Spec: with a host agent's child and an unmarked
/// grandchild that ignores SIGHUP running, the session's entry made
/// terminal-less and the runtime that held the host ended (premises
/// checked), a delete guarded on the agent having ended is refused because
/// the agent still runs in a terminal host, and nothing dies; an
/// unconditional delete succeeds, the host is gone when it returns, the
/// child and grandchild go, and the session is no longer listed.
#[farhelm_testtrace::test]
async fn delete_finds_the_host_of_a_session_without_a_terminal() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let Installed { host, pid, socket } =
        install_host(&h, &session.id, &hangup_immune_tree_script(work.path())).await;
    let tree = hangup_immune_tree(work.path()).await;
    write_launch_record(&h, &session.id, 0, &socket);
    h.sup
        .forget_agent_terminal_for_test(&session.id)
        .await
        .expect("forget the session's terminal");
    // The runtime ends once its last handle goes, closing its connection;
    // until then it would take the host back from the connection Delete
    // makes when it settles the session's launch record.
    let mut status = host.subscribe();
    drop(host);
    let ended = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while status.changed().await.is_ok() {}
    })
    .await;
    assert!(
        ended.is_ok(),
        "premise: the runtime ends without its handles"
    );
    assert!(!process_is_gone(pid), "premise: the host runs on");

    let refused = h
        .client
        .delete_session_with(&session.id, farhelm_proto::DeleteGuard::AgentEnded)
        .await
        .expect_err("the agent still runs in its host");
    assert!(
        format!("{refused:#}").contains("still running in a terminal host"),
        "{refused:#}"
    );
    assert!(
        !process_is_gone(tree.child) && !process_is_gone(tree.grand) && !process_is_gone(pid),
        "a refused delete kills nothing"
    );

    h.client.delete_session(&session.id).await.expect("delete");
    assert!(process_is_gone(pid), "the host is gone when Delete returns");
    wait_until_pid_gone(tree.child, WAIT_SECS).await;
    wait_until_pid_gone(tree.grand, WAIT_SECS).await;
    assert!(!socket.exists(), "the host removed its socket");
    assert!(!is_listed(&h, &session.id).await, "the session is deleted");
}

/// Delete ends a host agent that is stopped, and so cannot take `Shutdown`,
/// with the verified signal.
///
/// Why: a host that is wedged or stopped never answers its protocol, and
/// Delete is the last thing that would ever end it; leaving it would leak
/// the host for good, while reporting the session deleted. Spec: with the
/// host's child and an unmarked grandchild that ignores SIGHUP running and
/// the host's process stopped with SIGSTOP (premises checked), an
/// unconditional delete succeeds, the host's process is gone when it
/// returns, and the child and grandchild go.
#[farhelm_testtrace::test]
async fn delete_ends_a_stopped_host_agent_with_the_verified_signal() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let installed = install_host(&h, &session.id, &hangup_immune_tree_script(work.path())).await;
    let tree = hangup_immune_tree(work.path()).await;
    // Ends the host should the test fail with it stopped.
    let _guard = PidKillGuard::arm(installed.pid);
    stop_process(installed.pid).await;

    h.client.delete_session(&session.id).await.expect("delete");
    assert!(
        process_is_gone(installed.pid),
        "the host is gone when Delete returns"
    );
    wait_until_pid_gone(tree.child, WAIT_SECS).await;
    wait_until_pid_gone(tree.grand, WAIT_SECS).await;
    assert!(!is_listed(&h, &session.id).await, "the session is deleted");
}

/// Delete's guard sees a live agent in a recorded host of another launch
/// generation beside a recorded agent terminal that has ended, and Delete
/// ends that host while another session's host is never touched.
///
/// Why: Delete ends every recorded host of the session, of any generation
/// (a leftover a crash left between a restart's new launch and the old
/// host's end). A delete whose confirmation said the agent had ended must
/// not kill one still running there, which the recorded terminal's probe
/// alone cannot see; and only the session's own records may lead Delete to
/// a host, so another session's host in the same state directory is never
/// touched. Spec: with the session's own agent stopped, a host tagged and
/// recorded as the session's agent of generation 3 running a child and an
/// unmarked grandchild that ignores SIGHUP (premises checked), and a host
/// tagged and recorded as another session's agent: a delete guarded on the
/// agent having ended is refused because the agent still runs in a terminal
/// host, and nothing dies; an unconditional delete ends the leftover host
/// (gone when it returns), its child and its grandchild, and the other
/// session's host still runs and answers with its tag.
#[farhelm_testtrace::test]
async fn delete_ends_another_generations_recorded_host_and_no_other_sessions() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    h.client.stop_session(&session.id).await.expect("stop");
    wait_for_non_live_status(&h.client, &session.id, WAIT_SECS).await;
    let leftover = start_host(
        &h,
        &session.id,
        HostRole::Agent { generation: 3 },
        &hangup_immune_tree_script(work.path()),
    )
    .await;
    write_launch_record(&h, &session.id, 3, &leftover.socket);
    let tree = hangup_immune_tree(work.path()).await;
    let other_session = uuid::Uuid::new_v4().to_string();
    let other = start_host(
        &h,
        &other_session,
        HostRole::Agent { generation: 0 },
        "sleep 1000",
    )
    .await;
    write_launch_record(&h, &other_session, 0, &other.socket);

    let refused = h
        .client
        .delete_session_with(&session.id, farhelm_proto::DeleteGuard::AgentEnded)
        .await
        .expect_err("an earlier generation's agent still runs");
    assert!(
        format!("{refused:#}").contains("still running in a terminal host"),
        "{refused:#}"
    );
    assert!(
        !process_is_gone(tree.child)
            && !process_is_gone(tree.grand)
            && !process_is_gone(leftover.pid),
        "a refused delete kills nothing"
    );

    h.client.delete_session(&session.id).await.expect("delete");
    assert!(process_is_gone(leftover.pid), "the leftover host is gone");
    wait_until_pid_gone(tree.child, WAIT_SECS).await;
    wait_until_pid_gone(tree.grand, WAIT_SECS).await;
    assert!(!is_listed(&h, &session.id).await, "the session is deleted");
    assert!(
        !process_is_gone(other.pid),
        "another session's host is never touched"
    );
    end_unheld_host(&other, &other_session).await;
}

/// A recorded host of the session that is stopped makes a guarded Delete
/// refuse, and an unconditional Delete ends it with the verified signal.
///
/// Why: a stopped host answers nothing, so whether its program runs cannot
/// be told (a guarded Delete must not kill what nobody agreed to), and it is
/// still the session's by its launch record, so an unconditional Delete
/// must end it, by the process its connection named, or keep the session.
/// Spec: with the session's own agent stopped and a host tagged and
/// recorded as the session's agent of generation 2, stopped with SIGSTOP
/// (premise checked), a delete guarded on the agent having ended is refused
/// because the recorded host cannot be read, and the host is still there,
/// stopped; an unconditional delete succeeds and the host is gone when it
/// returns.
#[farhelm_testtrace::test]
async fn a_stopped_recorded_host_is_guarded_and_then_signalled() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    h.client.stop_session(&session.id).await.expect("stop");
    wait_for_non_live_status(&h.client, &session.id, WAIT_SECS).await;
    let leftover = start_host(
        &h,
        &session.id,
        HostRole::Agent { generation: 2 },
        "sleep 1000",
    )
    .await;
    write_launch_record(&h, &session.id, 2, &leftover.socket);
    // Ends the host should the test fail with it stopped.
    let _guard = PidKillGuard::arm(leftover.pid);
    stop_process(leftover.pid).await;

    let refused = h
        .client
        .delete_session_with(&session.id, farhelm_proto::DeleteGuard::AgentEnded)
        .await
        .expect_err("the stopped host cannot be read");
    assert!(
        format!("{refused:#}").contains("cannot be read"),
        "{refused:#}"
    );
    assert_eq!(
        process_state(leftover.pid),
        Some('T'),
        "a refused delete leaves the host as it was"
    );

    h.client.delete_session(&session.id).await.expect("delete");
    assert!(
        process_is_gone(leftover.pid),
        "the stopped host is gone when Delete returns"
    );
    assert!(!is_listed(&h, &session.id).await, "the session is deleted");
}

/// Delete ends the host a runtime adopted, by its process, after another
/// host was renamed over its socket and the runtime saw the replacement,
/// and neither shuts down nor signals that replacement.
///
/// Why: the runtime gave the replacement up rather than name it the host,
/// and so must Delete: one that connected to the socket again and shut down
/// whatever answers would end a process that is not this session's
/// terminal. The process the runtime adopted is still known, pinned at
/// adoption, and ending it (here: confirming it already ended) is what lets
/// Delete finish rather than keep the session until a restart. Spec: with
/// the session's agent in a host whose socket file is replaced by another
/// session's host (renamed over it) before the agent's host is killed with
/// SIGKILL, and the runtime reporting the socket replaced (premise
/// checked), an unconditional delete succeeds and the session is no longer
/// listed; the replacement still runs and answers at that socket with its
/// own tag.
#[farhelm_testtrace::test]
async fn delete_ends_the_adopted_host_and_never_one_that_replaced_it() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let installed = install_host(&h, &session.id, "printf 'ready\\n'; exec sleep 1000").await;
    let other_session = uuid::Uuid::new_v4().to_string();
    let replacement = start_host(
        &h,
        &other_session,
        HostRole::Agent { generation: 0 },
        "sleep 1000",
    )
    .await;
    let _replacement_guard = PidKillGuard::arm(replacement.pid);
    std::fs::rename(&replacement.socket, &installed.socket)
        .expect("put the replacement's socket in the agent host's place");
    let killed = std::process::Command::new("kill")
        .args(["-KILL", &installed.pid.to_string()])
        .status()
        .expect("run kill");
    assert!(killed.success(), "kill the agent's host");
    wait_for_liveness(
        &installed.host,
        "the runtime finding its host replaced",
        |liveness| {
            matches!(
                liveness,
                Liveness::Unavailable {
                    reason: farhelm_supervisor::hosts::Unavailability::Replaced,
                    ..
                }
            )
        },
    )
    .await;

    h.client.delete_session(&session.id).await.expect("delete");
    assert!(!is_listed(&h, &session.id).await, "the session is deleted");
    assert!(
        !process_is_gone(replacement.pid),
        "the replacement was neither shut down nor signalled"
    );
    end_unheld_host(
        &Started {
            socket: installed.socket.clone(),
            pid: replacement.pid,
        },
        &other_session,
    )
    .await;
}

/// A host agent attaches as a grid stream that shows the host child's
/// output.
///
/// Why: a viewer gets the terminal from the supervisor's emulator, fed by a
/// real host, and nothing else shows that the two meet end to end through a
/// real supervisor. Spec: the attach's first message is `hello`, a screen
/// row it sends reads `host child ready`, and `ReplayComplete` arrives; the
/// host's child runs on through it.
#[farhelm_testtrace::test]
async fn a_host_agent_attaches_as_a_grid() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let installed = install_host(&h, &session.id, &tree_script(work.path())).await;
    let (child, _) = tree_pids(work.path()).await;

    let (_channel, mut stream) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            farhelm_proto::TerminalSelector::default(),
            "",
        )
        .await
        .expect("a grid attach");
    let mut first = None;
    let mut ready_shown = false;
    let mut replay_complete = false;
    let waited = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        while !(ready_shown && replay_complete) {
            match stream.recv().await {
                Some(TermEvent::Data(bytes)) => {
                    let message =
                        farhelm_proto::grid::GridOutput::decode(&bytes).expect("a grid message");
                    first.get_or_insert_with(|| message.clone());
                    if let farhelm_proto::grid::GridOutput::Rows { rows, .. } = &message {
                        ready_shown |= rows.iter().any(|row| {
                            let text: String =
                                row.row.runs.iter().map(|run| run.text.as_str()).collect();
                            text.contains("host child ready")
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
        "ready shown {ready_shown}, replay complete {replay_complete}"
    );
    assert!(
        matches!(first, Some(farhelm_proto::grid::GridOutput::Hello { .. })),
        "{first:?}"
    );
    assert!(!process_is_gone(child), "the agent runs on");
    end_host(&installed).await;
}
