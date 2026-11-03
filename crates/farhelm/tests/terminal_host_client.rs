//! The supervisor's host client (`farhelm_supervisor::hosts::client`)
//! against real terminal hosts, started by the supervisor's spawn from the
//! real `farhelm` binary.
//!
//! These live here rather than in the supervisor's own tests because a host
//! forks: started in-process, its second stage would be a fork of a
//! multi-threaded test runner. The client's answers to hosts that a real
//! host never gives (busy to a take-over, a foreign version, broken frames)
//! are tested against stand-ins in the supervisor's unit tests
//! (`hosts::client::tests`).
//!
//! Sections: fixtures and a guard that ends every host a test starts, on
//! every path (`Shutdown` through the test-state cleanup's own client, then
//! SIGKILL, never relying on the client under test); connecting and its
//! outcome classes; the stream; input; ending a host.

use farhelm_supervisor::hosts::client::ClientDeadlines;
use farhelm_supervisor::hosts::{
    ChildSpec, ClientError, ConnectError, Gone, HostConnection, HostEvent, HostProcess, HostRole,
    HostTag, InputCounts, SnapshotReply, TerminateError, Termination, Unavailable, spawn_host,
    terminate_verified,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use terminal_host::protocol::{
    ChildExit, MAX_INPUT, MIN_JOURNAL_CAP, Position, Size, SnapshotRejection,
};

/// How long any single wait in these tests may take before it fails.
const WAIT: Duration = Duration::from_secs(10);

// ---- Fixtures ------------------------------------------------------------------

fn farhelm() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_farhelm"))
}

/// A child running `script` under `/bin/sh`, at 80x24.
fn shell(script: &str) -> ChildSpec {
    ChildSpec {
        argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
        cwd: "/".into(),
        env: vec![("PATH".into(), "/usr/bin:/bin".into())],
        size: Size { cols: 80, rows: 24 },
        journal_cap: 1024 * 1024,
        initial_snapshot: None,
    }
}

fn agent_tag() -> HostTag {
    HostTag {
        session_id: "0f8e2b9c-4c1d-4e7a-9a53-7d2f61c0b8e4".into(),
        created_ns: farhelm_supervisor::hosts::tag::creation_stamp(),
        role: HostRole::Agent { generation: 3 },
    }
}

/// Deadlines short enough that the timeout and refusal cases take a second
/// or two, long enough for a loaded machine to answer.
fn short() -> ClientDeadlines {
    ClientDeadlines {
        connect: Duration::from_secs(2),
        refusal_window: Duration::from_secs(1),
        ..ClientDeadlines::default()
    }
}

/// A real host this test started, in a state directory of its own. Dropping
/// it ends the host: `Shutdown` through the test-state cleanup's blocking
/// client, then SIGCONT and SIGKILL if it still runs. The pid is signalled
/// only while its command line still names this host's socket, which is
/// unique to the test, so a reused pid is never hit.
struct Host {
    socket: PathBuf,
    pid: u32,
    state: farhelm_teststate::TestDir,
}

impl Host {
    /// Start a host running `child`, tagged as an agent.
    async fn start(child: ChildSpec) -> Host {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let spawned = spawn_host(&farhelm(), state.path(), child, &agent_tag())
            .await
            .expect("the host starts");
        let pid = listener_pid(&spawned.socket).expect("the host's pid from its socket");
        Host {
            socket: spawned.socket,
            pid,
            state,
        }
    }

    fn is_ours(&self) -> bool {
        exists(self.pid) && command_line(self.pid).contains(self.socket.to_str().unwrap())
    }

    /// Send `signal` to the host, which must still be this test's.
    fn signal(&self, signal: libc::c_int) {
        assert!(
            self.is_ours(),
            "the host {} is no longer this test's",
            self.pid
        );
        // SAFETY: signalling the host this test started, checked above.
        assert_eq!(unsafe { libc::kill(self.pid as libc::pid_t, signal) }, 0);
    }

    async fn connect(&self) -> HostConnection {
        HostConnection::connect(&self.socket)
            .await
            .expect("connecting to the host")
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if !self.is_ours() {
            return;
        }
        let stop = farhelm_teststate::hosts::stop_host(&self.socket, Instant::now() + WAIT);
        if gone_within(self.pid) || !self.is_ours() {
            return;
        }
        eprintln!(
            "fixture: the host {} did not take Shutdown ({stop:?}); killing it",
            self.pid
        );
        // SAFETY: signalling the host this test started, checked above.
        unsafe {
            libc::kill(self.pid as libc::pid_t, libc::SIGCONT);
            libc::kill(self.pid as libc::pid_t, libc::SIGKILL);
        }
        if !gone_within(self.pid) {
            eprintln!("fixture: the host {} survived SIGKILL", self.pid);
        }
    }
}

/// The pid listening on `socket`, from the peer credentials of a fresh
/// connection: deliberately not the client under test.
fn listener_pid(socket: &Path) -> Option<u32> {
    use std::os::fd::AsRawFd as _;
    let stream = std::os::unix::net::UnixStream::connect(socket).ok()?;
    let fd = stream.as_raw_fd();
    #[cfg(target_os = "linux")]
    let pid = {
        // SAFETY: ucred is plain data; SO_PEERCRED fills one of this size.
        let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&raw mut cred).cast(),
                &mut len,
            )
        };
        (result == 0).then_some(cred.pid)?
    };
    #[cfg(target_os = "macos")]
    let pid = {
        let mut pid: libc::pid_t = 0;
        let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
        // SAFETY: LOCAL_PEERPID fills one pid_t.
        let result = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                (&raw mut pid).cast(),
                &mut len,
            )
        };
        (result == 0).then_some(pid)?
    };
    (pid > 1).then_some(pid as u32)
}

/// Whether `pid` exists and is not a zombie (`kill(pid, 0)` alone succeeds
/// for a zombie too). Reads that one process's state rather than running
/// `ps` where `/proc` has it: the waits below ask every 20 ms.
fn exists(pid: u32) -> bool {
    // SAFETY: signal 0 only checks for the process.
    if unsafe { libc::kill(pid as libc::pid_t, 0) } != 0 {
        return false;
    }
    #[cfg(target_os = "linux")]
    let state = {
        // The state follows the command name, which is in parentheses and
        // may itself contain spaces or parentheses.
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| stat.rsplit_once(") ").map(|(_, rest)| rest.to_owned()))
            .unwrap_or_default()
    };
    #[cfg(not(target_os = "linux"))]
    let state = {
        let output = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .expect("running ps");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    !state.is_empty() && !state.starts_with('Z')
}

/// `pid`'s command line, from `ps`, which reads it the same way on Linux
/// and macOS.
fn command_line(pid: u32) -> String {
    let output = std::process::Command::new("ps")
        .args(["-ww", "-o", "command=", "-p", &pid.to_string()])
        .output()
        .expect("running ps");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `pid`'s state letter (`T` when stopped), as `ps` reports it.
fn process_state(pid: u32) -> String {
    let output = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .expect("running ps");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Whether `condition` holds within [`WAIT`].
fn eventually(mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + WAIT;
    loop {
        if condition() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        // sleep-ok: polling interval for a process or file the test owns.
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn gone_within(pid: u32) -> bool {
    eventually(|| !exists(pid))
}

/// The next event, failing the test if none comes within [`WAIT`].
async fn next(connection: &mut HostConnection) -> Result<HostEvent, ClientError> {
    tokio::time::timeout(WAIT, connection.next_event())
        .await
        .expect("an event within the wait")
}

/// Read events until the output so far contains `needle`, and return the
/// output.
async fn output_until(connection: &mut HostConnection, needle: &str) -> String {
    let mut text = String::new();
    while !text.contains(needle) {
        if let HostEvent::Output { data, .. } = next(connection).await.expect("the stream") {
            text.push_str(&String::from_utf8_lossy(&data));
        }
    }
    text
}

/// Read events until an input acknowledgement resolves `end` bytes, and
/// return its counts.
async fn acknowledged(connection: &mut HostConnection, end: u64) -> InputCounts {
    loop {
        if let HostEvent::InputAcknowledged(counts) = next(connection).await.expect("the stream")
            && counts.written + counts.discarded >= end
        {
            return counts;
        }
    }
}

// ---- Connecting -------------------------------------------------------------------

/// A client connects with take-over, knows its host's process, and a second
/// client takes the host over from it.
///
/// Why: the supervisor always takes over (D-11): after a restart its old
/// connection may still be open, and refusing would lock it out of its own
/// terminals. The peer process is what makes a later verified SIGTERM
/// possible. Spec: the connection speaks version 1; its peer is the
/// listening process, which `Info` names as the host, with the child
/// running, the size given at spawn and the tag the spawn gave; a second
/// connection succeeds while the first is open, the first then ends with
/// `Closed`, and the second gets `Info`.
#[farhelm_testtrace::test]
async fn a_client_takes_over_and_knows_its_host() {
    let host = Host::start(shell("exec /bin/sleep 600")).await;
    let mut first = host.connect().await;
    assert_eq!(first.version(), 1);
    let peer = first.peer().expect("the peer process");
    assert_eq!(peer.pid, host.pid);
    let info = first.info().await.unwrap();
    assert_eq!(info.host_pid, host.pid);
    assert_eq!(info.child_exit, None);
    assert_eq!(info.initial_size, Size { cols: 80, rows: 24 });
    let tag = HostTag::decode(&info.tag).expect("the tag the spawn gave");
    assert_eq!(tag.role, HostRole::Agent { generation: 3 });
    first.start_read(Position::default()).await.unwrap();

    let mut second = host.connect().await;
    loop {
        match next(&mut first).await {
            Ok(_) => continue,
            Err(error) => {
                assert_eq!(error, ClientError::Closed);
                break;
            }
        }
    }
    assert_eq!(second.info().await.unwrap().host_pid, host.pid);
}

/// A stopped host times out as unavailable, and is ended by the verified
/// SIGTERM only once its identity and command line match.
///
/// Why: Delete and Restart must not leak a host the protocol cannot end,
/// and must never signal a process that is not that host. A stopped host
/// accepts connections (the kernel queues them) but never answers. Spec:
/// with the host stopped (SIGSTOP), connecting gives `Unavailable(Timeout)`
/// with the host as the peer; a termination with the start identity off by
/// one is `AlreadyGone` and one naming another socket is `NotThisHost`,
/// both leaving the host in place; the real one is `Terminated` (SIGTERM
/// and SIGCONT), the host process ends and its socket file goes.
#[farhelm_testtrace::test]
async fn a_stopped_host_times_out_and_is_terminated_after_its_identity_is_checked() {
    let host = Host::start(shell("exec /bin/sleep 600")).await;
    host.signal(libc::SIGSTOP);
    assert!(
        eventually(|| process_state(host.pid).starts_with('T')),
        "premise: the host stopped"
    );
    let peer = match HostConnection::connect_with(&host.socket, short()).await {
        Err(ConnectError::Unavailable {
            reason: Unavailable::Timeout,
            peer: Some(peer),
        }) => peer,
        other => panic!("expected a timeout with the peer, got {other:?}"),
    };
    assert_eq!(peer.pid, host.pid);

    let stale = HostProcess {
        start: peer.start.wrapping_add(1),
        ..peer
    };
    assert_eq!(
        terminate_verified(&stale, &host.socket).await,
        Ok(Termination::AlreadyGone)
    );
    let elsewhere = host.socket.with_file_name("other");
    assert!(matches!(
        terminate_verified(&peer, &elsewhere).await,
        Err(TerminateError::NotThisHost { .. })
    ));
    assert!(host.is_ours(), "a refused termination signalled the host");

    assert_eq!(
        terminate_verified(&peer, &host.socket).await,
        Ok(Termination::Terminated)
    );
    assert!(gone_within(host.pid), "the host survived");
    assert!(
        eventually(|| !host.socket.exists()),
        "the host left its socket file"
    );
}

/// A stopped host whose listen backlog is full is never judged gone: the
/// connect finds its process by its command line and names it.
///
/// Why: a stopped or wedged host never accepts, so every connection that
/// timed out against it stays in its backlog; once that is full, Linux
/// answers `EAGAIN` and macOS refuses, as for a socket nothing listens on,
/// and neither names a peer. Judged gone, the host would lose its socket
/// and leak; without a pid, Delete could not end it. Spec: with the host
/// stopped and its backlog filled with non-blocking connections until the
/// kernel turns one away, connecting gives `Unavailable` (not `Gone`) with
/// the host as the peer.
#[farhelm_testtrace::test]
async fn a_stopped_host_with_a_full_backlog_is_found_by_its_process() {
    let host = Host::start(shell("exec /bin/sleep 600")).await;
    host.signal(libc::SIGSTOP);
    assert!(
        eventually(|| process_state(host.pid).starts_with('T')),
        "premise: the host stopped"
    );
    let mut queued = Vec::new();
    let full = loop {
        match connect_nonblocking(&host.socket) {
            Ok(fd) => queued.push(fd),
            Err(error)
                if matches!(
                    error.raw_os_error(),
                    Some(libc::EAGAIN) | Some(libc::ECONNREFUSED)
                ) =>
            {
                break true;
            }
            Err(error) => panic!("filling the backlog: {error}"),
        }
        if queued.len() > 4096 {
            break false;
        }
    };
    assert!(full, "premise: the backlog fills");
    let result = HostConnection::connect_with(&host.socket, short()).await;
    match result {
        Err(ConnectError::Unavailable {
            peer: Some(peer), ..
        }) => assert_eq!(peer.pid, host.pid),
        other => panic!("expected an unavailable host naming its pid, got {other:?}"),
    }
    drop(queued);
    host.signal(libc::SIGCONT);
}

/// A non-blocking connection to `socket`, for filling a listener's backlog
/// without ever waiting in `connect`.
fn connect_nonblocking(socket: &Path) -> std::io::Result<std::os::fd::OwnedFd> {
    use std::os::fd::{AsRawFd as _, FromRawFd as _};
    use std::os::unix::ffi::OsStrExt as _;
    // SAFETY: sockaddr_un is plain data; all-zero bytes are valid.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    let bytes = socket.as_os_str().as_bytes();
    assert!(bytes.len() < address.sun_path.len(), "socket path too long");
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, byte) in address.sun_path.iter_mut().zip(bytes) {
        *slot = *byte as libc::c_char;
    }
    let length =
        (std::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1) as libc::socklen_t;
    // SAFETY: socket takes no pointers and returns a fresh descriptor.
    let raw = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if raw < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: raw is a fresh descriptor nothing else owns.
    let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(raw) };
    // Non-blocking set separately: macOS has no SOCK_NONBLOCK.
    // SAFETY: fcntl on a live descriptor this function owns.
    unsafe {
        let flags = libc::fcntl(fd.as_raw_fd(), libc::F_GETFL);
        libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    // SAFETY: address and length describe initialized storage; fd is live.
    if unsafe { libc::connect(fd.as_raw_fd(), (&raw const address).cast(), length) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(fd)
}

/// A killed host's socket is gone, but only after refusing for the refusal
/// window.
///
/// Why: a host killed with SIGKILL cannot remove its socket file, which then
/// refuses every connection; that is the one case a startup scan may clean
/// up, and it must be told from a live macOS host with a full backlog,
/// which refuses too. Spec: after SIGKILL ends the host, its socket file
/// remains, and connecting with a 1 s refusal window gives `Gone(Refused)`,
/// no sooner than 1 s.
#[farhelm_testtrace::test]
async fn a_killed_hosts_socket_is_gone_after_the_refusal_window() {
    let host = Host::start(shell("exec /bin/sleep 600")).await;
    host.signal(libc::SIGKILL);
    assert!(gone_within(host.pid), "premise: the host was killed");
    assert!(
        host.socket.exists(),
        "premise: a killed host leaves its socket"
    );
    let started = Instant::now();
    let result = HostConnection::connect_with(&host.socket, short()).await;
    assert_eq!(result.unwrap_err(), ConnectError::Gone(Gone::Refused));
    assert!(
        started.elapsed() >= Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
}

// ---- The stream -------------------------------------------------------------------

/// A read from the start decodes the initial snapshot record, the gap left
/// by trimming, the retained output and the exit, in that order.
///
/// Why: these are every kind of record a recovering supervisor rebuilds a
/// terminal from, and each carries the position the next one continues
/// from. Spec: a host spawned with an initial snapshot and the smallest
/// journal cap, whose child writes 200,000 bytes and exits 3, read from
/// (0, 0) after it exited, gives the snapshot record at (0, 0) with its
/// blob; a gap from 0 to some offset `t` > 0 at 80x24 with no resizes;
/// outputs that continue exactly from `t` to `Info`'s end, no more than the
/// cap; and the exit at that end, with status 3.
#[farhelm_testtrace::test]
async fn a_read_decodes_the_snapshot_record_the_gap_and_the_exit() {
    let mut child = shell("/usr/bin/yes | /usr/bin/head -c 200000; exit 3");
    child.journal_cap = MIN_JOURNAL_CAP;
    child.initial_snapshot = Some(b"seed".to_vec());
    let host = Host::start(child).await;
    let mut connection = host.connect().await;
    let deadline = Instant::now() + WAIT;
    let info = loop {
        let info = connection.info().await.unwrap();
        if info.child_exit.is_some() && info.gap {
            break info;
        }
        assert!(
            Instant::now() < deadline,
            "the child never finished: {info:?}"
        );
        // sleep-ok: polling interval while the host's child writes and exits.
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    connection.start_read(Position::default()).await.unwrap();

    assert_eq!(
        next(&mut connection).await.unwrap(),
        HostEvent::Snapshot {
            position: Position::default(),
            blob: b"seed".to_vec()
        }
    );
    let resume = match next(&mut connection).await.unwrap() {
        HostEvent::Gap {
            from: 0,
            to,
            resizes: 0,
            size: Size { cols: 80, rows: 24 },
        } if to > 0 => to,
        other => panic!("expected a gap from 0, got {other:?}"),
    };
    let mut at = resume;
    let exit = loop {
        match next(&mut connection).await.unwrap() {
            HostEvent::Output { offset, data } => {
                assert_eq!(offset, at, "the output does not continue");
                at += data.len() as u64;
            }
            HostEvent::Exited { offset, exit } => {
                assert_eq!(offset, at, "the exit is not after the output");
                break exit;
            }
            other => panic!("unexpected {other:?}"),
        }
    };
    assert_eq!(exit, ChildExit::Code(3));
    assert!(at >= info.end, "the stream ended before Info's end");
    assert!(
        at - resume <= MIN_JOURNAL_CAP,
        "more than the cap was retained"
    );
}

/// Resizes come back as records, and snapshots are accepted or rejected as
/// the journal decides; a later read starts from the accepted one.
///
/// Why: the supervisor resizes through the stream (the record is its
/// confirmation and its position), and its checkpoints depend on telling
/// an accepted snapshot from a rejected one. Spec: after the output
/// `hello`, a resize to 100x30 comes back as resize 0 and `Info` then
/// reports that size, one resize and the spawn's size as initial; a
/// snapshot at `Info`'s end is accepted at that position, one at (0, 0)
/// after it is rejected as before the current; a second `start_read` is
/// refused; and a new connection reading from (0, 0) first gets the
/// accepted snapshot.
#[farhelm_testtrace::test]
async fn resizes_and_snapshots_round_trip() {
    let host = Host::start(shell("printf hello; exec /bin/sleep 600")).await;
    let mut connection = host.connect().await;
    connection.start_read(Position::default()).await.unwrap();
    output_until(&mut connection, "hello").await;
    let size = Size {
        cols: 100,
        rows: 30,
    };
    connection.resize(size).await.unwrap();
    loop {
        if let HostEvent::Resized {
            index, size: got, ..
        } = next(&mut connection).await.unwrap()
        {
            assert_eq!((index, got), (0, size));
            break;
        }
    }
    let info = connection.info().await.unwrap();
    assert_eq!((info.size, info.resizes), (size, 1));
    assert_eq!(info.initial_size, Size { cols: 80, rows: 24 });

    let position = Position {
        offset: info.end,
        resizes: info.resizes,
    };
    assert_eq!(
        connection.snapshot(position, b"state".to_vec()).await,
        Ok(SnapshotReply::Accepted(position))
    );
    assert_eq!(
        connection
            .snapshot(Position::default(), b"older".to_vec())
            .await,
        Ok(SnapshotReply::Rejected {
            position: Position::default(),
            reason: SnapshotRejection::BeforeCurrent
        })
    );
    assert_eq!(
        connection.start_read(Position::default()).await,
        Err(ClientError::ReadAlreadyStarted)
    );

    let mut again = host.connect().await;
    again.start_read(Position::default()).await.unwrap();
    assert_eq!(
        next(&mut again).await.unwrap(),
        HostEvent::Snapshot {
            position,
            blob: b"state".to_vec()
        }
    );
}

// ---- Input -----------------------------------------------------------------------

/// Inputs are acknowledged in the order they were sent, and reach the
/// program in that order.
///
/// Why: the window and the report of what was written both read the
/// acknowledgements as a growing prefix of what was sent. Spec: four line
/// inputs sent to `cat` are all acknowledged as written, nothing discarded
/// or unknown; across every acknowledgement the written count never goes
/// down and the resolved inputs are always a prefix of the four; the lines
/// appear in `cat`'s output (the terminal's echo is off, so the output is
/// only what `cat` read) in the order sent.
#[farhelm_testtrace::test]
async fn inputs_are_acknowledged_in_order() {
    let host = Host::start(shell("stty -echo; echo ready; exec /bin/cat")).await;
    let mut connection = host.connect().await;
    connection.start_read(Position::default()).await.unwrap();
    output_until(&mut connection, "ready").await;
    let lines = ["first\n", "second\n", "third\n", "fourth\n"];
    let mut tickets = Vec::new();
    for line in lines {
        tickets.push(connection.send_input(line.as_bytes()).await.unwrap());
    }
    let total = tickets.last().unwrap().end;
    let mut text = String::new();
    let mut written = 0;
    let mut done = false;
    while !(done && text.contains("fourth")) {
        match next(&mut connection).await.unwrap() {
            HostEvent::Output { data, .. } => text.push_str(&String::from_utf8_lossy(&data)),
            HostEvent::InputAcknowledged(counts) => {
                assert!(counts.written >= written, "the written count went down");
                written = counts.written;
                let resolved: Vec<bool> = tickets.iter().map(|t| counts.resolved(*t)).collect();
                assert!(
                    resolved.windows(2).all(|pair| pair[0] || !pair[1]),
                    "inputs resolved out of order: {resolved:?}"
                );
                done = counts.written + counts.discarded == total;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    let counts = connection.input_counts();
    assert_eq!(
        (counts.written, counts.discarded, counts.unknown()),
        (total, 0, 0)
    );
    let positions: Vec<usize> = lines
        .iter()
        .map(|line| text.find(line.trim()).expect("every line reached cat"))
        .collect();
    assert!(positions.is_sorted(), "{text:?}");
}

/// Shift+Enter's two bytes reach a waiting program in one read.
///
/// Why: programs tell Shift+Enter from Escape-then-Enter by whether the two
/// bytes arrive together; the client sends them as one `Input` so that the
/// host writes them in one call. Spec: a raw-mode `dd` already waiting to
/// read one block receives both bytes (ESC and CR, as `od -c` prints them)
/// in that block.
#[farhelm_testtrace::test]
async fn shift_enter_reaches_the_program_in_one_read() {
    let host = Host::start(shell(
        "stty raw -echo; echo ready; dd bs=64 count=1 2>/dev/null | od -An -c; exec /bin/sleep 600",
    ))
    .await;
    let mut connection = host.connect().await;
    connection.start_read(Position::default()).await.unwrap();
    output_until(&mut connection, "ready").await;
    // sleep-ok: let dd reach its read before the input; the property needs it already waiting.
    tokio::time::sleep(Duration::from_millis(200)).await;
    connection.send_input(b"\x1b\r").await.unwrap();
    let text = output_until(&mut connection, "\\r").await;
    assert!(
        text.contains("033  \\r") || text.contains("033 \\r"),
        "{text:?}"
    );
}

/// Input whose fate is unknown when its connection ends is never sent
/// again, and the program receives each byte at most once.
///
/// Why: some of that input may already be in the program (a paste, a
/// submitted line), so resending it after reconnecting would repeat it; the
/// counts must instead say what is known written and what is unknown.
/// Spec: with a raw-mode program that does not read yet, input of numbered
/// 8-byte records is sent until the window is full and no acknowledgement
/// frees it; a second client then takes over, and the first sees its
/// connection close with some input unknown and none discarded. Once the
/// program reads (`cat` to a file), a marker sent on the second connection
/// is written, and the second connection's counts hold only the marker.
/// The file then holds a prefix of what the first connection sent, at
/// least what was acknowledged written and at most that plus the unknown
/// part, followed by the marker.
#[farhelm_testtrace::test]
async fn input_is_never_resent_after_a_disconnect() {
    let scratch = farhelm_teststate::tempdir().expect("scratch dir");
    let go = scratch.path().join("go");
    let out = scratch.path().join("out");
    let host = Host::start(shell(&format!(
        "stty raw -echo; echo ready; while [ ! -e {go} ]; do /bin/sleep 0.05; done; exec /bin/cat > {out}",
        go = go.display(),
        out = out.display()
    )))
    .await;
    let mut first = host.connect().await;
    first.start_read(Position::default()).await.unwrap();
    output_until(&mut first, "ready").await;

    let mut sent = Vec::new();
    let records = MAX_INPUT / 8;
    'fill: for chunk in 0.. {
        assert!(chunk < 64, "premise: the pty never filled up");
        let data: Vec<u8> = (0..records)
            .flat_map(|i| format!("{:07}\n", chunk * records + i).into_bytes())
            .collect();
        loop {
            match first.send_input(&data).await {
                Ok(_) => {
                    sent.extend_from_slice(&data);
                    break;
                }
                Err(ClientError::InputWindowFull { .. }) => {
                    // Room comes back only through acknowledgements; when
                    // none comes for a while, the program is not reading
                    // and the window is full of input whose fate is open.
                    match tokio::time::timeout(Duration::from_millis(300), first.next_event()).await
                    {
                        Err(_) => break 'fill,
                        Ok(event) => {
                            event.expect("the stream");
                        }
                    }
                }
                Err(error) => panic!("sending input: {error}"),
            }
        }
    }

    let mut second = host.connect().await;
    loop {
        match next(&mut first).await {
            Ok(_) => continue,
            Err(error) => {
                assert_eq!(error, ClientError::Closed);
                break;
            }
        }
    }
    let fate = first.input_counts();
    assert_eq!(fate.sent, sent.len() as u64);
    assert_eq!(
        fate.discarded, 0,
        "premise: the program had a terminal throughout"
    );
    assert!(
        fate.unknown() > 0,
        "premise: input was in flight at the takeover: {fate:?}"
    );

    std::fs::write(&go, b"").unwrap();
    let marker = b"MARKER-OF-THE-SECOND-CONNECTION\n";
    let ticket = second.send_input(marker).await.unwrap();
    let counts = acknowledged(&mut second, ticket.end).await;
    assert_eq!(
        (counts.sent, counts.written),
        (marker.len() as u64, marker.len() as u64),
        "the second connection sent more than its marker"
    );
    assert!(
        eventually(|| std::fs::read(&out).is_ok_and(|bytes| bytes.ends_with(marker))),
        "the marker never reached the program"
    );
    let received = std::fs::read(&out).unwrap();
    let received = &received[..received.len() - marker.len()];
    let length = received.len() as u64;
    assert!(
        fate.written <= length && length <= fate.written + fate.unknown(),
        "received {length} bytes, but {fate:?}"
    );
    assert_eq!(
        received,
        &sent[..received.len()],
        "the program got a byte twice or out of order"
    );
}

/// Input after the child has exited and nothing holds the terminal is
/// acknowledged as discarded.
///
/// Why: a caller waiting on input to a finished terminal must learn it went
/// nowhere, rather than count it as typed or wait forever. Spec: after the
/// exit record of a child that exits at once, an input of 8 bytes is
/// acknowledged with 0 written and 8 discarded.
#[farhelm_testtrace::test]
async fn input_after_the_child_exits_is_discarded() {
    let host = Host::start(shell("exit 0")).await;
    let mut connection = host.connect().await;
    connection.start_read(Position::default()).await.unwrap();
    while !matches!(
        next(&mut connection).await.unwrap(),
        HostEvent::Exited { .. }
    ) {}
    let ticket = connection.send_input(b"too late").await.unwrap();
    let counts = acknowledged(&mut connection, ticket.end).await;
    assert_eq!((counts.written, counts.discarded), (0, 8));
}

// ---- Ending a host ------------------------------------------------------------------

/// `Shutdown` gets `Goodbye`, and the host then ends and removes its socket.
///
/// Why: this is how Delete and a tab's close end a host; the caller must be
/// able to tell that the host acted. Spec: `shutdown` reports `Goodbye`;
/// the host process ends and its socket file goes, each within the wait.
#[farhelm_testtrace::test]
async fn shutdown_ends_the_host() {
    let host = Host::start(shell("exec /bin/sleep 600")).await;
    let connection = host.connect().await;
    let outcome = connection.shutdown().await.unwrap();
    assert!(outcome.goodbye, "{outcome:?}");
    assert!(gone_within(host.pid), "the host survived Shutdown");
    assert!(
        eventually(|| !host.socket.exists()),
        "the host left its socket file"
    );
    assert!(host.state.path().exists());
}
