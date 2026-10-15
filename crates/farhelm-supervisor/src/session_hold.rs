//! New sessions hold their version through the login shell's startup
//! (SPEC_impl.md, "Side-by-side versions inside Farhelm.app", and the
//! race-free installer design it implements).
//!
//! A session's window must start the supervisor's own version (the launch
//! shim reads a launch description only that version writes), so it does
//! not go through the forwarder and nothing there claims the version. The
//! gap that leaves is the login shell's startup: between tmux starting the
//! window and the launch shim taking its own lock (`version_lock`, at the
//! top of `main`), the user's startup files run for as long as they like,
//! and an install could delete the version the shim is about to run.
//!
//! In the version layout the window therefore starts with a tiny hold step
//! ([`held_window_command`]) that runs, in order:
//!
//! 1. the shared lock, taken by `main` exactly as every versioned program
//!    takes it;
//! 2. a keeper ([`run_keeper`]), a detached `farhelm` that adopts that lock
//!    and keeps it, started and confirmed ready before going on: a session
//!    whose keeper cannot start fails to start, it never continues unheld;
//! 3. the "held" mark, carrying the hold step's pid (the pane's pid);
//! 4. `exec` of the login shell, which inherits neither the lock's
//!    descriptor nor its variable, so nothing the startup files start can
//!    keep the version pinned after the session moves on.
//!
//! The launch shim, once it holds its own lock, writes the "launched" mark
//! with its pid ([`mark_launched`]). Hold step, shell and shim replace one
//! another in place, so that pid is the one the keeper was told to watch;
//! the keeper accepts only that pid, exits on it, and exits as well when the
//! shell exits first. Both marks sit beside the launch spec
//! ([`held_mark_path`], [`launched_mark_path`]), never in the status file,
//! whose mere presence the supervisor reads as a failed launch.
//!
//! What stays open, by design: a window that has not reached "held" when
//! its supervisor stops, or whose keeper is killed before the shim holds
//! its own lock, is unprotected for that moment; an install that deletes
//! its version then makes that session fail to start, visibly, and starting
//! it again works. The supervisor's orderly shutdown narrows the first case
//! by waiting briefly for its pending launches to show "held".

use crate::launch::{AGENT_ID_ENV_VAR, SESSION_ID_ENV_VAR, TAB_ID_ENV_VAR};
use crate::version_lock::{self, LOCK_FD_ENV};
use anyhow::Context;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long the hold step waits for its keeper to report that it holds the
/// lock and watches the shell. Starting a program and registering one
/// kernel watch takes milliseconds; the bound only exists so a keeper that
/// hangs fails the session's start visibly instead of wedging its window.
pub const KEEPER_READY_WAIT: Duration = Duration::from_secs(10);

/// How often the keeper looks for the "launched" mark while shell startup
/// is still young, and how often once it has run long. A shell whose
/// startup files never reach the launch shim (one that `exec`s another
/// shell, say) keeps its keeper until the window closes; the slower pace
/// keeps that case from waking the machine ten times a second for hours.
const LAUNCHED_POLL_FAST: Duration = Duration::from_millis(100);
const LAUNCHED_POLL_SLOW: Duration = Duration::from_secs(1);
const LAUNCHED_FAST_FOR: Duration = Duration::from_secs(10);

/// The window argv in the version layout: the hold step on `spec_path`,
/// then, after `--`, the argv the window would otherwise run (the login
/// shell and its `-c` payload, see `launch::window_command`).
///
/// Only the supervisor's own versioned program may run the hold step, for
/// the same reason the shim has to be that program: it locks the version
/// it runs from, and that has to be the version this supervisor launches.
pub fn held_window_command(
    farhelm_exe: &Path,
    spec_path: &Path,
    window: Vec<String>,
) -> Vec<String> {
    // Both paths are valid UTF-8 for the reason `launch::window_command`
    // gives (supervisor startup refuses anything else), and tmux runs this
    // argv directly, so nothing here is quoted.
    let mut argv = vec![
        farhelm_exe.to_string_lossy().into_owned(),
        "internal".to_string(),
        "hold".to_string(),
        spec_path.to_string_lossy().into_owned(),
        "--".to_string(),
    ];
    argv.extend(window);
    argv
}

/// Where the hold step records that the keeper holds this launch's version:
/// `<state>/launch/<id>.<generation>.held`, holding the pane's pid.
pub fn held_mark_path(spec_path: &Path) -> PathBuf {
    spec_path.with_extension("held")
}

/// Where the launch shim records that it holds this launch's version
/// itself: `<state>/launch/<id>.<generation>.launched`, holding its pid.
pub fn launched_mark_path(spec_path: &Path) -> PathBuf {
    spec_path.with_extension("launched")
}

/// Write a mark holding `pid`. The content is compared exactly, so a reader
/// that catches the write half done sees a different number and simply
/// looks again; nothing here needs atomic replacement.
fn write_mark(path: &Path, pid: u32) -> io::Result<()> {
    std::fs::write(path, format!("{pid}\n"))
}

/// The pid a mark at `path` holds, or `None` while it is absent, partly
/// written or not a pid.
pub fn read_mark(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// The launch shim's half of the hand-off, called once it holds its own
/// lock and before it reads its spec: record "launched" with this process's
/// pid so the keeper can let go.
///
/// Best effort on purpose. A mark that cannot be written only means the
/// keeper keeps the version until the window's shell exits, which delays
/// pruning and nothing else, whereas failing here would fail a launch that
/// is fully protected by the shim's own lock.
pub fn mark_launched(spec_path: &Path) {
    let _ = write_mark(&launched_mark_path(spec_path), std::process::id());
}

/// Record a session start that failed before the agent could run, in the
/// launch's status file, so the session shows the failure instead of an
/// ordinary exit. Returns the report as written (or with the reason it
/// could not be).
pub fn record_start_failure(spec_path: &Path, report: String) -> String {
    crate::launch::record_launch_failure(
        &crate::launch::status_path_for_spec(spec_path),
        report,
        &crate::files::RealFs,
    )
}

/// `farhelm internal hold <spec> -- <window argv>`: the hold step. `main`
/// has already taken this process's lock; this starts the keeper, marks
/// "held" and becomes the window's argv. Returns only on failure, after
/// recording it in the launch's status file.
///
/// Without a lock (locks unsupported where the bundle lives) there is
/// nothing to keep, and the window argv simply runs.
pub fn run_hold(exe: &Path, spec_path: &Path, window: &[OsString]) -> anyhow::Error {
    let Some((program, args)) = window.split_first() else {
        let report = record_start_failure(spec_path, "the hold step got no command".to_string());
        return anyhow::anyhow!(report);
    };
    if let Some(lock) = version_lock::process_hold() {
        if let Err(error) = start_keeper(exe, spec_path, lock) {
            let report = record_start_failure(
                spec_path,
                format!(
                    "could not start the process that keeps this session's version of Farhelm \
                     while its shell starts: {error:#}"
                ),
            );
            return error.context(report);
        }
        if let Err(error) = write_mark(&held_mark_path(spec_path), std::process::id()) {
            let report = record_start_failure(
                spec_path,
                format!("writing {}: {error}", held_mark_path(spec_path).display()),
            );
            return anyhow::Error::from(error).context(report);
        }
    }
    // The lock's descriptor is close-on-exec and `main` removed its
    // variable, so the shell gets neither.
    let error = Command::new(program).args(args).exec();
    let report = record_start_failure(
        spec_path,
        format!(
            "exec_failed argv0={} errno={}",
            program.to_string_lossy(),
            error.raw_os_error().unwrap_or(-1)
        ),
    );
    anyhow::Error::from(error).context(report)
}

/// Start the keeper detached, hand it `lock`, and wait until it reports
/// that it holds the lock and watches this process.
///
/// The keeper is a separate `farhelm` (`internal keep-version`), so it runs
/// from this same version and adopts the lock exactly the way a program
/// started by the forwarder does: through [`LOCK_FD_ENV`], checked against
/// its own file. The open file it adopts is the one this process locked, so
/// the lock stays when this process `exec`s the shell and its own copy of
/// the descriptor closes.
///
/// Detaching is a double fork: the child starts its own session (so Ctrl-C
/// during shell startup and the terminal's hangup never reach the keeper),
/// forks the keeper and exits, and this process reaps it before going on,
/// so the keeper is nobody's child under the agent. The keeper also gets
/// `/dev/null` for its streams, so it never writes into the pane, and
/// ignores the terminal's job-control and hangup signals. It carries the
/// session's markers (as the shim sets them for the agent) so that stopping
/// the session finds and ends it; the shell never sees those markers.
fn start_keeper(exe: &Path, spec_path: &Path, lock: &File) -> anyhow::Result<()> {
    let (ready_read, ready_write) = cloexec_pipe().context("creating the keeper's ready pipe")?;
    let lock_fd = lock.as_raw_fd();
    let ready_fd = ready_write.as_raw_fd();
    let mut command = Command::new(exe);
    command
        .args(["internal", "keep-version", "--watch"])
        .arg(std::process::id().to_string())
        .arg("--ready-fd")
        .arg(ready_fd.to_string())
        .arg("--launched")
        .arg(launched_mark_path(spec_path))
        .env(LOCK_FD_ENV, lock_fd.to_string())
        .env_remove(TAB_ID_ENV_VAR)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(session_id) = session_id_of(spec_path) {
        command
            .env(SESSION_ID_ENV_VAR, session_id)
            .env(AGENT_ID_ENV_VAR, session_id);
    }
    // SAFETY: the closure runs in the forked child before `exec` and calls
    // only async-signal-safe functions (setsid, fork, _exit, signal, fcntl).
    // This process is single-threaded at this point (the hold step runs no
    // runtime), so the fork inside it duplicates no other thread's state.
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            match libc::fork() {
                -1 => return Err(io::Error::last_os_error()),
                0 => {}
                // The intermediate child: its only job was to make the
                // keeper an orphan. Exiting without unwinding is the only
                // safe exit in a forked child.
                _ => libc::_exit(0),
            }
            for signal in [libc::SIGINT, libc::SIGQUIT, libc::SIGTSTP, libc::SIGHUP] {
                libc::signal(signal, libc::SIG_IGN);
            }
            // Both descriptors are close-on-exec in this process; only the
            // keeper's copies may survive its exec.
            for fd in [lock_fd, ready_fd] {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    // `spawn` returns once the keeper has exec'd (or reports why it could
    // not), because the grandchild still holds std's exec-error pipe.
    let mut intermediate = command.spawn().context("starting the keeper")?;
    drop(ready_write);
    intermediate
        .wait()
        .context("reaping the keeper's intermediate process")?;
    wait_ready(ready_read, KEEPER_READY_WAIT)
}

/// The session id a launch spec's file name carries, for the keeper's
/// markers. `None` for a name that is not a launch file, which only a test
/// passes; the keeper then simply carries no markers.
fn session_id_of(spec_path: &Path) -> Option<&str> {
    let name = spec_path.file_name()?.to_str()?;
    crate::launch::parse_launch_file_name(name).map(|(id, _)| id)
}

/// A pipe whose both ends are close-on-exec, so neither leaks into the
/// shell or the agent. (macOS has no `pipe2`, hence the two steps; this
/// process is single-threaded, so no other thread can fork in between.)
fn cloexec_pipe() -> io::Result<(File, OwnedFd)> {
    let mut fds = [0 as RawFd; 2];
    // SAFETY: `fds` is a writable array of two descriptors, as `pipe`
    // requires.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `pipe` just returned these two descriptors, owned by nothing
    // else.
    let (read, write) = unsafe { (File::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    for fd in [read.as_raw_fd(), write.as_raw_fd()] {
        // SAFETY: plain fcntl on a descriptor this function owns.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok((read, write))
}

/// Wait until the keeper writes its one ready byte. End of input (the
/// keeper exited, or never got as far as holding the lock) or the deadline
/// is a failure.
fn wait_ready(mut ready: File, wait: Duration) -> anyhow::Result<()> {
    let mut poll = libc::pollfd {
        fd: ready.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let deadline = Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let millis = libc::c_int::try_from(left.as_millis()).unwrap_or(libc::c_int::MAX);
        // SAFETY: one valid pollfd for a descriptor this function owns.
        let polled = unsafe { libc::poll(&mut poll, 1, millis) };
        if polled < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error).context("waiting for the keeper");
        }
        if polled == 0 {
            anyhow::bail!("the keeper did not report ready within {wait:?}");
        }
        let mut byte = [0u8; 1];
        return match ready.read(&mut byte) {
            Ok(1) => Ok(()),
            Ok(_) => anyhow::bail!("the keeper exited before it held the version"),
            Err(error) => Err(error).context("reading the keeper's ready report"),
        };
    }
}

/// `farhelm internal keep-version`: the keeper. `main` has already adopted
/// the hold step's lock through [`LOCK_FD_ENV`]; this registers a watch on
/// the hold step's process (`watch`, which becomes the shell and then the
/// launch shim), reports ready on `ready_fd`, and returns, releasing the
/// lock with the process, once the "launched" mark at `launched` names
/// `watch` or `watch` has exited.
///
/// The watch is registered before reporting ready, so before the shell
/// starts; a process that is already gone by then (even an unreaped
/// zombie) is reported by the registration itself, and the keeper exits
/// without reporting ready.
pub fn run_keeper(watch: i32, launched: &Path, ready_fd: RawFd) -> anyhow::Result<()> {
    anyhow::ensure!(ready_fd >= 0, "invalid ready descriptor {ready_fd}");
    // SAFETY: the hold step passed this descriptor for exactly this
    // process; nothing else here owns it.
    let mut ready = unsafe { File::from_raw_fd(ready_fd) };
    anyhow::ensure!(
        version_lock::process_hold().is_some(),
        "the keeper did not receive its version's lock"
    );
    let Some(exit) = ExitWatch::register(watch).context("watching the session's shell")? else {
        return Ok(());
    };
    ready
        .write_all(b"r")
        .context("reporting ready to the hold step")?;
    drop(ready);
    let started = Instant::now();
    let watch_pid = u32::try_from(watch).ok();
    loop {
        if watch_pid.is_some() && read_mark(launched) == watch_pid {
            return Ok(());
        }
        let pace = if started.elapsed() < LAUNCHED_FAST_FOR {
            LAUNCHED_POLL_FAST
        } else {
            LAUNCHED_POLL_SLOW
        };
        if exit.exited_within(pace)? {
            return Ok(());
        }
    }
}

/// Notice when one process exits: kqueue's `NOTE_EXIT` on macOS, which
/// reports the exit itself (a zombie counts as exited) as soon as it
/// happens.
#[cfg(target_os = "macos")]
struct ExitWatch {
    kqueue: OwnedFd,
}

#[cfg(target_os = "macos")]
impl ExitWatch {
    /// Watch `pid`, or `None` when it has already exited.
    fn register(pid: i32) -> io::Result<Option<Self>> {
        // SAFETY: plain syscall; the result is checked before use.
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `kqueue` just returned this descriptor.
        let kqueue = unsafe { OwnedFd::from_raw_fd(kq) };
        let change = libc::kevent {
            ident: pid as libc::uintptr_t,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ONESHOT,
            fflags: libc::NOTE_EXIT,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        // SAFETY: one valid change record, no event buffer, no timeout.
        let registered = unsafe {
            libc::kevent(
                kqueue.as_raw_fd(),
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        if registered < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                return Ok(None);
            }
            return Err(error);
        }
        Ok(Some(Self { kqueue }))
    }

    /// Whether the process exited, waiting at most `wait` for it.
    fn exited_within(&self, wait: Duration) -> io::Result<bool> {
        let timeout = libc::timespec {
            tv_sec: wait.as_secs() as libc::time_t,
            tv_nsec: wait.subsec_nanos() as libc::c_long,
        };
        // SAFETY: an all-zero kevent is a valid output buffer.
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        loop {
            // SAFETY: no changes, one output slot, a valid timeout.
            let got = unsafe {
                libc::kevent(
                    self.kqueue.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    &mut event,
                    1,
                    &timeout,
                )
            };
            if got < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            return Ok(got > 0);
        }
    }
}

/// Everywhere else the version layout exists only in tests, so a plain
/// liveness poll stands in for kqueue: `kill(pid, 0)` failing with ESRCH
/// means the process is gone. Unlike `NOTE_EXIT` it keeps seeing a zombie
/// until its parent reaps it, which in a test is the test's own child
/// handling.
#[cfg(not(target_os = "macos"))]
struct ExitWatch {
    pid: i32,
}

#[cfg(not(target_os = "macos"))]
impl ExitWatch {
    /// Watch `pid`, or `None` when it has already exited.
    fn register(pid: i32) -> io::Result<Option<Self>> {
        let watch = Self { pid };
        Ok((!watch.gone()).then_some(watch))
    }

    fn gone(&self) -> bool {
        // SAFETY: signal 0 only checks that the process exists.
        let failed = unsafe { libc::kill(self.pid, 0) } < 0;
        failed && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    }

    /// Whether the process exited, waiting at most `wait` for it.
    fn exited_within(&self, wait: Duration) -> io::Result<bool> {
        if self.gone() {
            return Ok(true);
        }
        std::thread::sleep(wait);
        Ok(self.gone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hold step goes first and the window's own argv follows `--`
    /// untouched, so whatever the shell and its payload were, the hold step
    /// becomes exactly that after it has the version held.
    #[farhelm_testtrace::test]
    fn the_held_window_runs_the_hold_step_then_the_window_argv() {
        let argv = held_window_command(
            Path::new("/A.app/Contents/Versions/1.2.3/farhelm"),
            Path::new("/state/launch/s.0.json"),
            vec!["/bin/zsh".into(), "-l".into(), "-c".into(), "exec x".into()],
        );
        assert_eq!(
            argv,
            [
                "/A.app/Contents/Versions/1.2.3/farhelm",
                "internal",
                "hold",
                "/state/launch/s.0.json",
                "--",
                "/bin/zsh",
                "-l",
                "-c",
                "exec x",
            ]
        );
    }

    /// The marks are the spec's siblings with their own extensions, which
    /// is what lets the launch directory's sweeps and deletes find them by
    /// the same name parsing as the spec and the status file.
    #[farhelm_testtrace::test]
    fn the_marks_sit_beside_the_spec() {
        let spec = Path::new("/state/launch/abc.4.json");
        assert_eq!(held_mark_path(spec), Path::new("/state/launch/abc.4.held"));
        assert_eq!(
            launched_mark_path(spec),
            Path::new("/state/launch/abc.4.launched")
        );
        assert_eq!(session_id_of(spec), Some("abc"));
    }

    /// A mark is accepted only as a whole pid: a half-written or foreign
    /// content reads as absent, so the keeper looks again rather than
    /// letting go on a number that is not the one it watches.
    #[farhelm_testtrace::test]
    fn a_mark_reads_as_a_whole_pid_or_not_at_all() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mark = dir.path().join("s.0.launched");
        assert_eq!(read_mark(&mark), None, "absent");
        std::fs::write(&mark, "12").expect("write");
        assert_eq!(read_mark(&mark), Some(12));
        std::fs::write(&mark, "12x").expect("write");
        assert_eq!(read_mark(&mark), None, "not a pid");
        write_mark(&mark, 4321).expect("write");
        assert_eq!(read_mark(&mark), Some(4321));
    }
}
