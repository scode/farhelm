//! Bounded execution for read-only runtime observations.
//!
//! Uninstall will use probes to ask a specific tmux socket about retained
//! state. A query that wedges, prints unbounded output, or leaves a pipe open
//! must become uncertainty, never an excuse to infer that nothing is running.
//! This module owns only its direct child; it never broadens cleanup to a
//! process group or to descendants that happen to inherit a pipe.

use std::{
    fmt, io,
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
};

/// Caller-chosen resource limits for one observation command.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProbeLimits {
    pub(crate) elapsed: Duration,
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
    pub(crate) cleanup_elapsed: Duration,
}

/// Raw result from a completed probe; a nonzero status remains evidence.
#[derive(Debug)]
pub(crate) struct ProbeOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

/// A safe category for a failed probe observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeFailure {
    Spawn,
    Read,
    Wait,
    Timeout,
    StdoutLimit,
    StderrLimit,
}

/// The operation whose OS error made a probe or its cleanup uncertain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeOperation {
    Spawn,
    StdoutRead,
    StderrRead,
    Wait,
    CleanupInspect,
    CleanupKill,
    CleanupWait,
}

/// One concrete OS failure, labelled without retaining command data.
#[derive(Debug)]
pub(crate) struct ProbeIoError {
    pub(crate) operation: ProbeOperation,
    pub(crate) source: io::Error,
}

/// Failure details that deliberately omit the caller-constructed command.
///
/// `failure` is safe to use for control flow. The optional primary error and
/// cleanup evidence exist for diagnostics: callers must be able to explain
/// whether uncertainty came from the query itself, cleanup, or both without
/// ever formatting the command that may contain credentials.
#[derive(Debug)]
pub(crate) struct ProbeError {
    pub(crate) failure: ProbeFailure,
    pub(crate) source: Option<ProbeIoError>,
    pub(crate) cleanup_errors: Vec<ProbeIoError>,
    pub(crate) cleanup_timed_out: bool,
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "probe observation failed: {:?}", self.failure)?;
        if let Some(error) = &self.source {
            write!(formatter, "; {:?}: {}", error.operation, error.source)?;
        }
        for error in &self.cleanup_errors {
            write!(formatter, "; {:?}: {}", error.operation, error.source)?;
        }
        if self.cleanup_timed_out {
            formatter.write_str("; CleanupWait: timed out")?;
        }
        Ok(())
    }
}

impl std::error::Error for ProbeError {}

/// Run one caller-built command while bounding every resource it can retain.
///
/// The caller owns command construction, including its environment and
/// working directory. This function installs null stdin and piped output, then
/// treats a nonzero exit as normal observation data. Once spawning succeeds,
/// one deadline covers reads and reaping so an inherited pipe cannot outlive
/// the query. Failure cleanup has its own deadline: a pathological child must
/// not convert a bounded read into an unbounded wait.
pub(crate) async fn execute(
    command: &mut Command,
    limits: ProbeLimits,
) -> Result<ProbeOutput, ProbeError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.kill_on_drop(true);
    let mut child = command.spawn().map_err(|source| ProbeError {
        failure: ProbeFailure::Spawn,
        source: Some(ProbeIoError {
            operation: ProbeOperation::Spawn,
            source,
        }),
        cleanup_errors: Vec::new(),
        cleanup_timed_out: false,
    })?;
    let stdout = child
        .stdout
        .take()
        .expect("piped stdout exists after spawn");
    let stderr = child
        .stderr
        .take()
        .expect("piped stderr exists after spawn");

    let observation = tokio::time::timeout(limits.elapsed, async {
        tokio::try_join!(
            read_bounded(
                stdout,
                limits.stdout_bytes,
                ProbeFailure::StdoutLimit,
                ProbeOperation::StdoutRead,
            ),
            read_bounded(
                stderr,
                limits.stderr_bytes,
                ProbeFailure::StderrLimit,
                ProbeOperation::StderrRead,
            ),
            async {
                child.wait().await.map_err(|source| ObservationFailure {
                    failure: ProbeFailure::Wait,
                    source: Some(ProbeIoError {
                        operation: ProbeOperation::Wait,
                        source,
                    }),
                })
            },
        )
    })
    .await;

    match observation {
        Ok(Ok((stdout, stderr, status))) => Ok(ProbeOutput {
            status,
            stdout,
            stderr,
        }),
        Ok(Err(failure)) => Err(failed_probe(&mut child, limits.cleanup_elapsed, failure).await),
        Err(_) => Err(failed_probe(
            &mut child,
            limits.cleanup_elapsed,
            ObservationFailure {
                failure: ProbeFailure::Timeout,
                source: None,
            },
        )
        .await),
    }
}

/// A primary observation failure before owned-child cleanup is attempted.
#[derive(Debug)]
struct ObservationFailure {
    failure: ProbeFailure,
    source: Option<ProbeIoError>,
}

/// Drain one output pipe without retaining bytes beyond the caller's cap.
async fn read_bounded(
    mut reader: impl AsyncRead + Unpin,
    limit: usize,
    overflow: ProbeFailure,
    operation: ProbeOperation,
) -> Result<Vec<u8>, ObservationFailure> {
    let mut output = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|source| ObservationFailure {
                failure: ProbeFailure::Read,
                source: Some(ProbeIoError { operation, source }),
            })?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > limit {
            return Err(ObservationFailure {
                failure: overflow,
                source: None,
            });
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

/// Reap only the child this probe started, with a deadline independent of it.
async fn failed_probe(
    child: &mut Child,
    cleanup_elapsed: Duration,
    failure: ObservationFailure,
) -> ProbeError {
    let mut cleanup_errors = Vec::new();
    let needs_reap = match child.try_wait() {
        // `try_wait` establishes that this direct child already reaped. A
        // descendant may still hold a pipe, but there is nothing left to kill.
        Ok(Some(_)) => false,
        // `start_kill` only initiates delivery; the separately bounded wait
        // below is the cleanup deadline promised to the observation caller.
        Ok(None) => true,
        Err(source) => {
            cleanup_errors.push(ProbeIoError {
                operation: ProbeOperation::CleanupInspect,
                source,
            });
            true
        }
    };
    let mut cleanup_timed_out = false;
    if needs_reap {
        if let Err(source) = child.start_kill() {
            cleanup_errors.push(ProbeIoError {
                operation: ProbeOperation::CleanupKill,
                source,
            });
        }
        match tokio::time::timeout(cleanup_elapsed, child.wait()).await {
            Ok(Ok(_)) => {}
            Ok(Err(source)) => cleanup_errors.push(ProbeIoError {
                operation: ProbeOperation::CleanupWait,
                source,
            }),
            Err(_) => cleanup_timed_out = true,
        }
    }
    ProbeError {
        failure: failure.failure,
        source: failure.source,
        cleanup_errors,
        cleanup_timed_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::CString,
        fs,
        os::{
            fd::{FromRawFd as _, OwnedFd},
            unix::ffi::OsStrExt as _,
        },
        path::{Path, PathBuf},
        pin::Pin,
        task::{Context, Poll},
    };
    use tokio::{io::ReadBuf, task::JoinHandle};

    /// Kill a fixture descendant even when an assertion unwinds the test.
    struct DescendantGuard {
        pid_file: Option<PathBuf>,
    }

    impl DescendantGuard {
        /// Stop guarding once explicit cleanup has handled the recorded PID.
        fn disarm(&mut self) {
            self.pid_file = None;
        }
    }

    impl Drop for DescendantGuard {
        fn drop(&mut self) {
            if let Some(pid_file) = &self.pid_file
                && let Ok(pid) = read_pid(pid_file)
            {
                // SAFETY: `pid` came from the owned fixture, and SIGKILL has
                // no pointer or memory-safety preconditions.
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
        }
    }

    /// Abort a concurrent probe if a readiness assertion unwinds the test.
    struct ProbeTask(Option<JoinHandle<Result<ProbeOutput, ProbeError>>>);

    impl ProbeTask {
        /// Join the probe before making assertions about its final result.
        async fn join(&mut self) -> Result<ProbeOutput, ProbeError> {
            self.0
                .take()
                .expect("probe task is joined once")
                .await
                .expect("probe task did not panic")
        }

        /// Readiness must be established while the bounded probe is pending.
        fn is_finished(&self) -> bool {
            self.0.as_ref().is_none_or(JoinHandle::is_finished)
        }
    }

    impl Drop for ProbeTask {
        fn drop(&mut self) {
            if let Some(task) = &self.0 {
                task.abort();
            }
        }
    }

    /// Inject one concrete read error without depending on a platform pipe race.
    struct FailingReader(io::Error);

    impl AsyncRead for FailingReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            _buffer: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(Err(io::Error::from_raw_os_error(
                self.0.raw_os_error().expect("fixture errno"),
            )))
        }
    }

    /// Give ordinary fixtures room to complete while keeping failures bounded.
    fn limits() -> ProbeLimits {
        ProbeLimits {
            elapsed: Duration::from_secs(2),
            stdout_bytes: 128,
            stderr_bytes: 128,
            cleanup_elapsed: Duration::from_secs(1),
        }
    }

    /// Construct a probe command without inheriting the test runner's state.
    fn fixture_shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.env_clear().args(["-c", script]);
        command
    }

    /// Read a PID written by an owned fixture after its readiness boundary.
    fn read_pid(path: &Path) -> Result<libc::pid_t, Box<dyn std::error::Error>> {
        Ok(fs::read_to_string(path)?.parse()?)
    }

    /// Create a FIFO used as a deterministic fixture handshake or blocker.
    fn make_fifo(path: &Path) {
        let path = CString::new(path.as_os_str().as_bytes()).expect("FIFO path has no NUL");
        // SAFETY: the C string remains alive for the call and names a path in
        // the test's private temporary directory.
        let result = unsafe { libc::mkfifo(path.as_ptr(), 0o600) };
        assert_eq!(
            result,
            0,
            "create fixture FIFO: {}",
            io::Error::last_os_error()
        );
    }

    /// Wait for a fixture PID without using the probe deadline as readiness.
    async fn wait_for_pid(path: &Path, probe: &ProbeTask) -> libc::pid_t {
        for _ in 0..100 {
            assert!(
                !probe.is_finished(),
                "probe completed before fixture readiness"
            );
            if let Ok(pid) = read_pid(path) {
                return pid;
            }
            // sleep-ok: bounded polling waits for the owned fixture's PID handoff.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("fixture did not publish {}", path.display());
    }

    /// Open the FIFO's writer only after the fixture has reached its read side.
    async fn wait_for_fifo_reader(path: &Path, probe: &ProbeTask) -> OwnedFd {
        let path = CString::new(path.as_os_str().as_bytes()).expect("FIFO path has no NUL");
        for _ in 0..100 {
            assert!(
                !probe.is_finished(),
                "probe completed before FIFO readiness"
            );
            // SAFETY: the C string is valid for this call; the returned fd is
            // immediately transferred into an owned descriptor.
            let fd = unsafe { libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_NONBLOCK) };
            if fd >= 0 {
                // SAFETY: `open` returned a new descriptor owned by this test.
                return unsafe { OwnedFd::from_raw_fd(fd) };
            }
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(libc::ENXIO),
                "unexpected FIFO readiness failure"
            );
            // sleep-ok: bounded polling waits for the owned fixture to open the FIFO reader.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("fixture did not open its FIFO reader");
    }

    /// Wait until the executor has reaped the direct child while still pending.
    async fn wait_until_reaped(pid: libc::pid_t, probe: &ProbeTask) {
        for _ in 0..100 {
            assert!(
                !probe.is_finished(),
                "probe completed before child-reaping premise"
            );
            // SAFETY: signal zero only inspects the numeric process identity.
            if unsafe { libc::kill(pid, 0) } == -1
                && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            {
                assert_reaped(pid);
                return;
            }
            // sleep-ok: bounded polling waits for the probe's direct wait to reap the exited child.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("direct child was not reaped while inherited output remained open");
    }

    /// Prove tokio consumed the direct child's exit rather than leaving a zombie.
    fn assert_reaped(pid: libc::pid_t) {
        let mut status = 0;
        // SAFETY: `status` is writable and this fixture has no waiter other
        // than the probe executor for `pid`.
        let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        assert_eq!(waited, -1, "direct child still waitable: {waited}");
        assert_eq!(
            io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );

        // A zombie still answers kill(2) with signal zero. ESRCH therefore
        // complements ECHILD with the native liveness observation.
        // SAFETY: signal zero only inspects the numeric process identity.
        let alive = unsafe { libc::kill(pid, 0) };
        assert_eq!(alive, -1, "reaped child PID unexpectedly remains live");
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
    }

    /// Stop the fixture orphan and prove it can no longer execute.
    ///
    /// A container init is not required to reap adopted children, so a killed
    /// orphan may remain observable as a zombie. That is a terminated process,
    /// and unlike a live child it cannot retain the probe pipe or continue
    /// executing.
    async fn stop_descendant(pid: libc::pid_t) {
        // SAFETY: `pid` was reported by the owned descendant.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        for _ in 0..100 {
            // SAFETY: signal zero only checks whether the PID still exists.
            if unsafe { libc::kill(pid, 0) } == -1
                && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            {
                return;
            }
            #[cfg(target_os = "linux")]
            if fs::read_to_string(format!("/proc/{pid}/stat"))
                .ok()
                .and_then(|stat| {
                    stat.rsplit_once(") ")
                        .map(|(_, rest)| rest.starts_with("Z "))
                })
                == Some(true)
            {
                return;
            }
            // sleep-ok: bounded polling waits for init to reap the killed fixture orphan.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("fixture descendant {pid} survived cleanup");
    }

    /// A nonzero query result is evidence the eventual inspector must retain.
    #[tokio::test]
    async fn retains_output_and_nonzero_exit_status() {
        let mut command = fixture_shell("printf out; printf err >&2; exit 7");
        let output = execute(&mut command, limits())
            .await
            .expect("probe completes");
        assert_eq!(output.stdout, b"out");
        assert_eq!(output.stderr, b"err");
        assert_eq!(output.status.code(), Some(7));
    }

    /// Spawn refusal is separate from any child-observation failure.
    #[tokio::test]
    async fn reports_spawn_failure_without_command_text() {
        let mut command = Command::new("/definitely-not-a-probe-command");
        command.env_clear();
        let error = execute(&mut command, limits())
            .await
            .expect_err("spawn must fail");
        assert_eq!(error.failure, ProbeFailure::Spawn);
        assert_eq!(
            error.source.as_ref().map(|error| error.operation),
            Some(ProbeOperation::Spawn)
        );
        assert_eq!(
            error
                .source
                .as_ref()
                .and_then(|error| error.source.raw_os_error()),
            Some(libc::ENOENT)
        );
        assert!(!error.to_string().contains("definitely-not"));
        assert!(error.to_string().contains("os error"));
    }

    /// Output caps prevent one noisy query from becoming retained evidence.
    #[tokio::test]
    async fn refuses_output_overflow() {
        let mut command = fixture_shell("printf 123456789");
        let mut constrained = limits();
        constrained.stdout_bytes = 8;
        let error = execute(&mut command, constrained)
            .await
            .expect_err("output overflows");
        assert_eq!(error.failure, ProbeFailure::StdoutLimit);

        let mut exact = fixture_shell("printf 12345678; printf 12345678 >&2");
        constrained.stderr_bytes = 8;
        let output = execute(&mut exact, constrained)
            .await
            .expect("both streams fit exactly at their limits");
        assert_eq!(output.stdout, b"12345678");
        assert_eq!(output.stderr, b"12345678");

        let mut stderr = fixture_shell("printf 123456789 >&2");
        let error = execute(&mut stderr, constrained)
            .await
            .expect_err("stderr overflows independently");
        assert_eq!(error.failure, ProbeFailure::StderrLimit);
    }

    /// Read failures retain both their stream operation and concrete errno.
    #[tokio::test]
    async fn preserves_concrete_read_failure() {
        let error = read_bounded(
            FailingReader(io::Error::from_raw_os_error(libc::EIO)),
            8,
            ProbeFailure::StdoutLimit,
            ProbeOperation::StdoutRead,
        )
        .await
        .expect_err("injected read fails");
        assert_eq!(error.failure, ProbeFailure::Read);
        let source = error.source.expect("concrete read source");
        assert_eq!(source.operation, ProbeOperation::StdoutRead);
        assert_eq!(source.source.raw_os_error(), Some(libc::EIO));
    }

    /// A timed-out direct child is absent from both wait and process tables.
    ///
    /// The PID file is written before the FIFO open blocks, so it establishes
    /// readiness independently of the timeout that triggers cleanup.
    #[tokio::test]
    async fn times_out_a_hung_child() {
        let fixture = tempfile::tempdir().expect("fixture directory");
        let pid_file = fixture.path().join("direct.pid");
        let blocker = fixture.path().join("blocker");
        make_fifo(&blocker);
        let script = format!(
            "printf '%s' \"$$\" > {}; exec /bin/cat {}",
            shell_words::quote(pid_file.to_str().expect("UTF-8 fixture path")),
            shell_words::quote(blocker.to_str().expect("UTF-8 fixture path")),
        );
        let mut command = fixture_shell(&script);
        let mut constrained = limits();
        constrained.elapsed = Duration::from_secs(2);
        let mut probe = ProbeTask(Some(tokio::spawn(async move {
            execute(&mut command, constrained).await
        })));
        let pid = wait_for_pid(&pid_file, &probe).await;
        let _fifo_writer = wait_for_fifo_reader(&blocker, &probe).await;
        // SAFETY: signal zero proves the ready, FIFO-blocked child is alive.
        assert_eq!(unsafe { libc::kill(pid, 0) }, 0);
        let error = probe.join().await.expect_err("probe times out");
        assert_eq!(error.failure, ProbeFailure::Timeout);
        assert!(error.cleanup_errors.is_empty());
        assert!(!error.cleanup_timed_out);
        assert_reaped(pid);
    }

    /// A live descendant-held pipe remains bounded after the direct child exits.
    ///
    /// The handshake FIFO orders the descendant's PID write and inherited
    /// descriptor before the direct shell exits. The direct PID is then
    /// independently shown to have been reaped while the descendant is still
    /// alive, separating the pipe boundary from child cleanup behavior.
    #[tokio::test]
    async fn bounds_an_inherited_pipe_after_child_exit() {
        let fixture = tempfile::tempdir().expect("fixture directory");
        let direct_pid = fixture.path().join("direct.pid");
        let descendant_pid = fixture.path().join("descendant.pid");
        let handshake = fixture.path().join("handshake");
        let blocker = fixture.path().join("blocker");
        make_fifo(&handshake);
        make_fifo(&blocker);
        let mut descendant_guard = DescendantGuard {
            pid_file: Some(descendant_pid.clone()),
        };
        let quote = |path: &Path| {
            shell_words::quote(path.to_str().expect("UTF-8 fixture path")).into_owned()
        };
        let descendant = format!(
            "printf '%s' \"$$\" > {}; printf ready > {}; exec /bin/cat {}",
            quote(&descendant_pid),
            quote(&handshake),
            quote(&blocker),
        );
        let script = format!(
            "printf '%s' \"$$\" > {}; /bin/sh -c {} & IFS= read -r _ < {} || :",
            quote(&direct_pid),
            shell_words::quote(&descendant),
            quote(&handshake),
        );
        let mut command = fixture_shell(&script);
        let mut constrained = limits();
        constrained.elapsed = Duration::from_secs(2);
        let mut probe = ProbeTask(Some(tokio::spawn(async move {
            execute(&mut command, constrained).await
        })));
        let direct_pid = wait_for_pid(&direct_pid, &probe).await;
        let descendant_pid = wait_for_pid(&descendant_pid, &probe).await;
        let _fifo_writer = wait_for_fifo_reader(&blocker, &probe).await;
        wait_until_reaped(direct_pid, &probe).await;
        // SAFETY: signal zero verifies the descriptor-holding descendant is live.
        assert_eq!(unsafe { libc::kill(descendant_pid, 0) }, 0);

        let error = probe.join().await.expect_err("pipe remains open");
        assert_eq!(error.failure, ProbeFailure::Timeout);
        assert_reaped(direct_pid);
        // SAFETY: signal zero only checks whether the owned descendant is alive.
        assert_eq!(unsafe { libc::kill(descendant_pid, 0) }, 0);
        stop_descendant(descendant_pid).await;
        descendant_guard.disarm();
    }
}
