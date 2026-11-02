//! A terminal host as a process: who answers at a socket, which process
//! serves a socket that does not answer, and the one way the supervisor
//! ends a host without its consent.
//!
//! A host is normally ended through its protocol (`Shutdown`). Some hosts
//! cannot be: one that is wedged or stopped never answers, and one whose
//! protocol this supervisor does not speak cannot be asked. Delete and
//! Restart must still never leak them, so the goal allows signalling a host
//! whose pid was learned from a connection's peer credentials (or, when no
//! connection gets that far, from the process table). Signalling a pid is
//! only safe while it still names that host, and a host can live for weeks,
//! so everything here is about not signalling the wrong process:
//!
//! - [`HostProcess`] pins a pid to its kernel start identity (the existing
//!   `procs::ProcessIdentity`), so a pid that was reused since is never
//!   mistaken for the host.
//! - A host's command line ends with `--socket <its socket>`, which is the
//!   host's contract
//!   (`crates/terminal-host/SPEC.md`, "Command line and child spec"), and
//!   socket paths are never reused while a host holds one (the host refuses
//!   an existing path). [`terminate_verified`] checks that
//!   before every signal: the start identity alone cannot catch a host that
//!   died before its identity was first read, since the identity would then
//!   be the reusing process's own; nor a "host" that was never one, such as
//!   a stand-in listener in a test process. [`socket_process`] finds a host
//!   by the same test when its socket does not answer at all.

use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;
use std::time::Duration;

use crate::procs::{self, ProcessIdentity, ProcessState};

/// How long [`terminate_verified`] waits for a host to go after SIGTERM
/// before it sends SIGKILL. A host ends on SIGTERM as soon as its loop sees
/// it; this is room for a loaded machine.
pub const TERMINATE_GRACE: Duration = Duration::from_secs(5);

/// How long [`terminate_verified`] waits for a host to go after SIGKILL.
pub const KILL_GRACE: Duration = Duration::from_secs(5);

/// How often a termination re-reads the process while waiting for it to go.
const TERMINATE_POLL: Duration = Duration::from_millis(50);

/// A process pinned by pid and kernel start identity: the process that
/// answered at a host socket when the connection was made, or the one
/// [`socket_process`] found serving it.
///
/// Holding one authorizes nothing by itself. It is what
/// [`terminate_verified`] re-reads and compares before it signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HostProcess {
    pub pid: u32,
    /// The kernel's start token for the process (see
    /// `procs::ProcessIdentity`): a later process reusing the pid has
    /// another.
    pub start: u64,
}

impl HostProcess {
    /// The live process `pid` now, or `None` when there is none (exited, a
    /// zombie, or unreadable).
    pub fn current(pid: u32) -> Option<Self> {
        ProcessIdentity::read(pid).map(|identity| Self {
            pid: identity.pid,
            start: identity.start,
        })
    }

    /// Whether this exact process still runs: same pid, same start
    /// identity, not a zombie (a stopped process runs, for this purpose).
    /// An error means the process table could not be read, which callers
    /// must not take as gone.
    fn is_running(&self) -> Result<bool, String> {
        Ok(matches!(
            procs::read_process(self.pid)?,
            Some((_, start, ProcessState::Running)) if start == self.start
        ))
    }
}

/// The process listening on the other end of `stream`, from the
/// connection's peer credentials (`SO_PEERCRED` on Linux, `LOCAL_PEERPID` on
/// macOS), pinned to its start identity.
///
/// For a connection to a listening socket the peer is the process that
/// listens, whether or not it has accepted yet, so this names a host that is
/// stopped or wedged too. `None` when the platform reports no pid, the pid
/// is 0 or 1 (a signal to either would reach a process group or init), or
/// the process is already gone.
///
/// Read this before sending anything on the connection. A host that later
/// answers was then alive both before and after the identity was read, so
/// the identity is its own; one that never answers may have died just
/// before the read and left its pid to another process, which the command
/// line check in [`terminate_verified`] catches.
pub fn peer_process(stream: &tokio::net::UnixStream) -> Option<HostProcess> {
    let pid = peer_pid(stream)?;
    HostProcess::current(pid)
}

/// The raw peer pid of `stream`, if the platform reports one and it is
/// above 1.
fn peer_pid(stream: &tokio::net::UnixStream) -> Option<u32> {
    use std::os::fd::AsRawFd as _;
    let fd = stream.as_raw_fd();
    #[cfg(target_os = "linux")]
    let pid = {
        // SAFETY: ucred is plain data, and SO_PEERCRED fills at most one of
        // the length given.
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
        if result != 0 {
            return None;
        }
        cred.pid
    };
    #[cfg(target_os = "macos")]
    let pid = {
        let mut pid: libc::pid_t = 0;
        let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
        // SAFETY: LOCAL_PEERPID fills one pid_t, the length given.
        let result = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                (&raw mut pid).cast(),
                &mut len,
            )
        };
        if result != 0 {
            return None;
        }
        pid
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let pid: libc::pid_t = {
        let _ = fd;
        0
    };
    u32::try_from(pid).ok().filter(|&pid| pid > 1)
}

/// Whether `argv` is the command line of the host on `socket` (already made
/// absolute): it ends with `--socket <socket>`.
fn names_socket(argv: &[Vec<u8>], socket: &Path) -> bool {
    matches!(
        argv,
        [.., flag, path] if flag == b"--socket" && path.as_slice() == socket.as_os_str().as_bytes()
    )
}

/// A live process of this user that serves `socket`: its command line ends
/// with `--socket <socket>` (made absolute), as a host's does.
///
/// For a socket that does not answer at all, where no connection names a
/// peer. A host that is stopped or wedged stops accepting, so the
/// connections that time out against it stay queued in its listen backlog;
/// once that is full, Linux answers every new connection with `EAGAIN` and
/// macOS with a refusal, exactly as for a socket nothing listens on. A host
/// whose socket file was removed under it is unreachable the same way. Its
/// process is then the only evidence it exists, and judging the socket gone
/// would leak it for good.
///
/// Walks every process of this user and reads each command line, so it is
/// for that rare path, not for every connect. A row the walk could not read
/// is skipped; a walk that could not run at all is an error, never "none".
pub fn socket_process(socket: &Path) -> Result<Option<HostProcess>, String> {
    let socket = std::path::absolute(socket)
        .map_err(|error| format!("resolving {}: {error}", socket.display()))?;
    let (table, _unreadable_rows) = procs::snapshot()?;
    let own = std::process::id();
    for &pid in table.keys() {
        if pid <= 1 || pid == own {
            continue;
        }
        let Some(argv) = procs::read_process_argv(pid) else {
            continue;
        };
        if !names_socket(&argv, &socket) {
            continue;
        }
        // Pinned after the command line was seen, so the identity is the
        // process that had it unless it exec'd in between, which a later
        // `terminate_verified` re-checks anyway.
        if let Some(host) = HostProcess::current(pid) {
            return Ok(Some(host));
        }
    }
    Ok(None)
}

/// What [`terminate_verified`] found or did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// The pinned process no longer runs (it exited, or its pid now belongs
    /// to another process); nothing was signalled.
    AlreadyGone,
    /// SIGTERM (with SIGCONT) was sent and the process went within the
    /// grace.
    Terminated,
    /// The process outlived SIGTERM's grace, and went after SIGKILL.
    Killed,
    /// The process survived SIGKILL's grace too (on Linux, a process in
    /// uninterruptible sleep can, for a while). It is still the host as far
    /// as anything here knows.
    StillRunning,
}

/// Why [`terminate_verified`] signalled nothing more, or could not confirm
/// what it did. None of these is "gone": the caller must keep treating the
/// host as possibly running.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminateError {
    /// The process runs, but its command line does not say it is the host
    /// on that socket. It was not signalled (again).
    #[error("process {pid} is not the terminal host on {socket}")]
    NotThisHost { pid: u32, socket: String },
    /// The process or its command line could not be read while the process
    /// still ran. It was not signalled (again).
    #[error("could not identify process {pid}: {reason}")]
    Unreadable { pid: u32, reason: String },
    /// Sending a signal failed for a reason other than the process being
    /// gone.
    #[error("signalling process {pid}: {reason}")]
    Signal { pid: u32, reason: String },
}

/// End the host `host` serving `socket` by signal: SIGTERM, then, if it is
/// still there after [`TERMINATE_GRACE`], SIGKILL, waiting up to
/// [`KILL_GRACE`]. Every signal is sent only while the process is still
/// exactly that process and its command line says it is the host on
/// `socket`.
///
/// For hosts the protocol cannot end: one that never answers (wedged, or
/// stopped), one that is permanently busy, or one whose protocol version
/// this supervisor does not speak.
///
/// - **SIGTERM first**, because a host that can act on it ends cleanly: it
///   closes its pty (so its child gets the terminal's hangup) and removes
///   its socket file.
/// - **SIGCONT right after it**: a host handles SIGTERM, and a stopped
///   process keeps a handled signal pending until it runs again, so a host
///   stopped by a debugger or a job-control signal would otherwise never
///   end. systemd does the same when it stops a unit.
/// - **SIGKILL after the grace**, because the host's handler only sets a
///   flag and wakes its event loop (`crates/terminal-host/src/host.rs`); a
///   host whose loop is wedged never looks at the flag, so SIGTERM alone
///   can never end the very hosts this exists for. SIGKILL still closes the
///   pty, so the child gets its hangup, but the socket file stays behind:
///   it refuses connections from then on, and a later connect judges it
///   gone once no process serves it.
///
/// The identity is read before the command line and again after it, before
/// each signal, so a pid that changed hands while it was being checked is
/// never signalled. The pid could still pass to another process between the
/// last check and a signal, the same short window the session sweep accepts
/// (SPEC_impl.md, "Process-number reuse inside a short window is
/// accepted"); SIGCONT follows SIGTERM without a check of its own.
///
/// `socket` is compared, made absolute, with the command line's last
/// argument, byte for byte; pass the path the host was started with (a
/// spawn always gives it an absolute one).
pub async fn terminate_verified(
    host: &HostProcess,
    socket: &Path,
) -> Result<Termination, TerminateError> {
    terminate_within(host, socket, TERMINATE_GRACE, KILL_GRACE).await
}

/// [`terminate_verified`] with its graces injected.
async fn terminate_within(
    host: &HostProcess,
    socket: &Path,
    term_grace: Duration,
    kill_grace: Duration,
) -> Result<Termination, TerminateError> {
    let socket = std::path::absolute(socket).map_err(|error| TerminateError::Unreadable {
        pid: host.pid,
        reason: format!("resolving {}: {error}", socket.display()),
    })?;
    if !verify(host, &socket)? || !signal(host, libc::SIGTERM)? {
        return Ok(Termination::AlreadyGone);
    }
    let pid = host.pid as libc::pid_t;
    // SAFETY: kill has no memory preconditions; see the docs for the window
    // this one is sent in. A failure means the process went already, which
    // the wait below observes.
    unsafe { libc::kill(pid, libc::SIGCONT) };
    if wait_gone(host, term_grace).await? {
        return Ok(Termination::Terminated);
    }
    if !verify(host, &socket)? || !signal(host, libc::SIGKILL)? {
        // It went between the grace and the check.
        return Ok(Termination::Terminated);
    }
    Ok(if wait_gone(host, kill_grace).await? {
        Termination::Killed
    } else {
        Termination::StillRunning
    })
}

/// Whether `host` is still that process and still the host on `socket`.
/// `Ok(false)` when it is gone, at any of the reads (a process that exits
/// while its command line is read reads as unreadable or mismatched, and
/// is gone, not either of those); an error when it runs but is not, or
/// cannot be shown to be, the host.
fn verify(host: &HostProcess, socket: &Path) -> Result<bool, TerminateError> {
    let running = || {
        host.is_running()
            .map_err(|reason| TerminateError::Unreadable {
                pid: host.pid,
                reason,
            })
    };
    if !running()? {
        return Ok(false);
    }
    let argv = procs::read_process_argv(host.pid);
    // Whatever the command line said, a process that is gone now is gone.
    if !running()? {
        return Ok(false);
    }
    match argv {
        None => Err(TerminateError::Unreadable {
            pid: host.pid,
            reason: "its command line could not be read".into(),
        }),
        Some(argv) if !names_socket(&argv, socket) => Err(TerminateError::NotThisHost {
            pid: host.pid,
            socket: socket.display().to_string(),
        }),
        Some(_) => Ok(true),
    }
}

/// Send `signal` to `host`'s pid. `Ok(false)` when the process is already
/// gone (`ESRCH`).
fn signal(host: &HostProcess, signal: libc::c_int) -> Result<bool, TerminateError> {
    let pid = libc::pid_t::try_from(host.pid).map_err(|_| TerminateError::Signal {
        pid: host.pid,
        reason: "process number out of range".into(),
    })?;
    // SAFETY: kill has no memory preconditions; the caller has just
    // verified that `pid` is the host (see `terminate_verified`).
    if unsafe { libc::kill(pid, signal) } == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(false);
    }
    Err(TerminateError::Signal {
        pid: host.pid,
        reason: error.to_string(),
    })
}

/// Wait up to `grace` for `host` to stop running.
async fn wait_gone(host: &HostProcess, grace: Duration) -> Result<bool, TerminateError> {
    let deadline = tokio::time::Instant::now() + grace;
    loop {
        if !host
            .is_running()
            .map_err(|reason| TerminateError::Unreadable {
                pid: host.pid,
                reason,
            })?
        {
            return Ok(true);
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(TERMINATE_POLL).await;
    }
}

#[cfg(test)]
mod tests {
    //! The identity checks and the signal sequence against stand-in
    //! processes (shells whose command lines end the way a host's does), so
    //! that every refusal can be shown to signal nothing, and each signal to
    //! be needed. A real host stopped with SIGSTOP and ended this way is in
    //! `crates/farhelm/tests/terminal_host_client.rs`.

    use super::*;
    use std::os::unix::process::ExitStatusExt as _;
    use std::process::{Child, Command, ExitStatus, Stdio};

    /// A stand-in host this test started, killed and reaped when it drops,
    /// whatever the test did to it (a stopped one included: SIGKILL ends a
    /// stopped process too).
    struct StandIn {
        child: Child,
        host: HostProcess,
    }

    impl StandIn {
        /// A shell running `script`, whose command line ends with
        /// `--socket <socket>` as a host's does, pinned once its command
        /// line is its own (right after the spawn it may still be the
        /// forked test process's).
        ///
        /// `script` creates the file named by `$READY` once its traps are
        /// set; this returns only after that, since a signal sent before
        /// the shell has run `trap` meets the default disposition.
        fn start(script: &str, socket: &Path) -> StandIn {
            let ready = socket.with_extension("ready");
            let child = Command::new("/bin/sh")
                .args(["-c", script])
                .arg("--socket")
                .arg(socket)
                .env("READY", &ready)
                .stdin(Stdio::piped())
                // Its `sleep`s can outlive it by up to a second; on the
                // test's own stdout they would hold the test open.
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("fixture: starting a stand-in host");
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let host = loop {
                let argv = procs::read_process_argv(child.id()).unwrap_or_default();
                if names_socket(&argv, socket)
                    && ready.exists()
                    && let Some(host) = HostProcess::current(child.id())
                {
                    break host;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "fixture: the stand-in never ran its own command line"
                );
                // sleep-ok: polling interval for the stand-in's exec.
                std::thread::sleep(Duration::from_millis(10));
            };
            StandIn { child, host }
        }

        /// Whether it still runs, without reaping it.
        fn alive(&mut self) -> bool {
            self.child
                .try_wait()
                .expect("fixture: polling the stand-in")
                .is_none()
        }

        /// Stop it, and wait until the kernel reports it stopped: `kill`
        /// returning is not the stop having taken effect.
        fn stop(&self) {
            let pid = self.host.pid;
            // SAFETY: stopping the stand-in this test started.
            assert_eq!(unsafe { libc::kill(pid as libc::pid_t, libc::SIGSTOP) }, 0);
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while !process_state(pid).starts_with('T') {
                assert!(
                    std::time::Instant::now() < deadline,
                    "fixture: the stand-in never stopped"
                );
                // sleep-ok: polling interval for the stop to take effect.
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        /// Close its stdin, which ends a stand-in waiting in `read`.
        fn release(&mut self) {
            drop(self.child.stdin.take());
        }

        /// Reap it and return how it ended.
        fn wait(&mut self) -> ExitStatus {
            self.child.wait().expect("fixture: reaping the stand-in")
        }
    }

    impl Drop for StandIn {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// `pid`'s state letter (`T` when stopped), as `ps` reports it.
    fn process_state(pid: u32) -> String {
        let output = Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .expect("fixture: running ps");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// A stand-in that handles SIGTERM the way a host does (by acting on it
    /// once it runs) and loops until then.
    const HANDLES_TERM: &str =
        "trap 'exit 143' TERM; : > \"$READY\"; while :; do /bin/sleep 1; done";

    /// A stand-in that never acts on SIGTERM, as a host whose loop is
    /// wedged never does.
    const IGNORES_TERM: &str = "trap '' TERM; : > \"$READY\"; while :; do /bin/sleep 1; done";

    /// A stopped process that handles SIGTERM is ended by SIGTERM, which
    /// needs the SIGCONT that follows it.
    ///
    /// Why: a host handles SIGTERM, and a stopped process keeps a handled
    /// signal pending until it runs again; without SIGCONT a stopped host
    /// would outlive SIGTERM and be killed, losing its socket cleanup.
    /// Spec: a stand-in that exits 143 from a TERM trap, confirmed stopped,
    /// gives `Terminated` (not `Killed`) and exited with 143.
    #[farhelm_testtrace::test]
    async fn a_stopped_host_is_terminated_with_sigterm_and_sigcont() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        let mut stand_in = StandIn::start(HANDLES_TERM, &socket);
        stand_in.stop();
        let outcome = terminate_within(
            &stand_in.host,
            &socket,
            Duration::from_secs(10),
            Duration::from_secs(10),
        )
        .await;
        assert_eq!(outcome, Ok(Termination::Terminated));
        assert_eq!(stand_in.wait().code(), Some(143));
    }

    /// A process that does not act on SIGTERM is killed after the grace.
    ///
    /// Why: a host whose event loop is wedged never acts on its SIGTERM
    /// flag, and Delete must still not leak it. Spec: a stand-in that
    /// ignores SIGTERM, with a 1 s grace, gives `Killed` and ended by
    /// SIGKILL.
    #[farhelm_testtrace::test]
    async fn a_host_that_does_not_act_on_sigterm_is_killed() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        let mut stand_in = StandIn::start(IGNORES_TERM, &socket);
        let outcome = terminate_within(
            &stand_in.host,
            &socket,
            Duration::from_secs(1),
            Duration::from_secs(10),
        )
        .await;
        assert_eq!(outcome, Ok(Termination::Killed));
        assert_eq!(stand_in.wait().signal(), Some(libc::SIGKILL));
    }

    /// A process whose command line does not name the socket is never
    /// signalled.
    ///
    /// Why: the pid learned from a connection may belong to a process that
    /// is not a host at all (a stand-in listener in a test process, or a
    /// process that took the pid of a host that died before its identity
    /// was read), and signalling it would kill something unrelated. Spec:
    /// with the stand-in naming another socket, the result is `NotThisHost`
    /// and the stand-in still runs.
    #[farhelm_testtrace::test]
    async fn a_process_naming_another_socket_is_not_signalled() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        let mut stand_in = StandIn::start(HANDLES_TERM, &socket);
        let other = dir.path().join("other");
        let host = stand_in.host;
        let outcome = terminate_within(
            &host,
            &other,
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .await;
        assert!(
            matches!(outcome, Err(TerminateError::NotThisHost { pid, .. }) if pid == host.pid),
            "{outcome:?}"
        );
        assert!(stand_in.alive(), "the stand-in was signalled");
    }

    /// The same pid with another start identity is another process, and is
    /// never signalled.
    ///
    /// Why: a host recorded long ago may have exited and its pid been
    /// reused, by something that might even be a host for the same path
    /// later. Spec: with the start identity changed by one, the result is
    /// `AlreadyGone` and the stand-in still runs.
    #[farhelm_testtrace::test]
    async fn another_start_identity_is_already_gone() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        let mut stand_in = StandIn::start(HANDLES_TERM, &socket);
        let stale = HostProcess {
            start: stand_in.host.start.wrapping_add(1),
            ..stand_in.host
        };
        let outcome = terminate_within(
            &stale,
            &socket,
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(outcome, Ok(Termination::AlreadyGone));
        assert!(stand_in.alive(), "the stand-in was signalled");
    }

    /// A process that has exited, even one not yet reaped, is already gone.
    ///
    /// Why: a host racing its own `Shutdown`, or one an earlier call just
    /// ended, is gone, and reporting it as unreadable (its command line
    /// reads as nothing once it exited) would make a caller keep a host
    /// that no longer exists. Spec: a stand-in that exits once its stdin
    /// closes, waited for until it is a zombie but not reaped, gives `AlreadyGone`.
    #[farhelm_testtrace::test]
    async fn an_exited_process_is_already_gone() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        let mut stand_in = StandIn::start(": > \"$READY\"; read line; exit 0", &socket);
        let pid = stand_in.host.pid;
        stand_in.release();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !process_state(pid).starts_with('Z') {
            assert!(
                std::time::Instant::now() < deadline,
                "fixture: the stand-in never became a zombie"
            );
            // sleep-ok: polling interval for the stand-in's exit.
            std::thread::sleep(Duration::from_millis(10));
        }
        let outcome = terminate_within(
            &stand_in.host,
            &socket,
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(outcome, Ok(Termination::AlreadyGone));
        assert!(stand_in.wait().success());
    }

    /// The process serving a socket is found by its command line, and no
    /// process is found for a socket nothing serves.
    ///
    /// Why: a stopped host with a full backlog, or one whose socket file was
    /// removed, answers no connection, and only this keeps it from being
    /// judged gone. Spec: with a stand-in naming one socket,
    /// `socket_process` gives its identity for that socket and `None` for
    /// another path.
    #[farhelm_testtrace::test]
    async fn the_process_serving_a_socket_is_found_by_its_command_line() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("h");
        let stand_in = StandIn::start(HANDLES_TERM, &socket);
        assert_eq!(socket_process(&socket), Ok(Some(stand_in.host)));
        assert_eq!(socket_process(&dir.path().join("other")), Ok(None));
    }
}
