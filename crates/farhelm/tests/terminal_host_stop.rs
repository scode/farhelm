//! `farhelm internal stop-terminal-hosts` against real terminal hosts,
//! started by the supervisor's spawn from the real `farhelm` binary and
//! stopped by that same binary's command.
//!
//! The command is what removes a host's last chance of outliving an
//! install: a newer helm runs it on remote hosts whose `farhelm` may be
//! older, so its command line and the meaning of exit status 0 are a
//! contract (SPEC_impl.md, "Terminal host"). These tests pin that contract
//! on the shipped path, and every success they accept is checked against
//! the host processes at the moment the command returns, not after: live
//! hosts are shut down and their programs get the hangup, a killed host's
//! socket file is removed, a stopped host is ended by the verified signal,
//! a state directory reached through a symlink stops its live hosts but
//! signals none, a `hosts` that is itself a symlink leads nowhere (the
//! hosts it points at are left alone), a missing directory succeeds,
//! something that cannot be confirmed stopped (a stand-in that is not a
//! host, one that acknowledges `Shutdown` and stays) fails the run without
//! being signalled, a relative or empty state directory is a usage error,
//! and a stderr nobody reads cannot hold the exit back. The search's edges, the budget and the confirmation rule are
//! also unit tests in the supervisor (`hosts::stop::tests`).
//!
//! Hosts are started here, never inside a test process, for the reason
//! `terminal_host_spawn.rs` gives (a host forks). Every host a test starts
//! is ended by its [`HostGuard`], confirmed by pid, whatever the command
//! under test did.
//!
//! Sections: fixtures (starting hosts, the guard, stand-ins, running the
//! command); the tests.

use farhelm_supervisor::hosts::stop::COMMAND_BUDGET;
use farhelm_supervisor::hosts::{ChildSpec, HostConnection, HostRole, HostTag, spawn_host};
use std::io::{Read as _, Write as _};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};
use terminal_host::protocol::{
    ClientMessage, HostMessage, MAX_FRAME, Size, decode_client, encode_host, split_frame,
};

/// How long any single wait in these tests may take before it fails.
const WAIT: Duration = Duration::from_secs(10);

/// How long one run of the command may take before the test kills it: well
/// past the command's own bound, so that a run that overstays is reported
/// by [`within_budget`] rather than cut short here.
const RUN_WAIT: Duration = Duration::from_secs(90);

/// Whether a run took no longer than the command's own bound
/// ([`COMMAND_BUDGET`]), with room for starting a process on a loaded
/// machine.
fn within_budget(took: Duration) -> bool {
    took <= COMMAND_BUDGET + Duration::from_secs(5)
}

// ---- Fixtures ------------------------------------------------------------------

fn farhelm() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_farhelm"))
}

/// A child running `script` under `/bin/sh`.
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

/// Start a host running `script` in `state_dir`'s socket directory and
/// take charge of it.
async fn start_host(state_dir: &Path, script: &str) -> HostGuard {
    let spawned = spawn_host(&farhelm(), state_dir, shell(script), &agent_tag())
        .await
        .expect("the host starts");
    let connection = HostConnection::connect(&spawned.socket)
        .await
        .expect("connecting to the new host");
    let pid = connection
        .peer()
        .expect("the host's pid from its socket")
        .pid;
    HostGuard {
        socket: spawned.socket,
        pid,
    }
}

/// A real host a test started. Dropping it ends the host if it still runs:
/// `Shutdown` through the test-state cleanup's own blocking client, then
/// SIGCONT and SIGKILL, so a test never relies on the command under test to
/// clean up after it. The pid is signalled only while its command line
/// still names this host's socket, which is unique to the test.
struct HostGuard {
    socket: PathBuf,
    pid: u32,
}

impl HostGuard {
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
}

impl Drop for HostGuard {
    fn drop(&mut self) {
        if !self.is_ours() {
            return;
        }
        let stop = farhelm_teststate::hosts::stop_host(&self.socket, Instant::now() + WAIT);
        if eventually(|| !exists(self.pid)) || !self.is_ours() {
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
        if !eventually(|| !exists(self.pid)) {
            eprintln!("fixture: the host {} survived SIGKILL", self.pid);
        }
    }
}

/// Whether `pid` exists and is not a zombie (`kill(pid, 0)` alone succeeds
/// for a zombie too).
fn exists(pid: u32) -> bool {
    // SAFETY: signal 0 only checks for the process.
    if unsafe { libc::kill(pid as libc::pid_t, 0) } != 0 {
        return false;
    }
    let state = process_state(pid);
    !state.is_empty() && !state.starts_with('Z')
}

/// `pid`'s state letter (`T` when stopped, `Z` a zombie), as `ps` reports
/// it, which reads it the same way on Linux and macOS.
fn process_state(pid: u32) -> String {
    let output = Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .expect("running ps");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// `pid`'s command line, from `ps`.
fn command_line(pid: u32) -> String {
    let output = Command::new("ps")
        .args(["-ww", "-o", "command=", "-p", &pid.to_string()])
        .output()
        .expect("running ps");
    String::from_utf8_lossy(&output.stdout).into_owned()
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

/// What one run of the command did.
struct Run {
    status: ExitStatus,
    stdout: String,
    stderr: String,
    took: Duration,
}

/// Run `farhelm internal stop-terminal-hosts --state-dir <state_dir>` and
/// wait for it, killing it and failing the test past [`RUN_WAIT`].
///
/// The flag is always given: the command has no default state directory by
/// design, and a test that dropped the flag must never be able to reach the
/// state directory of a Farhelm actually running on this machine.
fn run_stop(state_dir: &Path) -> Run {
    let started = Instant::now();
    let child = stop_command(state_dir.as_os_str())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("starting the command");
    let pid = child.id();
    // Waited for on a thread of its own, so that a run that hangs fails the
    // test at the bound instead of holding it forever.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let output = match rx.recv_timeout(RUN_WAIT) {
        Ok(output) => output.expect("waiting for the command"),
        Err(_) => {
            // SAFETY: the command this test started; it has not been
            // reaped, since its waiting thread has not returned.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
            panic!("the command ran past {RUN_WAIT:?}");
        }
    };
    Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        took: started.elapsed(),
    }
}

/// The command with `--state-dir <value>` and stdin closed; the caller
/// sets its output.
fn stop_command(state_dir: &std::ffi::OsStr) -> Command {
    let mut command = Command::new(farhelm());
    command
        .args(["internal", "stop-terminal-hosts", "--state-dir"])
        .arg(state_dir)
        .stdin(Stdio::null());
    command
}

/// Wait for `child` until [`RUN_WAIT`], killing it and failing the test
/// past that, without reading its output.
fn wait_bounded(child: Child) -> (ExitStatus, Duration) {
    let started = Instant::now();
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut child = child;
    std::thread::spawn(move || {
        let _ = tx.send(child.wait());
    });
    match rx.recv_timeout(RUN_WAIT) {
        Ok(status) => (status.expect("waiting for the command"), started.elapsed()),
        Err(_) => {
            // SAFETY: the command this test started, not yet reaped.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
            panic!("the command ran past {RUN_WAIT:?}");
        }
    }
}

/// A stand-in that speaks the protocol from this test process: on its one
/// connection it reads `Hello`, answers `Welcome`, reads `Shutdown`,
/// answers `Goodbye`, and closes the connection, while it keeps listening
/// (its socket file stays) until the returned listener is dropped. It is
/// what a host that acknowledges `Shutdown` and does not end looks like.
fn goodbye_stand_in(listener: UnixListener) -> std::thread::JoinHandle<UnixListener> {
    std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + WAIT;
        let conn = loop {
            match listener.accept() {
                Ok((conn, _)) => break conn,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "nothing connected");
                    // sleep-ok: polling interval of a stand-in listener waiting for the command under test.
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("accepting: {error}"),
            }
        };
        conn.set_nonblocking(false).unwrap();
        conn.set_read_timeout(Some(WAIT)).unwrap();
        let mut reader = conn.try_clone().unwrap();
        let mut writer = conn;
        let mut buf = Vec::new();
        let mut next = || loop {
            if let Some(frame) = split_frame(&buf, MAX_FRAME).unwrap() {
                let message = decode_client(frame, 1).unwrap();
                let consumed = frame.consumed;
                buf.drain(..consumed);
                return message;
            }
            let mut chunk = [0u8; 256];
            let n = reader.read(&mut chunk).unwrap();
            assert!(n > 0, "the command closed early");
            buf.extend_from_slice(&chunk[..n]);
        };
        assert!(matches!(next(), ClientMessage::Hello { .. }));
        writer
            .write_all(&encode_host(&HostMessage::Welcome { version: 1 }))
            .unwrap();
        assert_eq!(next(), ClientMessage::Shutdown);
        writer
            .write_all(&encode_host(&HostMessage::Goodbye))
            .unwrap();
        listener
    })
}

// ---- The tests -----------------------------------------------------------------

/// Live hosts are shut down through the protocol, their programs get the
/// terminal's hangup, and the run succeeds only once they are gone.
///
/// Why: this is the command's whole purpose; an uninstall that relies on it
/// and leaves a host running leaves a program running in a terminal nobody
/// can reach. A host whose program has exited still runs (it keeps the
/// final screen until told to stop), so it must be stopped too. Spec: with
/// two hosts in one state directory, one whose program runs and one whose
/// program has exited (premises checked through `Info`), the command exits
/// 0 within its bound with nothing on stdout; when it returns, both host
/// processes and both socket files are already gone; the running program
/// ends; and stderr says each host was stopped through the protocol.
#[farhelm_testtrace::test]
async fn live_hosts_are_shut_down_and_their_programs_get_the_hangup() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let running = start_host(state.path(), "exec /bin/sleep 600").await;
    let exited = start_host(state.path(), "exit 3").await;

    let mut connection = HostConnection::connect(&running.socket).await.unwrap();
    let info = connection.info().await.unwrap();
    assert_eq!(info.child_exit, None, "premise: the program runs");
    let program = info.child_pid;
    drop(connection);
    let mut connection = HostConnection::connect(&exited.socket).await.unwrap();
    let deadline = Instant::now() + WAIT;
    while connection.info().await.unwrap().child_exit.is_none() {
        assert!(Instant::now() < deadline, "premise: the program exits");
        // sleep-ok: polling interval for the host to reap its child.
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(connection);
    assert!(
        running.is_ours() && exited.is_ours(),
        "premise: both hosts run"
    );

    let run = run_stop(state.path());
    let gone = [!exists(running.pid), !exists(exited.pid)];
    assert!(run.status.success(), "{:?}: {}", run.status, run.stderr);
    assert_eq!(gone, [true, true], "a host outlived the command's success");
    assert!(within_budget(run.took), "{:?}", run.took);
    assert_eq!(run.stdout, "");
    for host in [&running, &exited] {
        assert!(!host.socket.exists(), "{} was left", host.socket.display());
        assert!(
            run.stderr.contains(&format!(
                "{}: stopped through the protocol",
                host.socket.display()
            )),
            "{}",
            run.stderr
        );
    }
    assert!(
        eventually(|| !exists(program)),
        "the program outlived its host"
    );
}

/// A killed host's socket file is removed, and the run succeeds.
///
/// Why: a host killed with SIGKILL cannot remove its own socket file, and a
/// file left there would keep every later look at the directory guessing
/// whether a host serves it. Spec: after SIGKILL ends a host (premise:
/// its process is gone and the file is still there), the command exits 0,
/// the file is gone, and stderr says the host was already gone.
#[farhelm_testtrace::test]
async fn a_killed_hosts_socket_file_is_removed() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let host = start_host(state.path(), "exec /bin/sleep 600").await;
    host.signal(libc::SIGKILL);
    assert!(eventually(|| !exists(host.pid)), "premise: the host ended");
    assert!(host.socket.exists(), "premise: its socket file stayed");

    let run = run_stop(state.path());
    assert!(run.status.success(), "{:?}: {}", run.status, run.stderr);
    assert!(!host.socket.exists(), "the stale socket file was left");
    assert!(run.stderr.contains("already gone"), "{}", run.stderr);
}

/// A stopped host, which never answers, is ended by the verified SIGTERM,
/// and the run succeeds only once it is gone.
///
/// Why: a host whose process is stopped (or whose event loop is wedged)
/// cannot take `Shutdown`, and the command must still not leave it
/// running. Spec: with the host stopped by SIGSTOP (premise: `ps` reports
/// it stopped), the command exits 0 within its bound; when it returns the
/// host process is already gone; its socket file is gone (SIGTERM, with
/// the SIGCONT after it, lets the host remove it); and stderr says SIGTERM
/// ended it.
#[farhelm_testtrace::test]
async fn a_stopped_host_is_ended_by_the_verified_signal() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let host = start_host(state.path(), "exec /bin/sleep 600").await;
    host.signal(libc::SIGSTOP);
    assert!(
        eventually(|| process_state(host.pid).starts_with('T')),
        "premise: the host stopped"
    );

    let run = run_stop(state.path());
    let gone = !exists(host.pid);
    assert!(run.status.success(), "{:?}: {}", run.status, run.stderr);
    assert!(gone, "the host outlived the command's success");
    assert!(within_budget(run.took), "{:?}", run.took);
    assert!(!host.socket.exists(), "the host's socket file was left");
    assert!(run.stderr.contains("ended by SIGTERM"), "{}", run.stderr);
}

/// Through another spelling of the state directory (a symlink to it), a
/// live host is stopped through the protocol, and a stopped one is left
/// unconfirmed and unsignalled.
///
/// Why: this pins the narrow contract the signal keeps. A caller may reach
/// the state directory through another spelling than the supervisor used;
/// `Shutdown` and its confirmation (by the peer's pid) do not depend on the
/// spelling, so a live host must still stop. The verified signal does: it
/// requires the host's command line to name the socket exactly as given, so
/// a host that cannot take `Shutdown` under another spelling must fail the
/// run rather than be signalled on a guess. Spec: with the state directory
/// given as a symlink to the one the hosts were started in (premise: their
/// command lines spell it the other way), a live host makes the command
/// exit 0 with the host already gone when it returns and stopped through
/// the protocol; a host stopped by SIGSTOP (premise: `ps` reports it
/// stopped) makes it exit 1, saying it could not confirm that host
/// stopped, and the host still runs (it was not sent SIGTERM, which with
/// the SIGCONT after it would have ended it) and keeps its socket file.
#[farhelm_testtrace::test]
async fn a_state_directory_given_through_a_symlink_stops_only_what_answers() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let elsewhere = farhelm_teststate::tempdir().expect("another directory");
    let link = elsewhere.path().join("state");
    std::os::unix::fs::symlink(state.path(), &link).unwrap();
    let live = start_host(state.path(), "exec /bin/sleep 600").await;
    assert!(
        !command_line(live.pid).contains(link.to_str().unwrap()),
        "premise: the host's command line spells the directory another way"
    );

    let run = run_stop(&link);
    let gone = !exists(live.pid);
    assert!(run.status.success(), "{:?}: {}", run.status, run.stderr);
    assert!(gone, "the live host outlived the command's success");
    assert!(
        run.stderr.contains("stopped through the protocol"),
        "{}",
        run.stderr
    );

    let stopped = start_host(state.path(), "exec /bin/sleep 600").await;
    assert!(
        !command_line(stopped.pid).contains(link.to_str().unwrap()),
        "premise: the host's command line spells the directory another way"
    );
    stopped.signal(libc::SIGSTOP);
    assert!(
        eventually(|| process_state(stopped.pid).starts_with('T')),
        "premise: the host stopped"
    );
    let run = run_stop(&link);
    assert_eq!(run.status.code(), Some(1), "{}", run.stderr);
    assert!(
        run.stderr.contains("could not confirm it stopped"),
        "{}",
        run.stderr
    );
    assert!(stopped.is_ours(), "the stopped host was signalled");
    assert!(
        process_state(stopped.pid).starts_with('T'),
        "the stopped host was sent SIGCONT"
    );
    assert!(
        stopped.socket.exists(),
        "the stopped host lost its socket file"
    );
    // Ended here rather than by the guard, which would first wait out a
    // `Shutdown` a stopped host never answers.
    stopped.signal(libc::SIGKILL);
    assert!(
        eventually(|| !exists(stopped.pid)),
        "the host outlived SIGKILL"
    );
}

/// A socket directory that is a symlink is not followed: the hosts of the
/// directory it leads to are left alone, and the run fails.
///
/// Why: no supervisor puts hosts behind a symlink, and a `hosts` symlink
/// could lead to another state directory's hosts, perhaps those of an
/// install that is running. Stopping them, through the protocol or by
/// signal, would end someone else's terminals. Spec: with state directory
/// B's `hosts` a symlink to state directory A's, and a real host running
/// in A, the command on B exits 1 and says B's `hosts` is not a directory;
/// A's host still runs (a host ends on SIGTERM, so it was not signalled)
/// and its socket file is still there.
#[farhelm_testtrace::test]
async fn a_symlinked_socket_directory_leads_nowhere() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let host = start_host(state.path(), "exec /bin/sleep 600").await;
    let other = farhelm_teststate::tempdir().expect("another state dir");
    std::os::unix::fs::symlink(state.path().join("hosts"), other.path().join("hosts")).unwrap();
    assert!(host.is_ours(), "premise: the host runs");

    let run = run_stop(other.path());
    assert_eq!(run.status.code(), Some(1), "{}", run.stderr);
    assert!(run.stderr.contains("is not a directory"), "{}", run.stderr);
    assert!(host.is_ours(), "the host behind the symlink was ended");
    assert!(
        host.socket.exists(),
        "the host behind the symlink lost its socket"
    );
}

/// A state directory without a socket directory, and one that does not
/// exist, succeed at once with nothing to report.
///
/// Why: a caller that finds no socket directory need not run the command,
/// but one that does must not fail or wait. Spec: for a fresh state
/// directory and a path inside it that does not exist, the command exits 0
/// within a few seconds with nothing on stdout or stderr.
#[farhelm_testtrace::test]
async fn a_missing_directory_succeeds_at_once() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    for dir in [state.path().to_path_buf(), state.path().join("absent")] {
        let run = run_stop(&dir);
        assert!(run.status.success(), "{:?}: {}", run.status, run.stderr);
        assert!(run.took < Duration::from_secs(5), "{:?}", run.took);
        assert_eq!((run.stdout.as_str(), run.stderr.as_str()), ("", ""));
    }
}

/// Something that cannot be confirmed stopped fails the run, is never
/// signalled, and does not keep a live host beside it from stopping.
///
/// Why: exit status 1 is how a caller learns that something may still run
/// and the state directory must not be removed yet; and the one
/// destructive step, the signal, must reach only a process proven to be
/// that socket's host. Spec: beside a live host (premise: it runs), a
/// listener in this test process that never accepts sits in the socket
/// directory; the command exits 1, the live host is gone, the listener's
/// socket file is kept, stderr says that socket could not be confirmed
/// stopped, and this test process (the listener's, which the command's
/// connection names) was not signalled, which this test still running
/// shows.
#[farhelm_testtrace::test]
async fn an_unconfirmed_host_fails_the_run_and_others_still_stop() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let host = start_host(state.path(), "exec /bin/sleep 600").await;
    let stand_in = host.socket.with_file_name("aaaaaaaa");
    let listener = UnixListener::bind(&stand_in).unwrap();
    assert!(host.is_ours(), "premise: the live host runs");

    let run = run_stop(state.path());
    let gone = !exists(host.pid);
    assert_eq!(run.status.code(), Some(1), "{}", run.stderr);
    assert!(gone, "the live host survived");
    assert!(stand_in.exists(), "the unconfirmed socket file was removed");
    assert!(
        run.stderr.contains(&format!(
            "{}: could not confirm it stopped",
            stand_in.display()
        )),
        "{}",
        run.stderr
    );
    // As in `a_relative_or_empty_state_directory_is_a_usage_error`.
    drop(listener);
    std::fs::remove_file(&stand_in).unwrap();
}

/// A peer that answers `Shutdown` with `Goodbye` and does not end is not
/// confirmed stopped, and is not signalled.
///
/// Why: success must mean the host is gone; a `Goodbye` only says it meant
/// to end. Claiming a stop on it would let an uninstall remove a state
/// directory under a running host. Spec: with a stand-in in this test
/// process that answers `Welcome`, reads `Shutdown`, answers `Goodbye` and
/// keeps running with its socket present, the command exits 1, stderr says
/// it took `Shutdown` but did not end and that the process is not that
/// socket's host, the socket file is kept, and this test process was not
/// signalled.
#[farhelm_testtrace::test]
async fn a_goodbye_without_an_end_fails_the_run() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let hosts = farhelm_supervisor::hosts::ensure_hosts_dir(state.path()).unwrap();
    let socket = hosts.join("aaaaaaaa");
    let stand_in = goodbye_stand_in(UnixListener::bind(&socket).unwrap());

    let run = run_stop(state.path());
    let _listener = stand_in
        .join()
        .expect("the stand-in saw Hello and Shutdown");
    assert_eq!(run.status.code(), Some(1), "{}", run.stderr);
    assert!(
        run.stderr.contains("took Shutdown but did not end")
            && run.stderr.contains("is not the terminal host"),
        "{}",
        run.stderr
    );
    assert!(socket.exists(), "the stand-in's socket file was removed");
    // As in `a_relative_or_empty_state_directory_is_a_usage_error`.
    std::fs::remove_file(&socket).unwrap();
}

/// A relative or empty state directory is a usage error, refused before
/// anything is looked at.
///
/// Why: an empty value from a caller's unset variable, or a relative one,
/// would otherwise make the command act on whatever `./hosts` its working
/// directory holds, and an exit status 0 there would tell the caller the
/// install it meant had no hosts. Spec: run from a directory holding
/// `hosts/aaaaaaaa` and `relative/hosts/aaaaaaaa`, each a listener that
/// never accepts, `--state-dir ""` and `--state-dir relative` each exit 2,
/// and neither listener was connected to.
#[farhelm_testtrace::test]
async fn a_relative_or_empty_state_directory_is_a_usage_error() {
    let cwd = farhelm_teststate::tempdir().expect("working directory");
    let mut listeners = Vec::new();
    for socket in ["hosts/aaaaaaaa", "relative/hosts/aaaaaaaa"] {
        let path = cwd.path().join(socket);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        listeners.push(listener);
    }
    for value in ["", "relative"] {
        let child = stop_command(std::ffi::OsStr::new(value))
            .current_dir(cwd.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("starting the command");
        let (status, _) = wait_bounded(child);
        assert_eq!(status.code(), Some(2), "--state-dir {value:?}");
    }
    for listener in &listeners {
        assert_eq!(
            listener.accept().map_err(|error| error.kind()).err(),
            Some(std::io::ErrorKind::WouldBlock),
            "a refused invocation contacted a socket"
        );
    }
    // Removed here, so that the directory's cleanup does not spend its
    // refusal window on stand-ins it would only find stale.
    drop(listeners);
    for socket in ["hosts/aaaaaaaa", "relative/hosts/aaaaaaaa"] {
        std::fs::remove_file(cwd.path().join(socket)).unwrap();
    }
}

/// A stderr nobody reads does not hold the command's exit back.
///
/// Why: the bound is the caller's protection, and a caller whose stderr
/// pipe is full (its reader stuck, or reading only stdout) would otherwise
/// never get an exit status from a command that has finished its work.
/// Spec: with stderr a pipe filled to capacity beforehand and never read
/// (premise: a non-blocking write to it would block), and a state
/// directory whose `hosts` is a regular file (so the command has a line to
/// write and fails), the command exits 1 within a few seconds.
#[farhelm_testtrace::test]
async fn an_undrained_stderr_does_not_hold_the_exit() {
    use std::os::fd::AsRawFd as _;
    let state = farhelm_teststate::tempdir().expect("state dir");
    std::fs::write(state.path().join("hosts"), b"").unwrap();
    let (_reader, mut writer) = std::io::pipe().unwrap();
    let fd = writer.as_raw_fd();
    // SAFETY: fcntl on a descriptor this test owns.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    assert_eq!(
        unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
    let chunk = [b'x'; 4096];
    loop {
        match writer.write(&chunk) {
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("filling the pipe: {error}"),
        }
    }
    // A chunk that does not fit can leave room for less than a chunk; not
    // even one byte may fit for the premise to hold.
    loop {
        match writer.write(b"x") {
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("filling the pipe: {error}"),
        }
    }
    assert_eq!(
        writer.write(b"x").map_err(|error| error.kind()).err(),
        Some(std::io::ErrorKind::WouldBlock),
        "premise: the pipe is full"
    );
    // Blocking again, so that the command's writes wait for a reader
    // rather than failing at once.
    // SAFETY: as above.
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_SETFL, flags) }, 0);

    let child = stop_command(state.path().as_os_str())
        .stdout(Stdio::null())
        .stderr(writer)
        .spawn()
        .expect("starting the command");
    let (status, took) = wait_bounded(child);
    assert_eq!(status.code(), Some(1));
    assert!(took < Duration::from_secs(5), "{took:?}");
}
