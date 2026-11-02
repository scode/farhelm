//! Real terminal hosts, started by the supervisor's spawn
//! (`farhelm_supervisor::hosts`) from the real `farhelm` binary, and ended
//! by the test-state cleanup (`farhelm_teststate`).
//!
//! These live here rather than in the supervisor's own tests because a host
//! forks: started in-process, its second stage would be a fork of a
//! multi-threaded test runner. Here the host is `farhelm internal
//! terminal-host`, exactly the program and entry point a supervisor runs, so
//! what is tested is the shipped path. Run under nextest (one test per
//! process), which the zombie check below relies on.
//!
//! Sections: helpers that observe processes portably (`/proc` on Linux,
//! `ps` on macOS); a guard that ends every host a test starts, on every
//! path; the spawn's tests (readiness, failure, environment and command
//! line); the cleanup's tests (a normal run, an interrupted run, nested
//! state directories, and a host that cannot be confirmed ended).
//!
//! Every host a test starts is ended by its [`HostGuard`], confirmed by
//! pid; the cleanup under test is never the only thing standing between a
//! failing test and a leaked host.

use farhelm_supervisor::hosts::{ChildSpec, HostRole, HostTag, SpawnError, spawn_host};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::FileTypeExt as _;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use terminal_host::protocol::{
    ClientMessage, HostMessage, MAX_FRAME, Size, decode_host, encode_client, split_frame,
};

/// How long any single wait in these tests may take before it fails.
const WAIT: Duration = Duration::from_secs(10);

// ---- Fixtures -----------------------------------------------------------------

fn farhelm() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_farhelm"))
}

/// A child that runs until its host ends it.
fn sleeper() -> ChildSpec {
    ChildSpec {
        argv: vec!["/bin/sleep".into(), "600".into()],
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

/// Start a sleeping host in `state_dir` and take charge of it.
async fn start_host(state_dir: &Path) -> HostGuard {
    let spawned = spawn_host(&farhelm(), state_dir, sleeper(), &agent_tag())
        .await
        .expect("the host starts");
    HostGuard::adopt(spawned.socket)
}

/// A dead run's directory as the sweep sees one: a family name with its lock
/// file present and nobody holding it, which is what a killed harness
/// leaves.
fn dead_run(root: &Path, name: &str) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir(&dir).unwrap();
    std::fs::File::create(dir.join(farhelm_teststate::LOCK_FILE_NAME)).unwrap();
    dir
}

/// A sweep that judges every dead run reapable at once.
fn instant_policy() -> farhelm_teststate::SweepPolicy {
    farhelm_teststate::SweepPolicy {
        grace: Duration::ZERO,
        backstop: Duration::ZERO,
    }
}

// ---- Observing processes --------------------------------------------------------

/// The pid of the process listening on `socket`, from the connection's peer
/// credentials (the listener's, whether or not it ever accepts).
fn listener_pid(socket: &Path) -> Option<u32> {
    let stream = UnixStream::connect(socket).ok()?;
    use std::os::fd::AsRawFd as _;
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
                (&mut cred as *mut libc::ucred).cast(),
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
                (&mut pid as *mut libc::pid_t).cast(),
                &mut len,
            )
        };
        (result == 0).then_some(pid)?
    };
    // A pid of 0 or 1 would make a signal hit this test's process group or
    // init.
    (pid > 1).then_some(pid as u32)
}

/// One line per process: pid, parent pid, state and command line, from
/// `ps`, which reads the same table on Linux and macOS.
fn process_table() -> Vec<(u32, u32, String, String)> {
    let output = Command::new("ps")
        .args(["-A", "-ww", "-o", "pid=,ppid=,stat=,command="])
        .output()
        .expect("running ps");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse().ok()?;
            let ppid = fields.next()?.parse().ok()?;
            let stat = fields.next()?.to_string();
            let command = fields.collect::<Vec<_>>().join(" ");
            Some((pid, ppid, stat, command))
        })
        .collect()
}

/// Whether `pid` exists and is not a zombie (`kill(pid, 0)` alone succeeds
/// for a zombie too). Reads that one process's state, not the whole table:
/// the waits below ask this every 20 ms on a machine other agents share.
fn running(pid: u32) -> bool {
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
    #[cfg(target_os = "macos")]
    let state = {
        let output = Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .expect("running ps");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    !state.is_empty() && !state.starts_with('Z')
}

/// Whether `pid` stops running within [`WAIT`].
fn gone_within(pid: u32) -> bool {
    eventually(|| !running(pid))
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
        // sleep-ok: polling interval for an observable owned by the test.
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// `pid`'s command line, its arguments joined by spaces.
fn command_line(pid: u32) -> String {
    #[cfg(target_os = "linux")]
    {
        let raw = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
        String::from_utf8_lossy(&raw).replace('\0', " ")
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ps")
            .args(["-ww", "-o", "command=", "-p", &pid.to_string()])
            .output()
            .expect("running ps");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}

/// `pid`'s environment as it was when it started its program. On macOS `ps
/// -E` prints it after the command line, so the result holds both; the
/// callers look only for names and values that never appear on these
/// command lines.
fn environment(pid: u32) -> String {
    #[cfg(target_os = "linux")]
    {
        let raw = std::fs::read(format!("/proc/{pid}/environ")).expect("reading the environment");
        String::from_utf8_lossy(&raw).replace('\0', "\n")
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ps")
            .args(["-E", "-ww", "-o", "command=", "-p", &pid.to_string()])
            .output()
            .expect("running ps");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}

// ---- Ending hosts on every path ---------------------------------------------------

/// A host this test started, ended when the guard drops, so that a failing
/// assertion cannot leave it running: `Shutdown` through its socket, then
/// SIGKILL if it still runs.
///
/// The pid is signalled only while its command line still names this
/// guard's socket path, which is unique to the test: a pid that was reused
/// since the host ended belongs to someone else.
struct HostGuard {
    socket: PathBuf,
    pid: u32,
}

impl HostGuard {
    fn adopt(socket: PathBuf) -> HostGuard {
        let pid = listener_pid(&socket).expect("the host's pid from its socket");
        HostGuard { socket, pid }
    }

    fn is_ours(&self) -> bool {
        running(self.pid) && command_line(self.pid).contains(self.socket.to_str().unwrap())
    }

    /// Send `signal` to the host, which must still be this guard's (a
    /// stopped host still counts: it exists and its command line is
    /// unchanged).
    fn signal(&self, signal: libc::c_int) {
        // SAFETY: signal 0 only checks for the process.
        let exists = unsafe { libc::kill(self.pid as libc::pid_t, 0) } == 0;
        assert!(
            exists && command_line(self.pid).contains(self.socket.to_str().unwrap()),
            "the host {} is no longer this test's",
            self.pid
        );
        // SAFETY: signalling the host this guard started, checked above.
        assert_eq!(unsafe { libc::kill(self.pid as libc::pid_t, signal) }, 0);
    }
}

impl Drop for HostGuard {
    fn drop(&mut self) {
        if !self.is_ours() {
            return;
        }
        if let Ok(mut stream) = UnixStream::connect(&self.socket) {
            let mut bytes = encode_client(&ClientMessage::Hello {
                version: 1,
                take_over: true,
            });
            bytes.extend(encode_client(&ClientMessage::Shutdown));
            let _ = stream.write_all(&bytes);
        }
        if !gone_within(self.pid) && self.is_ours() {
            // SAFETY: signalling the host this guard started, checked above.
            unsafe { libc::kill(self.pid as libc::pid_t, libc::SIGKILL) };
            if !gone_within(self.pid) {
                eprintln!(
                    "fixture: the host (pid {}) on {} did not end",
                    self.pid,
                    self.socket.display()
                );
            }
        }
    }
}

/// Connect, take over, and return the host's `Info` reply.
fn info(socket: &Path) -> terminal_host::protocol::Info {
    let mut stream = UnixStream::connect(socket).expect("connecting to the host");
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let mut bytes = encode_client(&ClientMessage::Hello {
        version: 1,
        take_over: true,
    });
    bytes.extend(encode_client(&ClientMessage::Info));
    stream.write_all(&bytes).unwrap();
    let deadline = Instant::now() + WAIT;
    let mut buf = Vec::new();
    loop {
        while let Some(frame) = split_frame(&buf, MAX_FRAME).expect("a well-formed frame") {
            let message = decode_host(frame, 1).expect("a well-formed message");
            let consumed = frame.consumed;
            buf.drain(..consumed);
            if let HostMessage::InfoReply(info) = message {
                return info;
            }
        }
        assert!(Instant::now() < deadline, "no Info reply within {WAIT:?}");
        let mut chunk = [0u8; 4096];
        match stream.read(&mut chunk) {
            Ok(0) => panic!("the host closed the connection"),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => panic!("reading from the host: {error}"),
        }
    }
}

// ---- Spawning ----------------------------------------------------------------------

/// When the spawn returns, the host is listening at its socket, its first
/// stage is reaped, and it holds the tag it was given.
///
/// Why: the supervisor connects right after a spawn and records the host;
/// a spawn that returned early would race its own host, and a first stage
/// left unreaped is a zombie per terminal for the supervisor's lifetime.
/// Spec: the socket lies in `<state>/hosts/`, exists and is a socket; the
/// listener's parent is not this test process (the first stage that forked
/// it has exited); no child of this test process is a zombie; and the
/// host's `Info` carries the encoded tag, which decodes back to the tag,
/// with its child running.
#[farhelm_testtrace::test]
async fn a_spawned_host_is_listening_and_its_first_stage_is_reaped() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let tag = agent_tag();
    let spawned = spawn_host(&farhelm(), state.path(), sleeper(), &tag)
        .await
        .expect("the host starts");
    let host = HostGuard::adopt(spawned.socket.clone());
    assert_eq!(spawned.socket.parent(), Some(&*state.path().join("hosts")));
    let meta = std::fs::symlink_metadata(&spawned.socket).expect("the socket exists");
    assert!(meta.file_type().is_socket());

    let me = std::process::id();
    let table = process_table();
    let parent = table
        .iter()
        .find(|(pid, ..)| *pid == host.pid)
        .map(|(_, ppid, ..)| *ppid)
        .expect("the host is in the process table");
    assert_ne!(parent, me, "the host is still this process's child");
    let zombies: Vec<_> = table
        .iter()
        .filter(|(_, ppid, stat, _)| *ppid == me && stat.starts_with('Z'))
        .collect();
    assert!(zombies.is_empty(), "zombie children left: {zombies:?}");

    let info = info(&spawned.socket);
    assert_eq!(info.tag, tag.encode().unwrap());
    assert_eq!(HostTag::decode(&info.tag), Ok(tag));
    assert_eq!(info.host_pid, host.pid);
    assert_eq!(info.child_exit, None, "the child runs");
}

/// A host whose child cannot start reports why, and leaves no process and
/// no socket.
///
/// Why: a terminal that fails to start must not leave a host behind that
/// nothing will ever end, nor a socket that a later supervisor would take
/// for a host that died. Spec: a child program that does not exist gives
/// `Refused` with exit status 1 and a nonempty message; the socket
/// directory is empty; and within the wait no process's command line names
/// the socket directory (the host's second stage, a fork of the first,
/// would).
#[farhelm_testtrace::test]
async fn a_failed_spawn_leaves_nothing_running_and_no_socket() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let mut child = sleeper();
    child.argv = vec!["/nonexistent/farhelm-test-program".into()];
    let result = spawn_host(&farhelm(), state.path(), child, &agent_tag()).await;
    match &result {
        Err(SpawnError::Refused { status, message }) => {
            assert_eq!(*status, 1);
            assert!(!message.is_empty(), "the host said why");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    let hosts = state.path().join("hosts");
    assert_eq!(std::fs::read_dir(&hosts).unwrap().count(), 0);
    let hosts = hosts.to_str().unwrap().to_owned();
    let left = || {
        process_table()
            .into_iter()
            .filter(|(_, _, stat, command)| !stat.starts_with('Z') && command.contains(&hosts))
            .collect::<Vec<_>>()
    };
    assert!(
        eventually(|| left().is_empty()),
        "a process of the failed start is still running: {:?}",
        left()
    );
}

/// The host's own environment carries no session markers and no
/// credential, and its command line carries no credential, while its
/// child's environment has both.
///
/// Why: Stop's process sweep finds a session's processes by the
/// `FARHELM_*` markers in their environment, and must find the child, never
/// the host; and a host lives for weeks with its command line and
/// environment readable by any same-user process, while a tab's environment
/// holds `FARHELM_SESSION_TOKEN`. Spec: with markers and a token in the
/// child spec's environment, the child (a shell that records its pid, then
/// becomes `sleep`) is the host's child and its environment holds both
/// marker values and the token; the host's environment holds no `FARHELM_`
/// name and not the token (and on Linux is empty); and the host's command
/// line names its socket but not the token.
///
/// On macOS this cannot catch an inherited environment: `ps -E` mixes the
/// environment into the command line, so only the marker and token checks
/// run, and the recorder scrubs `FARHELM_*` from this test process while the
/// token was never in it. The supervisor's own unit test with a stand-in
/// first stage (`hosts::spawn`) pins the empty environment on every
/// platform.
#[farhelm_testtrace::test]
async fn the_host_carries_no_markers_and_no_credential_but_its_child_does() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let pid_file = state.path().join("child.pid");
    let session = "0f8e2b9c-4c1d-4e7a-9a53-7d2f61c0b8e4";
    let tab = "6b1c0d3e-2f4a-4b5c-8d6e-7f8091a2b3c4";
    let token = "credential-6c1f0e2a-never-on-a-host";
    let mut child = sleeper();
    child.argv = vec![
        "/bin/sh".into(),
        "-c".into(),
        format!("echo $$ > {}; exec /bin/sleep 600", pid_file.display()).into(),
    ];
    child.env.extend([
        ("FARHELM_SESSION_ID".into(), session.into()),
        ("FARHELM_TAB_ID".into(), tab.into()),
        ("FARHELM_SESSION_TOKEN".into(), token.into()),
    ]);
    let tag = HostTag {
        session_id: session.into(),
        created_ns: farhelm_supervisor::hosts::tag::creation_stamp(),
        role: HostRole::Tab {
            tab_id: tab.into(),
            scoped: false,
        },
    };
    let spawned = spawn_host(&farhelm(), state.path(), child, &tag)
        .await
        .expect("the host starts");
    let host = HostGuard::adopt(spawned.socket.clone());
    let read_child = || {
        std::fs::read_to_string(&pid_file)
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok())
    };
    assert!(
        eventually(|| read_child().is_some()),
        "the child never wrote its pid"
    );
    let child_pid = read_child().unwrap();
    let parent = process_table()
        .into_iter()
        .find(|(pid, ..)| *pid == child_pid)
        .map(|(_, ppid, ..)| ppid);
    assert_eq!(parent, Some(host.pid), "premise: this is the host's child");

    let child_env = environment(child_pid);
    for expected in [
        format!("FARHELM_SESSION_ID={session}"),
        format!("FARHELM_TAB_ID={tab}"),
        format!("FARHELM_SESSION_TOKEN={token}"),
    ] {
        assert!(
            child_env.contains(&expected),
            "premise: the child's environment has {expected}: {child_env}"
        );
    }
    let host_env = environment(host.pid);
    // The recorder scrubs `FARHELM_*` from this test process, so the marker
    // check alone could not tell an inherited environment from an empty
    // one; on Linux, where the environment reads apart from the command
    // line, the host's must be empty outright.
    #[cfg(target_os = "linux")]
    assert!(
        host_env.trim().is_empty(),
        "the host inherited an environment: {host_env}"
    );
    assert!(
        !host_env.contains("FARHELM_"),
        "the host's environment has Farhelm markers: {host_env}"
    );
    assert!(
        !host_env.contains(token),
        "the host's environment has the token"
    );
    let host_command = command_line(host.pid);
    assert!(
        host_command.contains(spawned.socket.to_str().unwrap()),
        "premise: the host's command line names its socket: {host_command}"
    );
    assert!(
        !host_command.contains(token),
        "the host's command line has the token: {host_command}"
    );
}

// ---- Test-state cleanup ----------------------------------------------------------------

/// Dropping a test directory ends the hosts in it before removing it.
///
/// Why: a test whose supervisor starts terminals in hosts ends without
/// stopping them whenever it fails an assertion, and a host whose directory
/// is deleted keeps running with no socket to reach it by. Spec: with a
/// live host in a `TestDir`'s state directory, dropping the `TestDir` ends
/// the host (its pid stops running within the wait) and removes the
/// directory.
#[farhelm_testtrace::test]
async fn dropping_a_test_directory_ends_its_hosts() {
    let root = tempfile::tempdir().unwrap();
    let run = farhelm_teststate::tempdir_in(root.path()).expect("a run directory");
    let container = run.path().parent().unwrap().to_path_buf();
    let host = start_host(run.path()).await;
    assert!(host.is_ours(), "premise: the host runs");
    drop(run);
    assert!(gone_within(host.pid), "the host survived its directory");
    assert!(!container.exists(), "the directory was removed");
}

/// The sweep ends the hosts a killed run left behind, including those of a
/// nested state directory, and then reclaims the run.
///
/// Why: a run killed with SIGKILL never drops its directories, so the sweep
/// is the only thing that ever ends its hosts; and the stack script nests a
/// second supervisor's state (`remote/`) whose hosts a top-level-only search
/// would miss. Spec: a dead Rust-harness run (`fh-it.`, state in `d/`) with
/// one host, and a dead stack-script run (`fh-e2e.`) with a host at its top
/// level and one in `remote/`, are both reaped by one sweep, and all three
/// hosts stop running.
#[farhelm_testtrace::test]
async fn the_sweep_ends_the_hosts_of_dead_runs_including_nested_ones() {
    let root = tempfile::tempdir().unwrap();
    let rust_run = dead_run(root.path(), "fh-it.dead01");
    std::fs::create_dir(rust_run.join("d")).unwrap();
    let stack_run = dead_run(root.path(), "fh-e2e.nest01");
    std::fs::create_dir(stack_run.join("remote")).unwrap();
    let hosts = [
        start_host(&rust_run.join("d")).await,
        start_host(&stack_run).await,
        start_host(&stack_run.join("remote")).await,
    ];
    for host in &hosts {
        assert!(host.is_ours(), "premise: the host runs");
    }

    let mut outcome = farhelm_teststate::sweep(root.path(), &instant_policy());
    outcome.reaped.sort();
    let mut runs = vec![rust_run, stack_run];
    runs.sort();
    assert_eq!(outcome.reaped, runs);
    for host in &hosts {
        assert!(
            gone_within(host.pid),
            "the host at {} survived the sweep",
            host.socket.display()
        );
    }
}

/// A run whose host cannot be confirmed ended is kept, socket and all, and
/// a later sweep reclaims it.
///
/// Why: deleting the directory of a host that is still running would leave
/// it unreachable for good; keeping it lets a later sweep retry through the
/// socket. Spec: with the host stopped (SIGSTOP), a sweep reaps nothing and
/// leaves the run, the socket and the host process in place; after SIGCONT,
/// a second sweep reaps the run and the host stops running. (The stopped
/// host still queued the first sweep's connection, and the host acts on its
/// `Shutdown` once it runs again, so either sweep's request may be the one
/// that ends it; what is pinned is that the run is kept, then reclaimed.)
#[farhelm_testtrace::test]
async fn a_run_whose_host_cannot_be_confirmed_ended_is_kept_for_a_retry() {
    let root = tempfile::tempdir().unwrap();
    let run = dead_run(root.path(), "fh-it.stop01");
    let host = start_host(&run).await;
    assert!(host.is_ours(), "premise: the host runs");
    host.signal(libc::SIGSTOP);

    let outcome = farhelm_teststate::sweep(root.path(), &instant_policy());
    assert!(outcome.reaped.is_empty(), "{:?}", outcome.reaped);
    assert!(run.exists() && host.socket.exists(), "the run was not kept");
    // SAFETY: signal 0 only checks for the process (stopped, not gone).
    assert_eq!(unsafe { libc::kill(host.pid as libc::pid_t, 0) }, 0);

    host.signal(libc::SIGCONT);
    let outcome = farhelm_teststate::sweep(root.path(), &instant_policy());
    assert_eq!(outcome.reaped, vec![run]);
    assert!(gone_within(host.pid), "the host survived the retry");
}

/// A test directory whose host cannot be confirmed ended is kept when it
/// drops, with its lock released, so the sweep reclaims it later.
///
/// Why: a test's own cleanup is the common path, and deleting the directory
/// of a host that still runs would strand it exactly as a sweep would.
/// Spec: with the host stopped, dropping the `TestDir` leaves its container
/// and the socket in place; after SIGCONT, a sweep (which needs the lock to
/// be free) reaps the container and the host stops running.
#[farhelm_testtrace::test]
async fn a_test_directory_whose_host_cannot_be_confirmed_ended_is_kept_on_drop() {
    let root = tempfile::tempdir().unwrap();
    let run = farhelm_teststate::tempdir_in(root.path()).expect("a run directory");
    let container = run.path().parent().unwrap().to_path_buf();
    let host = start_host(run.path()).await;
    assert!(host.is_ours(), "premise: the host runs");
    host.signal(libc::SIGSTOP);
    drop(run);
    assert!(
        container.exists() && host.socket.exists(),
        "the directory of an unconfirmed host was removed"
    );
    host.signal(libc::SIGCONT);
    let outcome = farhelm_teststate::sweep(root.path(), &instant_policy());
    assert_eq!(outcome.reaped, vec![container]);
    assert!(gone_within(host.pid), "the host survived the sweep");
}

/// Closing a test directory whose host cannot be confirmed ended fails and
/// keeps the directory.
///
/// Why: `close` is for tests that must know the directory went; one that
/// silently kept it, or removed it under a running host, would mislead them
/// or strand the host. Spec: with the host stopped, `close` returns an
/// error and the container and socket remain; after SIGCONT a sweep reaps
/// the container and the host stops running.
#[farhelm_testtrace::test]
async fn closing_a_test_directory_whose_host_cannot_be_confirmed_ended_fails() {
    let root = tempfile::tempdir().unwrap();
    let run = farhelm_teststate::tempdir_in(root.path()).expect("a run directory");
    let container = run.path().parent().unwrap().to_path_buf();
    let host = start_host(run.path()).await;
    assert!(host.is_ours(), "premise: the host runs");
    host.signal(libc::SIGSTOP);
    let closed = run.close();
    assert!(closed.is_err(), "close confirmed an unconfirmed host");
    assert!(container.exists() && host.socket.exists());
    host.signal(libc::SIGCONT);
    let outcome = farhelm_teststate::sweep(root.path(), &instant_policy());
    assert_eq!(outcome.reaped, vec![container]);
    assert!(gone_within(host.pid), "the host survived the sweep");
}

/// A stopped host whose listen backlog is full is kept, not judged gone
/// (Linux).
///
/// Why: a host that cannot accept is still running, and a cleanup that read
/// its refused or unanswered connections as "nothing listens" would unlink
/// its socket and strand it. Spec: with the host stopped and its backlog
/// filled with non-blocking connections until the kernel refuses one more
/// (`EAGAIN`), a sweep reaps nothing and the socket and the host remain.
///
/// Linux only. There a full backlog answers `EAGAIN`, which the cleanup
/// never takes as evidence. macOS answers `ECONNREFUSED` instead, the same
/// as for a socket nothing listens on; the cleanup believes that only after
/// it has lasted `STALE_REFUSAL_WINDOW`, and a host that stays stopped with a
/// full backlog longer than that is indistinguishable from a dead one there,
/// so this regression cannot hold on macOS by construction.
#[cfg(target_os = "linux")]
#[farhelm_testtrace::test]
async fn a_stopped_host_with_a_full_backlog_is_kept() {
    let root = tempfile::tempdir().unwrap();
    let run = dead_run(root.path(), "fh-it.full01");
    let host = start_host(&run).await;
    assert!(host.is_ours(), "premise: the host runs");
    host.signal(libc::SIGSTOP);
    let mut queued = Vec::new();
    let full = loop {
        match connect_nonblocking(&host.socket) {
            Ok(fd) => queued.push(fd),
            Err(error) if error.raw_os_error() == Some(libc::EAGAIN) => break true,
            Err(error) => panic!("filling the backlog: {error}"),
        }
        if queued.len() > 4096 {
            break false;
        }
    };
    assert!(full, "premise: the backlog fills");

    let outcome = farhelm_teststate::sweep(root.path(), &instant_policy());
    assert!(outcome.reaped.is_empty(), "{:?}", outcome.reaped);
    assert!(run.exists() && host.socket.exists(), "the run was not kept");
    // SAFETY: signal 0 only checks for the process (stopped, not gone).
    assert_eq!(unsafe { libc::kill(host.pid as libc::pid_t, 0) }, 0);
    drop(queued);
    host.signal(libc::SIGCONT);
}

/// A non-blocking connection to `socket`, for filling a listener's backlog
/// without ever waiting in `connect`.
#[cfg(target_os = "linux")]
fn connect_nonblocking(socket: &Path) -> std::io::Result<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd as _;
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
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    if raw < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: raw is a fresh descriptor nothing else owns.
    let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(raw) };
    use std::os::fd::AsRawFd as _;
    // SAFETY: address and length describe initialized storage; fd is live.
    if unsafe { libc::connect(fd.as_raw_fd(), (&raw const address).cast(), length) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(fd)
}
