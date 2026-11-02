//! Fixtures shared by the tests that start real terminal hosts from the
//! built `farhelm`: the program, a child that just sleeps, a bounded poll,
//! and a host started with a raw child spec.
//!
//! Only `terminal_host_registry.rs` uses this so far. The older
//! terminal-host test binaries (`terminal_host_spawn.rs`,
//! `terminal_host_client.rs`, `terminal_host_entry.rs`) still carry their
//! own copies of these helpers; they move here when those files are next
//! changed, rather than in a change that other work in flight also edits.

use farhelm_supervisor::hosts::ChildSpec;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use terminal_host::protocol::{Spawn, encode_spawn};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// How long any single wait in these tests may take before it fails.
pub const WAIT: Duration = Duration::from_secs(10);

/// The `farhelm` binary under test.
pub fn farhelm() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_farhelm"))
}

/// A child that sleeps for ten minutes at 80x24, long enough to outlive any
/// test: the host it runs in ends with the test's state directory.
pub fn sleeper() -> ChildSpec {
    ChildSpec {
        argv: vec!["/bin/sleep".into(), "600".into()],
        cwd: "/".into(),
        env: vec![("PATH".into(), "/usr/bin:/bin".into())],
        size: terminal_host::protocol::Size { cols: 80, rows: 24 },
        journal_cap: 1024 * 1024,
        initial_snapshot: None,
    }
}

/// Whether `condition` holds within [`WAIT`], asking every 20 ms.
///
/// It blocks the calling thread between asks, so an async test must not
/// call it while it needs its runtime to drive something else (a
/// connection it holds, a stand-in it serves).
pub fn eventually(mut condition: impl FnMut() -> bool) -> bool {
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

/// Start a host at `socket` from `farhelm internal terminal-host` with the
/// raw child spec `spawn`, bypassing the supervisor's spawn: for hosts the
/// supervisor would never start, such as one whose tag it cannot encode.
///
/// Bounded like the supervisor's own spawn: the spec is written, stderr
/// read and the first stage's exit awaited together, within [`WAIT`]. On a
/// timeout the first stage is killed and reaped before the test fails. A
/// second stage that had already daemonized is then left at `socket`, which
/// must lie inside a `farhelm_teststate` state directory the test still
/// owns: that directory's drop ends every host in it.
pub async fn start_raw_host(socket: &Path, spawn: &Spawn) {
    let mut first = tokio::process::Command::new(farhelm())
        .args(["internal", "terminal-host", "--socket"])
        .arg(socket)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("running farhelm internal terminal-host");
    let mut stdin = first.stdin.take().expect("stdin is piped");
    let mut stderr = first.stderr.take().expect("stderr is piped");
    let spec = encode_spawn(spawn);
    let started = tokio::time::timeout(WAIT, async {
        // A write error means the first stage stopped reading; its exit
        // status says why.
        let _ = stdin.write_all(&spec).await;
        drop(stdin);
        let mut message = Vec::new();
        // The first stage's stderr closes when it exits, unless a
        // daemonized second stage kept it; the outer timeout bounds that.
        let _ = (&mut stderr).take(4096).read_to_end(&mut message).await;
        let status = first.wait().await.expect("waiting for the first stage");
        (status, message)
    })
    .await;
    match started {
        Ok((status, message)) => assert!(
            status.success(),
            "the host did not start ({status}): {}",
            String::from_utf8_lossy(&message)
        ),
        Err(_) => {
            let _ = first.start_kill();
            let _ = tokio::time::timeout(WAIT, first.wait()).await;
            panic!(
                "the host at {} did not report readiness within {WAIT:?}",
                socket.display()
            );
        }
    }
}
