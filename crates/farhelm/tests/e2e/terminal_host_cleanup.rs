//! The e2e fixtures end the terminal hosts in their state directories.
//!
//! A terminal host outlives the supervisor that started it by design, so a
//! test whose supervisor starts hosts leaves them running unless its
//! fixture ends them, and once the fixture has removed the state directory
//! nothing can reach them any more. Every [`Harness`] and every
//! [`SupervisorProcess`] holds its state as a `farhelm_teststate::TestDir`,
//! whichever constructor built it, and that type's drop ends every host
//! with a socket in the directory before removing it (and keeps the
//! directory for the sweep when it cannot confirm that). These tests pin
//! that dropping each fixture really goes through it: the in-process
//! harness and the real-process supervisor.
//!
//! Nothing in a supervisor starts hosts yet, so each test starts one itself
//! in the fixture's state directory, with `farhelm-fixtures
//! spawn-terminal-host` (the supervisor's own spawn behind a command line,
//! which keeps the fork out of this test process), standing in for a
//! terminal the supervisor under test would have started. Each planted
//! host is also held by a [`HostGuard`] of the test's own, so that a
//! regression in the cleanup under test fails the test without leaving a
//! host running for good. The teststate cleanup's own cases (nested state,
//! a host that does not answer, a dead run's sweep) are in
//! `crates/farhelm/tests/terminal_host_spawn.rs`.

use crate::harness::*;
use std::path::{Path, PathBuf};

/// How long a fixture's teardown gets to make its host go: the teststate
/// cleanup's own deadline per host (5 s) and room for a loaded machine.
const HOST_GONE_SECS: u64 = 20;

/// How long starting a planted host may take before the test fails.
const PLANT_WAIT: Duration = Duration::from_secs(30);

/// How long the guard waits for a signalled host to go before it escalates
/// (SIGTERM to SIGKILL) or gives up.
const GUARD_WAIT: Duration = Duration::from_secs(5);

/// A real terminal host this test started in a fixture's state directory,
/// ended on drop if it is still running, whatever the cleanup under test
/// did.
///
/// The test's own fallback, independent of that cleanup: it signals the
/// pid only while it still has this host's command line, which names a
/// socket unique to the test, so a reused pid is never hit. SIGTERM (with
/// SIGCONT, should the host be stopped) ends a host at once; SIGKILL
/// follows only for one that is still there after [`GUARD_WAIT`].
struct HostGuard {
    pid: u32,
    socket: PathBuf,
}

impl HostGuard {
    /// Whether the host's pid is still a live process whose command line
    /// carries `--socket <its socket>`, as a host's own command line does.
    fn serves(&self) -> bool {
        if process_is_gone(self.pid) {
            return false;
        }
        let Ok(raw) = std::fs::read(format!("/proc/{}/cmdline", self.pid)) else {
            return false;
        };
        let args: Vec<&[u8]> = raw.split(|byte| *byte == 0).collect();
        let socket = self.socket.as_os_str().as_encoded_bytes();
        args.windows(2)
            .any(|pair| pair[0] == b"--socket" && pair[1] == socket)
    }

    /// Send `signals` while the host still serves, then wait (bounded) for
    /// it to stop serving; say whether it did.
    fn signal_and_wait(&self, signals: &[libc::c_int]) -> bool {
        for &signal in signals {
            if !self.serves() {
                return true;
            }
            // SAFETY: signalling the process this test started, checked by
            // its command line just above.
            unsafe { libc::kill(self.pid as libc::pid_t, signal) };
        }
        let deadline = std::time::Instant::now() + GUARD_WAIT;
        while self.serves() {
            if std::time::Instant::now() >= deadline {
                return false;
            }
            // sleep-ok: polling interval for a process this test owns.
            std::thread::sleep(Duration::from_millis(20));
        }
        true
    }
}

impl Drop for HostGuard {
    fn drop(&mut self) {
        if !self.serves() {
            return;
        }
        eprintln!(
            "fixture: the planted host {} still runs; ending it",
            self.pid
        );
        if !self.signal_and_wait(&[libc::SIGTERM, libc::SIGCONT])
            && !self.signal_and_wait(&[libc::SIGKILL])
        {
            eprintln!("fixture: the planted host {} survived SIGKILL", self.pid);
        }
    }
}

/// Start a host in `state_dir`'s socket directory, running a program that
/// waits to be hung up, take it into a [`HostGuard`], and check that it
/// runs as that socket's host before any test relies on it.
async fn plant_host(state_dir: &Path) -> HostGuard {
    let output = tokio::time::timeout(
        PLANT_WAIT,
        tokio::process::Command::new(fixtures_bin())
            .args([
                "spawn-terminal-host",
                "--farhelm",
                farhelm_bin(),
                "--state-dir",
            ])
            .arg(state_dir)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("the host started within the bound")
    .expect("running farhelm-fixtures spawn-terminal-host");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = stdout
        .trim_end()
        .split_once(' ')
        .and_then(|(pid, socket)| Some((pid.parse::<u32>().ok()?, PathBuf::from(socket))));
    // The guard exists as soon as the host's identity is known, before any
    // assertion that could unwind.
    let guard = parsed.map(|(pid, socket)| HostGuard { pid, socket });
    assert!(
        output.status.success(),
        "spawn-terminal-host failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let guard = guard.unwrap_or_else(|| panic!("spawn-terminal-host printed {stdout:?}"));
    assert!(
        guard.serves(),
        "premise: process {} serves {}",
        guard.pid,
        guard.socket.display()
    );
    guard
}

/// Dropping an in-process harness ends the hosts in its state directory and
/// then removes the directory.
///
/// Why: the in-process harness is what almost every supervisor test stands
/// on, and once supervisors start hosts, a harness that only killed its
/// tmux server and removed its directory would leave every host its test
/// started running for good, unreachable. Spec: with a host started in the
/// harness's state directory, dropping the harness makes the host's
/// process go within the cleanup's bound, and the state directory is gone.
#[farhelm_testtrace::test]
async fn dropping_the_harness_ends_its_state_directorys_hosts() {
    let h = harness().await;
    let state = h.state.path().to_path_buf();
    let host = plant_host(&state).await;

    drop(h);
    wait_until_pid_gone(host.pid, HOST_GONE_SECS).await;
    assert!(!state.exists(), "the state directory was kept");
}

/// Dropping a real supervisor process's fixture ends the hosts in its
/// state directory and then removes the directory.
///
/// Why: real-process supervisor tests are the ones that will run
/// `farhelm supervisor run` with hosts on, and the process fixture kills
/// its supervisor without asking it to end anything, so its state
/// directory's drop is the only thing between those hosts and a leak.
/// Spec: with a host started in the running supervisor's state directory,
/// dropping the fixture makes the host's process go within the cleanup's
/// bound, and the state directory is gone.
#[farhelm_testtrace::test]
async fn dropping_a_supervisor_process_ends_its_state_directorys_hosts() {
    let supervisor = supervisor_process().await;
    let state = supervisor.state.path().to_path_buf();
    let host = plant_host(&state).await;

    drop(supervisor);
    wait_until_pid_gone(host.pid, HOST_GONE_SECS).await;
    assert!(!state.exists(), "the state directory was kept");
}
