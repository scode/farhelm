//! Discovery (`farhelm_supervisor::hosts::registry`) against real terminal
//! hosts, started from the real `farhelm` binary.
//!
//! Here rather than in the supervisor's unit tests because a host forks, so
//! it is never started inside a test process. The classes a real host never
//! produces (another protocol version, busy, silence) are covered against
//! stand-ins in `hosts::registry::tests`; what this adds is the real thing:
//! hosts the supervisor's own spawn tagged, a host carrying a tag that is not
//! Farhelm's, and the socket a killed host leaves.
//!
//! Every host lives in the test's own `farhelm_teststate` state directory,
//! whose drop ends every host in its `hosts` directory (`Shutdown`, never a
//! signal), so a failing assertion leaves nothing running. The one signal a
//! test sends goes to a host it started, found by the command line naming
//! its own socket. Every socket name here is random, as a real host's is:
//! the process search matches a socket's file name in any directory, so a
//! fixed name could find the host of a run of this test in another checkout.

mod terminal_host_support;

use farhelm_supervisor::hosts::client::ClientDeadlines;
use farhelm_supervisor::hosts::process::socket_process;
use farhelm_supervisor::hosts::{
    Gone, HostName, HostProcess, HostRegistry, HostRole, HostTag, ScanOptions, SocketRemoval,
    SpawnedHost, UnreadableTag, WhyUnavailable, ensure_hosts_dir, spawn_host,
};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use terminal_host::protocol::{Size, Spawn};
use terminal_host_support::{eventually, farhelm, sleeper, start_raw_host};

const SESSION: &str = "0f8e2b9c-4c1d-4e7a-9a53-7d2f61c0b8e4";

/// Start a host in `state` with `tag`, through the supervisor's own spawn.
async fn start(state: &Path, tag: HostTag) -> SpawnedHost {
    spawn_host(&farhelm(), state, sleeper(), &tag)
        .await
        .expect("the host starts")
}

/// Start a host at `<state>/hosts/<a random name>` whose tag is `tag`'s raw
/// bytes: the supervisor's spawn only writes tags it can encode, and this
/// one must be unreadable.
async fn start_with_raw_tag(state: &Path, tag: &[u8]) -> PathBuf {
    let socket = std::path::absolute(ensure_hosts_dir(state).unwrap())
        .unwrap()
        .join(HostName::random().unwrap().as_str());
    let spawn = Spawn {
        argv: vec![b"/bin/sleep".to_vec(), b"600".to_vec()],
        cwd: b"/".to_vec(),
        env: vec![b"PATH=/usr/bin:/bin".to_vec()],
        size: Size { cols: 80, rows: 24 },
        journal_cap: 1024 * 1024,
        tag: tag.to_vec(),
        initial_snapshot: None,
    };
    start_raw_host(&socket, &spawn).await;
    socket
}

/// The process serving `socket`, found by its command line.
fn host_process(socket: &Path) -> HostProcess {
    socket_process(socket)
        .expect("searching the process table")
        .expect("a process serving the socket")
}

/// A scan of real hosts finds the supervisor's own by their tags, keeps a
/// host with a foreign tag as unavailable, and judges a killed host's
/// socket gone; removing that socket leaves the live hosts running.
///
/// Why: this is what a restarted supervisor will do to find its terminals
/// again, against the processes and sockets real hosts leave, including
/// the one socket it may clean up (a killed host's) beside hosts it must
/// never touch. Spec: in one state directory, an agent host (generation 3
/// of a session), two tab hosts of that session spawned with creation
/// stamps 200 (tab `a`, first) and 100 (tab `b`), an agent host of another
/// session, a host started directly with the tag `not a farhelm tag`, and
/// a host killed with SIGKILL after it started; scanned with a 1 s refusal
/// window, the agent lookup gives the first host (its connection's peer is
/// the host's process, and the connection answers `Info` after the host
/// is taken from the registry), the tab lookup gives `b` then `a` (stamp
/// order, against both tab-id and spawn order) with no duplicates, the
/// other session's agent is found under its own session, the foreign-tag
/// host is unavailable as not Farhelm's with its process as the peer, the
/// killed host's socket is gone (refused), and the scan is not complete;
/// removing the gone socket removes its file, and every other host is
/// still running afterwards.
#[farhelm_testtrace::test]
async fn a_scan_of_real_hosts_finds_them_by_their_tags() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let agent = start(
        state.path(),
        HostTag {
            session_id: SESSION.into(),
            created_ns: 1,
            role: HostRole::Agent { generation: 3 },
        },
    )
    .await;
    let tab = |tab_id: &str, created_ns| HostTag {
        session_id: SESSION.into(),
        created_ns,
        role: HostRole::Tab {
            tab_id: tab_id.into(),
            scoped: false,
        },
    };
    let tab_a = start(state.path(), tab("a", 200)).await;
    let tab_b = start(state.path(), tab("b", 100)).await;
    let other = start(
        state.path(),
        HostTag {
            session_id: "another session".into(),
            created_ns: 1,
            role: HostRole::Agent { generation: 1 },
        },
    )
    .await;
    let foreign = start_with_raw_tag(state.path(), b"not a farhelm tag").await;
    let killed = start(
        state.path(),
        HostTag {
            session_id: "killed session".into(),
            created_ns: 1,
            role: HostRole::Agent { generation: 1 },
        },
    )
    .await;
    let killed_process = host_process(&killed.socket);
    // SAFETY: the pid was found serving this test's own socket just above.
    assert_eq!(
        unsafe { libc::kill(killed_process.pid as libc::pid_t, libc::SIGKILL) },
        0
    );
    assert!(
        eventually(|| HostProcess::current(killed_process.pid) != Some(killed_process)),
        "premise: the killed host ended"
    );
    assert!(
        killed.socket.exists(),
        "premise: a killed host leaves its socket"
    );
    let live_hosts = [
        &agent.socket,
        &tab_a.socket,
        &tab_b.socket,
        &other.socket,
        &foreign,
    ]
    .map(|socket| (socket.clone(), host_process(socket)));

    let options = ScanOptions {
        deadlines: ClientDeadlines {
            refusal_window: Duration::from_secs(1),
            ..ClientDeadlines::default()
        },
        ..ScanOptions::default()
    };
    let mut registry = HostRegistry::scan(state.path(), &options)
        .await
        .expect("the scan runs");

    let found = registry
        .agent(SESSION, 3)
        .expect("one agent host")
        .expect("the agent host");
    assert_eq!(found.socket, agent.socket);
    assert_eq!(found.peer(), Some(host_process(&agent.socket)));
    let tabs = registry.tabs(SESSION);
    assert!(tabs.duplicates.is_empty());
    let tab_ids: Vec<&HostRole> = tabs.hosts.into_iter().map(|host| &host.tag.role).collect();
    assert_eq!(
        tab_ids,
        [
            &HostRole::Tab {
                tab_id: "b".into(),
                scoped: false
            },
            &HostRole::Tab {
                tab_id: "a".into(),
                scoped: false
            },
        ]
    );
    assert_eq!(
        registry
            .agent("another session", 1)
            .unwrap()
            .map(|host| host.socket.clone()),
        Some(other.socket.clone())
    );

    let [unavailable] = registry.unavailable() else {
        panic!(
            "expected one unavailable host: {:?}",
            registry.unavailable()
        );
    };
    assert_eq!(unavailable.socket, foreign);
    assert!(
        matches!(
            unavailable.why,
            WhyUnavailable::Tag {
                tag: UnreadableTag::NotFarhelm,
                ..
            }
        ),
        "{:?}",
        unavailable.why
    );
    assert_eq!(unavailable.peer, Some(host_process(&foreign)));

    let [gone] = registry.gone() else {
        panic!("expected one gone socket: {:?}", registry.gone());
    };
    assert_eq!(gone.socket, killed.socket);
    assert_eq!(gone.why, Gone::Refused);
    assert!(!registry.is_complete());

    assert_eq!(gone.remove_socket().await, SocketRemoval::Removed);
    assert!(!killed.socket.exists());

    let mut taken = registry
        .take(OsStr::new(agent.name.as_str()))
        .expect("taken");
    let info = taken
        .connection
        .info()
        .await
        .expect("the scan's connection");
    assert_eq!(Some(info.host_pid), taken.peer().map(|peer| peer.pid));
    drop(taken);
    drop(registry);
    for (socket, process) in live_hosts {
        assert_eq!(
            HostProcess::current(process.pid),
            Some(process),
            "the host at {} ended",
            socket.display()
        );
        assert!(socket.exists(), "{} lost its socket", socket.display());
    }
}
