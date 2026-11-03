//! The desktop app's record of the supervisor it started, and the only way
//! anything stops that supervisor.
//!
//! A supervisor the desktop app started can keep running after the app
//! quits, as a Linux supervisor runs under systemd. Two things then need to
//! stop it: the app, when it finds the supervisor it started running a
//! different program than it would start now (an update) or holding the
//! state directory without answering; and uninstall, which cannot remove a
//! version a running supervisor holds (SPEC.md, "Operator prerequisites and
//! failure behavior"). Neither may ever stop a supervisor the app did not
//! start, and the record outlives many app launches, so a bare pid is not
//! enough. The record pins the process by pid and kernel start identity
//! (see `procs::ProcessIdentity`) and names the program file it was started
//! from, and every action first re-reads the live process
//! ([`ManagedSupervisor::observe`]): same pid, same start identity, and
//! still running that program. A record whose process is gone, whose pid
//! now belongs to another process, or whose process now runs something
//! else authorizes nothing.
//!
//! The program's file identity (device and inode) is also what decides
//! whether the running supervisor is the one the app would start now: every
//! untagged build reports the same version string, so comparing versions
//! would leave each development build after the first talking to the first
//! build's supervisor. That identity has to be read before the spawn
//! ([`ProgramFile::read`]): read after it, a file replaced in between would
//! be recorded as the program the supervisor runs.
//!
//! The stop is the supervisor's own orderly shutdown: SIGTERM, which runs the
//! same output-client teardown as a systemd stop. A kill after the grace is
//! the caller's choice, because killing the supervisor can take its tmux
//! server and every session with it; uninstall never kills. A stop that
//! cannot be confirmed (the process could not be read or signalled) is an
//! error, never "gone": callers keep the record and refuse, so a supervisor
//! that is still running keeps the record that lets it be stopped later.
//!
//! "Gone", for a stop, means finished exiting, not merely that the main
//! thread reads as exited. On Linux an exiting process's main thread can
//! read as a zombie while its other threads are still on their way out,
//! holding the state directory's lock and the supervisor socket; a
//! successor the app started then found both taken (see
//! `procs::finished_exiting`). So a stop reports the process gone only once
//! every thread has finished, and [`ManagedSupervisor::wait_exited`] gives
//! the same wait to a caller that found the process already gone.

use crate::procs::{ProcessIdentity, ProcessState};
use serde::{Deserialize, Serialize};
use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The record's file name inside the supervisor's state directory.
pub const RECORD_FILE: &str = "desktop-supervisor.json";

/// How long an orderly stop waits for the supervisor before giving up (or,
/// for the app's own wedged supervisor, killing it).
///
/// Longer than the supervisor's own budget for closing its terminal-output
/// clients ([`crate::service::SHUTDOWN_OUTPUT_BUDGET`]), so a supervisor doing
/// exactly that is never cut off; only one still running well past it counts
/// as stuck.
pub const ORDERLY_STOP_GRACE: Duration =
    crate::service::SHUTDOWN_OUTPUT_BUDGET.saturating_add(Duration::from_secs(5));

/// How often a stop re-reads the process while waiting for it to exit.
const POLL: Duration = Duration::from_millis(50);

/// How long [`ManagedSupervisor::observe`] lets a process whose executable
/// can no longer be read finish exiting before it reports the image as
/// unreadable.
const EXITING_WINDOW: Duration = Duration::from_secs(1);

/// How long a stop waits for the process to disappear after a kill, and
/// for one it found already exiting to finish.
const AFTER_KILL: Duration = Duration::from_secs(5);

/// A program file, identified by its resolved path and its device and
/// inode at the moment it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramFile {
    /// The path with every link resolved, which is how the kernel reports a
    /// running process's executable.
    pub path: PathBuf,
    pub dev: u64,
    pub ino: u64,
}

impl ProgramFile {
    /// Read `program`'s resolved path and file identity. Call it before
    /// spawning from `program` (see the module docs for why).
    pub fn read(program: &Path) -> io::Result<Self> {
        let path = std::fs::canonicalize(program)?;
        let metadata = std::fs::metadata(&path)?;
        Ok(Self {
            path,
            dev: metadata.dev(),
            ino: metadata.ino(),
        })
    }
}

/// The supervisor a desktop app started: the exact process, and the exact
/// program file it was started from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedSupervisor {
    /// The process id when the app started it.
    pub pid: u32,
    /// The kernel start identity that tells this process apart from a later
    /// one reusing the pid.
    pub start: u64,
    /// The program the app ran, as its resolved path.
    pub program: PathBuf,
    /// That program file's device, read before the spawn.
    pub program_dev: u64,
    /// That program file's inode, read before the spawn.
    pub program_ino: u64,
}

/// What a fresh look at a recorded process found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    /// The recorded process: same pid and start identity, running the
    /// recorded program.
    Matches,
    /// No process with that pid and start identity is running (it exited,
    /// is a zombie, or the pid now belongs to another process).
    ///
    /// On Linux this is read from the main thread alone, so a process can
    /// read as `Gone` while its other threads are still exiting and still
    /// hold its descriptors. [`ManagedSupervisor::stop`] waits for those; a
    /// caller that needs what the process held to be free must stop it, or
    /// wait with [`ManagedSupervisor::wait_exited`], rather than act on this
    /// alone.
    Gone,
    /// The recorded process is running but not the recorded program (it
    /// exec'd something else, or the record names the wrong program). It
    /// authorizes nothing.
    RunsSomethingElse,
}

/// What [`ManagedSupervisor::stop`] found or did.
///
/// `AlreadyGone`, `Stopped` and `Killed` mean the process has finished
/// exiting: no thread of it can run any more, so the descriptors it held,
/// the state directory's lock and the supervisor socket among them, are
/// closed (copies in a child it forked and that had not exec'd yet aside).
/// It may still be an unreaped zombie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOutcome {
    /// The recorded process was already gone, or already exiting and then
    /// finished within a few seconds; nothing was signalled.
    AlreadyGone,
    /// The process exited after the orderly stop.
    Stopped,
    /// The process ignored the orderly stop within the grace and was killed.
    Killed,
    /// The process has not finished: it outlived the grace and the caller
    /// did not allow a kill, or it had not finished within a few seconds of
    /// the kill. Either way it may still be running, or it may have ended
    /// with a thread still exiting (one that outlived the grace, or one
    /// stuck in the kernel after the kill).
    StillRunning,
    /// The process was already exiting, or exited on its own before the
    /// kill, but some thread of it had not finished within a few seconds
    /// (`AFTER_KILL`), so it may still hold its descriptors. A thread
    /// stuck in the kernel (a hung network filesystem, a long core dump)
    /// does that. Nothing was killed.
    StillExiting,
}

impl ManagedSupervisor {
    /// Describe the live process `pid`, started from `program`, as a record.
    ///
    /// `program` must have been read before the spawn. Returns `None` when
    /// the process is already gone: there is nothing to record, and a record
    /// naming a dead pid would only wait to be mistaken for a later process.
    pub fn for_process(pid: u32, program: &ProgramFile) -> Option<Self> {
        let identity = ProcessIdentity::read(pid)?;
        Some(Self {
            pid,
            start: identity.start,
            program: program.path.clone(),
            program_dev: program.dev,
            program_ino: program.ino,
        })
    }

    /// Read the record in `state_dir`, if there is a usable one.
    ///
    /// A missing file is `Ok(None)`. So is a file that does not parse: an
    /// unreadable record authorizes nothing, which is the safe direction for
    /// every caller (they then treat the supervisor as one the app did not
    /// start). Other I/O errors are returned.
    pub fn read(state_dir: &Path) -> io::Result<Option<Self>> {
        match std::fs::read(state_dir.join(RECORD_FILE)) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Write this record into `state_dir`, replacing any earlier one.
    ///
    /// Durable (file and directory synced): a record lost to a crash would
    /// make the app treat its own supervisor as foreign and never replace it
    /// on an update.
    pub fn write(&self, state_dir: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec(self).map_err(io::Error::other)?;
        crate::files::write_durable_sync(
            &state_dir.join(RECORD_FILE),
            &bytes,
            &crate::files::RealFs,
        )
    }

    /// Remove the record from `state_dir`; a missing record is fine.
    pub fn remove(state_dir: &Path) -> io::Result<()> {
        match std::fs::remove_file(state_dir.join(RECORD_FILE)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }

    /// Whether this supervisor was started from `program`: the same
    /// resolved path and the same file (device and inode).
    ///
    /// This compares the record with a file as it is now, which is what "is
    /// the supervisor running the one I would start now?" needs.
    pub fn started_from(&self, program: &ProgramFile) -> bool {
        program.path == self.program
            && (program.dev, program.ino) == (self.program_dev, self.program_ino)
    }

    /// Look at the recorded process afresh.
    ///
    /// The image check compares the running executable's path with the
    /// recorded one and, where the platform exposes the running image's own
    /// file identity (Linux), that identity too; on macOS only the path can
    /// be compared. An error means the process could not be read, which
    /// callers must not take as gone.
    pub fn observe(&self) -> Result<Observation, String> {
        let same_process = |process: Option<(u32, u64, ProcessState)>| matches!(process, Some((_, start, ProcessState::Running)) if start == self.start);
        if !same_process(crate::procs::read_process(self.pid)?) {
            return Ok(Observation::Gone);
        }
        let image = match crate::procs::process_image(self.pid) {
            Ok(image) => image,
            // An exiting process gives up its executable a moment before
            // the kernel reports it as a zombie, so an unreadable image on
            // a process that still reads as running is first given
            // `EXITING_WINDOW` to finish going. Only one that is still
            // running after that is reported as unreadable.
            Err(error) => {
                let deadline = Instant::now() + EXITING_WINDOW;
                loop {
                    if !same_process(crate::procs::read_process(self.pid)?) {
                        return Ok(Observation::Gone);
                    }
                    if let Ok(image) = crate::procs::process_image(self.pid) {
                        break image;
                    }
                    if Instant::now() >= deadline {
                        return Err(error);
                    }
                    std::thread::sleep(POLL / 5);
                }
            }
        };
        let same_file = image
            .file
            .is_none_or(|file| file == (self.program_dev, self.program_ino));
        Ok(if image.path == self.program && same_file {
            Observation::Matches
        } else {
            Observation::RunsSomethingElse
        })
    }

    /// Stop the recorded process the orderly way, if it is still exactly
    /// that process running that program.
    ///
    /// Sends SIGTERM and waits up to `grace` for the process to go. With
    /// `kill_after`, a process still running after the grace is killed and
    /// given a few more seconds. A process found already gone is still
    /// given those seconds to finish exiting, since its main thread may
    /// have exited before its others (see [`StopOutcome`] for what every
    /// outcome promises). The process is observed afresh before every
    /// signal, so a pid that has meanwhile passed to another process is
    /// never signalled. Errors when the process could not be read or
    /// signalled, or runs another program: the stop is then unconfirmed, and
    /// the caller must neither proceed as if it had succeeded nor forget the
    /// record.
    pub fn stop(&self, grace: Duration, kill_after: bool) -> Result<StopOutcome, String> {
        self.stop_observed(grace, kill_after, AFTER_KILL, &|| {})
    }

    /// Wait up to a few seconds (`AFTER_KILL`) for a recorded process that
    /// reads as [`Observation::Gone`] to finish exiting, as a stop would.
    /// `Ok(true)` once it has (see [`StopOutcome`] for what that promises),
    /// `Ok(false)` if a thread of it is still exiting after that. For a
    /// caller that found the process gone and is about to start something
    /// that needs what it held. A process that still matches is not gone,
    /// and is waited for until the time is up.
    pub fn wait_exited(&self) -> Result<bool, String> {
        self.wait_gone(AFTER_KILL, &|| {})
    }

    /// [`Self::stop`] with its few seconds after a kill, or for a process
    /// found exiting, given as `exit_bound`, and calling `on_exiting` each
    /// time a wait finds the process reading as gone with a thread of it
    /// still exiting. Both are for tests: the hook lets a test hold that
    /// thread until the stop has demonstrably looked at it, and the bound
    /// lets one reach the refusal without waiting out `AFTER_KILL`.
    fn stop_observed(
        &self,
        grace: Duration,
        kill_after: bool,
        exit_bound: Duration,
        on_exiting: &dyn Fn(),
    ) -> Result<StopOutcome, String> {
        if !self.signal(libc::SIGTERM)? {
            return self.once_finished(StopOutcome::AlreadyGone, exit_bound, on_exiting);
        }
        if self.wait_gone(grace, on_exiting)? {
            return Ok(StopOutcome::Stopped);
        }
        if !kill_after {
            return Ok(StopOutcome::StillRunning);
        }
        if !self.signal(libc::SIGKILL)? {
            // It went on its own between the grace ending and the kill.
            return self.once_finished(StopOutcome::Stopped, exit_bound, on_exiting);
        }
        Ok(if self.wait_gone(exit_bound, on_exiting)? {
            StopOutcome::Killed
        } else {
            StopOutcome::StillRunning
        })
    }

    /// Send `signal` to the recorded process if it still matches. Returns
    /// `Ok(false)` when it is gone (nothing sent), `Ok(true)` when sent.
    fn signal(&self, signal: libc::c_int) -> Result<bool, String> {
        match self.observe()? {
            Observation::Gone => return Ok(false),
            Observation::RunsSomethingElse => {
                return Err(format!(
                    "process {} does not run {}",
                    self.pid,
                    self.program.display()
                ));
            }
            Observation::Matches => {}
        }
        let pid = libc::pid_t::try_from(self.pid)
            .map_err(|_| format!("process number {} is out of range", self.pid))?;
        // SAFETY: kill has no memory preconditions. The observation just
        // above makes it this process; the pid could still pass to another
        // process between that check and this call, which is the same short
        // window the session sweep accepts (SPEC_impl.md, "Process-number
        // reuse inside a short window is accepted").
        if unsafe { libc::kill(pid, signal) } == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(false);
        }
        Err(format!("signalling process {}: {error}", self.pid))
    }

    /// `outcome` once the recorded process, which already reads as gone,
    /// has finished exiting; `StillExiting` if it has not within
    /// `exit_bound`.
    fn once_finished(
        &self,
        outcome: StopOutcome,
        exit_bound: Duration,
        on_exiting: &dyn Fn(),
    ) -> Result<StopOutcome, String> {
        Ok(if self.wait_gone(exit_bound, on_exiting)? {
            outcome
        } else {
            StopOutcome::StillExiting
        })
    }

    /// Wait up to `limit` for the recorded process to be gone and to have
    /// finished exiting (no thread of it able to run; see
    /// [`crate::procs::finished_exiting`]), calling `on_exiting` on each
    /// look that finds it gone but not finished.
    fn wait_gone(&self, limit: Duration, on_exiting: &dyn Fn()) -> Result<bool, String> {
        let deadline = Instant::now() + limit;
        loop {
            if self.observe()? == Observation::Gone {
                if crate::procs::finished_exiting(self.pid, self.start)? {
                    return Ok(true);
                }
                on_exiting();
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            std::thread::sleep(POLL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt as _;
    use std::process::{Child, Command, ExitStatus};

    /// A private copy of `sleep`, so a stand-in runs a program file the test
    /// owns (and can replace) rather than a shared system binary.
    ///
    /// Copied by `cp`, not in this process: a file this multi-threaded test
    /// process holds open for writing can be inherited by a child another
    /// test is spawning at that moment, and exec'ing the copy then fails
    /// with "Text file busy".
    fn sleep_copy(dir: &Path, name: &str) -> PathBuf {
        let program = dir.join(name);
        let copied = Command::new("cp")
            .arg("/bin/sleep")
            .arg(&program)
            .status()
            .expect("fixture: running cp");
        assert!(copied.success(), "fixture: copying sleep");
        program
    }

    /// Run `program 60` as a stand-in supervisor, with SIGTERM ignored when
    /// `ignore_term`.
    ///
    /// The ignoring stand-in starts as a shell that ignores SIGTERM and
    /// exec's `program`; an ignored disposition survives exec, so `program`
    /// itself ignores it. (A `pre_exec` hook would do the same without the
    /// shell, but forces the slower fork path, which widens the window in
    /// which the child holds other tests' descriptors.) It is waited for
    /// until it runs `program`, so the caller's record matches at once.
    fn stand_in(program: &Path, ignore_term: bool) -> Child {
        if !ignore_term {
            return Command::new(program)
                .arg("60")
                .spawn()
                .expect("fixture: spawning a stand-in supervisor");
        }
        let child = Command::new("/bin/sh")
            .args(["-c", "trap '' TERM; exec \"$0\" 60"])
            .arg(program)
            .spawn()
            .expect("fixture: spawning a stand-in supervisor");
        let image = std::fs::canonicalize(program).expect("fixture: resolving the program");
        let deadline = Instant::now() + Duration::from_secs(10);
        while crate::procs::process_image(child.id()).map(|found| found.path) != Ok(image.clone()) {
            assert!(
                Instant::now() < deadline,
                "fixture: the stand-in never exec'd its program"
            );
            // sleep-ok: polling interval for the stand-in's exec.
            std::thread::sleep(Duration::from_millis(10));
        }
        child
    }

    /// The record of `child`, started from `program` (read before the spawn
    /// by the caller, as the app must).
    fn record_of(child: &Child, program: &ProgramFile) -> ManagedSupervisor {
        ManagedSupervisor::for_process(child.id(), program).expect("fixture: the stand-in is alive")
    }

    /// Collect `child` on a thread while a stop polls, as init would for a
    /// supervisor whose app has quit, so the stopped process reads as gone
    /// rather than as this test's zombie.
    fn reap(mut child: Child) -> std::thread::JoinHandle<ExitStatus> {
        std::thread::spawn(move || child.wait().expect("waiting for the stand-in"))
    }

    /// The record pins the process: it round-trips through the state
    /// directory, matches the live process, and stops matching once that
    /// process is gone.
    ///
    /// Why it matters: the record outlives many app launches, so "matches"
    /// must mean this exact process, not this pid.
    #[farhelm_testtrace::test]
    fn a_record_matches_its_process_until_that_process_exits() {
        let dir = tempfile::tempdir().unwrap();
        let program = sleep_copy(dir.path(), "supervisor");
        let file = ProgramFile::read(&program).unwrap();
        let mut sleeper = stand_in(&program, false);
        let record = record_of(&sleeper, &file);
        record.write(dir.path()).unwrap();
        let read = ManagedSupervisor::read(dir.path())
            .unwrap()
            .expect("record");
        assert_eq!(read, record);
        assert_eq!(read.observe(), Ok(Observation::Matches));
        sleeper.kill().unwrap();
        sleeper.wait().unwrap();
        assert_eq!(
            read.observe(),
            Ok(Observation::Gone),
            "a reaped process no longer matches"
        );
        assert_eq!(
            read.stop(Duration::from_secs(1), true),
            Ok(StopOutcome::AlreadyGone)
        );
    }

    /// The same pid with another start identity is another process, and is
    /// never signalled.
    ///
    /// Why it matters: a stale record must never stop whatever process later
    /// got its pid. The stop also returns at once rather than waiting out its
    /// few seconds for the live process's threads: the recorded process is
    /// the one that has finished, which `procs::finished_exiting` decides by
    /// the start identity too.
    #[farhelm_testtrace::test]
    fn a_different_start_identity_authorizes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let program = sleep_copy(dir.path(), "supervisor");
        let file = ProgramFile::read(&program).unwrap();
        let mut sleeper = stand_in(&program, false);
        let mut record = record_of(&sleeper, &file);
        record.start = record.start.wrapping_add(1);
        assert_eq!(record.observe(), Ok(Observation::Gone));
        assert_eq!(
            record.stop(Duration::from_millis(200), true),
            Ok(StopOutcome::AlreadyGone)
        );
        assert!(
            sleeper.try_wait().unwrap().is_none(),
            "the real process with that pid must not have been signalled"
        );
        sleeper.kill().unwrap();
        sleeper.wait().unwrap();
    }

    /// A process that matches by pid and start identity but runs another
    /// program is not the recorded supervisor: observing it says so, and a
    /// stop errors instead of signalling it.
    ///
    /// Why it matters: a record could name the wrong program (damaged, or a
    /// process that then exec'd something else), and pid plus start identity
    /// alone would then authorize stopping a process that is not a
    /// supervisor the app started.
    #[farhelm_testtrace::test]
    fn a_process_running_another_program_authorizes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let program = sleep_copy(dir.path(), "supervisor");
        let other = sleep_copy(dir.path(), "other");
        let mut sleeper = stand_in(&other, false);
        let record = record_of(&sleeper, &ProgramFile::read(&program).unwrap());
        assert_eq!(record.observe(), Ok(Observation::RunsSomethingElse));
        assert!(record.stop(Duration::from_millis(200), true).is_err());
        assert!(
            sleeper.try_wait().unwrap().is_none(),
            "nothing was signalled"
        );
        sleeper.kill().unwrap();
        sleeper.wait().unwrap();
    }

    /// The orderly stop is SIGTERM, and a process that honours it is
    /// reported stopped. A process that ignores it stays running unless the
    /// caller allows a kill, and then it is SIGKILLed.
    ///
    /// Why it matters: uninstall must never kill (a kill can take the tmux
    /// server and its sessions), while the app may kill only its own wedged
    /// supervisor. The exit signal is checked, not just the outcome, because
    /// a kill would also make the process go.
    #[farhelm_testtrace::test]
    fn the_orderly_stop_is_sigterm_and_a_kill_needs_permission() {
        let dir = tempfile::tempdir().unwrap();
        let program = sleep_copy(dir.path(), "supervisor");
        let file = ProgramFile::read(&program).unwrap();

        let polite = stand_in(&program, false);
        let record = record_of(&polite, &file);
        let polite = reap(polite);
        assert_eq!(
            record.stop(Duration::from_secs(5), false),
            Ok(StopOutcome::Stopped)
        );
        assert_eq!(polite.join().unwrap().signal(), Some(libc::SIGTERM));

        let mut stubborn = stand_in(&program, true);
        let record = record_of(&stubborn, &file);
        assert_eq!(
            record.stop(Duration::from_millis(300), false),
            Ok(StopOutcome::StillRunning),
            "without permission to kill, a process ignoring SIGTERM keeps running"
        );
        assert!(stubborn.try_wait().unwrap().is_none());
        let stubborn = reap(stubborn);
        assert_eq!(
            record.stop(Duration::from_millis(300), true),
            Ok(StopOutcome::Killed)
        );
        assert_eq!(stubborn.join().unwrap().signal(), Some(libc::SIGKILL));
    }

    /// A stand-in whose main thread exits on its own while another thread
    /// keeps the process, and a lock it holds, alive until the test closes
    /// its stdin. It prints `ready` once it holds the lock, then exits its
    /// main thread when it reads a line.
    ///
    /// The main thread leaves through the raw `exit` system call (its number
    /// is the first argument), which ends only the calling thread; every
    /// ordinary way out of a program ends the whole process. Python, because
    /// it can make that call without a compiler, and its `ctypes` call
    /// releases the interpreter lock first, so the other thread can run on.
    #[cfg(target_os = "linux")]
    const MAIN_THREAD_EXITS_FIRST: &str = r#"
import ctypes, fcntl, os, sys, threading
lock = open(sys.argv[2], "w")
fcntl.flock(lock, fcntl.LOCK_EX)
main_left = threading.Event()
def hold():
    main_left.wait()
    sys.stdin.buffer.read()
    os._exit(0)
threading.Thread(target=hold).start()
print("ready", flush=True)
sys.stdin.buffer.readline()
main_left.set()
ctypes.CDLL(None).syscall(int(sys.argv[1]), 0)
"#;

    /// Whether `path` can be locked, which it cannot while a stand-in holds
    /// it. The probe's own lock goes when its file is dropped here.
    #[cfg(target_os = "linux")]
    fn lock_is_free(path: &Path) -> bool {
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .expect("fixture: opening the stand-in's lock file")
            .try_lock()
            .is_ok()
    }

    /// A [`MAIN_THREAD_EXITS_FIRST`] stand-in, its main thread already
    /// gone, while its other thread still holds its lock until `release`
    /// is dropped.
    #[cfg(target_os = "linux")]
    struct ExitingStandIn {
        _dir: tempfile::TempDir,
        child: Child,
        release: Option<std::process::ChildStdin>,
        record: ManagedSupervisor,
        lock: PathBuf,
    }

    #[cfg(target_os = "linux")]
    impl ExitingStandIn {
        /// Start the stand-in, record it while it still runs, make its main
        /// thread exit, and check the premise every test of it depends on:
        /// the record reads as gone while the lock is held and a thread of
        /// the process still runs.
        fn start() -> Self {
            use std::io::{BufRead as _, Write as _};
            let dir = tempfile::tempdir().unwrap();
            let lock = dir.path().join("held.lock");
            let python = Path::new("/usr/bin/python3");
            let file =
                ProgramFile::read(python).expect("fixture: this stand-in needs /usr/bin/python3");
            let mut child = Command::new(python)
                .arg("-c")
                .arg(MAIN_THREAD_EXITS_FIRST)
                .arg(libc::SYS_exit.to_string())
                .arg(&lock)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("fixture: spawning the stand-in");
            let mut ready = String::new();
            std::io::BufReader::new(child.stdout.take().unwrap())
                .read_line(&mut ready)
                .expect("fixture: reading the stand-in's readiness");
            assert_eq!(ready, "ready\n", "fixture: the stand-in must hold its lock");
            let record = record_of(&child, &file);
            assert_eq!(record.observe(), Ok(Observation::Matches));

            let mut release = child.stdin.take().unwrap();
            release
                .write_all(b"exit the main thread\n")
                .expect("fixture: telling the stand-in's main thread to exit");
            let deadline = Instant::now() + Duration::from_secs(10);
            while record.observe() != Ok(Observation::Gone) {
                assert!(
                    Instant::now() < deadline,
                    "fixture: the stand-in's main thread never exited"
                );
                // sleep-ok: polling interval for the main thread's exit.
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                !lock_is_free(&lock),
                "premise: a process that reads as gone still holds its lock"
            );
            assert_eq!(
                crate::procs::finished_exiting(record.pid, record.start),
                Ok(false),
                "premise: a thread of the process still runs"
            );
            Self {
                _dir: dir,
                child,
                release: Some(release),
                record,
                lock,
            }
        }

        /// Where the fixture stands, for a wait that timed out: whether
        /// the lock is still held and whether the stand-in has exited.
        fn state(&mut self) -> String {
            format!(
                "lock free: {}, stand-in exit: {:?}",
                lock_is_free(&self.lock),
                self.child.try_wait()
            )
        }

        /// Let the holding thread go, and reap the stand-in.
        fn finish(mut self) {
            drop(self.release.take());
            self.child.wait().expect("reaping the stand-in");
        }
    }

    /// A stop reports a process gone only once every thread of it has
    /// exited, not when its main thread reads as exited.
    ///
    /// Why it matters: on Linux a killed supervisor's main thread can read as
    /// a zombie while its other threads are still exiting, and until the last
    /// of them has, the process still holds the state directory's lock and
    /// the supervisor socket. The desktop app took the stop's word in that
    /// window and started the next supervisor, which found the directory
    /// taken, came up read-only and never served, so the app never opened
    /// (the Linux desktop smoke's frozen-supervisor leg, about two runs in
    /// eight). A kill's window is too short to hold open in a test, so the
    /// stand-in makes the same state last: its main thread exits on its own
    /// while another thread holds a lock until the test lets it go. Pinned,
    /// after checking that premise (the record reads as gone while the lock
    /// is held and a thread still runs): the stop reaches its check of an
    /// unfinished exit (the `on_exiting` hook) before it returns, with the
    /// lock still held there; the thread is let go only then, and the stop
    /// reports the process gone with the lock already free when it returns.
    /// Holding the thread until the hook fires is what makes a stop that
    /// believed the main thread fail on every schedule, rather than only
    /// when it happens to run before the thread is let go.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    fn a_stop_waits_for_every_thread_of_a_process_whose_main_thread_exited() {
        use std::sync::mpsc::RecvTimeoutError;
        let mut stand_in = ExitingStandIn::start();

        /// What the stopping thread reports, in the order it happens.
        #[derive(Debug)]
        enum Event {
            /// The stop found the process gone with a thread still exiting.
            Exiting,
            /// The stop returned, with whether the lock was free just then.
            Returned(Result<StopOutcome, String>, bool),
        }
        let (events_tx, events) = std::sync::mpsc::channel();
        let stopper = {
            let record = stand_in.record.clone();
            let lock = stand_in.lock.clone();
            std::thread::spawn(move || {
                let first_look = std::sync::Mutex::new(Some(events_tx.clone()));
                let outcome =
                    record.stop_observed(Duration::from_secs(1), true, AFTER_KILL, &|| {
                        if let Some(tx) = first_look.lock().unwrap().take() {
                            tx.send(Event::Exiting).unwrap();
                        }
                    });
                events_tx
                    .send(Event::Returned(outcome, lock_is_free(&lock)))
                    .unwrap();
            })
        };
        match events.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Exiting) => {}
            Ok(early) => {
                panic!(
                    "the stop ended with {early:?} without waiting for the thread that still ran"
                )
            }
            Err(RecvTimeoutError::Timeout) => panic!(
                "the stop never looked at the exiting process ({})",
                stand_in.state()
            ),
            Err(RecvTimeoutError::Disconnected) => panic!("the stop died: {:?}", stopper.join()),
        }
        assert!(
            !lock_is_free(&stand_in.lock),
            "premise: the stop looked while the thread still held the lock"
        );
        drop(stand_in.release.take());
        let returned = events.recv_timeout(Duration::from_secs(10));
        let Ok(Event::Returned(outcome, lock_was_free)) = returned else {
            panic!(
                "the stop must return once the last thread has exited, got {returned:?} ({})",
                stand_in.state()
            );
        };
        assert_eq!(outcome, Ok(StopOutcome::AlreadyGone));
        assert!(
            lock_was_free,
            "a process the stop reports gone must have let go of its lock"
        );
        stopper.join().unwrap();
        stand_in.finish();
    }

    /// A stop, and the launch's wait, refuse to call a process gone while a
    /// thread of it is still exiting at the end of their bound.
    ///
    /// Why it matters: this is the other half of waiting for every thread.
    /// A supervisor with a thread stuck exiting (a hung network filesystem,
    /// a long core dump) still holds the state directory's lock and its
    /// socket; reporting it gone at the deadline would make the app remove
    /// its record and start a successor beside it, which is the read-only
    /// start this wait exists to prevent. Pinned with the stand-in's thread
    /// held for the whole bound (shortened through the test path, rather
    /// than the real few seconds): the stop returns `StillExiting` with the
    /// lock still held when it returns, and the wait the launch uses
    /// (`wait_exited`'s) answers not finished; once the thread is let go,
    /// that wait answers finished.
    #[cfg(target_os = "linux")]
    #[farhelm_testtrace::test]
    fn a_stop_refuses_while_a_thread_is_still_exiting_at_its_bound() {
        let mut stand_in = ExitingStandIn::start();
        let bound = Duration::from_millis(300);
        let outcome = stand_in
            .record
            .stop_observed(Duration::from_secs(1), true, bound, &|| {});
        let lock_was_free = lock_is_free(&stand_in.lock);
        assert_eq!(
            outcome,
            Ok(StopOutcome::StillExiting),
            "{}",
            stand_in.state()
        );
        assert!(
            !lock_was_free,
            "premise: the thread held the lock for the whole bound"
        );
        assert_eq!(
            stand_in.record.wait_gone(bound, &|| {}),
            Ok(false),
            "the launch's wait must not call a process with a thread still exiting finished"
        );
        drop(stand_in.release.take());
        assert_eq!(
            stand_in.record.wait_gone(Duration::from_secs(10), &|| {}),
            Ok(true),
            "the wait must answer finished once the last thread has exited ({})",
            stand_in.state()
        );
        assert!(lock_is_free(&stand_in.lock));
        stand_in.finish();
    }

    /// Whether a supervisor was started from a program compares the record
    /// with the file at that path now: the same path with a replaced file is
    /// a different program.
    ///
    /// Why it matters: a development build replaces the program file at the
    /// same path, and that must count as a program the app would start
    /// differently from the running supervisor's.
    #[farhelm_testtrace::test]
    fn the_same_path_with_a_replaced_file_is_a_different_program() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("farhelm");
        std::fs::write(&program, b"first build").unwrap();
        let first = ProgramFile::read(&program).unwrap();
        let record = ManagedSupervisor {
            pid: 1,
            start: 0,
            program: first.path.clone(),
            program_dev: first.dev,
            program_ino: first.ino,
        };
        assert!(record.started_from(&first));
        // Replaced the way an atomic install does it: a new file renamed over.
        let staged = dir.path().join("farhelm.new");
        std::fs::write(&staged, b"second build").unwrap();
        std::fs::rename(&staged, &program).unwrap();
        assert!(!record.started_from(&ProgramFile::read(&program).unwrap()));
    }

    /// A running supervisor still matches its record after its program file
    /// was replaced at the same path.
    ///
    /// Why it matters: that is exactly the supervisor a development build
    /// needs to stop (every build renames a new file over the old one), so
    /// the replacement must not make it unrecognizable. Linux follows the
    /// running image (`/proc/<pid>/exe` keeps its identity); macOS compares
    /// only the path `proc_pidpath` reports for it (see
    /// [`ManagedSupervisor::observe`]), and this test is what shows on macOS
    /// CI that it still reports the recorded path for an unlinked image.
    #[farhelm_testtrace::test]
    fn a_running_supervisor_matches_after_its_program_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let program = sleep_copy(dir.path(), "supervisor");
        let file = ProgramFile::read(&program).unwrap();
        let mut sleeper = stand_in(&program, false);
        let record = record_of(&sleeper, &file);
        let staged = sleep_copy(dir.path(), "supervisor.new");
        std::fs::rename(&staged, &program).unwrap();
        assert_eq!(record.observe(), Ok(Observation::Matches));
        sleeper.kill().unwrap();
        sleeper.wait().unwrap();
    }

    /// An unparseable record reads as no record, and removing a missing
    /// record is fine.
    ///
    /// Why it matters: a damaged record must fail towards "not ours", never
    /// towards stopping something.
    #[farhelm_testtrace::test]
    fn a_damaged_or_missing_record_authorizes_nothing() {
        let state = tempfile::tempdir().unwrap();
        assert_eq!(ManagedSupervisor::read(state.path()).unwrap(), None);
        std::fs::write(state.path().join(RECORD_FILE), b"{not json").unwrap();
        assert_eq!(ManagedSupervisor::read(state.path()).unwrap(), None);
        ManagedSupervisor::remove(state.path()).unwrap();
        ManagedSupervisor::remove(state.path()).unwrap();
    }
}
