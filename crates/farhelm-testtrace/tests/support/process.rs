//! Bounded Unix child supervision shared by the tracing contract binaries.
//!
//! The helper drains nonblocking pipes on the supervising thread, retains only a configured
//! prefix of each stream, and owns the child's process group until the direct child is reaped.
//! It deliberately stops after the direct child exits and the currently buffered bytes are
//! drained; an inherited pipe held by a descendant can therefore never stall the test harness.

use std::fmt;
use std::io::{self, Read};
use std::os::fd::{AsRawFd as _, RawFd};
use std::os::unix::process::CommandExt as _;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// A completed child's verdict and independently bounded output streams.
#[derive(Debug)]
pub struct CommandResult {
    /// Exit status reported by the reaped direct child.
    pub status: ExitStatus,
    /// Prefixes drained before the direct child completed.
    pub output: CapturedOutput,
}

/// Retained prefixes from the child's standard streams.
#[derive(Debug, Default)]
pub struct CapturedOutput {
    /// At most the configured number of stdout bytes.
    pub stdout: Vec<u8>,
    /// At most the configured number of stderr bytes.
    pub stderr: Vec<u8>,
}

impl CapturedOutput {
    /// Returns a diagnostic view without requiring arbitrary child bytes to be valid UTF-8.
    pub fn display(&self) -> String {
        format!(
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&self.stdout),
            String::from_utf8_lossy(&self.stderr)
        )
    }
}

/// The bounded reason a child could not produce a normal verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// The direct child remained alive past its deadline.
    Timeout,
    /// One stream produced a byte after filling its retained prefix.
    OutputOverflow,
    /// Spawn, pipe configuration, polling, reading, or reaping failed.
    Supervision,
}

/// A failed run retains useful prefixes and the direct PID whose reaping was attempted.
pub struct RunFailure {
    /// Stable category for focused supervision assertions.
    pub kind: FailureKind,
    /// Direct child PID when spawning succeeded.
    pub child_pid: Option<u32>,
    /// Useful bounded output retained before cleanup.
    pub output: CapturedOutput,
    detail: String,
}

impl fmt::Display for RunFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "bounded child run failed ({:?}): {}\n{}",
            self.kind,
            self.detail,
            self.output.display()
        )
    }
}

impl fmt::Debug for RunFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// Owns cleanup until normal completion has explicitly reaped the direct child.
struct ChildGroup {
    child: Option<Child>,
    pid: u32,
}

impl ChildGroup {
    /// Takes both configured pipes only after cleanup ownership is armed.
    fn take_pipes(&mut self) -> io::Result<(ChildStdout, ChildStderr)> {
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| io::Error::other("owned child was already reaped"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("piped stdout was unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("piped stderr was unavailable"))?;
        Ok((stdout, stderr))
    }

    /// Observes exit without reaping, keeping the leader's PID reserved through group cleanup.
    ///
    /// `Child::try_wait` consumes the exit status. Signaling the numeric group after that could
    /// hit a reused ID, so this guard alone waits for its child and uses `WNOWAIT` until all
    /// group signals are finished. Callers must not install a competing child reaper.
    fn has_exited(&mut self) -> io::Result<bool> {
        if self.child.is_none() {
            return Err(io::Error::other("owned child was already reaped"));
        }
        // SAFETY: zero is a valid initial representation for this output-only C structure.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: the PID belongs to our unreaped child and `info` is writable. WNOWAIT leaves
        // the exit status available for Child::wait after the last process-group signal.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.pid as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            return Ok(info.si_signo == libc::SIGCHLD);
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ECHILD) {
            // Ownership was lost outside this helper. Disarm rather than signal a possibly
            // recycled numeric group in Drop while reporting the supervision failure.
            self.child.take();
        }
        if error.kind() == io::ErrorKind::Interrupted {
            return Ok(false);
        }
        Err(error)
    }

    /// Confirms reaping before disarming the cleanup guard.
    fn reap(&mut self) -> io::Result<ExitStatus> {
        let result = self
            .child
            .as_mut()
            .ok_or_else(|| io::Error::other("owned child was already reaped"))?
            .wait();
        if result.is_ok()
            || result
                .as_ref()
                .is_err_and(|error| error.raw_os_error() == Some(libc::ECHILD))
        {
            self.child.take();
        }
        result
    }

    /// Stops descendants that inherited the group or pipes after the direct child exited.
    fn terminate_group(&self) {
        // SAFETY: the child is the group leader and has not been reaped. Its reserved PID keeps
        // this group number from naming an unrelated group until reap disarms the guard.
        unsafe {
            libc::kill(-(self.pid as i32), libc::SIGKILL);
        }
    }

    /// Delivers the fixture's requested abnormal exit while the direct PID remains owned.
    ///
    /// This shared module is also compiled into a macro contract binary that uses only bounded
    /// completion, so this method is intentionally dormant in that consumer.
    #[allow(dead_code)]
    fn signal_group(&self, signal: libc::c_int) -> io::Result<()> {
        // SAFETY: the unreaped group leader reserves this process-group identity for the guard.
        if unsafe { libc::kill(-(self.pid as i32), signal) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

impl Drop for ChildGroup {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        self.terminate_group();
        let _ = child.wait();
    }
}

/// Runs one process group with a deadline and a separate retained-byte cap for each stream.
pub fn run_bounded(
    mut command: Command,
    timeout: Duration,
    stream_limit: usize,
) -> Result<CommandResult, RunFailure> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let child = command.spawn().map_err(|error| RunFailure {
        kind: FailureKind::Supervision,
        child_pid: None,
        output: CapturedOutput::default(),
        detail: format!("spawn: {error}"),
    })?;
    let pid = child.id();
    let mut group = ChildGroup {
        child: Some(child),
        pid,
    };
    let (mut stdout, mut stderr) = group.take_pipes().map_err(|error| RunFailure {
        kind: FailureKind::Supervision,
        child_pid: Some(pid),
        output: CapturedOutput::default(),
        detail: format!("take pipes: {error}"),
    })?;
    if let Err(error) =
        set_nonblocking(stdout.as_raw_fd()).and_then(|()| set_nonblocking(stderr.as_raw_fd()))
    {
        return Err(RunFailure {
            kind: FailureKind::Supervision,
            child_pid: Some(pid),
            output: CapturedOutput::default(),
            detail: format!("configure nonblocking pipes: {error}"),
        });
    }

    let mut output = CapturedOutput::default();
    let deadline = Instant::now() + timeout;
    loop {
        if let Err(failure) =
            drain_available(&mut stdout, &mut output.stdout, stream_limit, "stdout").and_then(
                |()| drain_available(&mut stderr, &mut output.stderr, stream_limit, "stderr"),
            )
        {
            return Err(RunFailure {
                kind: failure.kind(),
                child_pid: Some(pid),
                output,
                detail: failure.to_string(),
            });
        }
        match group.has_exited() {
            Ok(true) => {
                group.terminate_group();
                if let Err(failure) =
                    drain_available(&mut stdout, &mut output.stdout, stream_limit, "stdout")
                        .and_then(|()| {
                            drain_available(&mut stderr, &mut output.stderr, stream_limit, "stderr")
                        })
                {
                    return Err(RunFailure {
                        kind: failure.kind(),
                        child_pid: Some(pid),
                        output,
                        detail: failure.to_string(),
                    });
                }
                let status = match group.reap() {
                    Ok(status) => status,
                    Err(error) => {
                        return Err(RunFailure {
                            kind: FailureKind::Supervision,
                            child_pid: Some(pid),
                            output,
                            detail: format!("reap: {error}"),
                        });
                    }
                };
                return Ok(CommandResult { status, output });
            }
            Ok(false) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(false) => {
                return Err(RunFailure {
                    kind: FailureKind::Timeout,
                    child_pid: Some(pid),
                    output,
                    detail: format!("deadline of {timeout:?} expired"),
                });
            }
            Err(error) => {
                return Err(RunFailure {
                    kind: FailureKind::Supervision,
                    child_pid: Some(pid),
                    output,
                    detail: format!("poll: {error}"),
                });
            }
        }
    }
}

/// Waits for an exact append-readiness marker, then signals and reaps the owned child group.
///
/// The marker is evidence from the child after its append call returned. Poll timing establishes
/// only the supervision deadline; it is never accepted as evidence that persistence completed.
/// This shared module's macro contract consumer does not run abnormal-exit fixtures.
#[allow(dead_code)]
pub fn run_until_stdout_then_signal(
    mut command: Command,
    ready_marker: &[u8],
    signal: libc::c_int,
    timeout: Duration,
    stream_limit: usize,
) -> Result<CommandResult, RunFailure> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let child = command.spawn().map_err(|error| RunFailure {
        kind: FailureKind::Supervision,
        child_pid: None,
        output: CapturedOutput::default(),
        detail: format!("spawn: {error}"),
    })?;
    let pid = child.id();
    let mut group = ChildGroup {
        child: Some(child),
        pid,
    };
    let (mut stdout, mut stderr) = group.take_pipes().map_err(|error| RunFailure {
        kind: FailureKind::Supervision,
        child_pid: Some(pid),
        output: CapturedOutput::default(),
        detail: format!("take pipes: {error}"),
    })?;
    if let Err(error) =
        set_nonblocking(stdout.as_raw_fd()).and_then(|()| set_nonblocking(stderr.as_raw_fd()))
    {
        return Err(RunFailure {
            kind: FailureKind::Supervision,
            child_pid: Some(pid),
            output: CapturedOutput::default(),
            detail: format!("configure nonblocking pipes: {error}"),
        });
    }

    let mut output = CapturedOutput::default();
    let deadline = Instant::now() + timeout;
    let mut signaled = false;
    loop {
        if let Err(failure) =
            drain_available(&mut stdout, &mut output.stdout, stream_limit, "stdout").and_then(
                |()| drain_available(&mut stderr, &mut output.stderr, stream_limit, "stderr"),
            )
        {
            return Err(RunFailure {
                kind: failure.kind(),
                child_pid: Some(pid),
                output,
                detail: failure.to_string(),
            });
        }
        if !signaled
            && output
                .stdout
                .windows(ready_marker.len())
                .any(|window| window == ready_marker)
        {
            if let Err(error) = group.signal_group(signal) {
                return Err(RunFailure {
                    kind: FailureKind::Supervision,
                    child_pid: Some(pid),
                    output,
                    detail: format!("signal ready child: {error}"),
                });
            }
            signaled = true;
        }
        match group.has_exited() {
            Ok(true) if signaled => {
                group.terminate_group();
                if let Err(failure) =
                    drain_available(&mut stdout, &mut output.stdout, stream_limit, "stdout")
                        .and_then(|()| {
                            drain_available(&mut stderr, &mut output.stderr, stream_limit, "stderr")
                        })
                {
                    return Err(RunFailure {
                        kind: failure.kind(),
                        child_pid: Some(pid),
                        output,
                        detail: failure.to_string(),
                    });
                }
                let status = match group.reap() {
                    Ok(status) => status,
                    Err(error) => {
                        return Err(RunFailure {
                            kind: FailureKind::Supervision,
                            child_pid: Some(pid),
                            output,
                            detail: format!("reap signaled child: {error}"),
                        });
                    }
                };
                return Ok(CommandResult { status, output });
            }
            Ok(true) => {
                return Err(RunFailure {
                    kind: FailureKind::Supervision,
                    child_pid: Some(pid),
                    output,
                    detail: "child exited before append readiness".to_owned(),
                });
            }
            Ok(false) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(false) => {
                return Err(RunFailure {
                    kind: FailureKind::Timeout,
                    child_pid: Some(pid),
                    output,
                    detail: format!("append readiness deadline of {timeout:?} expired"),
                });
            }
            Err(error) => {
                return Err(RunFailure {
                    kind: FailureKind::Supervision,
                    child_pid: Some(pid),
                    output,
                    detail: format!("poll ready child: {error}"),
                });
            }
        }
    }
}

/// Distinguishes an enforced output boundary from an operating-system read failure.
enum DrainFailure {
    Overflow(String),
    Io(String),
}

impl DrainFailure {
    /// Maps internal drain failures onto the child runner's stable public categories.
    fn kind(&self) -> FailureKind {
        match self {
            Self::Overflow(_) => FailureKind::OutputOverflow,
            Self::Io(_) => FailureKind::Supervision,
        }
    }
}

impl fmt::Display for DrainFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow(detail) | Self::Io(detail) => formatter.write_str(detail),
        }
    }
}

/// Makes pipe reads return `WouldBlock` once all currently buffered bytes are drained.
fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: `fd` is a live owned pipe descriptor for the duration of both calls.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the original flags remain intact and `O_NONBLOCK` is valid for a pipe.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Drains only bytes immediately available and rejects the first byte beyond the retained cap.
fn drain_available(
    reader: &mut impl Read,
    retained: &mut Vec<u8>,
    limit: usize,
    stream: &'static str,
) -> Result<(), DrainFailure> {
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(count) => {
                let available = limit.saturating_sub(retained.len());
                retained.extend_from_slice(&chunk[..count.min(available)]);
                if count > available {
                    return Err(DrainFailure::Overflow(format!(
                        "{stream} exceeded its {limit}-byte retained cap"
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(DrainFailure::Io(format!("read {stream}: {error}"))),
        }
    }
}

/// Proves failure cleanup and normal-exit cleanup preserve child ownership through signaling.
pub fn assert_supervision_contracts() {
    let mut timeout = Command::new("sh");
    timeout.args(["-c", "exec sleep 30"]);
    let failure = run_bounded(timeout, Duration::from_millis(50), 128)
        .expect_err("sleeping child must exceed the deadline");
    assert_eq!(failure.kind, FailureKind::Timeout);
    assert_reaped(failure.child_pid.expect("spawned timeout child has a PID"));

    let mut overflow = Command::new("sh");
    overflow.args(["-c", "while :; do printf 1234567890; done"]);
    let failure = run_bounded(overflow, Duration::from_secs(2), 128)
        .expect_err("unbounded writer must exceed the output cap");
    assert_eq!(failure.kind, FailureKind::OutputOverflow);
    assert_eq!(failure.output.stdout.len(), 128);
    assert_reaped(failure.child_pid.expect("spawned overflow child has a PID"));

    assert_exit_remains_owned_until_cleanup();
}

/// A background descendant keeps stdout open after the leader exits; cleanup must close it.
///
/// Observing the same pending exit twice proves that polling has not released the leader's
/// identity before the group signal. EOF then proves the inherited writer was closed without
/// waiting for its thirty-second sleep. The guard also cleans up if an assertion unwinds.
fn assert_exit_remains_owned_until_cleanup() {
    let child = Command::new("sh")
        .args(["-c", "sleep 30 & exit 7"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .expect("spawn inherited-pipe fixture");
    let pid = child.id();
    let mut group = ChildGroup {
        child: Some(child),
        pid,
    };
    let (mut stdout, _stderr) = group.take_pipes().expect("take inherited pipe");
    set_nonblocking(stdout.as_raw_fd()).expect("configure inherited pipe");
    let deadline = Instant::now() + Duration::from_secs(2);
    while !group.has_exited().expect("observe leader exit") {
        assert!(Instant::now() < deadline, "leader did not exit");
        thread::sleep(Duration::from_millis(10));
    }
    assert!(group.has_exited().expect("exit must remain waitable"));
    let mut byte = [0];
    assert_eq!(
        stdout
            .read(&mut byte)
            .expect_err("descendant must still hold stdout")
            .kind(),
        io::ErrorKind::WouldBlock,
    );
    group.terminate_group();
    assert_eq!(group.reap().expect("reap owned leader").code(), Some(7));
    assert_reaped(pid);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match stdout.read(&mut byte) {
            Ok(0) => break,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                assert!(
                    Instant::now() < deadline,
                    "descendant retained stdout after cleanup"
                );
                thread::sleep(Duration::from_millis(10));
            }
            other => panic!("unexpected inherited-pipe output: {other:?}"),
        }
    }
}

/// Checks that `wait` reaped the direct child rather than merely sending it a signal.
fn assert_reaped(pid: u32) {
    let mut status = 0;
    // SAFETY: this exact-PID, nonblocking wait can only observe our own child. Unlike kill(0),
    // it does not mistake an unrelated process that reused the PID for a leaked child.
    let result = unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) };
    assert_eq!(result, -1, "child PID {pid} was still ours after cleanup");
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD),
        "child PID {pid} was not reaped"
    );
}
