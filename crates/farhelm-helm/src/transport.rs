//! Host transports are the seam between connection actors and local or SSH
//! connections.

use crate::store::{HostId, HostKind, HostRow};
use anyhow::Context as _;
use std::{
    collections::VecDeque,
    future::Future,
    path::PathBuf,
    pin::Pin,
    process::{ExitStatus, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Notify, watch};
use tracing::warn;

/// One opened connection's reader/writer pair, type-erased.
///
/// Boxed `dyn` rather than generics because the manager holds a
/// heterogeneous set of actors behind one trait object: the local row
/// speaks over a unix socket, ssh rows over an exec channel, and a test
/// peer over an in-memory duplex. `SupervisorClient` is already
/// transport-blind by construction (SPEC_impl.md's transport section), so
/// erasing the type here costs nothing it was relying on.
pub type TransportPair = (
    Box<dyn AsyncRead + Send + Unpin>,
    Box<dyn AsyncWrite + Send + Unpin>,
);

/// An opened byte stream and, for SSH only, its attempt's process ownership.
///
/// The client consumes `io` without knowing its transport. The actor keeps
/// `ssh` until connection failure has settled, so dropping the client's reader
/// does not kill the child before its natural exit status can be observed.
pub struct TransportConnection {
    pub io: TransportPair,
    pub ssh: Option<SshDiagnostics>,
}

impl TransportConnection {
    /// Local sockets and scripted peers have no SSH evidence or child to retain.
    pub fn plain(io: TransportPair) -> Self {
        Self { io, ssh: None }
    }
}

/// How an actor opens a FRESH connection to its host.
///
/// One method, called once per connection attempt, returning a brand-new
/// pair every time — there is no reuse and no pooling, because a reconnect
/// after a loss must not be able to hand back the corpse of the connection
/// that just died.
///
/// A trait rather than a concrete enum so tests inject scripted supervisor
/// peers over `tokio::io::duplex` without a real process, a real socket,
/// or a real ssh anywhere in the loop — which is what makes the full
/// state machine (backoff timing, skew, identity, duplicates) testable
/// under a paused clock at all.
///
/// The returned future is boxed by hand rather than declared with `async
/// fn`: an `async fn` in a trait is not dyn-compatible, and dyn dispatch
/// is the entire point here.
pub trait HostTransport: Send + Sync + 'static {
    /// Open a connection for `host`. The row is passed whole (not just its
    /// destination) because an ssh row's `remote_farhelm` and
    /// `remote_state_dir` are part of how it is reached, and a test
    /// transport keys off `id`.
    fn connect<'a>(
        &'a self,
        host: &'a HostRow,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<TransportConnection>> + Send + 'a>>;
}

/// The production transport: unix socket for the reserved local row, the
/// user's own `ssh` running `farhelm internal stdio` for an ssh row.
///
/// The exact two paths `connect_supervisor` in `lib.rs` has taken since
/// M1, generalized from "whatever argv said" to "whatever this row says".
/// The ssh argv itself is still built by [`crate::ssh::ssh_stdio_args`] — shared
/// rather than reimplemented, because its quoting rules are the subtlest
/// correctness surface in the transport and two copies would eventually
/// disagree.
pub struct SystemTransport {
    /// The helm's own state directory: where the local supervisor's socket
    /// lives, and where ssh ControlMaster sockets are kept.
    state_dir: std::path::PathBuf,
    /// Injecting the executable keeps process-ordering tests independent of
    /// the running test process's PATH and the operator's SSH configuration.
    ssh_program: PathBuf,
}

impl SystemTransport {
    /// `state_dir` must be the helm's OWN state directory, already
    /// established at `0700` by its caller — it is both where the local
    /// supervisor's socket is looked for and where ssh ControlMaster
    /// sockets are written, and neither is a location this type is free to
    /// choose for itself.
    pub fn new(state_dir: impl Into<std::path::PathBuf>) -> SystemTransport {
        SystemTransport {
            state_dir: state_dir.into(),
            ssh_program: PathBuf::from("ssh"),
        }
    }

    /// Keep the real SSH transport and its argument construction in actor
    /// tests while replacing only the process being launched. This seam is
    /// instance-local; it never changes PATH or the test process environment.
    #[cfg(test)]
    pub(crate) fn with_ssh_program(
        state_dir: impl Into<PathBuf>,
        ssh_program: impl Into<PathBuf>,
    ) -> Self {
        Self {
            state_dir: state_dir.into(),
            ssh_program: ssh_program.into(),
        }
    }
}

/// Marker attached to a local-row dial that failed because nothing is
/// listening — the evidence `manager::HostActor` classifies
/// [`crate::manager::UnreachableCause::LocalSupervisorNotRunning`] from.
///
/// A typed payload rather than a string match on the error, for the same
/// reason `farhelm_proto::io::ClosedBeforeHello` is one: the message text
/// is a diagnostic for humans and must stay free to change, while a state
/// machine reading it would silently break on a rewording.
#[derive(Debug, thiserror::Error)]
#[error("no supervisor is running on this machine")]
pub struct LocalSupervisorNotRunning;

impl HostTransport for SystemTransport {
    fn connect<'a>(
        &'a self,
        host: &'a HostRow,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<TransportConnection>> + Send + 'a>> {
        Box::pin(async move {
            match host.kind {
                HostKind::Local => {
                    let stream = farhelm_supervisor::service::connect(&self.state_dir)
                        .await
                        .map_err(classify_local_dial)?;
                    let (r, w) = tokio::io::split(stream);
                    Ok(TransportConnection::plain((
                        Box::new(r) as Box<dyn AsyncRead + Send + Unpin>,
                        Box::new(w) as Box<dyn AsyncWrite + Send + Unpin>,
                    )))
                }
                HostKind::Ssh => {
                    let dest = host.destination.as_deref().context(
                        "an ssh registry row has no destination; the schema's CHECK constraint \
                         should have made this impossible",
                    )?;

                    let mut cmd = tokio::process::Command::new(&self.ssh_program);
                    cmd.args(crate::ssh::ssh_stdio_args(
                        dest,
                        &self.state_dir,
                        host.remote_farhelm.as_deref().unwrap_or("farhelm"),
                        host.remote_state_dir.as_deref(),
                    )?);
                    // stderr is PIPED and relayed as tracing events, not
                    // inherited. Inheriting is what the M1 single-host path
                    // does, and it was defensible there: one connection the
                    // user started by hand, whose ssh diagnostics belong on
                    // the terminal they are watching. Here the far end is a
                    // registered host running a command the helm chose but
                    // the REMOTE side controls the output of — and anything
                    // written to an inherited stderr reaches the operator's
                    // terminal as raw bytes, unbounded, with escape
                    // sequences intact. A remote that repaints the screen,
                    // hides the cursor, or simply never stops writing would
                    // be doing it to the helm's own console. Relaying
                    // instead keeps ssh's genuinely actionable diagnostics
                    // (auth failure, unresolvable host) while making them
                    // bounded, escaped, and attributable to a host.
                    let child = cmd
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .kill_on_drop(true)
                        .spawn()
                        .context("spawning ssh")?;
                    Ok(ssh_connection(child, host.id, dest.to_string()))
                }
            }
        })
    }
}

/// Keep the process alive through either an active reader or failure diagnosis.
///
/// No detached task owns or waits for the child. Dropping the last anchor
/// aborts stderr drainage and drops the kill-on-drop child, including when an
/// attempt is cancelled or a descendant keeps the stderr pipe open forever.
struct SshLifetime {
    child: tokio::sync::Mutex<tokio::process::Child>,
    stderr_task: tokio::task::AbortHandle,
}

impl Drop for SshLifetime {
    fn drop(&mut self) {
        self.stderr_task.abort();
    }
}

/// The reader keeps successful connections alive after the actor drops its
/// diagnostics owner. Failure diagnosis temporarily holds the other anchor.
struct SshChannel {
    stdout: tokio::process::ChildStdout,
    _lifetime: Arc<SshLifetime>,
}

impl AsyncRead for SshChannel {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

/// Natural termination evidence, without inventing a numeric status for a
/// signal or for a failed wait. `None` in a snapshot means not yet observed.
#[derive(Clone, Debug)]
pub(crate) enum SshExit {
    Status(ExitStatus),
    WaitFailed,
}

/// Raw peer bytes from one diagnostic line, never more than 512 bytes.
///
/// `truncated` means bytes beyond that prefix were drained and discarded.
/// Callers may decode lossily and add a truncation suffix, but must pass the
/// assembled failure through the manager's existing escaping boundary once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SshStderrLine {
    pub bytes: Vec<u8>,
    pub truncated: bool,
}

/// Evidence available now; it makes no claim that process or stderr is done.
///
/// The tail contains at most two lines, oldest first, including the current
/// unterminated segment. A completed connected marker is never diagnostic
/// text. EOF and a read error are distinct, and neither implies child exit.
#[derive(Clone, Debug, Default)]
pub(crate) struct SshSnapshot {
    pub connected_marker_seen: bool,
    pub stderr_tail: Vec<SshStderrLine>,
    pub stderr_eof: bool,
    pub stderr_read_failed: bool,
    pub exit: Option<SshExit>,
}

/// Read-only observations can outlive the attempt without keeping SSH alive.
///
/// This is deliberately separate from `SshDiagnostics`: the manager may retain
/// evidence of an earlier marker for capability detection, but that historical
/// memory must not become a hidden owner of an obsolete child or drainer.
#[derive(Clone)]
pub struct SshObservation(Arc<SshObserved>);

/// Synchronous state locks cover only bounded copies and parsing. Notification
/// waits never hold that lock; the drainer owns no process lifetime through it.
struct SshObserved {
    state: Mutex<SshState>,
    connected: watch::Sender<bool>,
    finished: Notify,
}

/// Two complete diagnostic lines plus one capped pending segment are enough
/// to expose the newest two without evicting a diagnostic for a marker line.
#[derive(Default)]
struct SshState {
    complete: VecDeque<SshStderrLine>,
    pending: Vec<u8>,
    pending_truncated: bool,
    connected_marker_seen: bool,
    stderr_eof: bool,
    stderr_read_failed: bool,
    exit: Option<SshExit>,
}

impl SshObservation {
    /// Observations hold no child, pipes, task handles, or cancellation anchor.
    fn new() -> Self {
        Self(Arc::new(SshObserved {
            state: Mutex::new(SshState::default()),
            connected: watch::channel(false).0,
            finished: Notify::new(),
        }))
    }

    /// Take a bounded synchronous view, without waiting for or polling a child.
    pub(crate) fn snapshot(&self) -> SshSnapshot {
        let state = self.0.state.lock().expect("ssh observation mutex");
        let mut tail: Vec<_> = state.complete.iter().cloned().collect();
        if !state.pending.is_empty() || state.pending_truncated {
            tail.push(SshStderrLine {
                bytes: state.pending.clone(),
                truncated: state.pending_truncated,
            });
        }
        if tail.len() > 2 {
            tail.remove(0);
        }
        SshSnapshot {
            connected_marker_seen: state.connected_marker_seen,
            stderr_tail: tail,
            stderr_eof: state.stderr_eof,
            stderr_read_failed: state.stderr_read_failed,
            exit: state.exit.clone(),
        }
    }

    /// Consume a fixed-size read chunk without allocating uncapped lines.
    /// Marker recognition uses the full line's truncation bit and newline,
    /// so a long line with a marker prefix cannot impersonate progress.
    fn observe_chunk(&self, bytes: &[u8]) -> Vec<SshStderrLine> {
        let mut state = self.0.state.lock().expect("ssh observation mutex");
        let mut diagnostic_lines = Vec::new();
        let mut first_marker = false;
        for byte in bytes {
            if *byte != b'\n' {
                if state.pending.len() < SSH_STDERR_LINE_CAP {
                    state.pending.push(*byte);
                } else {
                    state.pending_truncated = true;
                }
                continue;
            }
            let line = SshStderrLine {
                bytes: std::mem::take(&mut state.pending),
                truncated: std::mem::take(&mut state.pending_truncated),
            };
            if !line.truncated
                && line.bytes == farhelm_proto::io::SUPERVISOR_CONNECTED_MARKER.as_bytes()
            {
                first_marker |= !state.connected_marker_seen;
                state.connected_marker_seen = true;
            } else {
                state.complete.push_back(line.clone());
                if state.complete.len() > 2 {
                    state.complete.pop_front();
                }
                diagnostic_lines.push(line);
            }
        }
        drop(state);
        if first_marker {
            self.0.connected.send_replace(true);
        }
        diagnostic_lines
    }

    /// Keep a final unterminated segment as diagnostics, never as a marker.
    /// A read error preserves those bytes too, but is not reported as EOF.
    fn finish_stderr(&self, failed: bool) -> Option<SshStderrLine> {
        let mut state = self.0.state.lock().expect("ssh observation mutex");
        state.stderr_read_failed = failed;
        state.stderr_eof = !failed;
        let pending = (!state.pending.is_empty() || state.pending_truncated).then(|| {
            let line = SshStderrLine {
                bytes: std::mem::take(&mut state.pending),
                truncated: std::mem::take(&mut state.pending_truncated),
            };
            state.complete.push_back(line.clone());
            if state.complete.len() > 2 {
                state.complete.pop_front();
            }
            line
        });
        drop(state);
        self.0.finished.notify_waiters();
        pending
    }

    /// Record an actual wait result; absent results remain unknown.
    fn record_exit(&self, exit: SshExit) {
        self.0.state.lock().expect("ssh observation mutex").exit = Some(exit);
    }
}

/// An attempt owner permits natural exit diagnosis after the reader has gone.
/// Dropping all diagnostics and the reader cancels the child and stderr task.
#[derive(Clone)]
pub struct SshDiagnostics {
    lifetime: Arc<SshLifetime>,
    observation: SshObservation,
}

impl SshDiagnostics {
    /// Retain historical evidence without retaining a process lifetime.
    pub fn observation(&self) -> SshObservation {
        self.observation.clone()
    }

    /// Opportunistically reap a completed child, without waiting or acquiring a
    /// contended async lock. A deadline handler can call this synchronously.
    pub(crate) fn snapshot(&self) -> SshSnapshot {
        if self.observation.snapshot().exit.is_none()
            && let Ok(mut child) = self.lifetime.child.try_lock()
        {
            match child.try_wait() {
                Ok(Some(status)) => self.observation.record_exit(SshExit::Status(status)),
                Err(_) => self.observation.record_exit(SshExit::WaitFailed),
                Ok(None) => {}
            }
        }
        self.observation.snapshot()
    }

    /// Wait for natural child exit and stderr completion under one allowance.
    ///
    /// Only early transport failures use this; the actor's unchanged whole
    /// attempt deadline surrounds it. A deadline expiry snapshots immediately.
    /// Cancellation of `Child::wait` leaves ownership here for kill-on-drop.
    pub(crate) async fn settle(&self) -> SshSnapshot {
        let _ = tokio::time::timeout(Duration::from_secs(2), async {
            if self.observation.snapshot().exit.is_none() {
                let result = self.lifetime.child.lock().await.wait().await;
                self.observation.record_exit(match result {
                    Ok(status) => SshExit::Status(status),
                    Err(_) => SshExit::WaitFailed,
                });
            }
            loop {
                let finished = self.observation.0.finished.notified();
                tokio::pin!(finished);
                finished.as_mut().enable();
                let snapshot = self.observation.snapshot();
                if snapshot.stderr_eof || snapshot.stderr_read_failed {
                    break;
                }
                finished.await;
            }
        })
        .await;
        self.snapshot()
    }

    /// Remain pending until the first exact connected line. EOF is not
    /// progress and must not create a ready-loop while hello is still pending.
    pub(crate) async fn wait_connected(&self) {
        let mut connected = self.observation.0.connected.subscribe();
        loop {
            if *connected.borrow_and_update() {
                return;
            }
            // The sender lives inside this observation, which this borrow
            // retains. The channel cannot close while this future exists.
            connected.changed().await.expect("ssh observation sender");
        }
    }
}

/// Attach exactly one drainer and one shared child anchor to piped SSH.
/// The drainer holds observations only, so cancellation cannot form a cycle.
fn ssh_connection(
    mut child: tokio::process::Child,
    host: HostId,
    destination: String,
) -> TransportConnection {
    let stdout = child.stdout.take().expect("piped stdout");
    let stdin = child.stdin.take().expect("piped stdin");
    let stderr = child.stderr.take().expect("piped stderr");
    let observation = SshObservation::new();
    let stderr_task = tokio::spawn(relay_ssh_stderr(
        host,
        destination,
        stderr,
        observation.clone(),
    ));
    let lifetime = Arc::new(SshLifetime {
        child: tokio::sync::Mutex::new(child),
        stderr_task: stderr_task.abort_handle(),
    });
    TransportConnection {
        io: (
            Box::new(SshChannel {
                stdout,
                _lifetime: lifetime.clone(),
            }),
            Box::new(stdin),
        ),
        ssh: Some(SshDiagnostics {
            lifetime,
            observation,
        }),
    }
}

/// Longest single line of an ssh child's stderr this side will relay.
///
/// Not a formatting preference: the far side decides how long a "line" is,
/// and a remote that writes megabytes without a newline would otherwise be
/// choosing this process's memory usage. A truncated line is marked as
/// such, so an operator can tell "ssh said this" from "ssh said this and
/// more".
const SSH_STDERR_LINE_CAP: usize = 512;

/// How many stderr lines one ssh child may have relayed before the rest are
/// dropped.
///
/// A per-CHILD budget rather than a rate: an ssh that has said two hundred
/// things has said everything diagnostic it was going to say, and the
/// remainder is either a loop or an attack on the log. The actor makes a
/// new child per attempt, so a genuinely chatty host still gets a fresh
/// budget on every reconnect rather than going permanently quiet.
const SSH_STDERR_LINE_BUDGET: usize = 200;

/// Relay one ssh child's stderr into the tracing trail, bounded and
/// escaped, and attributed to the host it came from.
///
/// Every line is `Debug`-formatted, which is the same defense the
/// supervisor applies to tmux's exit reasons and for the same reason: this
/// is text a REMOTE party influences, arriving at a log an operator reads
/// in a terminal emulator. `Display` would replay control bytes verbatim —
/// cursor moves, screen clears, an OSC sequence retitling the window —
/// while `Debug` escapes them into something legible and inert.
///
/// Child exit and stderr EOF are independent: a descendant can retain stderr.
/// The lifetime anchor aborts this task when its final owner drops.
async fn relay_ssh_stderr<R>(
    host: HostId,
    destination: String,
    mut stderr: R,
    observation: SshObservation,
) where
    R: AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt as _;

    let mut bytes = [0u8; 4096];
    let mut relayed = 0usize;
    loop {
        match stderr.read(&mut bytes).await {
            Ok(0) => {
                if let Some(line) = observation.finish_stderr(false) {
                    log_ssh_line(host, &destination, &line, &mut relayed);
                }
                break;
            }
            Ok(count) => {
                // Observation continues past the warning budget. Otherwise a
                // chatty peer could hide its connected marker or final error.
                for line in observation.observe_chunk(&bytes[..count]) {
                    log_ssh_line(host, &destination, &line, &mut relayed);
                }
            }
            Err(_) => {
                if let Some(line) = observation.finish_stderr(true) {
                    log_ssh_line(host, &destination, &line, &mut relayed);
                }
                break;
            }
        }
    }
}

/// Warnings stop at the existing per-child budget; drainage never stops there.
/// Saturating the count avoids growing a counter on an endless stderr flood.
fn log_ssh_line(host: HostId, destination: &str, line: &SshStderrLine, relayed: &mut usize) {
    if *relayed >= SSH_STDERR_LINE_BUDGET {
        return;
    }
    *relayed += 1;
    if *relayed == SSH_STDERR_LINE_BUDGET {
        warn!(
            host,
            destination,
            budget = SSH_STDERR_LINE_BUDGET,
            "the ssh child for this host has said enough; dropping the rest of its \
             stderr for this connection"
        );
        return;
    }
    let text = String::from_utf8_lossy(&line.bytes);
    warn!(
        host,
        destination,
        truncated = line.truncated,
        // Debug-formatted deliberately; see relay_ssh_stderr's docs.
        message = ?text,
        "ssh reported a problem for this host"
    );
}

/// Tag a failed local-socket dial that means "no supervisor here" with
/// [`LocalSupervisorNotRunning`], leaving every other failure untouched.
///
/// The two `io::ErrorKind`s below are the same pair
/// `farhelm_supervisor::service::connect` itself keys its remedy message
/// off — nothing is listening on the socket, or there is no socket file at
/// all. That duplication is deliberate and narrow: this side needs the
/// answer as a TYPE (a state-machine input), that side needs it as prose
/// (an operator's remedy), and re-deriving it here from the kinds is
/// cheaper and clearer than either parsing that prose or reshaping a
/// public API for one caller. Every other kind — permission denied, a
/// non-directory path component — keeps its original error, because
/// "start a supervisor" would be wrong advice for all of them.
pub(crate) fn classify_local_dial(error: anyhow::Error) -> anyhow::Error {
    let refused = error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|io| {
            matches!(
                io.kind(),
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
            )
        });
    if refused {
        error.context(LocalSupervisorNotRunning)
    } else {
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    /// A timeout must see the capped current segment even before EOF. A
    /// newline-free peer must not choose the helm's retained allocation.
    #[farhelm_testtrace::test]
    async fn a_newline_free_stderr_flood_stays_bounded() {
        let observation = SshObservation::new();
        let chunk = [b'x'; 4096];
        for _ in 0..1024 {
            assert!(observation.observe_chunk(&chunk).is_empty());
        }
        let snapshot = observation.snapshot();
        assert!(!snapshot.stderr_eof);
        assert_eq!(snapshot.stderr_tail.len(), 1);
        assert_eq!(snapshot.stderr_tail[0].bytes.len(), SSH_STDERR_LINE_CAP);
        assert!(snapshot.stderr_tail[0].truncated);
        let state = observation.0.state.lock().unwrap();
        assert!(state.complete.is_empty());
        assert_eq!(state.pending.len(), SSH_STDERR_LINE_CAP);
    }

    /// The current partial line participates in the newest-two view. Invalid
    /// UTF-8 and controls remain raw until the existing failure-text boundary.
    #[farhelm_testtrace::test]
    async fn stderr_tail_keeps_two_lines_including_the_current_partial() {
        let observation = SshObservation::new();
        observation.observe_chunk(b"old\nfirst\nsecond\n\xff\x1bpartial");
        let snapshot = observation.snapshot();
        assert_eq!(snapshot.stderr_tail.len(), 2);
        assert_eq!(snapshot.stderr_tail[0].bytes, b"second");
        assert_eq!(snapshot.stderr_tail[1].bytes, b"\xff\x1bpartial");
        assert!(!snapshot.stderr_tail[1].truncated);
        observation.finish_stderr(false);
        let ended = observation.snapshot();
        assert!(ended.stderr_eof);
        assert!(!ended.stderr_read_failed);
        assert_eq!(ended.stderr_tail, snapshot.stderr_tail);
    }

    /// Only an exact, newline-terminated, untruncated line proves progress.
    /// Chunk boundaries and an oversized preceding banner cannot hide it.
    #[farhelm_testtrace::test]
    async fn connected_marker_requires_exact_framing_and_survives_split_reads() {
        let marker = farhelm_proto::io::SUPERVISOR_CONNECTED_MARKER.as_bytes();
        let observation = SshObservation::new();
        let mut spoof = marker.to_vec();
        spoof.extend(vec![b'x'; SSH_STDERR_LINE_CAP]);
        spoof.push(b'\n');
        observation.observe_chunk(&spoof);
        assert!(!observation.snapshot().connected_marker_seen);
        observation.observe_chunk(&[vec![b'y'; 4096], b"\n".to_vec()].concat());
        observation.observe_chunk(&marker[..10]);
        observation.observe_chunk(&marker[10..]);
        assert!(!observation.snapshot().connected_marker_seen);
        let lines = observation.observe_chunk(b"\n");
        assert!(lines.is_empty(), "the marker is not warning text");
        assert!(observation.snapshot().connected_marker_seen);
        assert!(
            observation
                .snapshot()
                .stderr_tail
                .iter()
                .all(|line| line.bytes != marker)
        );

        for bytes in [
            [b"prefix-".as_slice(), marker, b"\n"].concat(),
            [marker, b"-suffix\n"].concat(),
            marker.to_vec(),
        ] {
            let invalid = SshObservation::new();
            invalid.observe_chunk(&bytes);
            invalid.finish_stderr(false);
            assert!(!invalid.snapshot().connected_marker_seen);
        }
    }

    /// A noisy peer cannot spend the log budget to prevent later progress or
    /// error capture. Exercise the real relay, not only its line parser.
    #[farhelm_testtrace::test]
    async fn observation_continues_after_the_warning_budget() {
        let mut input = b"banner\n".repeat(SSH_STDERR_LINE_BUDGET + 10);
        input.extend_from_slice(farhelm_proto::io::SUPERVISOR_CONNECTED_MARKER.as_bytes());
        input.extend_from_slice(b"\nlate error\nfinal segment");
        let observation = SshObservation::new();
        relay_ssh_stderr(1, "fixture".into(), input.as_slice(), observation.clone()).await;
        let snapshot = observation.snapshot();
        assert!(snapshot.connected_marker_seen);
        assert!(snapshot.stderr_eof);
        assert_eq!(snapshot.stderr_tail[0].bytes, b"late error");
        assert_eq!(snapshot.stderr_tail[1].bytes, b"final segment");
    }

    /// An I/O error preserves the partial diagnostic but must not manufacture
    /// EOF or a connected marker from stderr merely having stopped.
    #[farhelm_testtrace::test]
    async fn stderr_read_error_is_not_eof_or_connected_progress() {
        let observation = SshObservation::new();
        observation.observe_chunk(b"partial");
        observation.finish_stderr(true);
        let snapshot = observation.snapshot();
        assert!(snapshot.stderr_read_failed);
        assert!(!snapshot.stderr_eof);
        assert!(!snapshot.connected_marker_seen);
        assert_eq!(snapshot.stderr_tail[0].bytes, b"partial");
    }

    /// A tiny POSIX executable controls descriptor order and blocks on its
    /// own stdin, so tests observe a real readiness boundary instead of sleep.
    /// The script ignores SSH argv; only this transport instance uses it.
    fn fake_ssh(body: &str) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().unwrap();
        let program = directory.path().join("ssh-fixture");
        std::fs::write(&program, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        (directory, program)
    }

    /// An ordinary SSH row exercises executable injection and real argument
    /// construction without using a real login or editing process environment.
    fn ssh_row() -> HostRow {
        HostRow {
            id: 1,
            entry_uuid: "00000000-0000-4000-8000-000000000001".into(),
            kind: HostKind::Ssh,
            destination: Some("fixture".into()),
            alias: None,
            remote_farhelm: None,
            remote_state_dir: None,
            host_identity: None,
            cache_truncated: false,
            yolo_without_asking: false,
            commands_without_asking: false,
            links_without_asking: false,
            local_links_without_asking: false,
            icon: farhelm_proto::host_appearance::HostIcon::default(),
            color: farhelm_proto::host_appearance::HostColor::default(),
        }
    }

    /// stdout may close before the final stderr and natural status. Keeping
    /// diagnostics after the read half drops must preserve that late evidence.
    #[farhelm_testtrace::test]
    async fn stdout_closure_does_not_kill_the_child_before_failure_settles() {
        let body = format!(
            "exec 1>&-\nprintf '%s\\n' '{}' >&2\nIFS= read -r proceed\nprintf 'late error\\n' >&2\nexit 127",
            farhelm_proto::io::SUPERVISOR_CONNECTED_MARKER
        );
        let (directory, program) = fake_ssh(&body);
        let transport = SystemTransport::with_ssh_program(directory.path(), program);
        let connection = transport.connect(&ssh_row()).await.unwrap();
        let ((mut reader, mut writer), diagnostics) = (connection.io, connection.ssh.unwrap());
        tokio::time::timeout(Duration::from_secs(5), diagnostics.wait_connected())
            .await
            .expect("fixture reached its stdin gate");
        assert!(
            diagnostics.snapshot().exit.is_none(),
            "the child is still gated"
        );
        assert_eq!(reader.read(&mut [0u8; 1]).await.unwrap(), 0);
        drop(reader);
        assert!(
            diagnostics.snapshot().exit.is_none(),
            "reader drop kept the child alive"
        );
        writer.write_all(b"continue\n").await.unwrap();
        drop(writer);
        let snapshot = diagnostics.settle().await;
        assert!(snapshot.stderr_eof);
        assert!(
            matches!(snapshot.exit, Some(SshExit::Status(status)) if status.code() == Some(127))
        );
        assert_eq!(snapshot.stderr_tail.len(), 1);
        assert_eq!(snapshot.stderr_tail[0].bytes, b"late error");
    }

    /// Poll only the owned child's disappearance. The readiness handshake in
    /// the caller proves this is cancellation of a live process, not a child
    /// that had already exited naturally before its lifetime anchor dropped.
    async fn wait_child_gone(pid: u32) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                // SAFETY: signal zero only checks this owned child's existence.
                if unsafe { libc::kill(pid as libc::pid_t, 0) } == -1
                    && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
                {
                    return;
                }
                // sleep-ok: bounded polling of the owned kill-on-drop child's reaping.
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("cancelled owned child was killed and reaped");
    }

    /// Historical observations must not keep a cancelled attempt alive. The
    /// direct child is blocked on stdin when its last owner is dropped.
    #[farhelm_testtrace::test]
    async fn retaining_observations_does_not_retain_the_child() {
        let body = format!(
            "printf '%s\\n' '{}' >&2\nIFS= read -r proceed\nexit 0",
            farhelm_proto::io::SUPERVISOR_CONNECTED_MARKER
        );
        let (_directory, program) = fake_ssh(&body);
        let child = tokio::process::Command::new(program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();
        let connection = ssh_connection(child, 1, "fixture".into());
        let ((reader, writer), diagnostics) = (connection.io, connection.ssh.unwrap());
        tokio::time::timeout(Duration::from_secs(5), diagnostics.wait_connected())
            .await
            .expect("fixture reached its stdin gate");
        assert!(diagnostics.snapshot().exit.is_none());
        let observation = diagnostics.observation();
        let lifetime = Arc::downgrade(&diagnostics.lifetime);
        drop(reader);
        drop(diagnostics);
        assert!(lifetime.upgrade().is_none());
        // Keep stdin open: EOF is not what made this process exit.
        wait_child_gone(pid).await;
        assert!(observation.snapshot().connected_marker_seen);
        drop(writer);
    }

    /// EOF without a marker must not make the progress future immediately
    /// ready: the actor races it against hello, and a ready EOF would spin.
    #[farhelm_testtrace::test]
    async fn connected_wait_remains_pending_after_stderr_eof() {
        let child = tokio::process::Command::new("true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let connection = ssh_connection(child, 1, "fixture".into());
        let diagnostics = connection.ssh.unwrap();
        let snapshot = diagnostics.settle().await;
        assert!(snapshot.stderr_eof);
        assert!(!snapshot.connected_marker_seen);
        tokio::select! {
            biased;
            _ = diagnostics.wait_connected() => panic!("EOF is not connected progress"),
            _ = std::future::ready(()) => {}
        }
        drop(connection.io);
    }

    /// stderr can outlive the direct child. Reap first, then pause time so
    /// OS scheduling cannot masquerade as the shared settle deadline expiring.
    /// Dropping the final lifetime owner must abort this still-open drainer.
    #[farhelm_testtrace::test]
    async fn natural_exit_with_open_stderr_has_one_bounded_settle_allowance() {
        let mut child = tokio::process::Command::new("true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let status = child.wait().await.unwrap();
        assert!(status.success());
        let (stderr, held_open) = tokio::io::duplex(16);
        let observation = SshObservation::new();
        observation.record_exit(SshExit::Status(status));
        let task = tokio::spawn(relay_ssh_stderr(
            1,
            "fixture".into(),
            stderr,
            observation.clone(),
        ));
        let diagnostics = SshDiagnostics {
            lifetime: Arc::new(SshLifetime {
                child: tokio::sync::Mutex::new(child),
                stderr_task: task.abort_handle(),
            }),
            observation,
        };
        assert!(!diagnostics.snapshot().stderr_eof);
        tokio::time::pause();
        let started = tokio::time::Instant::now();
        let snapshot = diagnostics.settle().await;
        // Tokio rounds deadlines up to a millisecond tick, even with time
        // paused. Permit that rounding while still rejecting an early return
        // or an extended wait for the still-open stderr stream.
        let elapsed = started.elapsed();
        assert!(
            (Duration::from_secs(2)..=Duration::from_millis(2001)).contains(&elapsed),
            "one settle allowance, including timer rounding: {elapsed:?}"
        );
        assert!(!snapshot.stderr_eof);
        assert!(matches!(snapshot.exit, Some(SshExit::Status(status)) if status.success()));
        drop(diagnostics);
        assert!(
            task.await.unwrap_err().is_cancelled(),
            "the open drainer was aborted"
        );
        drop(held_open);
    }
}
