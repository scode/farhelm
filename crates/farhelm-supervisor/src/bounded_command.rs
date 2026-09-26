//! Running a helper program asynchronously with a deadline, bounded output,
//! and a kill that reaches everything the program started.
//!
//! The supervisor shells out to small helpers (`systemctl --user`, `git`)
//! whose output it parses and whose runtime it cannot predict: a wedged user
//! manager or a misconfigured Git can hang, print without end, or fork a
//! child that keeps the pipes open after the parent exits. [`GroupChild`]
//! holds the three limits every such call needs. The program runs in its own
//! process group, so one signal kills whatever it forked. Each output stream
//! is read up to a byte cap, and one byte past it counts as a refusal rather
//! than a truncation. And the whole run, reading included, ends at a
//! deadline.
//!
//! The synchronous tmux probe (`tmux::probe_tmux`) keeps its own runner on
//! purpose: `farhelm helm setup` calls it with no async runtime available.

use std::io;
use std::process::{ExitStatus, Output, Stdio};
use tokio::io::{AsyncRead, AsyncReadExt as _};
use tokio::process::{Child, Command};
use tokio::time::Instant;

/// How much of each output stream a run keeps.
#[derive(Debug, Clone, Copy)]
pub struct OutputCaps {
    /// The most stdout bytes accepted. More is [`Ended::OutputOverCap`].
    pub stdout: usize,
    /// The most stderr bytes accepted, or `None` to discard stderr entirely
    /// (it is then connected to the null device, never read).
    pub stderr: Option<usize>,
}

/// How a bounded run ended.
#[derive(Debug)]
pub enum Ended {
    /// The program exited on its own, within the deadline and the caps. Its
    /// stderr is empty when [`OutputCaps::stderr`] was `None`.
    Exited(Output),
    /// The deadline passed first. The group was killed and reaped.
    TimedOut,
    /// A stream exceeded its cap. The group was killed and reaped.
    OutputOverCap,
}

/// A spawned program leading its own process group.
///
/// Dropping one that has not been reaped kills the whole group, so a
/// cancelled caller never leaves the program or its descendants running.
/// The kill is a signal, which `Drop` can send without awaiting; reaping the
/// leader is then left to tokio (`kill_on_drop`), and the caller that needs
/// to know the child is gone must reap it through [`GroupChild::kill_and_reap`]
/// or [`GroupChild::finish`] instead of dropping it.
#[derive(Debug)]
pub struct GroupChild {
    child: Child,
    /// The group id, which `process_group(0)` makes equal to the leader's
    /// pid. `None` only if the OS reported no pid for a live spawn.
    group: Option<i32>,
    reaped: bool,
    stdout_open: bool,
}

impl GroupChild {
    /// Spawn `command` as the leader of a new process group, with stdin
    /// closed and stdout piped. Stderr is piped only when `caps.stderr` asks
    /// for it; otherwise it goes to the null device.
    pub fn spawn(command: &mut Command, caps: OutputCaps) -> io::Result<Self> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(if caps.stderr.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .process_group(0)
            .kill_on_drop(true);
        let child = command.spawn()?;
        let group = child.id().and_then(|pid| i32::try_from(pid).ok());
        Ok(GroupChild {
            child,
            group,
            reaped: false,
            stdout_open: true,
        })
    }

    /// Whether the leader has been waited for, so its exit is known.
    pub fn is_reaped(&self) -> bool {
        self.reaped
    }

    /// Read both streams to their end and wait for the leader, all before
    /// `deadline` and within `caps`; on timeout or overflow, kill the group
    /// and reap it.
    ///
    /// Callable once. An `Err` is an I/O failure reading or waiting, after
    /// which the group has been signalled but may not be reaped: check
    /// [`GroupChild::is_reaped`] before treating the program as gone.
    pub async fn finish(&mut self, deadline: Instant, caps: OutputCaps) -> io::Result<Ended> {
        assert!(self.stdout_open, "GroupChild::finish called twice");
        self.stdout_open = false;
        let stdout = self.child.stdout.take().expect("stdout is piped at spawn");
        let stderr = self.child.stderr.take();
        // `try_join!` so the first stream past its cap ends BOTH reads at
        // once: a program that floods stdout while holding stderr open must
        // be refused now, not left running until the deadline.
        let read = async {
            tokio::try_join!(read_capped(stdout, caps.stdout), async {
                match (stderr, caps.stderr) {
                    (Some(stderr), Some(cap)) => read_capped(stderr, cap).await,
                    _ => Ok(Vec::new()),
                }
            })
        };
        let (stdout, stderr) = match tokio::time::timeout_at(deadline, read).await {
            Err(_) => {
                self.kill_and_reap().await?;
                return Ok(Ended::TimedOut);
            }
            Ok(Err(error)) if is_over_cap(&error) => {
                self.kill_and_reap().await?;
                return Ok(Ended::OutputOverCap);
            }
            Ok(Err(error)) => {
                self.kill_group();
                return Err(error);
            }
            Ok(Ok(streams)) => streams,
        };
        let status = match tokio::time::timeout_at(deadline, self.child.wait()).await {
            Err(_) => {
                self.kill_and_reap().await?;
                return Ok(Ended::TimedOut);
            }
            Ok(status) => status?,
        };
        self.reaped = true;
        Ok(Ended::Exited(Output {
            status,
            stdout,
            stderr,
        }))
    }

    /// Kill the whole group and wait for the leader.
    pub async fn kill_and_reap(&mut self) -> io::Result<ExitStatus> {
        self.kill_group();
        let status = self.child.wait().await?;
        self.reaped = true;
        Ok(status)
    }

    /// Send SIGKILL to the group, and to the leader directly in case it has
    /// already left the group. Harmless if they are gone.
    pub fn kill_group(&mut self) {
        if let Some(group) = self.group {
            // SAFETY: `kill` with a negative pid signals a process group and
            // touches no memory. A group that is already gone answers ESRCH,
            // which is the outcome this wants anyway.
            unsafe { libc::kill(-group, libc::SIGKILL) };
        }
        let _ = self.child.start_kill();
    }
}

impl Drop for GroupChild {
    fn drop(&mut self) {
        if !self.reaped {
            self.kill_group();
        }
    }
}

/// Spawn `command` and [`GroupChild::finish`] it: the one-call form for a
/// caller with nothing to hold across the run.
pub async fn run(command: &mut Command, deadline: Instant, caps: OutputCaps) -> io::Result<Ended> {
    GroupChild::spawn(command, caps)?
        .finish(deadline, caps)
        .await
}

/// The marker [`read_capped`] fails with when a stream passes its cap.
///
/// An error rather than a value so `try_join!` stops the other stream's read
/// the moment one overflows; [`is_over_cap`] tells it apart from a real I/O
/// failure.
#[derive(Debug)]
struct OverCap;

impl std::fmt::Display for OverCap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("output exceeded its cap")
    }
}

impl std::error::Error for OverCap {}

fn is_over_cap(error: &io::Error) -> bool {
    error.get_ref().is_some_and(|inner| inner.is::<OverCap>())
}

/// Read `stream` to its end, failing with [`OverCap`] once it yields more
/// than `cap` bytes. Reads at most one byte past the cap, so a flood is
/// classified without being buffered.
async fn read_capped(mut stream: impl AsyncRead + Unpin, cap: usize) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    // On the heap, not an array: the buffer is held across an await, so an
    // array would live inside this future, and inside every future that
    // awaits a run. Two concurrent reads made that 8 KiB in each caller's
    // state, which overflowed a test thread's stack in a debug build of the
    // deeply nested teardown path.
    let mut chunk = vec![0_u8; 4096];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(output);
        }
        let room = cap + 1 - output.len();
        output.extend_from_slice(&chunk[..read.min(room)]);
        if output.len() > cap {
            return Err(io::Error::other(OverCap));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const CAPS: OutputCaps = OutputCaps {
        stdout: 16,
        stderr: Some(16),
    };

    fn sh(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(script);
        command
    }

    fn soon() -> Instant {
        Instant::now() + Duration::from_secs(10)
    }

    /// Spec: a program that exits within the limits returns its status and
    /// both streams exactly.
    #[farhelm_testtrace::test]
    async fn an_ordinary_run_returns_status_and_both_streams() {
        let ended = run(&mut sh("printf out; printf err >&2; exit 3"), soon(), CAPS)
            .await
            .unwrap();
        let Ended::Exited(output) = ended else {
            panic!("expected an exit: {ended:?}");
        };
        assert_eq!(output.status.code(), Some(3));
        assert_eq!(output.stdout, b"out");
        assert_eq!(output.stderr, b"err");
    }

    /// Spec: output exactly at the cap is accepted, and one byte more is a
    /// refusal, on either stream.
    ///
    /// The boundary is the contract callers parse against: accepting a
    /// truncated prefix as if it were the whole answer is the failure the
    /// cap exists to prevent.
    #[farhelm_testtrace::test]
    async fn the_cap_admits_exactly_its_size() {
        let at_cap = run(&mut sh("printf 0123456789abcdef"), soon(), CAPS)
            .await
            .unwrap();
        assert!(matches!(at_cap, Ended::Exited(ref out) if out.stdout.len() == 16));
        let over = run(&mut sh("printf 0123456789abcdefX"), soon(), CAPS)
            .await
            .unwrap();
        assert!(matches!(over, Ended::OutputOverCap), "{over:?}");
        let over_err = run(&mut sh("printf 0123456789abcdefX >&2"), soon(), CAPS)
            .await
            .unwrap();
        assert!(matches!(over_err, Ended::OutputOverCap), "{over_err:?}");
    }

    /// Spec: overflowing one stream ends the run at once, even while the
    /// other stream is still open.
    ///
    /// A reader that waited for both streams would leave such a program
    /// running until the deadline and then report a timeout, which is the
    /// wrong answer to the wrong question. `exec sleep` keeps the other
    /// pipe open for the whole run, so only an immediate refusal passes.
    #[farhelm_testtrace::test]
    async fn an_overflow_is_refused_while_the_other_stream_is_open() {
        for script in [
            "printf 0123456789abcdefX; exec sleep 300",
            "printf 0123456789abcdefX >&2; exec sleep 300",
        ] {
            let started = Instant::now();
            let ended = run(&mut sh(script), soon(), CAPS).await.unwrap();
            assert!(matches!(ended, Ended::OutputOverCap), "{script}: {ended:?}");
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{script}: the overflow waited for the deadline"
            );
        }
    }

    /// Whether `pid` has terminated: gone from the process table, or a
    /// zombie awaiting its parent's wait.
    ///
    /// The zombie case counts because the descendant below is not this
    /// test's child: once killed, it is reaped by whatever adopts it, on
    /// that process's schedule, so existence alone would make the test
    /// depend on an unrelated process's reaping.
    fn terminated(pid: i32) -> bool {
        let out = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .expect("run ps");
        let stat = String::from_utf8_lossy(&out.stdout);
        let stat = stat.trim();
        stat.is_empty() || stat.starts_with('Z')
    }

    /// Spec: a deadline ends the run even when a forked descendant holds
    /// the output pipe open, and the kill reaches that descendant.
    ///
    /// This is why the program gets its own process group. Killing only the
    /// leader would leave the background `sleep` holding stdout, so the
    /// read would never see its end, and the sleep would outlive the call.
    /// The descendant is confirmed running before the deadline starts, so a
    /// slow fork cannot turn this into a test of an earlier timeout.
    #[farhelm_testtrace::test]
    async fn a_timeout_kills_the_whole_group() {
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join("descendant.pid");
        let staged = dir.path().join("descendant.pid.tmp");
        // Published by rename, so a reader never sees a partial pid.
        let script = format!(
            "sleep 300 & echo $! > {staged}; mv {staged} {pidfile}; wait",
            staged = staged.display(),
            pidfile = pidfile.display()
        );
        let mut child = GroupChild::spawn(&mut sh(&script), CAPS).unwrap();
        let ready_by = std::time::Instant::now() + Duration::from_secs(10);
        let pid: i32 = loop {
            if let Ok(text) = std::fs::read_to_string(&pidfile) {
                break text.trim().parse().expect("a whole pid");
            }
            assert!(
                std::time::Instant::now() < ready_by,
                "the shell never published its descendant's pid (pidfile {}, staged {})",
                pidfile.exists(),
                staged.exists()
            );
            // sleep-ok: polling interval while the shell forks its descendant.
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        assert!(
            !terminated(pid),
            "the descendant {pid} exited before the test began"
        );

        let started = Instant::now();
        let ended = child
            .finish(Instant::now() + Duration::from_millis(200), CAPS)
            .await
            .unwrap();
        assert!(matches!(ended, Ended::TimedOut), "{ended:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
        let gone_by = std::time::Instant::now() + Duration::from_secs(5);
        while !terminated(pid) {
            assert!(
                std::time::Instant::now() < gone_by,
                "descendant {pid} survived the group kill"
            );
            // sleep-ok: polling interval while the signalled descendant dies.
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Spec: with stderr discarded, a program writing to it still runs to
    /// completion and reports empty stderr.
    #[farhelm_testtrace::test]
    async fn discarded_stderr_is_never_read() {
        let caps = OutputCaps {
            stdout: 16,
            stderr: None,
        };
        let ended = run(&mut sh("printf noise >&2; printf ok"), soon(), caps)
            .await
            .unwrap();
        let Ended::Exited(output) = ended else {
            panic!("expected an exit: {ended:?}");
        };
        assert_eq!(output.stdout, b"ok");
        assert!(output.stderr.is_empty());
    }
}
