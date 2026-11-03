//! The supervisor's terminal switch points with a real terminal host behind
//! a session's agent: exit recording, Stop, Restart's refusal, Delete (its
//! guard and its process root) and attach.
//!
//! Each test here wants a host child of its own choosing rather than an
//! agent's launch chain, so it creates an ordinary session and moves its
//! agent into a host it started itself
//! (`Supervisor::install_host_agent_for_test`, which adopts the host,
//! records the row as a host row and kills the created tmux session); a
//! create that starts its agent in a host under the supervisor's internal
//! switch is tested in `host_creates.rs`, which also uses the helpers
//! shared from here. The
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
//! What needs a host to behave on cue (status, the never-started excerpt,
//! Stop before the host's exit record, an unreadable host, the ticker's
//! wake and a failed tmux probe, the hook anchor, the hold wait, prompts) is
//! tested against a stand-in host in the supervisor's own unit tests
//! (`service::core::tests::host_terminals`).

use crate::harness::*;
use farhelm_supervisor::hosts::{
    ChildSpec, HostRole, HostTag, HostTerminal, InputFate, Liveness, spawn_host,
};
use std::path::Path;
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
}

/// Start a host in `h`'s state directory running `script` under `/bin/sh`,
/// tagged as `session_id`'s agent, and make it the session's agent terminal.
async fn install_host(h: &Harness, session_id: &str, script: &str) -> Installed {
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
        role: HostRole::Agent { generation: 0 },
    };
    let spawned = spawn_host(Path::new(farhelm_bin()), h.state.path(), child, &tag)
        .await
        .expect("the host starts");
    let pid = host_process(&spawned.socket).expect("the host's process serves its socket");
    let host = h
        .sup
        .install_host_agent_for_test(session_id, &spawned.socket)
        .await
        .expect("the host becomes the session's agent terminal");
    Installed { host, pid }
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

/// Restart of a host agent is refused before anything is stopped or
/// changed.
///
/// Why: a restart that read a host agent as gone would relaunch beside it,
/// two agents on one conversation (the inventory's top risk 6); and a host
/// runs one child, so the agent can only restart into a new host, which
/// this supervisor cannot start yet. A refusal that stopped the agent first
/// would leave the user a stopped session they never asked for. Spec: with
/// the host's child and an unmarked grandchild running, a consented Restart
/// fails saying the agent runs in a terminal host and nothing was stopped;
/// both processes still run; the stored outcome is still running and the
/// session still lists as live.
#[farhelm_testtrace::test]
async fn restart_of_a_host_agent_is_refused_before_anything_stops() {
    let h = harness().await;
    let (session, work) = resumable_basic_session(&h).await;
    let installed = install_host(&h, &session.id, &tree_script(work.path())).await;
    let (child, grand) = tree_pids(work.path()).await;

    let refused = h
        .client
        .restart_session(&session.id, true)
        .await
        .expect_err("a host agent cannot be restarted into a new host yet");
    let refused = format!("{refused:#}");
    assert!(refused.contains("terminal host"), "{refused}");
    assert!(refused.contains("nothing was stopped"), "{refused}");
    assert!(
        !process_is_gone(child) && !process_is_gone(grand),
        "the agent's tree still runs"
    );
    assert_eq!(stored_outcome(&h, &session.id).await, LastOutcome::Running);
    let info = listed_info(&h, &session.id).await;
    assert!(info.status.is_live(), "{:?}", info.status);
    end_host(&installed).await;
}

/// Delete's guard refuses while a host agent runs, and Delete kills the
/// agent's whole tree from the host's child.
///
/// Why: a guard that read a running host agent as ended would let a guarded
/// delete kill it without the consent prompt (the inventory's top risk 8),
/// and a delete without the child as its root would leave an unmarked
/// descendant running (top risk 7). Spec: with the host's child and an
/// unmarked grandchild running, a delete guarded on the agent having ended
/// is refused because the agent is still running, and nothing dies; an
/// unconditional delete succeeds, both processes go, and the session is no
/// longer listed.
#[farhelm_testtrace::test]
async fn delete_guards_and_roots_a_host_agent() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let installed = install_host(&h, &session.id, &tree_script(work.path())).await;
    let (child, grand) = tree_pids(work.path()).await;

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
        !process_is_gone(child) && !process_is_gone(grand),
        "a refused delete kills nothing"
    );

    h.client.delete_session(&session.id).await.expect("delete");
    wait_until_pid_gone(child, WAIT_SECS).await;
    wait_until_pid_gone(grand, WAIT_SECS).await;
    let listing = h.client.list_sessions().await.expect("list");
    assert!(
        listing.sessions.iter().all(|info| info.id != session.id),
        "the session is deleted"
    );
    // Ending the host itself is a later step of Delete's; until then it is
    // left running, as here.
    end_host(&installed).await;
}

/// Attaching a host agent is refused, with the grid client's refusal for a
/// client that cannot show grids.
///
/// Why: a host terminal has no byte stream; sending a bytes-only client
/// anything would show it nothing it could draw, and the refusal is how a
/// viewer learns why. Spec: a bytes-only attach is refused with the
/// supervisor's permanent "needs a client that can show grid terminals"
/// message, a grid-capable one with "cannot stream it as a grid yet"; the
/// host's child runs on through both.
#[farhelm_testtrace::test]
async fn attaching_a_host_agent_is_refused() {
    let h = harness().await;
    let (session, work) = basic_session(&h).await;
    let installed = install_host(&h, &session.id, &tree_script(work.path())).await;
    let (child, _) = tree_pids(work.path()).await;

    let Err(refused) = h.client.attach(&session.id, 80, 24).await else {
        panic!("a bytes-only client cannot attach a host terminal");
    };
    assert!(
        format!("{refused:#}").contains("needs a client that can show grid terminals"),
        "{refused:#}"
    );
    let Err(refused) = h
        .client
        .attach_terminal(
            &session.id,
            80,
            24,
            farhelm_proto::TerminalSelector::default(),
            "",
            farhelm_helm::ViewerStreams::BytesOrGrid,
        )
        .await
    else {
        panic!("no grid stream yet");
    };
    assert!(
        format!("{refused:#}").contains("cannot stream it as a grid yet"),
        "{refused:#}"
    );
    assert!(!process_is_gone(child), "the agent runs on");
    end_host(&installed).await;
}
