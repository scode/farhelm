//! Process-level proof that a new session's window holds its version of
//! Farhelm through the login shell's startup, with the real `farhelm`
//! binary in a fake Mac app layout (see `farhelm_supervisor::session_hold`).
//!
//! The hold step's whole point is a hand-off between processes that replace
//! one another in place: the hold step becomes the shell, which becomes the
//! launch shim, while a detached keeper keeps the version locked in between.
//! None of that can be observed in one process, so each test runs the real
//! hold step (or launch shim) as a child, with a small `sh` script standing
//! in for the user's login shell, and watches the version's lock from
//! outside the way an install would: with an exclusive no-wait attempt.
//!
//! Every test owns its children through [`ChildGuard`] and bounds every
//! read and wait, so a hung program fails the test instead of hanging it.

mod cli_support;

use std::fs::{File, TryLockError};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Generous bound for anything a test waits on; the programs here answer
/// in well under a second.
const DEADLINE: Duration = Duration::from_secs(30);

/// A session id for one test's launch files. Unique per test because the
/// tests run concurrently and one of them finds the keeper by its session
/// marker; a shared id would find every test's keeper.
fn session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// A fake app bundle holding one version, 1.2.3, whose `farhelm` is a copy
/// of the binary under test, plus a launch directory beside it. Returns the
/// version's program and the path a launch spec for `session` would have
/// (the hold step never reads the spec, so it is not written).
fn fake_layout(root: &Path, session: &str) -> (PathBuf, PathBuf) {
    let version = root.join("Farhelm.app/Contents/Versions/1.2.3");
    std::fs::create_dir_all(&version).expect("fixture: version folder");
    std::fs::create_dir_all(root.join("Farhelm.app/Contents/MacOS")).expect("fixture: MacOS");
    let program = version.join("farhelm");
    std::fs::copy(env!("CARGO_BIN_EXE_farhelm"), &program).expect("fixture: the version's farhelm");
    let launch = root.join("state/launch");
    std::fs::create_dir_all(&launch).expect("fixture: launch dir");
    (program, launch.join(format!("{session}.0.json")))
}

/// Whether an exclusive no-wait attempt on `program` (what pruning and
/// uninstall do) fails because someone holds the version. A successful
/// attempt is released at once.
fn version_is_held(program: &Path) -> bool {
    let probe = File::open(program).expect("open the version's program");
    matches!(probe.try_lock(), Err(TryLockError::WouldBlock))
}

/// Poll until `program`'s version is no longer held, failing at the
/// deadline with `what`.
fn wait_until_released(program: &Path, deadline: Instant, what: &str) {
    while version_is_held(program) {
        assert!(
            Instant::now() < deadline,
            "{what}: still held at the deadline"
        );
        // sleep-ok: polling interval while waiting for the keeper to exit, bounded by the deadline above.
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The hold step's command for `program` and `spec`, running `script` under
/// `sh` in place of the login shell, with `args` as its positional
/// parameters. Stray version-lock and session variables from the test's own
/// environment are removed, so the child sees only what the hold step sets.
fn hold_command(program: &Path, spec: &Path, script: &str, args: &[&Path]) -> Command {
    let mut command = Command::new(program);
    command
        .args(["internal", "hold"])
        .arg(spec)
        .args(["--", "/bin/sh", "-c", script, "sh"])
        .args(args)
        .env_remove("FARHELM_VERSION_LOCK_FD")
        .env_remove("FARHELM_SESSION_ID")
        .env_remove("FARHELM_AGENT_ID");
    command
}

/// A child process the test owns: killed and reaped when dropped, so no
/// failure path leaves it running.
struct ChildGuard(Child);

impl ChildGuard {
    /// Wait for the child to exit, at most until `deadline`.
    fn wait_until(&mut self, deadline: Instant, what: &str) -> ExitStatus {
        loop {
            if let Some(status) = self.0.try_wait().expect("poll the child") {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "{what}: still running at the deadline"
            );
            // sleep-ok: polling interval while waiting for a child's exit, bounded by the deadline above.
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn stdin(&mut self) -> &mut ChildStdin {
        self.0.stdin.as_mut().expect("the child's stdin is piped")
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Read one line from `reader` on a helper thread, at most 4 KiB, waiting
/// at most until `deadline`. `None` means end of input before a line.
fn read_line_by(
    reader: impl Read + Send + 'static,
    deadline: Instant,
    what: &str,
) -> Option<String> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let read = BufReader::new(reader.take(4096)).read_line(&mut line);
        let _ = send.send(read.map(|n| (n, line)));
    });
    let timeout = deadline.saturating_duration_since(Instant::now());
    match receive.recv_timeout(timeout) {
        Ok(Ok((0, _))) => None,
        Ok(Ok((_, line))) => Some(line),
        Ok(Err(error)) => panic!("{what}: reading failed: {error}"),
        Err(_) => panic!("{what}: no line before the deadline"),
    }
}

/// The pid a mark file holds, if it exists and holds one.
fn mark_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// The whole hand-off, in the order the design requires. While the shell's
/// startup runs, the version is held although the shell itself holds no
/// lock and was handed no lock variable, so only the keeper can be holding
/// it; the "held" mark names the pane's process. Once the shell (standing
/// in for the launch shim) marks "launched" with its own pid, the keeper
/// lets go while the shell is still running, which proves both that the
/// keeper was the holder and that it reacts to the mark. A keeper that
/// never started, a hold step that leaked its lock into the shell, or a
/// keeper that ignored "launched" each fails one of the checks.
#[farhelm_testtrace::test]
fn the_keeper_holds_the_version_through_shell_startup_until_launched() {
    let root = farhelm_teststate::tempdir().expect("isolated root");
    let (program, spec) = fake_layout(root.path(), &session_id());
    let launched = spec.with_extension("launched");
    assert!(
        !version_is_held(&program),
        "control: nothing holds the version yet"
    );

    let script = "echo \"shell $$ ${FARHELM_VERSION_LOCK_FD-unset}\"; \
                  read go; echo $$ > \"$1\"; echo marked; read go; exit 0";
    let mut child = ChildGuard(
        hold_command(&program, &spec, script, &[&launched])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start the hold step"),
    );
    let deadline = Instant::now() + DEADLINE;
    let stdout = child.0.stdout.take().expect("piped stdout");
    let (lines_send, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if lines_send.send(line).is_err() {
                return;
            }
        }
    });
    let next_line = |what: &str| -> String {
        lines
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("{what}: no line before the deadline"))
            .expect("read the shell's output")
    };

    let started = next_line("the shell starting");
    assert_eq!(
        started,
        format!("shell {} unset", child.0.id()),
        "the shell runs in the hold step's own process, without the lock variable"
    );
    assert_eq!(
        mark_pid(&spec.with_extension("held")),
        Some(child.0.id()),
        "the held mark names the pane's process"
    );
    assert!(
        version_is_held(&program),
        "the version stays held while the shell's startup runs"
    );

    child
        .stdin()
        .write_all(b"go\n")
        .expect("let the shell mark launched");
    assert_eq!(next_line("the shell marking launched"), "marked");
    wait_until_released(
        &program,
        deadline,
        "after launched, with the shell still running",
    );

    child
        .stdin()
        .write_all(b"go\n")
        .expect("let the shell exit");
    assert!(child.wait_until(deadline, "the shell").success());
}

/// A shell that exits before anything marks "launched" (its startup files
/// failed, or the window was closed) releases the version: the keeper
/// watches the pane's process and goes away with it, rather than holding the
/// version until something else happens to kill it.
#[farhelm_testtrace::test]
fn the_keeper_lets_go_when_the_shell_exits_first() {
    let root = farhelm_teststate::tempdir().expect("isolated root");
    let (program, spec) = fake_layout(root.path(), &session_id());
    let mut child = ChildGuard(
        hold_command(&program, &spec, "echo started; read go; exit 3", &[])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start the hold step"),
    );
    let deadline = Instant::now() + DEADLINE;
    let stdout = child.0.stdout.take().expect("piped stdout");
    assert_eq!(
        read_line_by(stdout, deadline, "the shell starting").as_deref(),
        Some("started\n")
    );
    assert!(version_is_held(&program), "held while the shell runs");
    child
        .stdin()
        .write_all(b"go\n")
        .expect("let the shell exit");
    assert_eq!(child.wait_until(deadline, "the shell").code(), Some(3));
    wait_until_released(&program, deadline, "after the shell exited");
}

/// A process's parent pid and session id, from `/proc/<pid>/stat` (the
/// fields after the parenthesized command name are state, ppid, pgrp,
/// session).
#[cfg(target_os = "linux")]
fn parent_and_session(pid: u32) -> (u32, u32) {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).expect("read stat");
    let fields: Vec<&str> = stat
        .rsplit_once(')')
        .expect("stat has a command name")
        .1
        .split_whitespace()
        .collect();
    (
        fields[1].parse().expect("ppid"),
        fields[3].parse().expect("session"),
    )
}

/// The keeper is found by the session's stop sweep and survives the
/// terminal: it carries the session's markers in its own environment (the
/// shell does not), and it runs in a session of its own rather than as a
/// child of the pane's process. Linux only, because it reads `/proc`; the
/// detaching code is the same on every platform.
#[cfg(target_os = "linux")]
#[farhelm_testtrace::test]
fn the_keeper_is_detached_and_carries_the_session_markers() {
    let root = farhelm_teststate::tempdir().expect("isolated root");
    let session = session_id();
    let (program, spec) = fake_layout(root.path(), &session);
    let script = "echo \"${FARHELM_AGENT_ID-unset}\"; read go; exit 0";
    let mut child = ChildGuard(
        hold_command(&program, &spec, script, &[])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start the hold step"),
    );
    let deadline = Instant::now() + DEADLINE;
    let stdout = child.0.stdout.take().expect("piped stdout");
    assert_eq!(
        read_line_by(stdout, deadline, "the shell starting").as_deref(),
        Some("unset\n"),
        "the shell does not get the session markers"
    );

    let marker = format!("FARHELM_AGENT_ID={session}");
    let keepers: Vec<u32> = std::fs::read_dir("/proc")
        .expect("list /proc")
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            std::fs::read(format!("/proc/{pid}/environ"))
                .is_ok_and(|env| env.split(|b| *b == 0).any(|var| var == marker.as_bytes()))
        })
        .collect();
    assert_eq!(
        keepers.len(),
        1,
        "exactly one process carries the marker: {keepers:?}"
    );
    let keeper = keepers[0];
    let (keeper_parent, keeper_session) = parent_and_session(keeper);
    let (_, pane_session) = parent_and_session(child.0.id());
    assert_ne!(
        keeper_parent,
        child.0.id(),
        "the keeper is not a child of the pane's process"
    );
    // The double fork's point: a session other than the pane's (so the
    // terminal's signals do not reach it), whose leader is the
    // intermediate process that exited, so the keeper can never acquire a
    // controlling terminal.
    assert_ne!(
        keeper_session, pane_session,
        "the keeper left the pane's session"
    );
    assert_ne!(
        keeper_session, keeper,
        "the keeper does not lead its session"
    );

    child
        .stdin()
        .write_all(b"go\n")
        .expect("let the shell exit");
    assert!(child.wait_until(deadline, "the shell").success());
    wait_until_released(&program, deadline, "after the shell exited");
}

/// A window whose version an install holds exclusively cannot start
/// unprotected: the hold step refuses after the shared wait, records why in
/// the launch's status file (so the session shows an error, not an ordinary
/// exit), and never runs the shell.
#[farhelm_testtrace::test]
fn a_hold_step_that_cannot_hold_its_version_records_a_failed_start() {
    let root = farhelm_teststate::tempdir().expect("isolated root");
    let (program, spec) = fake_layout(root.path(), &session_id());
    let installer = File::open(&program).expect("open");
    installer
        .try_lock()
        .expect("the stand-in install holds the version exclusively");
    let marker = root.path().join("shell-ran");
    let output =
        cli_support::output_with_timeout(hold_command(&program, &spec, "touch \"$1\"", &[&marker]));
    assert!(!output.status.success(), "the hold step refuses");
    assert!(!marker.exists(), "the shell never ran");
    let status = std::fs::read_to_string(spec.with_extension("status"))
        .expect("the failure is recorded in the launch's status file");
    assert!(status.contains("being updated or uninstalled"), "{status}");
    assert!(
        !spec.with_extension("held").exists(),
        "nothing claims to hold it"
    );
}

/// The launch shim's half of the hand-off: once it holds its own lock, it
/// marks "launched" with its pid before it does anything else, so even a
/// launch that then fails (here, its spec is missing) lets the keeper go.
/// The shim and the shell share a pid in a real window, which is why the
/// pid is the one the keeper waits for.
#[farhelm_testtrace::test]
fn the_launch_shim_marks_launched_with_its_own_pid() {
    let root = farhelm_teststate::tempdir().expect("isolated root");
    let (program, spec) = fake_layout(root.path(), &session_id());
    let mut command = Command::new(&program);
    command
        .args(["internal", "launch"])
        .arg(&spec)
        .env_remove("FARHELM_VERSION_LOCK_FD")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = ChildGuard(command.spawn().expect("start the launch shim"));
    let pid = child.0.id();
    let mut child = child;
    assert!(
        !child
            .wait_until(Instant::now() + DEADLINE, "the shim")
            .success(),
        "a missing spec fails the launch"
    );
    assert_eq!(mark_pid(&spec.with_extension("launched")), Some(pid));
}

/// Outside the version layout the hold step has nothing to hold: it runs
/// the window's command as it is and writes no mark. A supervisor never
/// starts it there, but a window it did start must still reach its shell.
#[farhelm_testtrace::test]
fn outside_the_version_layout_the_hold_step_just_runs_the_shell() {
    let root = farhelm_teststate::tempdir().expect("isolated root");
    let spec = root.path().join(format!("{}.0.json", session_id()));
    let output = cli_support::output_with_timeout(hold_command(
        Path::new(env!("CARGO_BIN_EXE_farhelm")),
        &spec,
        "echo ran",
        &[],
    ));
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ran\n");
    assert!(!spec.with_extension("held").exists());
}
