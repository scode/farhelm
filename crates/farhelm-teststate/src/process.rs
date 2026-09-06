//! Bounded synchronous command execution for test-harness diagnostics.
//!
//! This module owns only the direct [`std::process::Child`] it creates. It
//! never waits for EOF after that child exits: an escaped descendant can keep
//! an inherited output pipe open forever. Callers get the output observed so
//! far, explicit incomplete-tail markers, and the direct child's actual exit
//! status when the kernel made it available.
//!
//! The implementation is deliberately Unix-only. Its nonblocking pipe reads
//! are the mechanism behind the bounded-output promise; a blocking fallback
//! would look portable while silently changing that contract.

#[cfg(not(unix))]
compile_error!("farhelm-teststate's bounded command runner requires Unix nonblocking pipes");

use std::fmt;
use std::fmt::Write as _;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const READ_CHUNK: usize = 8 * 1024;
const MAX_ERRORS: usize = 8;
const MAX_ERROR_TEXT: usize = 240;

/// The declared resource bounds for one [`run_bounded`] call.
///
/// `total_timeout` is the wall-clock allowance, including cleanup, provided
/// command spawning completes promptly (see [`run_bounded`]).
/// `cleanup_reserve` is held back from normal execution so timeout cleanup can
/// still poll the owned direct child without extending that total. Output caps
/// bound retained prefixes; a zero cap is valid and records discard-only
/// evidence. `post_exit_read_budget` bounds bytes read after the direct child
/// exits, because a still-writing escaped descendant can otherwise keep a
/// pipe immediately readable forever.
#[derive(Clone, Debug)]
pub struct CommandRunLimits {
    total_timeout: Duration,
    cleanup_reserve: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    post_exit_read_budget: usize,
}

impl CommandRunLimits {
    /// Build limits whose execution window is `total_timeout - cleanup_reserve`.
    ///
    /// Both durations must be positive and cleanup must be strictly smaller
    /// than the total. The constructor does not inspect a clock; callers still
    /// receive [`CommandRunConfigError::DeadlineOverflow`] if `Instant` cannot
    /// represent the requested deadline when the command is started.
    pub fn new(
        total_timeout: Duration,
        cleanup_reserve: Duration,
        stdout_limit: usize,
        stderr_limit: usize,
        post_exit_read_budget: usize,
    ) -> Result<Self, CommandRunConfigError> {
        if total_timeout.is_zero() {
            return Err(CommandRunConfigError::ZeroTotalTimeout);
        }
        if cleanup_reserve.is_zero() {
            return Err(CommandRunConfigError::ZeroCleanupReserve);
        }
        if cleanup_reserve >= total_timeout {
            return Err(CommandRunConfigError::CleanupNotSmallerThanTotal);
        }
        Ok(Self {
            total_timeout,
            cleanup_reserve,
            stdout_limit,
            stderr_limit,
            post_exit_read_budget,
        })
    }
}

/// A limits configuration that cannot support the runner's deadline contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandRunConfigError {
    /// A run without wall-clock allowance would depend on scheduler timing.
    ZeroTotalTimeout,
    /// Cleanup needs a positive, explicit part of the stated total allowance.
    ZeroCleanupReserve,
    /// Reserving all of the timeout would leave no execution window.
    CleanupNotSmallerThanTotal,
    /// The monotonic clock cannot represent the requested deadline.
    DeadlineOverflow,
}

impl fmt::Display for CommandRunConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::ZeroTotalTimeout => "total timeout must be positive",
            Self::ZeroCleanupReserve => "cleanup reserve must be positive",
            Self::CleanupNotSmallerThanTotal => {
                "cleanup reserve must be smaller than total timeout"
            }
            Self::DeadlineOverflow => "timeout cannot be represented by the monotonic clock",
        };
        formatter.write_str(text)
    }
}

impl std::error::Error for CommandRunConfigError {}

/// Partial evidence retained for one output stream.
///
/// `prefix` never exceeds the configured cap. `observed_bytes` counts every
/// byte actually read, including discarded bytes; `omitted_bytes` counts the
/// observed bytes outside `prefix`. Either total becomes `None` on counter
/// overflow. `complete` means this runner observed EOF. It is false when the
/// runner had to close a pipe before EOF, so neither byte count claims to
/// describe a possible unread tail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedOutput {
    pub prefix: Vec<u8>,
    pub observed_bytes: Option<u64>,
    pub omitted_bytes: Option<u64>,
    pub complete: bool,
}

impl CapturedOutput {
    fn new() -> Self {
        Self {
            prefix: Vec::new(),
            observed_bytes: Some(0),
            omitted_bytes: Some(0),
            complete: false,
        }
    }

    fn record(&mut self, bytes: &[u8], limit: usize) {
        add_count(&mut self.observed_bytes, bytes.len());
        let retained = limit.saturating_sub(self.prefix.len()).min(bytes.len());
        self.prefix.extend_from_slice(&bytes[..retained]);
        add_count(&mut self.omitted_bytes, bytes.len() - retained);
    }
}

/// A bounded description of a runner failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandRunError {
    pub stage: CommandRunStage,
    pub kind: io::ErrorKind,
    pub raw_os_error: Option<i32>,
    pub message: String,
}

/// The operation that failed without turning a command's own nonzero status
/// into a runner failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandRunStage {
    Spawn,
    StdoutSetup,
    StderrSetup,
    StdoutRead,
    StderrRead,
    ChildPoll,
    ChildKill,
    ChildReap,
    ChildOwnershipLost,
}

/// The evidence returned for one bounded command attempt.
///
/// A nonzero `status` is the command's result, not an entry in `errors`.
/// `timed_out` means normal execution reached its reserved cleanup boundary.
/// `direct_child_reaped` is false when the runner could not honestly observe
/// the direct child exit before the total allowance expired or ownership was
/// lost; no descendant is ever signalled to compensate.
#[derive(Clone, Debug)]
pub struct CommandRunOutcome {
    pub status: Option<ExitStatus>,
    pub timed_out: bool,
    pub stdout: CapturedOutput,
    pub stderr: CapturedOutput,
    pub errors: Vec<CommandRunError>,
    pub errors_truncated: bool,
    pub direct_child_reaped: bool,
    /// The runner lost exclusive wait ownership and deliberately sent no more
    /// signals through a potentially stale `Child` handle.
    pub ownership_lost: bool,
}

impl CommandRunOutcome {
    fn new() -> Self {
        Self {
            status: None,
            timed_out: false,
            stdout: CapturedOutput::new(),
            stderr: CapturedOutput::new(),
            errors: Vec::new(),
            errors_truncated: false,
            direct_child_reaped: false,
            ownership_lost: false,
        }
    }

    /// Admit one operational error while capping both the number of fields and
    /// each rendered message. Later errors are summarized by
    /// `errors_truncated` rather than growing a diagnostic result without
    /// bound during a cascading cleanup failure.
    fn record_error(&mut self, stage: CommandRunStage, error: io::Error) {
        if self.errors.len() == MAX_ERRORS {
            self.errors_truncated = true;
            return;
        }
        self.errors.push(CommandRunError {
            stage,
            kind: error.kind(),
            raw_os_error: error.raw_os_error(),
            message: format_error_bounded(&error),
        });
    }
}

/// Run one explicitly configured command with bounded output and direct-child
/// cleanup.
///
/// The runner overrides only standard input and output routing: stdin becomes
/// null and stdout/stderr become captured pipes. It otherwise leaves the
/// caller's program, arguments, working directory, and child environment
/// untouched. It does not kill a process group, wait for escaped descendants,
/// or promise a bound over uninterruptible kernel I/O.
///
/// The caller must retain exclusive wait ownership: no competing reaper or
/// automatic child reaping may consume this child. Observed ownership loss
/// suppresses later signals, but cannot make concurrent reaping safe between
/// a wait probe and a numeric-PID signal. Spawning must also complete promptly;
/// a caller-supplied `pre_exec` hook must not block or fill an output pipe while
/// the parent is still waiting for the exec handshake.
pub fn run_bounded(
    command: &mut Command,
    limits: &CommandRunLimits,
) -> Result<CommandRunOutcome, CommandRunConfigError> {
    run_with_setup(command, limits, |pipe, deadline| {
        set_nonblocking(pipe, deadline)
    })
}

/// Keep setup injectable so a real spawned child proves the cleanup wiring,
/// without exhausting host descriptors or relying on a kernel failure.
fn run_with_setup(
    command: &mut Command,
    limits: &CommandRunLimits,
    mut setup: impl FnMut(&dyn AsRawFd, Instant) -> io::Result<()>,
) -> Result<CommandRunOutcome, CommandRunConfigError> {
    let start = Instant::now();
    let total_deadline = start
        .checked_add(limits.total_timeout)
        .ok_or(CommandRunConfigError::DeadlineOverflow)?;
    let execution_deadline = start
        .checked_add(limits.total_timeout - limits.cleanup_reserve)
        .ok_or(CommandRunConfigError::DeadlineOverflow)?;

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut outcome = CommandRunOutcome::new();
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            outcome.record_error(CommandRunStage::Spawn, error);
            return Ok(outcome);
        }
    };

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut setup_failed = false;
    match stdout.as_ref().ok_or_else(missing_pipe) {
        Ok(pipe) => {
            if let Err(error) = setup(pipe, total_deadline) {
                outcome.record_error(CommandRunStage::StdoutSetup, error);
                setup_failed = true;
            }
        }
        Err(error) => {
            outcome.record_error(CommandRunStage::StdoutSetup, error);
            setup_failed = true;
        }
    }
    match stderr.as_ref().ok_or_else(missing_pipe) {
        Ok(pipe) => {
            if let Err(error) = setup(pipe, total_deadline) {
                outcome.record_error(CommandRunStage::StderrSetup, error);
                setup_failed = true;
            }
        }
        Err(error) => {
            outcome.record_error(CommandRunStage::StderrSetup, error);
            setup_failed = true;
        }
    }
    if setup_failed {
        drop(stdout.take());
        drop(stderr.take());
        cleanup_direct_child(&mut child, &mut outcome, total_deadline);
        return Ok(outcome);
    }

    let mut failed = false;
    while outcome.status.is_none() && !failed {
        if Instant::now() >= execution_deadline {
            outcome.timed_out = true;
            break;
        }
        let stdout_turn = record_drain(
            drain_once(
                &mut stdout,
                &mut outcome.stdout,
                limits.stdout_limit,
                execution_deadline,
                None,
            ),
            CommandRunStage::StdoutRead,
            &mut outcome,
        );
        let stderr_turn = record_drain(
            drain_once(
                &mut stderr,
                &mut outcome.stderr,
                limits.stderr_limit,
                execution_deadline,
                None,
            ),
            CommandRunStage::StderrRead,
            &mut outcome,
        );
        failed = stdout_turn.failed || stderr_turn.failed;
        if failed {
            break;
        }
        match poll_child(&mut child, &mut outcome) {
            PollResult::Exited => break,
            PollResult::Running => {
                if !stdout_turn.read_any && !stderr_turn.read_any {
                    sleep_until(execution_deadline);
                }
            }
            PollResult::Failed => failed = true,
            PollResult::OwnershipLost => {
                outcome.ownership_lost = true;
                failed = true;
            }
        }
    }

    if outcome.status.is_some() {
        drain_after_exit(
            &mut stdout,
            &mut stderr,
            &mut outcome,
            limits,
            total_deadline,
        );
        drop(stdout);
        drop(stderr);
    } else {
        drop(stdout);
        drop(stderr);
        cleanup_if_owned(&mut child, &mut outcome, total_deadline);
    }
    Ok(outcome)
}

/// Finish direct-child observation after it has exited without allowing an
/// escaped descendant's writes to monopolize the call.
fn drain_after_exit<O: Read, E: Read>(
    stdout: &mut Option<O>,
    stderr: &mut Option<E>,
    outcome: &mut CommandRunOutcome,
    limits: &CommandRunLimits,
    deadline: Instant,
) {
    let mut remaining = limits.post_exit_read_budget;
    while remaining > 0 && Instant::now() < deadline {
        let stdout_turn = record_drain(
            drain_once(
                stdout,
                &mut outcome.stdout,
                limits.stdout_limit,
                deadline,
                Some(&mut remaining),
            ),
            CommandRunStage::StdoutRead,
            outcome,
        );
        let stderr_turn = record_drain(
            drain_once(
                stderr,
                &mut outcome.stderr,
                limits.stderr_limit,
                deadline,
                Some(&mut remaining),
            ),
            CommandRunStage::StderrRead,
            outcome,
        );
        if stdout_turn.failed
            || stderr_turn.failed
            || (!stdout_turn.read_any && !stderr_turn.read_any)
        {
            break;
        }
    }
    // EOF is the only proof of a complete stream. Any stream whose `complete`
    // flag remains false has an explicitly uncertain tail when its inherited
    // pipe is closed below; direct-child completion must not wait on an
    // unrelated descendant that retained it.
}

struct DrainTurn {
    read_any: bool,
    failed: bool,
}

/// Admit a completed read turn or attach its bounded failure only after the
/// stream borrow has ended. This separation keeps each stream's evidence and
/// the outcome's shared error list independently mutable.
fn record_drain(
    result: Result<DrainTurn, io::Error>,
    stage: CommandRunStage,
    outcome: &mut CommandRunOutcome,
) -> DrainTurn {
    match result {
        Ok(turn) => turn,
        Err(error) => {
            outcome.record_error(stage, error);
            DrainTurn {
                read_any: false,
                failed: true,
            }
        }
    }
}

/// Read one bounded chunk from one nonblocking pipe.
///
/// One read per stream per turn is the fairness boundary: a flooding stdout
/// cannot defer stderr, child polling, or deadline checks by keeping itself
/// readable. `remaining` is used only after direct-child exit.
fn drain_once<R: Read>(
    stream: &mut Option<R>,
    capture: &mut CapturedOutput,
    limit: usize,
    deadline: Instant,
    remaining: Option<&mut usize>,
) -> Result<DrainTurn, io::Error> {
    let Some(stream) = stream.as_mut() else {
        return Ok(DrainTurn {
            read_any: false,
            failed: false,
        });
    };
    if capture.complete || Instant::now() >= deadline {
        return Ok(DrainTurn {
            read_any: false,
            failed: false,
        });
    }
    let size = remaining
        .as_ref()
        .map_or(READ_CHUNK, |remaining| READ_CHUNK.min(**remaining));
    if size == 0 {
        return Ok(DrainTurn {
            read_any: false,
            failed: false,
        });
    }
    let mut buffer = [0_u8; READ_CHUNK];
    loop {
        if Instant::now() >= deadline {
            return Ok(DrainTurn {
                read_any: false,
                failed: false,
            });
        }
        match stream.read(&mut buffer[..size]) {
            Ok(0) => {
                capture.complete = true;
                return Ok(DrainTurn {
                    read_any: false,
                    failed: false,
                });
            }
            Ok(read) => {
                capture.record(&buffer[..read], limit);
                if let Some(remaining) = remaining {
                    *remaining -= read;
                }
                return Ok(DrainTurn {
                    read_any: true,
                    failed: false,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Ok(DrainTurn {
                    read_any: false,
                    failed: false,
                });
            }
            Err(error) => {
                return Err(error);
            }
        }
    }
}

enum PollResult {
    Exited,
    Running,
    Failed,
    OwnershipLost,
}

/// The minimal direct-child operations the runner needs. Keeping this seam
/// private lets contract tests prove that ownership loss suppresses `kill`
/// without manufacturing a real concurrent reaper.
trait ChildControl {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
    fn kill(&mut self) -> io::Result<()>;
}

impl ChildControl for Child {
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Child::try_wait(self)
    }

    fn kill(&mut self) -> io::Result<()> {
        Child::kill(self)
    }
}

/// Poll only the child handle this runner owns. A wait error is never treated
/// as a successful exit because another actor may have reaped the child.
fn poll_child<C: ChildControl>(child: &mut C, outcome: &mut CommandRunOutcome) -> PollResult {
    match child.try_wait() {
        Ok(Some(status)) => {
            outcome.status = Some(status);
            outcome.direct_child_reaped = true;
            PollResult::Exited
        }
        Ok(None) => PollResult::Running,
        Err(error) => {
            if error.raw_os_error() == Some(libc::ECHILD) {
                outcome.ownership_lost = true;
                outcome.record_error(CommandRunStage::ChildOwnershipLost, error);
                PollResult::OwnershipLost
            } else {
                outcome.record_error(CommandRunStage::ChildPoll, error);
                PollResult::Failed
            }
        }
    }
}

/// Kill and reap the still-owned direct child within the already-declared
/// total deadline. `Child::wait` is deliberately absent: kill does not make a
/// blocking wait bounded, and an interrupted or kernel-stuck wait must remain
/// visible as an unreaped child rather than hanging the harness.
fn cleanup_if_owned<C: ChildControl>(
    child: &mut C,
    outcome: &mut CommandRunOutcome,
    deadline: Instant,
) {
    if outcome.ownership_lost {
        return;
    }
    cleanup_direct_child(child, outcome, deadline);
}

/// Complete the remaining safe cleanup only while the runner still owns the
/// direct child. Callers must use [`cleanup_if_owned`] after a poll failure so
/// an `ECHILD` result can never turn into a stale-PID signal.
fn cleanup_direct_child<C: ChildControl>(
    child: &mut C,
    outcome: &mut CommandRunOutcome,
    deadline: Instant,
) {
    if outcome.status.is_some() {
        return;
    }
    match poll_child(child, outcome) {
        PollResult::Exited | PollResult::OwnershipLost => return,
        PollResult::Running | PollResult::Failed => {}
    }
    if let Err(error) = child.kill() {
        outcome.record_error(CommandRunStage::ChildKill, error);
    }
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) => {
                outcome.status = Some(status);
                outcome.direct_child_reaped = true;
                return;
            }
            Ok(None) => sleep_until(deadline),
            Err(error) => {
                let stage = if error.raw_os_error() == Some(libc::ECHILD) {
                    outcome.ownership_lost = true;
                    CommandRunStage::ChildOwnershipLost
                } else {
                    CommandRunStage::ChildReap
                };
                outcome.record_error(stage, error);
                return;
            }
        }
    }
}

/// Set only the runner-owned read end of a child pipe nonblocking.
fn set_nonblocking<T: AsRawFd + ?Sized>(file: &T, deadline: Instant) -> io::Result<()> {
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "pipe setup deadline expired",
            ));
        }
        // SAFETY: F_GETFL reads flags from the valid descriptor borrowed from
        // the owned child pipe and does not retain the pointer or descriptor.
        let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
        if flags >= 0 {
            loop {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "pipe setup deadline expired",
                    ));
                }
                // SAFETY: F_SETFL updates flags on the same valid descriptor.
                if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) }
                    >= 0
                {
                    return Ok(());
                }
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// Sleep briefly while no stream was readable, without ever scheduling past a
/// deadline intentionally. Scheduler delays and uninterruptible kernel work
/// remain outside what a user-space polling deadline can guarantee.
fn sleep_until(deadline: Instant) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if !remaining.is_zero() {
        std::thread::sleep(remaining.min(Duration::from_millis(1)));
    }
}

/// Add one observed count without allowing an overflowing diagnostic counter
/// to wrap into a false precise value.
fn add_count(slot: &mut Option<u64>, count: usize) {
    let Some(prior) = *slot else { return };
    let Ok(count) = u64::try_from(count) else {
        *slot = None;
        return;
    };
    *slot = prior.checked_add(count);
}

fn missing_pipe() -> io::Error {
    io::Error::other("command did not return a configured output pipe")
}

/// Render an I/O error into a bounded UTF-8 buffer without first allocating
/// the error's unrestricted `Display` output.
fn format_error_bounded(error: &io::Error) -> String {
    let mut output = BoundedText::new(MAX_ERROR_TEXT);
    let _ = write!(&mut output, "{error}");
    output.into_string()
}

/// A `fmt::Write` sink that rejects bytes past its explicit diagnostic cap.
/// It splits only at UTF-8 character boundaries, so a partially rendered
/// foreign error is still valid text.
struct BoundedText {
    text: String,
    remaining: usize,
}

impl BoundedText {
    fn new(limit: usize) -> Self {
        Self {
            text: String::with_capacity(limit),
            remaining: limit,
        }
    }

    fn into_string(self) -> String {
        self.text
    }
}

impl fmt::Write for BoundedText {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if text.len() <= self.remaining {
            self.text.push_str(text);
            self.remaining -= text.len();
            return Ok(());
        }
        let end = text
            .char_indices()
            .take_while(|(index, character)| *index + character.len_utf8() <= self.remaining)
            .map(|(index, character)| index + character.len_utf8())
            .last()
            .unwrap_or(0);
        self.text.push_str(&text[..end]);
        self.remaining -= end;
        Err(fmt::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::os::unix::process::ExitStatusExt;
    use std::path::Path;

    const FIXTURE_READY: &str = "FARHELM_TESTSTATE_FIXTURE_READY";
    const FIXTURE_RELEASE: &str = "FARHELM_TESTSTATE_FIXTURE_RELEASE";

    /// Leaves enough time for a fixture to run while making a test hang fail
    /// quickly. The cleanup reserve is part of every test because it is the
    /// contract that turns an execution timeout into an owned-child reap.
    fn limits(stdout_limit: usize, stderr_limit: usize) -> CommandRunLimits {
        CommandRunLimits::new(
            Duration::from_secs(2),
            Duration::from_millis(250),
            stdout_limit,
            stderr_limit,
            16 * 1024,
        )
        .unwrap()
    }

    /// Build a direct, POSIX-shell fixture without depending on ambient PATH.
    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    /// Both output pipes can flood without growing retained allocations or
    /// letting stdout hide stderr. The loop is intentionally shell-only so
    /// this fixture does not depend on a developer's `yes` or `head` binary.
    #[test]
    fn bounds_stdout_and_stderr_floods() {
        let mut command =
            shell("i=0; while [ \"$i\" -lt 20000 ]; do printf x; printf y >&2; i=$((i + 1)); done");
        let outcome = run_bounded(&mut command, &limits(64, 32)).unwrap();

        assert!(outcome.status.unwrap().success());
        assert!(outcome.direct_child_reaped);
        assert_eq!(outcome.stdout.prefix.len(), 64);
        assert_eq!(outcome.stderr.prefix.len(), 32);
        assert!(outcome.stdout.observed_bytes.unwrap() > 64);
        assert!(outcome.stderr.observed_bytes.unwrap() > 32);
        assert!(outcome.stdout.omitted_bytes.unwrap() > 0);
        assert!(outcome.stderr.omitted_bytes.unwrap() > 0);
    }

    /// A command's own failure is useful diagnostic evidence, not a runner
    /// failure. Folding this into `errors` would hide whether collection or
    /// the command itself failed.
    #[test]
    fn preserves_a_nonzero_exit_status() {
        let mut command = shell("printf failed >&2; exit 7");
        let outcome = run_bounded(&mut command, &limits(64, 64)).unwrap();

        assert_eq!(outcome.status.unwrap().code(), Some(7));
        assert!(outcome.direct_child_reaped);
        assert_eq!(outcome.stderr.prefix, b"failed");
        assert!(outcome.errors.is_empty());
    }

    /// Spawn errors return the same bounded evidence shape as later failures,
    /// so a caller can record a failed executable lookup without inventing a
    /// separate logging path.
    #[test]
    fn records_a_spawn_failure() {
        let mut command = Command::new("/definitely-not-a-farhelm-command");
        let outcome = run_bounded(&mut command, &limits(64, 64)).unwrap();

        assert!(outcome.status.is_none());
        assert!(!outcome.direct_child_reaped);
        assert_eq!(outcome.errors[0].stage, CommandRunStage::Spawn);
    }

    /// A timeout kills and reaps the direct child before the total allowance
    /// expires. The recorded status is the kernel's status for that direct
    /// child, which is stronger evidence than assuming `kill` completed it.
    #[test]
    fn timeout_reaps_the_direct_child() {
        let mut command = shell("exec sleep 30");
        let outcome = run_bounded(&mut command, &limits(64, 64)).unwrap();

        assert!(outcome.timed_out);
        assert!(outcome.direct_child_reaped);
        assert_eq!(outcome.status.unwrap().signal(), Some(libc::SIGKILL));
    }

    /// A descendant retaining stdout does not make the runner wait for EOF.
    /// The actual descriptor holder writes readiness only after binding its
    /// release socket. `FixtureLease` owns a release connection in both the
    /// success and panic paths, while the holder also has a finite fallback;
    /// no numeric-PID signal is used after its parent is reaped.
    #[test]
    fn returns_when_an_escaped_descendant_retains_a_pipe() {
        let fixture_dir = tempfile::tempdir().unwrap();
        let lease = FixtureLease::new(fixture_dir.path());
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "process::tests::fixture_parent_holds_stdout",
            "--nocapture",
            "--test-threads=1",
        ]);
        command
            .env(FIXTURE_READY, lease.ready())
            .env(FIXTURE_RELEASE, lease.release());

        let outcome = run_bounded(&mut command, &limits(64, 64)).unwrap();

        lease.wait_ready();
        assert!(outcome.status.unwrap().success());
        assert!(outcome.direct_child_reaped);
        assert!(!outcome.stdout.complete);
        lease.release_holder();
    }

    /// A read failure after useful bytes preserves that prefix and reports the
    /// failed operation. This seam exercises the error branch directly rather
    /// than trying to exhaust descriptors or inject a kernel fault into a
    /// whole process.
    #[test]
    fn preserves_partial_output_on_a_read_failure() {
        struct BytesThenError {
            read: bool,
        }

        impl Read for BytesThenError {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                if self.read {
                    return Err(io::Error::other("fixture read failure"));
                }
                self.read = true;
                buffer[..4].copy_from_slice(b"part");
                Ok(4)
            }
        }

        let deadline = Instant::now() + Duration::from_secs(1);
        let mut stream = Some(BytesThenError { read: false });
        let mut capture = CapturedOutput::new();
        let mut outcome = CommandRunOutcome::new();
        let first = drain_once(&mut stream, &mut capture, 64, deadline, None).unwrap();
        let second = record_drain(
            drain_once(&mut stream, &mut capture, 64, deadline, None),
            CommandRunStage::StdoutRead,
            &mut outcome,
        );

        assert!(first.read_any);
        assert!(second.failed);
        assert_eq!(capture.prefix, b"part");
        assert_eq!(outcome.errors[0].stage, CommandRunStage::StdoutRead);
    }

    /// Counter overflow means the runner no longer claims an exact observed
    /// extent, while ordinary retention still distinguishes stored bytes from
    /// bytes deliberately discarded.
    #[test]
    fn output_accounting_is_exact_until_overflow() {
        let mut output = CapturedOutput::new();
        output.record(b"abcdef", 3);
        assert_eq!(output.prefix, b"abc");
        assert_eq!(output.observed_bytes, Some(6));
        assert_eq!(output.omitted_bytes, Some(3));

        output.observed_bytes = Some(u64::MAX);
        add_count(&mut output.observed_bytes, 1);
        assert_eq!(output.observed_bytes, None);
    }

    /// Ownership loss is a hard stop for cleanup. The fake child turns the
    /// otherwise untestable concurrent-reaper result into an assertion that
    /// `kill` is never called after `ECHILD`.
    #[test]
    fn ownership_loss_never_signals_the_child_again() {
        let mut child = FakeChild::ownership_lost();
        let mut outcome = CommandRunOutcome::new();
        assert!(matches!(
            poll_child(&mut child, &mut outcome),
            PollResult::OwnershipLost
        ));
        outcome.ownership_lost = true;
        cleanup_if_owned(
            &mut child,
            &mut outcome,
            Instant::now() + Duration::from_secs(1),
        );

        assert_eq!(child.kill_calls, 0);
        assert!(!outcome.direct_child_reaped);
        assert!(outcome.ownership_lost);

        // Cleanup must also notice an ownership loss that happened since the
        // last execution-loop poll, before attempting its first signal.
        let mut newly_lost = FakeChild::ownership_lost();
        let mut fresh_outcome = CommandRunOutcome::new();
        cleanup_if_owned(&mut newly_lost, &mut fresh_outcome, Instant::now());
        assert_eq!(newly_lost.kill_calls, 0);
        assert!(fresh_outcome.ownership_lost);
    }

    /// A failure after one pipe is configured must still reap the real child.
    /// Injecting only the second setup result exercises the runner's cleanup
    /// wiring without exhausting descriptors or depending on a kernel error.
    #[test]
    fn partial_setup_failure_still_reaps_the_owned_child() {
        let mut command = shell("exec sleep 30");
        let mut setup_calls = 0;
        let outcome = run_with_setup(&mut command, &limits(64, 64), |pipe, deadline| {
            setup_calls += 1;
            if setup_calls == 2 {
                Err(io::Error::other("fixture stderr setup failure"))
            } else {
                set_nonblocking(pipe, deadline)
            }
        })
        .unwrap();

        assert_eq!(setup_calls, 2);
        assert!(outcome.direct_child_reaped);
        assert_eq!(outcome.status.unwrap().signal(), Some(libc::SIGKILL));
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(outcome.errors[0].stage, CommandRunStage::StderrSetup);
    }

    /// A continuously readable inherited pipe cannot keep the post-exit drain
    /// running or exceed its independent byte allowance. The reader seam
    /// makes throughput and process scheduling irrelevant to this premise.
    #[test]
    fn post_exit_flood_stops_at_the_explicit_read_budget() {
        let mut stdout = Some(io::repeat(b'x'));
        let mut stderr = Some(io::repeat(b'y'));
        let mut outcome = CommandRunOutcome::new();
        let limits = CommandRunLimits::new(
            Duration::from_secs(2),
            Duration::from_millis(250),
            64,
            32,
            READ_CHUNK * 3 + 17,
        )
        .unwrap();
        drain_after_exit(
            &mut stdout,
            &mut stderr,
            &mut outcome,
            &limits,
            Instant::now() + Duration::from_secs(1),
        );
        assert_eq!(
            outcome.stdout.observed_bytes.unwrap() + outcome.stderr.observed_bytes.unwrap(),
            (READ_CHUNK * 3 + 17) as u64
        );
        assert_eq!(outcome.stdout.prefix, vec![b'x'; 64]);
        assert_eq!(outcome.stderr.prefix, vec![b'y'; 32]);
        assert!(!outcome.stdout.complete);
        assert!(!outcome.stderr.complete);
    }

    /// An idle inherited writer gets one immediate read attempt per stream,
    /// not polling until EOF or the deadline. Counting read calls catches that
    /// regression without turning host scheduling speed into a test premise.
    #[test]
    fn post_exit_idle_pipe_is_not_polled_until_the_deadline() {
        struct IdlePipe(usize);
        impl Read for IdlePipe {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                self.0 += 1;
                Err(io::ErrorKind::WouldBlock.into())
            }
        }
        let mut stdout = Some(IdlePipe(0));
        let mut stderr = Some(IdlePipe(0));
        let mut outcome = CommandRunOutcome::new();
        drain_after_exit(
            &mut stdout,
            &mut stderr,
            &mut outcome,
            &limits(64, 64),
            Instant::now() + Duration::from_secs(1),
        );
        assert_eq!(stdout.unwrap().0, 1);
        assert_eq!(stderr.unwrap().0, 1);
        assert!(!outcome.stdout.complete);
        assert!(!outcome.stderr.complete);
    }

    /// Cleanup never falls back to blocking `wait`: after its deadline a
    /// still-running owned child remains explicitly unreaped, although the
    /// permitted direct-child kill attempt has occurred.
    #[test]
    fn reap_deadline_leaves_the_child_honestly_unreaped() {
        let mut child = FakeChild::never_exits();
        let mut outcome = CommandRunOutcome::new();
        cleanup_if_owned(&mut child, &mut outcome, Instant::now());

        assert_eq!(child.kill_calls, 1);
        assert!(!outcome.direct_child_reaped);
        assert!(outcome.status.is_none());
    }

    /// The direct fixture test starts the real descriptor holder and waits for
    /// its own readiness handoff before returning. It runs as a child test
    /// executable only when the outer contract test supplies both paths.
    #[test]
    #[allow(
        clippy::zombie_processes,
        reason = "the fixture must outlive this direct parent to retain its pipe; the outer lease releases it and its finite fallback bounds failure"
    )]
    fn fixture_parent_holds_stdout() {
        let (Some(ready), Some(release)) = (
            std::env::var_os(FIXTURE_READY),
            std::env::var_os(FIXTURE_RELEASE),
        ) else {
            return;
        };
        let mut holder = Command::new(std::env::current_exe().unwrap());
        holder.args([
            "--exact",
            "process::tests::fixture_descriptor_holder",
            "--nocapture",
            "--test-threads=1",
        ]);
        holder
            .env(FIXTURE_READY, &ready)
            .env(FIXTURE_RELEASE, &release);
        holder.spawn().unwrap();
        wait_for_file(Path::new(&ready));
    }

    /// The actual stdout-pipe holder announces readiness after its release
    /// listener exists, then exits on the owned release connection or its
    /// finite fallback deadline. It intentionally inherits the parent test
    /// executable's stdout pipe.
    #[test]
    fn fixture_descriptor_holder() {
        let (Some(ready), Some(release)) = (
            std::env::var_os(FIXTURE_READY),
            std::env::var_os(FIXTURE_RELEASE),
        ) else {
            return;
        };
        let listener = UnixListener::bind(&release).unwrap();
        listener.set_nonblocking(true).unwrap();
        let staged = Path::new(&ready).with_extension("staged");
        fs::write(&staged, b"ready").unwrap();
        fs::rename(staged, &ready).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match listener.accept() {
                Ok(_) => return,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "fixture release never arrived");
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("fixture release listener failed: {error}"),
            }
        }
    }

    /// Owns the descriptor-holder release protocol so a failing assertion
    /// cannot leave the escaped fixture waiting for its fallback lifetime.
    struct FixtureLease {
        ready: std::path::PathBuf,
        release: std::path::PathBuf,
        released: std::cell::Cell<bool>,
    }

    impl FixtureLease {
        fn new(root: &Path) -> Self {
            Self {
                ready: root.join("ready"),
                release: root.join("release.sock"),
                released: std::cell::Cell::new(false),
            }
        }

        fn ready(&self) -> &Path {
            &self.ready
        }

        fn release(&self) -> &Path {
            &self.release
        }

        fn wait_ready(&self) {
            wait_for_file(&self.ready);
        }

        fn release_holder(&self) {
            if !self.released.replace(true) {
                let _ = UnixStream::connect(&self.release);
            }
        }
    }

    impl Drop for FixtureLease {
        fn drop(&mut self) {
            self.release_holder();
        }
    }

    /// A private `ChildControl` seam for cleanup paths whose kernel failures
    /// cannot be reproduced deterministically in a test process.
    struct FakeChild {
        mode: FakeChildMode,
        kill_calls: usize,
    }

    #[derive(Clone, Copy)]
    enum FakeChildMode {
        OwnershipLost,
        NeverExits,
    }

    impl FakeChild {
        fn ownership_lost() -> Self {
            Self {
                mode: FakeChildMode::OwnershipLost,
                kill_calls: 0,
            }
        }

        fn never_exits() -> Self {
            Self {
                mode: FakeChildMode::NeverExits,
                kill_calls: 0,
            }
        }
    }

    impl ChildControl for FakeChild {
        fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            match self.mode {
                FakeChildMode::OwnershipLost => Err(io::Error::from_raw_os_error(libc::ECHILD)),
                FakeChildMode::NeverExits => Ok(None),
            }
        }

        fn kill(&mut self) -> io::Result<()> {
            self.kill_calls += 1;
            Ok(())
        }
    }

    /// Wait for a fixture's explicit readiness handoff. This is deliberately
    /// bounded: the wait is test setup, not a substitute for the runner's
    /// deadline, and a missing handoff must fail rather than hang the suite.
    fn wait_for_file(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while !path.exists() {
            assert!(Instant::now() < deadline, "fixture never wrote {path:?}");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(fs::read(path).unwrap(), b"ready");
    }
}
