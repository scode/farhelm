//! Process-level proof of version locks with the real `farhelm` binary (see
//! `farhelm_supervisor::version_lock` and the forwarder text in
//! `scripts/install.sh`).
//!
//! The version-lock module's unit tests cover the lock itself, in process,
//! and the shell harness (`scripts/test-install-sh.sh`) covers the
//! forwarder's protocol with stand-in programs, including that it keeps
//! descriptor 8 open across its exec. What only a real process can show is
//! here: that the real helper's claim survives the helper process exiting,
//! that a real program refuses to run a version it cannot hold (silently,
//! for an agent hook), and that the whole chain through the shipped
//! forwarder text holds a version exactly as long as the program it started.
//!
//! Every test owns its children through [`ChildGuard`], which kills and
//! reaps them on any exit from the test, and bounds every read and wait, so
//! a hung program fails the test with a diagnosis instead of hanging it.

use std::fs::{File, TryLockError};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Generous bound for anything a test waits on: the programs here answer in
/// well under a second, except where a test makes one wait out the version
/// lock's five-second shared wait.
const DEADLINE: Duration = Duration::from_secs(30);

/// The forwarder script exactly as `scripts/install.sh` writes it, taken
/// from the installer's heredoc so this test exercises the shipped text.
fn forwarder_text() -> String {
    let installer = include_str!("../../../scripts/install.sh");
    let start_marker = "<<'FORWARDER_EOF'\n";
    let start = installer
        .find(start_marker)
        .expect("the installer embeds the forwarder")
        + start_marker.len();
    let end = start
        + installer[start..]
            .find("\nFORWARDER_EOF\n")
            .expect("the forwarder heredoc ends");
    format!("{}\n", &installer[start..end])
}

/// A fake app bundle holding the forwarder and one version, 1.2.3, whose
/// `farhelm` is a copy of the binary under test. Returns the forwarder and
/// the version's program path.
fn fake_bundle(root: &Path) -> (PathBuf, PathBuf) {
    let contents = root.join("Farhelm.app/Contents");
    let macos = contents.join("MacOS");
    let version = contents.join("Versions/1.2.3");
    std::fs::create_dir_all(&macos).expect("fixture: MacOS");
    std::fs::create_dir_all(&version).expect("fixture: version folder");
    let forwarder = macos.join("farhelm");
    std::fs::write(&forwarder, forwarder_text()).expect("fixture: forwarder");
    std::fs::set_permissions(&forwarder, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let program = version.join("farhelm");
    std::fs::copy(env!("CARGO_BIN_EXE_farhelm"), &program).expect("fixture: the version's farhelm");
    std::fs::write(contents.join("Versions/installed"), "1.2.3\n")
        .expect("fixture: Installed record");
    (forwarder, program)
}

/// Whether an exclusive no-wait attempt on `program` (what pruning and
/// uninstall do) fails because someone holds the version. A successful
/// attempt is released at once.
fn version_is_held(program: &Path) -> bool {
    let probe = File::open(program).expect("open the version's program");
    matches!(probe.try_lock(), Err(TryLockError::WouldBlock))
}

/// A child process the test owns: killed and reaped when dropped, so no
/// failure path leaves it running.
struct ChildGuard(Child);

impl ChildGuard {
    /// Wait for the child to exit, at most until `deadline`. Panics with
    /// whether it was still alive if it did not.
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

/// The real helper's claim outlives the helper process. The test opens the
/// version's program, hands the helper a copy of that descriptor (as the
/// forwarder does with descriptor 8), and lets the helper exit; the lock the
/// helper took must still keep a deleter out while the test's descriptor is
/// open, and go away when it closes. A helper that unlocked on its way out,
/// or locked a file of its own instead of the inherited one, fails the
/// middle check. The first check is the control: the same observation sees
/// an unheld version as unheld.
#[farhelm_testtrace::test]
fn the_real_helpers_claim_outlives_the_helper_process() {
    let root = farhelm_teststate::tempdir().expect("isolated root");
    let (_forwarder, program) = fake_bundle(root.path());
    assert!(
        !version_is_held(&program),
        "control: nothing holds the version yet"
    );

    let claim = File::open(&program).expect("the forwarder's descriptor, opened by the test");
    let fd = claim.as_raw_fd();
    let mut command = Command::new(&program);
    command
        .args(["internal", "version-lock", "3"])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: only async-signal-safe calls between fork and exec: dup2 to
    // place the claim's descriptor at 3 without close-on-exec, which is how
    // the forwarder hands descriptor 8 to the helper.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(fd, 3) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut helper = ChildGuard(command.spawn().expect("run the helper"));
    let status = helper.wait_until(Instant::now() + DEADLINE, "the helper");
    assert_eq!(
        status.code(),
        Some(0),
        "the helper reports the version held"
    );

    assert!(
        version_is_held(&program),
        "the claim stays after the helper exits, held through the test's descriptor"
    );
    drop(claim);
    assert!(
        !version_is_held(&program),
        "the claim goes away with the last descriptor"
    );
}

/// A version an update or uninstall holds exclusively is never run
/// unprotected. An agent hook gives up silently with status 0 after the
/// bounded wait (its streams are the agent's terminal) and does not serve;
/// any other command refuses with one line saying what is happening.
#[farhelm_testtrace::test]
fn a_program_whose_version_is_held_exclusively_refuses_to_run() {
    let root = farhelm_teststate::tempdir().expect("isolated root");
    let (_forwarder, program) = fake_bundle(root.path());
    let deleter = File::open(&program).expect("open");
    deleter
        .try_lock()
        .expect("the test holds the version exclusively");

    let mut hook = ChildGuard(
        Command::new(&program)
            .args(["internal", "goose-hook"])
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start the hook"),
    );
    let mut stdin = hook.0.stdin.take().expect("stdin");
    // Written before the hook gives up; a hook that served anyway would
    // answer it.
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
        .expect("send a ping");
    let stdout = hook.0.stdout.take().expect("stdout");
    let stderr = hook.0.stderr.take().expect("stderr");
    let deadline = Instant::now() + DEADLINE;
    assert_eq!(
        read_line_by(stdout, deadline, "the hook's stdout"),
        None,
        "the hook never serves a version it cannot hold"
    );
    let status = hook.wait_until(deadline, "the hook");
    assert_eq!(status.code(), Some(0), "a hook gives up with status 0");
    assert_eq!(
        read_line_by(stderr, deadline, "the hook's stderr"),
        None,
        "and silently"
    );
    drop(stdin);

    let mut other = ChildGuard(
        Command::new(&program)
            .args(["internal", "stdio", "--state-dir"])
            .arg(root.path().join("no-state"))
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start another command"),
    );
    let stderr = other.0.stderr.take().expect("stderr");
    let message = read_line_by(stderr, deadline, "the command's stderr").unwrap_or_default();
    let status = other.wait_until(deadline, "the command");
    assert!(!status.success(), "any other command refuses: {status}");
    assert!(
        message.contains("is being updated or uninstalled"),
        "with a one-line reason: {message}"
    );
}

/// The whole chain through the shipped forwarder text, as a smoke test of
/// the parts the other tests prove separately: a program started through the
/// forwarder holds its version while it runs, and nothing holds it after it
/// exits (so nothing leaked the lock to outlive it).
#[farhelm_testtrace::test]
fn a_program_started_through_the_forwarder_holds_its_version_while_it_runs() {
    let root = farhelm_teststate::tempdir().expect("isolated root");
    let (forwarder, program) = fake_bundle(root.path());
    assert!(
        !version_is_held(&program),
        "control: nothing holds the version yet"
    );

    let mut child = ChildGuard(
        Command::new("/bin/sh")
            .arg(&forwarder)
            .args(["internal", "goose-hook"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start the forwarder"),
    );
    let mut stdin = child.0.stdin.take().expect("stdin");
    let stdout = child.0.stdout.take().expect("stdout");
    let deadline = Instant::now() + DEADLINE;

    // Readiness oracle: an answer to a ping can only come from the farhelm
    // the forwarder exec'd (the claim's helper runs with its streams on
    // /dev/null), so once it arrives the started program is the one running.
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
        .expect("send a ping");
    stdin.flush().expect("flush");
    let answer = read_line_by(stdout, deadline, "the program's answer").unwrap_or_default();
    assert!(
        answer.contains("\"id\":1"),
        "the started program answered: {answer}"
    );

    assert!(
        version_is_held(&program),
        "the running program holds its version"
    );

    drop(stdin);
    let status = child.wait_until(deadline, "the program");
    assert!(
        status.success(),
        "the program exits cleanly at end of input: {status}"
    );
    assert!(
        !version_is_held(&program),
        "nothing holds the version after it exits"
    );
}
