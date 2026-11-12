//! The startup reclaim of orphaned terminal hosts (the plan's PR 20,
//! `service::core::host_reclaim`): a serving supervisor ends the hosts whose
//! tags name a session nobody knows (no row, no reservation, no launch
//! record), or a tab of a known session nothing records; removes what hosts
//! that are gone left behind, records included; and leaves every other host
//! as it is: a known session's (a session only its launch records know, as
//! a database that went backwards leaves it, included), one this build
//! cannot read, a placeholder's (never even contacted), one behind a refused
//! socket directory, and every host when the database is missing or empty,
//! or when discovery runs out of time. A dead tab of an existing session
//! found after a restart goes as a dead tab does.
//!
//! Every supervisor here is the e2e suite's in-process one with the internal
//! switch on (`host_creates::host_seams`), and every host is started from the
//! built `farhelm` (a create's, a tab open's, or [`start_tagged_host`]'s),
//! never inside this test process. The reclaim runs in `serve`, which
//! `ServeTask::spawn` returns from only once the accept loop runs, so every
//! assertion after it sees the reclaim finished. A database that went
//! backwards is made the way it happens outside a test: the supervisor ends
//! without ending its hosts (dropped, as at a crash), and then the session's
//! row and reservations leave the database while no supervisor runs (the
//! store's own delete, as an older copy would lack them), or the database
//! files are removed. An orphan no database could know is a host the test
//! starts with a tag of its own and no record, as a leftover made by hand
//! would be.

use crate::harness::*;
use crate::host_creates::{counted_agent, host_processes_in, host_seams, interrupted_seams};
use crate::host_restarts::{Stack, Stopped, agent_host, retire};
use crate::host_tabs::{
    create, host_socket_names, tab_host, tab_record, tab_seams, wait_for_condition,
};
use crate::host_terminals::{WAIT_SECS, host_process};
use crate::terminal_tabs::listed_tabs;
use farhelm_supervisor::hosts::{
    ChildSpec, HostOutcome, HostRole, HostTag, Liveness, StopLimits, spawn_host,
};
use std::path::{Path, PathBuf};

// ---- Fixtures -------------------------------------------------------------------

/// The store of `state`, opened without the owner's authority, so that no
/// retention cleanup runs: what a test removes is exactly what it says.
async fn store_of(state: &Path) -> SessionStore {
    SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the store")
}

/// Make `session_id` unknown to the store while its launch records stay, as
/// an older copy of the database restored over this one would leave it: its
/// row and every reservation of it go.
async fn forget_session(state: &Path, session_id: &str) {
    store_of(state)
        .await
        .delete_session_settling_reservations(session_id)
        .await
        .expect("delete the session's row and reservations");
}

/// Start a host in `state`'s socket directory tagged for `session_id` in
/// `role`, with no launch record, running a shell that waits, and return its
/// socket: a host no record names, as an older build or a record removed by
/// hand would leave one.
async fn start_tagged_host(state: &Path, session_id: &str, role: HostRole) -> PathBuf {
    start_host_running(state, session_id, role, "exec sleep 1000", None).await
}

/// [`start_tagged_host`] running `script` under `/bin/sh -c`, with
/// `snapshot` as the host's initial snapshot (stored as given: a host never
/// reads it, so a blob that is not a snapshot makes a host whose terminal
/// this build cannot restore).
async fn start_host_running(
    state: &Path,
    session_id: &str,
    role: HostRole,
    script: &str,
    snapshot: Option<Vec<u8>>,
) -> PathBuf {
    let child = ChildSpec {
        argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
        cwd: "/".into(),
        env: vec![("PATH".into(), "/usr/bin:/bin".into())],
        size: terminal_host::protocol::Size { cols: 80, rows: 24 },
        journal_cap: 1024 * 1024,
        initial_snapshot: snapshot,
    };
    let tag = HostTag {
        session_id: session_id.to_string(),
        created_ns: farhelm_supervisor::hosts::tag::creation_stamp(),
        role,
    };
    spawn_host(Path::new(farhelm_bin()), state, child, &tag)
        .await
        .expect("the host starts")
        .socket
}

/// Start a host in `state`'s socket directory whose client tag is `tag`'s
/// raw bytes, which the supervisor's own spawn would never write, and return
/// its socket. The first stage is `farhelm internal terminal-host`, run from
/// the built binary as a child of this test, which forks the host and exits
/// once it is ready; the fork happens in that process, not in this one.
async fn start_raw_tagged_host(state: &Path, tag: &[u8]) -> PathBuf {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let dir = farhelm_supervisor::hosts::ensure_hosts_dir(state).expect("the socket directory");
    let socket = std::path::absolute(dir)
        .expect("an absolute socket directory")
        .join(
            farhelm_supervisor::hosts::HostName::random()
                .expect("a socket name")
                .as_str(),
        );
    let spec = terminal_host::protocol::encode_spawn(&terminal_host::protocol::Spawn {
        argv: vec![b"/bin/sleep".to_vec(), b"1000".to_vec()],
        cwd: b"/".to_vec(),
        env: vec![b"PATH=/usr/bin:/bin".to_vec()],
        size: terminal_host::protocol::Size { cols: 80, rows: 24 },
        journal_cap: 1024 * 1024,
        tag: tag.to_vec(),
        initial_snapshot: None,
    });
    let mut first = tokio::process::Command::new(farhelm_bin())
        .args(["internal", "terminal-host", "--socket"])
        .arg(&socket)
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("running farhelm internal terminal-host");
    let mut stdin = first.stdin.take().expect("stdin is piped");
    let mut stderr = first.stderr.take().expect("stderr is piped");
    let (status, message) = tokio::time::timeout(Duration::from_secs(WAIT_SECS), async {
        // A write error means the first stage stopped reading; its exit
        // status says why.
        let _ = stdin.write_all(&spec).await;
        drop(stdin);
        let mut message = Vec::new();
        let _ = (&mut stderr).take(4096).read_to_end(&mut message).await;
        (
            first.wait().await.expect("waiting for the first stage"),
            message,
        )
    })
    .await
    .expect("the raw-tagged host reported readiness in time");
    assert!(
        status.success(),
        "the raw-tagged host did not start ({status}): {}",
        String::from_utf8_lossy(&message)
    );
    socket
}

/// Write a launch record for `session_id`'s first agent launch naming
/// `socket` and a first stage that has already ended, as a create leaves one
/// once its host started (`service::core::host_launch`'s record format,
/// version 1).
fn write_agent_record(state: &Path, session_id: &str, socket: &Path) -> PathBuf {
    let mut ended = std::process::Command::new("true")
        .spawn()
        .expect("start a process to end");
    let pid = ended.id();
    ended.wait().expect("reap it");
    let launch = state.join("launch");
    std::fs::create_dir_all(&launch).expect("the launch directory");
    let record = agent_record(state, session_id);
    std::fs::write(
        &record,
        format!(
            "farhelm launch host 1\nsocket {}\nstarter {pid} 0\n",
            socket_name(socket)
        ),
    )
    .expect("write the launch record");
    record
}

/// A session that keeps the store from being empty: created by `client`'s
/// supervisor in `work`, so that the reclaim, which reclaims nothing for a
/// database that knows no session, runs at the next start.
async fn keeper(client: &SupervisorClient, work: &Path) -> SessionInfo {
    create(client, work).await
}

/// A session id no store has ever held.
fn unknown_session() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// The launch record of `session_id`'s first agent launch.
fn agent_record(state: &Path, session_id: &str) -> PathBuf {
    state.join("launch").join(format!("{session_id}.0.host"))
}

/// The first stage a launch record names, by pid.
fn recorded_starter(record: &Path) -> u32 {
    std::fs::read_to_string(record)
        .expect("the launch record")
        .lines()
        .find_map(|line| line.strip_prefix("starter "))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|pid| pid.parse().ok())
        .expect("the record names its first stage")
}

/// The socket `record` names, spelled absolute as the spawn spells it.
fn recorded_socket(state: &Path, record: &Path) -> PathBuf {
    let name = std::fs::read_to_string(record)
        .expect("the launch record")
        .lines()
        .find_map(|line| line.strip_prefix("socket ").map(str::to_string))
        .expect("the record names its socket");
    std::path::absolute(state.join("hosts").join(name)).expect("an absolute socket")
}

/// The file name of `socket`, as [`host_socket_names`] lists it.
fn socket_name(socket: &Path) -> String {
    socket
        .file_name()
        .expect("a socket path")
        .to_string_lossy()
        .into_owned()
}

/// End the host at `socket`, which no runtime holds, and require that it
/// ended: how a test ends the hosts the reclaim rightly left.
async fn end_left_host(socket: &Path) {
    let outcome = farhelm_supervisor::hosts::end_host_at(socket, StopLimits::default()).await;
    assert!(
        matches!(outcome, HostOutcome::Stopped(_)),
        "the test's host at {} ended: {outcome:?}",
        socket.display()
    );
}

// ---- Tests ----------------------------------------------------------------------

/// A recorded agent host and tab host of a session the database no longer
/// holds are left running, with their records, while a dead tab host of the
/// same session is cleared, and another session's runtime is never taken
/// over.
///
/// Why: a session the store does not know but whose launch records remain is
/// what a database that went backwards leaves (restored from an older copy,
/// replaced, or removed), not what any Delete leaves, since Delete keeps a
/// session's records and row until its hosts are confirmed ended. Ending
/// those hosts would destroy running terminals for good because of a
/// database accident; their records make their session known, so they are
/// left, while a recorded host that is already gone is still cleared. Spec:
/// a host agent's session with two host tabs, and a second session, are left
/// running by a supervisor dropped as at a crash; the first session's row and
/// reservations are then deleted (as an older database would lack them), and
/// one of its tab hosts is killed with SIGKILL (its socket file left). Once
/// the next supervisor serves, the first session's agent host and live tab
/// host still run at their sockets with their records; the killed tab's
/// record and socket file are gone; the second session's agent is still the
/// runtime the reload adopted, at its socket, with no reconnect.
#[farhelm_testtrace::test]
async fn a_recorded_host_of_a_session_missing_from_the_database_is_left() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let missing = create(&client, stack.work.path()).await;
    let live_tab = client.open_tab(&missing.id).await.expect("open a tab");
    let dead_tab = client.open_tab(&missing.id).await.expect("open a tab");
    let kept_work = farhelm_teststate::tempdir().expect("workdir");
    let kept = create(&client, kept_work.path()).await;
    let agent_socket = agent_host(&sup, &missing.id).await.socket().to_path_buf();
    let live_socket = tab_host(&sup, &missing.id, &live_tab.id)
        .await
        .socket()
        .to_path_buf();
    let dead_socket = tab_host(&sup, &missing.id, &dead_tab.id)
        .await
        .socket()
        .to_path_buf();
    let kept_socket = agent_host(&sup, &kept.id).await.socket().to_path_buf();
    let agent_pid = host_process(&agent_socket).expect("the agent's host runs");
    let live_pid = host_process(&live_socket).expect("the live tab's host runs");
    let dead_pid = host_process(&dead_socket).expect("the dead tab's host runs");
    retire(state, sup, client).await;
    forget_session(state, &missing.id).await;
    // SAFETY: ending a host this test's supervisor started, found by its
    // socket.
    assert_eq!(unsafe { libc::kill(dead_pid as i32, libc::SIGKILL) }, 0);
    wait_until_pid_gone(dead_pid, WAIT_SECS).await;
    let agent_record = agent_record(state, &missing.id);
    let live_record = tab_record(state, &missing.id, &live_tab.id);
    let dead_record = tab_record(state, &missing.id, &dead_tab.id);
    assert!(
        agent_record.exists() && live_record.exists() && dead_record.exists(),
        "premise: every host of the session is recorded"
    );
    assert!(
        host_socket_names(state).contains(&socket_name(&dead_socket)),
        "premise: the killed host's socket file is left"
    );

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    let sockets = host_socket_names(state);
    assert!(!process_is_gone(agent_pid), "the agent host still runs");
    assert!(!process_is_gone(live_pid), "the live tab's host still runs");
    assert!(
        sockets.contains(&socket_name(&agent_socket))
            && sockets.contains(&socket_name(&live_socket)),
        "their sockets are left: {sockets:?}"
    );
    assert!(
        agent_record.exists() && live_record.exists(),
        "their records are left"
    );
    assert!(!dead_record.exists(), "the dead tab's record is removed");
    assert!(
        !sockets.contains(&socket_name(&dead_socket)),
        "the dead tab's socket file is removed: {sockets:?}"
    );
    let kept_host = agent_host(&sup, &kept.id).await;
    assert_eq!(kept_host.socket(), kept_socket, "the same host");
    assert_eq!(
        kept_host.reconnects(),
        0,
        "nothing took the held host over from its runtime"
    );
    assert!(
        matches!(kept_host.liveness(), Liveness::Running { .. }),
        "{:?}",
        kept_host.liveness()
    );
    drop(kept_host);
    for socket in [&agent_socket, &live_socket] {
        end_left_host(socket).await;
    }
    client.delete_session(&kept.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// After the database went missing and a new session was made in its
/// replacement, a restart leaves the old database's hosts running.
///
/// Why: the empty-database guard holds only while the replacement database
/// is empty; the first session created in it would otherwise make every
/// host of the lost database an orphan at the next restart, ending them for
/// good. Their launch records, which live beside the database rather than in
/// it, keep their session known. Spec: a host agent's session with a host
/// tab, left running by a dropped supervisor; the session database (with
/// its journal files) is then removed; a supervisor starts, creates a
/// second session and is dropped; an unrecorded host tagged for a session no
/// store or record knows is started as a control. Once a third supervisor
/// serves, the control is gone (the reclaim ran), and the first session's
/// agent host and tab host still run, with their sockets and records.
#[farhelm_testtrace::test]
async fn a_missing_database_then_a_new_session_leaves_the_old_hosts() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let old = create(&client, stack.work.path()).await;
    let tab = client.open_tab(&old.id).await.expect("open a tab");
    let agent_socket = agent_host(&sup, &old.id).await.socket().to_path_buf();
    let tab_socket = tab_host(&sup, &old.id, &tab.id)
        .await
        .socket()
        .to_path_buf();
    let agent_pid = host_process(&agent_socket).expect("the agent's host runs");
    let tab_pid = host_process(&tab_socket).expect("the tab's host runs");
    retire(state, sup, client).await;
    for name in ["supervisor.db", "supervisor.db-wal", "supervisor.db-shm"] {
        match std::fs::remove_file(state.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("removing {name}: {error}"),
        }
    }

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let new_work = farhelm_teststate::tempdir().expect("workdir");
    let new = create(&client, new_work.path()).await;
    retire(state, sup, client).await;
    assert!(
        store_of(state)
            .await
            .session(&old.id)
            .await
            .unwrap()
            .is_none(),
        "premise: the new database does not know the old session"
    );

    let control =
        start_tagged_host(state, &unknown_session(), HostRole::Agent { generation: 0 }).await;
    let control_pid = host_process(&control).expect("the readable orphan runs");

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert!(
        process_is_gone(control_pid),
        "an orphan no database or record knows ended at the same start, so the reclaim ran"
    );
    let sockets = host_socket_names(state);
    for (what, pid, socket) in [
        ("the old agent host", agent_pid, &agent_socket),
        ("the old tab host", tab_pid, &tab_socket),
    ] {
        assert!(!process_is_gone(pid), "{what} still runs");
        assert!(
            sockets.contains(&socket_name(socket)),
            "{what}'s socket is left: {sockets:?}"
        );
    }
    assert!(
        agent_record(state, &old.id).exists(),
        "the agent's record is left"
    );
    assert!(
        tab_record(state, &old.id, &tab.id).exists(),
        "the tab's record is left"
    );
    for socket in [&agent_socket, &tab_socket] {
        end_left_host(socket).await;
    }
    client.delete_session(&new.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// Live hosts that no record names are ended at start when their tags name
/// a session nobody knows or a tab of a known session that nothing records,
/// and a socket file a killed host left is removed; the known session's own
/// processes run on.
///
/// Why: an older build's host, or one whose record was removed by hand, is
/// found only by scanning the socket directory, and a socket file nothing
/// serves costs every later scan its refusal window. A tab nothing records is
/// listed nowhere and reached by no Delete, so it is as lost as an unknown
/// session's (the approval's "no known session or tab"); ending it must not
/// reach anything else of the session. Spec: a store that knows one session
/// (a keeper, whose agent's program runs, premise checked); an agent host and
/// a tab host tagged for sessions no store or record names; a tab host tagged
/// for the keeper's session with a tab id no record names; and a fourth host
/// killed with SIGKILL (its socket file left). Once a supervisor serves
/// there, the three live hosts' processes are gone, of the sockets only the
/// keeper's is left, and the keeper's agent program still runs.
#[farhelm_testtrace::test]
async fn unrecorded_orphans_are_ended_and_a_stale_socket_removed_at_start() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let kept = keeper(&client, stack.work.path()).await;
    let kept_host = agent_host(&sup, &kept.id).await;
    let kept_socket = kept_host.socket().to_path_buf();
    let kept_program = match kept_host.status().liveness {
        Liveness::Running { child: Some(root) } => root.pid,
        other => panic!("premise: the keeper's agent runs: {other:?}"),
    };
    drop(kept_host);
    retire(state, sup, client).await;
    let agent =
        start_tagged_host(state, &unknown_session(), HostRole::Agent { generation: 0 }).await;
    let tab = start_tagged_host(
        state,
        &unknown_session(),
        HostRole::Tab {
            tab_id: uuid::Uuid::new_v4().to_string(),
            scoped: false,
        },
    )
    .await;
    let unrecorded_tab = start_tagged_host(
        state,
        &kept.id,
        HostRole::Tab {
            tab_id: uuid::Uuid::new_v4().to_string(),
            scoped: false,
        },
    )
    .await;
    let stale =
        start_tagged_host(state, &unknown_session(), HostRole::Agent { generation: 0 }).await;
    let agent_pid = host_process(&agent).expect("the agent host runs");
    let tab_pid = host_process(&tab).expect("the tab host runs");
    let unrecorded_pid = host_process(&unrecorded_tab).expect("the unrecorded tab host runs");
    let stale_pid = host_process(&stale).expect("the stale host runs");
    // SAFETY: ending a host this test started, found by its socket.
    assert_eq!(unsafe { libc::kill(stale_pid as i32, libc::SIGKILL) }, 0);
    wait_until_pid_gone(stale_pid, WAIT_SECS).await;
    assert!(
        host_socket_names(state).contains(&socket_name(&stale)),
        "premise: the killed host's socket file is left"
    );
    assert!(
        !process_is_gone(kept_program),
        "premise: the keeper's agent still runs"
    );

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert!(process_is_gone(agent_pid), "the orphaned agent host ended");
    assert!(process_is_gone(tab_pid), "the orphaned tab host ended");
    assert!(
        process_is_gone(unrecorded_pid),
        "the unrecorded tab's host ended"
    );
    assert_eq!(
        host_socket_names(state),
        vec![socket_name(&kept_socket)],
        "only the keeper's socket is left"
    );
    assert!(
        !process_is_gone(kept_program),
        "the keeper's agent program still runs"
    );
    client.delete_session(&kept.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// Hosts this build cannot read are never reclaimed, even when their tags
/// would make them orphans: hosts whose snapshot does not restore and a host
/// whose tag cannot be read, while an ordinary orphan beside them is ended.
///
/// Why: the goal's "Downgrades and unreadable hosts": a host this build
/// cannot fully read may be a newer supervisor's, which comes back after a
/// downgrade; ending it would destroy a terminal this build only failed to
/// understand. A snapshot's format shows only in the host's stream, so the
/// reclaim must read it before it ends anything. The ordinary orphan shows
/// that the reclaim ran at all. Spec: with a keeper session in the store,
/// unrecorded hosts whose initial snapshot is not a snapshot (one tagged for
/// an unknown session, one for an unrecorded tab of the keeper), an
/// unrecorded host whose raw tag is not a tag this build reads, and an
/// unrecorded readable host tagged for an unknown session. Once a supervisor
/// serves, the three unreadable hosts still run at their sockets and the
/// readable one is gone.
#[farhelm_testtrace::test]
async fn hosts_this_build_cannot_read_are_never_reclaimed() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let kept = keeper(&client, stack.work.path()).await;
    retire(state, sup, client).await;
    let not_a_snapshot = || Some(b"not a terminal snapshot".to_vec());
    let unknown = start_host_running(
        state,
        &unknown_session(),
        HostRole::Agent { generation: 0 },
        "exec sleep 1000",
        not_a_snapshot(),
    )
    .await;
    let kept_tab = start_host_running(
        state,
        &kept.id,
        HostRole::Tab {
            tab_id: uuid::Uuid::new_v4().to_string(),
            scoped: false,
        },
        "exec sleep 1000",
        not_a_snapshot(),
    )
    .await;
    let foreign = start_raw_tagged_host(state, b"not a farhelm tag").await;
    let control =
        start_tagged_host(state, &unknown_session(), HostRole::Agent { generation: 0 }).await;
    let control_pid = host_process(&control).expect("the readable orphan runs");
    let hosts: Vec<(&str, u32, &PathBuf)> = [
        (
            "the unknown session's host with an unreadable snapshot",
            &unknown,
        ),
        (
            "the keeper's unrecorded tab with an unreadable snapshot",
            &kept_tab,
        ),
        ("the host with a foreign tag", &foreign),
    ]
    .into_iter()
    .map(|(what, socket)| (what, host_process(socket).expect("the host runs"), socket))
    .collect();

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert!(
        process_is_gone(control_pid),
        "the readable orphan beside them ended, so the reclaim ran"
    );
    for (what, pid, socket) in &hosts {
        assert!(!process_is_gone(*pid), "{what} still runs");
        assert!(
            host_socket_names(state).contains(&socket_name(socket)),
            "{what}'s socket is left"
        );
    }
    for (_, _, socket) in &hosts {
        end_left_host(socket).await;
    }
    client.delete_session(&kept.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// A `hosts` that is a symlink stops the whole reclaim: a record naming a
/// socket file reached through it is not settled, so neither that file nor
/// the record is removed.
///
/// Why: the socket directory's shape is the only boundary a host has, and a
/// supervisor refuses a symlinked one because what it reaches may be another
/// installation's. The records name sockets by joining that directory, so the
/// record path must be refused as the scan is, or a removal or a `Shutdown`
/// reaches through the link. Spec: a store that knows a keeper whose host
/// was ended and whose state directory's socket directory was then removed,
/// so the state directory has no socket directory of its own; `hosts` made
/// a symlink to another directory's socket directory, holding
/// the socket file a host killed with SIGKILL left; a launch record of a
/// session no store holds naming that socket. Once a supervisor serves, the
/// other directory's socket file and the record are both still there.
#[farhelm_testtrace::test]
async fn a_symlinked_socket_directory_is_never_reclaimed_through() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let kept = keeper(&client, stack.work.path()).await;
    retire(state, sup, client).await;
    // The keeper only has to be known to the store (an empty database
    // reclaims nothing); its host and the socket directory it lived in go,
    // so that `hosts` can become the symlink under test.
    end_all_terminals(state).await;
    std::fs::remove_dir_all(state.join("hosts")).expect("remove the socket directory");
    assert!(
        !state.join("hosts").exists(),
        "premise: no socket directory of its own"
    );
    let other = farhelm_teststate::tempdir().expect("another state directory");
    let session = unknown_session();
    let socket = start_tagged_host(other.path(), &session, HostRole::Agent { generation: 0 }).await;
    let pid = host_process(&socket).expect("the other directory's host runs");
    // SAFETY: ending a host this test started, found by its socket.
    assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGKILL) }, 0);
    wait_until_pid_gone(pid, WAIT_SECS).await;
    assert!(socket.exists(), "premise: its socket file is left");
    std::os::unix::fs::symlink(other.path().join("hosts"), state.join("hosts"))
        .expect("link the socket directory");
    let record = write_agent_record(state, &session, &socket);

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert!(socket.exists(), "the other directory's socket file is left");
    assert!(record.exists(), "the record is left");
    std::fs::remove_file(state.join("hosts")).expect("unlink the socket directory");
    client.delete_session(&kept.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// A reclaim whose discovery outlasts its budget ends nothing, and the next
/// start ends the orphan it left.
///
/// Why: the reclaim runs before the supervisor binds its socket, so a
/// discovery that never finishes (a store or a launch directory that
/// stalls) must neither hold the start nor act on half of what it read.
/// Spec: with a keeper session in the store, an unrecorded host tagged for
/// an unknown session; a supervisor with a one-second reclaim budget whose
/// discovery is held for good serves, and the host still runs with its
/// socket. A later supervisor without the hold ends it and removes its
/// socket.
#[farhelm_testtrace::test]
async fn a_reclaim_whose_discovery_runs_out_of_time_ends_nothing() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let kept = keeper(&client, stack.work.path()).await;
    retire(state, sup, client).await;
    let socket =
        start_tagged_host(state, &unknown_session(), HostRole::Agent { generation: 0 }).await;
    let pid = host_process(&socket).expect("the orphan runs");

    let held = FaultHooks {
        reclaim_discovery_gate: Some(Arc::new(
            || -> Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
                Box::pin(std::future::pending())
            },
        )),
        ..FaultHooks::default()
    };
    let timeouts = SupervisorTimeouts {
        orphan_reclaim: Duration::from_secs(1),
        ..SupervisorTimeouts::default()
    };
    let (sup, client) = stack
        .supervisor_with(
            SupervisorSeams {
                faults: held,
                ..tab_seams(home.path())
            },
            timeouts,
        )
        .await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert!(!process_is_gone(pid), "the orphan still runs");
    assert!(socket.exists(), "its socket is left");
    serving.stop().await;
    retire(state, sup, client).await;

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert!(process_is_gone(pid), "the next start ended the orphan");
    assert!(!socket.exists(), "and removed its socket");
    client.delete_session(&kept.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// No host is reclaimed when the session database is missing or knows no
/// session at all.
///
/// Why: every host reads as an orphan of an empty database, and one that
/// went missing (moved aside, reset, a partial restore) is not the user
/// deleting every session: with tmux those sessions survived and came back
/// with their database, and hosts must not be ended for good instead. Spec:
/// a state directory with no database and an unrecorded host tagged for a
/// session no store holds; a supervisor serves there, and the host still
/// runs. Then, with the database that start created now there and still no
/// session in it (premise checked), a second supervisor serves, and the host
/// still runs.
#[farhelm_testtrace::test]
async fn nothing_is_reclaimed_for_a_missing_or_empty_database() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let socket =
        start_tagged_host(state, &unknown_session(), HostRole::Agent { generation: 0 }).await;
    let pid = host_process(&socket).expect("the host runs");
    assert!(
        !state.join("supervisor.db").exists(),
        "premise: no database yet"
    );
    for start in ["a missing database", "an empty database"] {
        if start == "an empty database" {
            assert!(
                state.join("supervisor.db").exists(),
                "premise: the first start created the database"
            );
            assert!(
                store_of(state)
                    .await
                    .known_session_ids()
                    .await
                    .expect("read the ids")
                    .is_empty(),
                "premise: the database knows no session"
            );
        }
        let (sup, client) = stack.supervisor(host_seams()).await;
        let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
        assert!(!process_is_gone(pid), "with {start}, the host still runs");
        assert!(socket.exists(), "with {start}, its socket is left");
        serving.stop().await;
        retire(state, sup, client).await;
    }
    end_left_host(&socket).await;
}

/// A startup placeholder's socket is never contacted by the reclaim.
///
/// Why: a placeholder stands for a session's host the restart could not
/// use, and its socket is re-adoption's; a contact would take over whatever
/// serves it from whatever re-adoption will judge, which survives only by
/// luck of what answers. Counting connections, not survival, is what shows
/// no contact happened. Spec: a host agent's session whose host is killed
/// while no supervisor runs; a stand-in that counts the connections it
/// accepts and answers each with a malformed frame (which no re-adoption
/// retries) is then bound at that socket. The next supervisor holds a
/// placeholder for the session's agent at that socket, and its reload has
/// connected at least once (premise); serving it adds no connection.
#[farhelm_testtrace::test]
async fn a_placeholders_socket_is_never_contacted() {
    use std::sync::atomic::AtomicUsize;
    use tokio::io::AsyncWriteExt as _;
    let stack = Stack::new().await;
    let state = stack.state.path();
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let session = create(&client, stack.work.path()).await;
    let socket = agent_host(&sup, &session.id).await.socket().to_path_buf();
    let original = host_process(&socket).expect("the session's host runs");
    retire(state, sup, client).await;
    // SAFETY: ending the host this test's supervisor started.
    assert_eq!(unsafe { libc::kill(original as i32, libc::SIGKILL) }, 0);
    wait_until_pid_gone(original, WAIT_SECS).await;
    std::fs::remove_file(&socket).expect("remove the killed host's socket file");
    let listener = tokio::net::UnixListener::bind(&socket).expect("bind the stand-in");
    let accepted = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&accepted);
    let stand_in = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            counted.fetch_add(1, Ordering::SeqCst);
            // An empty frame: malformed, so the client gives up at once.
            let _ = stream.write_all(&[0, 0, 0, 0]).await;
        }
    });

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let placeholder = agent_host(&sup, &session.id).await;
    assert!(
        placeholder.is_placeholder(),
        "premise: the session holds a placeholder: {:?}",
        placeholder.liveness()
    );
    assert_eq!(placeholder.socket(), socket, "premise: at that socket");
    let before = accepted.load(Ordering::SeqCst);
    assert!(
        before >= 1,
        "premise: the reload's scan reached the stand-in"
    );
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert_eq!(
        accepted.load(Ordering::SeqCst),
        before,
        "serving, the reclaim included, never connected to the placeholder's socket"
    );
    drop(placeholder);
    stand_in.abort();
    let _ = stand_in.await;
    std::fs::remove_file(&socket).expect("remove the stand-in's socket");
    client.delete_session(&session.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// Hosts whose session the database alone knows, by a row or only by a
/// reservation, and a host that cannot be read, are never reclaimed, while an
/// ordinary orphan beside them is ended.
///
/// Why: a session row means the session's own verbs own its hosts, a
/// reservation means a retry of its create will look for its host by the
/// tag, and a host that cannot be read may be a newer supervisor's (the
/// goal: such a host is never treated as an orphan); ending any of them would
/// kill a program the user still has. Launch records make a session known
/// too, so both sessions' records are removed here: what protects their
/// hosts is the database alone. The ordinary orphan shows that the reclaim
/// ran. Spec: a keyed create interrupted after its host's first stage took
/// the spec leaves its host running (its row is then rolled back, keeping
/// its reservation pending); another session is created; both sessions'
/// launch records are removed (premise: neither has a record); an
/// unrecorded host is tagged for the second session's agent at a launch
/// generation it never had; an unrecorded host tagged for a session no store
/// holds is stopped with SIGSTOP; and another such host is left readable.
/// Once the next supervisor serves, the first three hosts still run at their
/// sockets, the reservation is still pending, and the readable orphan is
/// gone.
#[farhelm_testtrace::test]
async fn hosts_of_known_sessions_and_unreadable_hosts_are_left_at_start() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let gate = fixture.path().join("gate");
    // Released from the start: the interrupted first stage goes on to start
    // its host, which then runs with nothing but a reservation to its name.
    std::fs::write(&gate, b"").expect("release the first stage");
    let (sup, client) = stack
        .supervisor(interrupted_seams(fixture.path(), &gate, None))
        .await;
    let runs = stack.work.path().join("runs");
    client
        .create_session_with_key(
            stack.work.path().to_str().unwrap(),
            &counted_agent(&runs),
            None,
            80,
            24,
            Some("reserved-start".to_string()),
        )
        .await
        .expect_err("the interrupted attempt fails");
    let reserved = store_of(state)
        .await
        .reservation("reserved-start")
        .await
        .expect("read the reservation")
        .expect("the reservation")
        .session_id;
    let kept_work = farhelm_teststate::tempdir().expect("workdir");
    let kept = create(&client, kept_work.path()).await;
    let reserved_record = agent_record(state, &reserved);
    let reserved_socket = state.join("hosts").join(
        std::fs::read_to_string(&reserved_record)
            .expect("the interrupted create's record")
            .lines()
            .find_map(|line| line.strip_prefix("socket "))
            .expect("the record names its socket"),
    );
    let reserved_socket = std::path::absolute(reserved_socket).expect("an absolute socket");
    wait_for_condition("the interrupted create's host runs", || async {
        host_process(&reserved_socket)
            .map(drop)
            .ok_or_else(|| "no host process yet".to_string())
    })
    .await;
    let reserved_pid = host_process(&reserved_socket).expect("the reserved session's host runs");
    retire(state, sup, client).await;
    store_of(state)
        .await
        .delete_session(&reserved, None)
        .await
        .expect("roll the row back, keeping the reservation");
    // Records make a session known as well; without them, only the
    // database knows these two sessions.
    let kept_record = agent_record(state, &kept.id);
    for record in [&reserved_record, &kept_record] {
        std::fs::remove_file(record).expect("remove the session's launch record");
    }
    let records: Vec<String> = std::fs::read_dir(state.join("launch"))
        .expect("the launch directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".host"))
        .collect();
    assert_eq!(
        records,
        Vec::<String>::new(),
        "premise: no host record is left"
    );
    let unadopted = start_tagged_host(state, &kept.id, HostRole::Agent { generation: 7 }).await;
    let unadopted_pid = host_process(&unadopted).expect("the unadopted host runs");
    let unreadable =
        start_tagged_host(state, &unknown_session(), HostRole::Agent { generation: 0 }).await;
    let unreadable_pid = host_process(&unreadable).expect("the unreadable host runs");
    let stopped = Stopped::stop(unreadable_pid);
    let control =
        start_tagged_host(state, &unknown_session(), HostRole::Agent { generation: 0 }).await;
    let control_pid = host_process(&control).expect("the readable orphan runs");
    assert!(
        store_of(state)
            .await
            .session(&reserved)
            .await
            .unwrap()
            .is_none(),
        "premise: the reserved session has no row"
    );
    assert_eq!(
        store_of(state)
            .await
            .reservation("reserved-start")
            .await
            .unwrap()
            .expect("the reservation")
            .outcome,
        ReservationOutcome::Pending,
        "premise: the reservation is pending"
    );

    let (sup, client) = stack.supervisor(host_seams()).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert!(
        process_is_gone(control_pid),
        "the readable orphan beside them ended, so the reclaim ran"
    );
    for (what, pid, socket) in [
        (
            "the reserved session's host",
            reserved_pid,
            &reserved_socket,
        ),
        (
            "the existing session's unadopted host",
            unadopted_pid,
            &unadopted,
        ),
        ("the unreadable host", unreadable_pid, &unreadable),
    ] {
        assert!(!process_is_gone(pid), "{what} still runs");
        assert!(
            host_socket_names(state).contains(&socket_name(socket)),
            "{what}'s socket is left"
        );
    }
    assert_eq!(
        store_of(state)
            .await
            .reservation("reserved-start")
            .await
            .unwrap()
            .expect("the reservation")
            .outcome,
        ReservationOutcome::Pending
    );
    drop(stopped);
    for socket in [&reserved_socket, &unadopted, &unreadable] {
        end_left_host(socket).await;
    }
    client.delete_session(&kept.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// A session's create, interrupted with its host's first stage holding the
/// spec and its session then missing from the database, is settled at the
/// next start, and the host the first stage brings up is left with its
/// record.
///
/// Why: a first stage that outlived its supervisor can still bind its
/// socket and start the agent after the reclaim has looked; reading its
/// record from the socket alone would see nothing and remove the record,
/// and the agent would then run with nothing that names it. The record is
/// settled by a retried create's rules (the first stage waited for, then
/// fenced) before anything is believed, and the host it brings up belongs to
/// a session its record makes known, so it is left. Spec: a keyed create is
/// interrupted after its first stage took the spec (held there), a keeper
/// session is created, the supervisor is dropped and the interrupted
/// session's row and reservations deleted (premises: the recorded first
/// stage runs, no socket but the keeper's); the next supervisor's settle
/// releases the first stage from its wait hook. Once it serves, the hook has
/// fired, the first stage is gone, a host process serves the recorded
/// socket, and the record is still there.
#[farhelm_testtrace::test]
async fn an_interrupted_start_of_a_missing_session_is_settled_and_its_host_left() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let gate = fixture.path().join("gate");
    let (sup, client) = stack
        .supervisor(interrupted_seams(fixture.path(), &gate, None))
        .await;
    let runs = stack.work.path().join("runs");
    client
        .create_session_with_key(
            stack.work.path().to_str().unwrap(),
            &counted_agent(&runs),
            None,
            80,
            24,
            Some("missing-start".to_string()),
        )
        .await
        .expect_err("the interrupted attempt fails");
    let kept_work = farhelm_teststate::tempdir().expect("workdir");
    let kept = keeper(&client, kept_work.path()).await;
    let kept_socket = agent_host(&sup, &kept.id).await.socket().to_path_buf();
    let id = store_of(state)
        .await
        .reservation("missing-start")
        .await
        .expect("read the reservation")
        .expect("the reservation")
        .session_id;
    let record = agent_record(state, &id);
    let starter = recorded_starter(&record);
    let recorded_socket = recorded_socket(state, &record);
    retire(state, sup, client).await;
    forget_session(state, &id).await;
    assert!(
        !process_is_gone(starter),
        "premise: the first stage is held"
    );
    assert_eq!(
        host_socket_names(state),
        vec![socket_name(&kept_socket)],
        "premise: no socket yet but the keeper's"
    );

    let released = gate.clone();
    let seams = SupervisorSeams {
        faults: FaultHooks {
            launch_starter_waiting: Some(Arc::new(move || {
                std::fs::write(&released, b"").expect("release the first stage");
            })),
            ..FaultHooks::default()
        },
        ..host_seams()
    };
    let (sup, client) = stack.supervisor(seams).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert!(gate.exists(), "the settle waited for the first stage");
    assert!(process_is_gone(starter), "the first stage is gone");
    assert!(
        host_process(&recorded_socket).is_some(),
        "the host the first stage brought up still runs"
    );
    assert!(record.exists(), "the record is left");
    end_left_host(&recorded_socket).await;
    client.delete_session(&kept.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// A session's create, interrupted with its host's first stage holding the
/// spec and its session then missing from the database, whose first stage
/// never finishes, is fenced at the next start and its record cleared.
///
/// Why: a record whose host is gone is what the reclaim clears, but a first
/// stage that still runs may yet bring that host up; only once it is fenced
/// (waited for, then ended with the verified signal) is "nothing at the
/// socket" an answer. Spec: as in the test above, but the first stage is
/// never released; once the next supervisor serves, the first stage is
/// gone, no process but the keeper's host names a socket in the socket
/// directory, no socket but the keeper's is left, and the record is
/// removed.
#[farhelm_testtrace::test]
async fn an_interrupted_start_that_never_finishes_is_fenced_and_its_record_cleared() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let fixture = farhelm_teststate::tempdir().expect("fixture directory");
    let gate = fixture.path().join("gate");
    let (sup, client) = stack
        .supervisor(interrupted_seams(fixture.path(), &gate, None))
        .await;
    let runs = stack.work.path().join("runs");
    client
        .create_session_with_key(
            stack.work.path().to_str().unwrap(),
            &counted_agent(&runs),
            None,
            80,
            24,
            Some("stuck-start".to_string()),
        )
        .await
        .expect_err("the interrupted attempt fails");
    let kept_work = farhelm_teststate::tempdir().expect("workdir");
    let kept = keeper(&client, kept_work.path()).await;
    let kept_socket = agent_host(&sup, &kept.id).await.socket().to_path_buf();
    let kept_pid = host_process(&kept_socket).expect("the keeper's host runs");
    let id = store_of(state)
        .await
        .reservation("stuck-start")
        .await
        .expect("read the reservation")
        .expect("the reservation")
        .session_id;
    let record = agent_record(state, &id);
    let starter = recorded_starter(&record);
    retire(state, sup, client).await;
    forget_session(state, &id).await;
    assert!(
        !process_is_gone(starter),
        "premise: the first stage is held"
    );

    let (sup, client) = stack.supervisor(host_seams()).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert!(process_is_gone(starter), "the first stage was ended");
    assert_eq!(
        host_processes_in(state),
        vec![kept_pid],
        "nothing but the keeper's host names a host socket"
    );
    assert_eq!(
        host_socket_names(state),
        vec![socket_name(&kept_socket)],
        "no socket is left but the keeper's"
    );
    assert!(!record.exists(), "the record is removed");
    client.delete_session(&kept.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// A launch directory that cannot be listed, at its open or partway through,
/// reclaims nothing that start: a live host of a session the database lacks
/// but its records know stays, and so does an orphan nothing knows.
///
/// Why: the records are what keep a session that went missing from the
/// database known; a listing that failed would make its hosts, or the tabs
/// whose records it never reached, look like orphans, and end terminals for
/// good over a transient read error. Only a complete listing is evidence.
/// Spec: a host agent's session with two host tabs, left running by a
/// dropped supervisor, then deleted from a database that still knows a
/// keeper session (its records remain); an unrecorded host tagged for a
/// session nobody knows. A supervisor whose listing fails at the open
/// serves, then one whose listing fails at the second of the session's tab
/// records (so at least one record was counted first): after each, every
/// one of the session's hosts runs with its socket and record, and the
/// unrecorded host still runs. A supervisor with a whole listing then ends
/// the unrecorded host and still leaves the session's hosts.
#[farhelm_testtrace::test]
async fn a_launch_directory_not_listed_whole_reclaims_nothing() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let missing = create(&client, stack.work.path()).await;
    let tabs = [
        client.open_tab(&missing.id).await.expect("open a tab"),
        client.open_tab(&missing.id).await.expect("open a tab"),
    ];
    let kept_work = farhelm_teststate::tempdir().expect("workdir");
    let kept = keeper(&client, kept_work.path()).await;
    let mut hosts = vec![(
        agent_host(&sup, &missing.id).await.socket().to_path_buf(),
        agent_record(state, &missing.id),
    )];
    for tab in &tabs {
        hosts.push((
            tab_host(&sup, &missing.id, &tab.id)
                .await
                .socket()
                .to_path_buf(),
            tab_record(state, &missing.id, &tab.id),
        ));
    }
    let pids: Vec<u32> = hosts
        .iter()
        .map(|(socket, _)| host_process(socket).expect("the session's host runs"))
        .collect();
    retire(state, sup, client).await;
    forget_session(state, &missing.id).await;
    let orphan =
        start_tagged_host(state, &unknown_session(), HostRole::Agent { generation: 0 }).await;
    let orphan_pid = host_process(&orphan).expect("the unrecorded orphan runs");
    let survives = |start: &str| {
        for ((socket, record), pid) in hosts.iter().zip(&pids) {
            assert!(
                !process_is_gone(*pid),
                "{start}: the host at {} runs",
                socket.display()
            );
            assert!(socket.exists(), "{start}: its socket is left");
            assert!(record.exists(), "{start}: its record is left");
        }
    };

    let tab_prefix = format!("{}.tab-", missing.id);
    let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let faults: [(&str, farhelm_supervisor::service::ReclaimListingFault); 2] = [
        (
            "a listing that fails at its open",
            Arc::new(|name: Option<&str>| name.is_none()),
        ),
        ("a listing that fails partway", {
            let seen = Arc::clone(&seen);
            Arc::new(move |name: Option<&str>| {
                name.is_some_and(|name| name.starts_with(&tab_prefix))
                    && seen.fetch_add(1, Ordering::SeqCst) == 1
            })
        }),
    ];
    for (start, fault) in faults {
        let seams = SupervisorSeams {
            faults: FaultHooks {
                reclaim_listing_fault: Some(fault),
                ..FaultHooks::default()
            },
            ..tab_seams(home.path())
        };
        let (sup, client) = stack.supervisor(seams).await;
        let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
        survives(start);
        assert!(
            !process_is_gone(orphan_pid),
            "{start}: the unrecorded orphan runs"
        );
        serving.stop().await;
        retire(state, sup, client).await;
    }
    assert_eq!(
        seen.load(Ordering::SeqCst),
        2,
        "premise: the partial listing failed at the second tab record, after counting the first"
    );

    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    assert!(
        process_is_gone(orphan_pid),
        "a whole listing ends the orphan"
    );
    survives("a whole listing");
    for (socket, _) in &hosts {
        end_left_host(socket).await;
    }
    client.delete_session(&kept.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}

/// After a restart, an existing session's tab whose shell exited while no
/// supervisor ran is reaped, and one whose host was killed meanwhile leaves
/// neither its record nor its socket file.
///
/// Why: a dead tab goes, record and all, when its supervisor watches it die
/// (PR 18's reap); one that died while none ran must not stay listed as a
/// corpse, nor leave a record and a socket file that every later start
/// settles again. Spec: a host agent's session with two host tabs; its
/// supervisor is dropped; one tab's shell is killed (its host records the
/// exit) and the other tab's host is killed with SIGKILL (its socket file
/// left). The next supervisor serves with a ten-minute ticker interval;
/// within the wait bound the session lists no tab, the first tab's host
/// process is gone, and neither tab's record nor socket is left, while the
/// agent's host still runs.
#[farhelm_testtrace::test]
async fn an_existing_sessions_dead_tabs_are_cleaned_up_after_a_restart() {
    let stack = Stack::new().await;
    let state = stack.state.path();
    let home = farhelm_teststate::tempdir().expect("home");
    let (sup, client) = stack.supervisor(tab_seams(home.path())).await;
    let session = create(&client, stack.work.path()).await;
    let exited = client.open_tab(&session.id).await.expect("open a tab");
    let killed = client.open_tab(&session.id).await.expect("open a tab");
    let exited_host = tab_host(&sup, &session.id, &exited.id).await;
    let shell = match exited_host.status().liveness {
        Liveness::Running { child: Some(root) } => root.pid,
        other => panic!("premise: the tab's shell runs: {other:?}"),
    };
    let exited_socket = exited_host.socket().to_path_buf();
    let exited_pid = host_process(&exited_socket).expect("the exiting tab's host runs");
    let killed_socket = tab_host(&sup, &session.id, &killed.id)
        .await
        .socket()
        .to_path_buf();
    let killed_pid = host_process(&killed_socket).expect("the killed tab's host runs");
    let agent_socket = agent_host(&sup, &session.id).await.socket().to_path_buf();
    drop(exited_host);
    retire(state, sup, client).await;
    // SAFETY: ending processes this test's supervisor started, found by
    // the tab's runtime and by the host's socket.
    assert_eq!(unsafe { libc::kill(shell as i32, libc::SIGKILL) }, 0);
    assert_eq!(unsafe { libc::kill(killed_pid as i32, libc::SIGKILL) }, 0);
    wait_until_pid_gone(shell, WAIT_SECS).await;
    wait_until_pid_gone(killed_pid, WAIT_SECS).await;
    assert!(
        host_socket_names(state).contains(&socket_name(&killed_socket)),
        "premise: the killed host's socket file is left"
    );

    let mut seams = tab_seams(home.path());
    seams.ticker_interval = Duration::from_secs(600);
    let (sup, client) = stack.supervisor(seams).await;
    let serving = crate::hook_identity::ServeTask::spawn(&sup, state).await;
    let exited_record = tab_record(state, &session.id, &exited.id);
    let killed_record = tab_record(state, &session.id, &killed.id);
    wait_for_condition("the dead tabs were cleaned up", || async {
        let listed = listed_tabs(&client, &session.id).await;
        let sockets = host_socket_names(state);
        if listed.is_empty()
            && process_is_gone(exited_pid)
            && !exited_record.exists()
            && !killed_record.exists()
            && !sockets.contains(&socket_name(&exited_socket))
            && !sockets.contains(&socket_name(&killed_socket))
        {
            Ok(())
        } else {
            Err(format!(
                "listed {listed:?}, exited host gone {}, records {} {}, sockets {sockets:?}",
                process_is_gone(exited_pid),
                exited_record.exists(),
                killed_record.exists(),
            ))
        }
    })
    .await;
    assert!(
        host_process(&agent_socket).is_some(),
        "the agent's host still runs"
    );
    client.delete_session(&session.id).await.expect("delete");
    serving.stop().await;
    retire(state, sup, client).await;
}
