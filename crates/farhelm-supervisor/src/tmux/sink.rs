//! The always-drained session client that keeps filtered panes readable.

use super::control_codec::read_command_block;
use super::{TmuxDriver, control_cleanup_retry_delay, shutdown_output_control_client};
use anyhow::Context as _;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tracing::warn;

impl TmuxDriver {
    /// Open `session`'s SINK control client: attached, receiving every
    /// pane, and destined to be drained into nothing. See [`SessionSink`]
    /// for what it is for; this is only how it is built.
    ///
    /// # Attached with output already ON, deliberately
    ///
    /// The replay client attaches `-f no-output` and flips the flag later,
    /// so that the attach handshake's reply block — read
    /// positionally — cannot have pane output racing into it. The sink
    /// does the opposite, and the difference is the whole point: a client
    /// that is attached but silent is exactly the shape whose
    /// pane-holding behavior tmux does not document (see [`SessionSink`]'s
    /// note on the rejected no-output design), so a sink that spent its
    /// first moments that way would be relying, for that window, on the
    /// property it exists to avoid relying on. Attaching output-on makes
    /// it a client tmux can demonstrably deliver to from the instant it
    /// registers.
    ///
    /// The reason that is safe here and not elsewhere: this client's
    /// handshake block is read with no own-pane, so a neighbour's output
    /// arriving around it is ordinary chatter rather than an ordering
    /// violation, and tmux does not interleave notifications INSIDE a
    /// command's reply block. Audited 2026-08-02 on tmux 3.4 and 3.7b
    /// against a 16 MB/s producer: 10 of 10 attaches read a clean
    /// handshake with no payload trapped inside it, and 10 of 10 were
    /// receiving pane output immediately after.
    ///
    /// Returns only once the client is attached, because the caller's
    /// ORDERING depends on it: no per-terminal client may turn panes off
    /// until tmux already has this one (see `silence_pane_args`).
    pub async fn open_session_sink(&self, session: &str) -> anyhow::Result<SessionSink> {
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        let mut child = self
            .command()
            .arg("-C")
            .arg("attach")
            .arg("-t")
            .arg(session)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("spawning tmux session-sink client")?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let sink = SessionSink {
            driver: Some(self.clone()),
            client_target: child
                .id()
                .map(|pid| format!("client-{pid}"))
                .context("tmux session-sink client has no process id")?,
            output_disabled: false,
            child,
            stdin: Some(stdin),
            reader: BufReader::new(stdout),
            line: Vec::with_capacity(8192),
        };
        let mut candidate = SessionSinkOpenCandidate::new(sink);
        if let Err(attach) = candidate
            .sink_mut()
            .read_block(deadline, "session-sink attach")
            .await
        {
            candidate.shutdown().await;
            return Err(attach);
        }
        Ok(candidate.install())
    }
}

/// The one control client per tmux SESSION that exists purely so tmux
/// always has somebody it can deliver every pane to.
///
/// # Why a session needs one at all
///
/// tmux stops reading a pane's PTY when no attached client is able to
/// consume that pane's output — which blocks the pane's own process on
/// its next `write`. Two ways into that state matter here, and this type
/// closes both:
///
/// - A per-terminal client that STALLS. Audited 2026-08-02 on tmux 3.4 and
///   3.7b: against a 16 MB/s producer, a control client that stopped
///   reading froze that producer's pane in 4 of 5 trials per version, for
///   the entire 45-second observation window — not for `pause-after` and
///   not only for the stalled viewer's own pane. With an always-drained
///   second client attached, 5 of 5 trials per version showed no stall at
///   all (longest gap 0.20s, against a 0.15s no-client noise floor).
/// - A per-terminal client that turns foreign panes OFF, which is what
///   `silence_pane_args` does and what tmux's own man page says
///   stops the pane being read once EVERY client has done it.
///
/// The two are why this had to land WITH the pane filter rather than
/// before or after it: the filter is only safe because the sink is never
/// one of the clients turning anything off, and the sink is only useful
/// because it is never the client that falls behind.
///
/// # The shape, and why each part of it
///
/// Attached to the session with every pane ON (no per-pane filter) and
/// `pause-after` deliberately UNSET — a paused sink is a sink that has
/// stopped doing its job, so the one client that must never be the
/// flow-control victim does not carry the flag that makes clients
/// victims. It declares no size (no `refresh-client -C`), like every other
/// Farhelm control client, so window geometry still comes only from
/// `resize-window`. During normal operation output is on from the attach
/// itself (see [`TmuxDriver::open_session_sink`]), the sink writes no commands,
/// and it reads nothing but bytes it throws away. Orderly teardown alone
/// writes `no-output` so tmux can discard this client's queues safely.
///
/// # The one window that remains
///
/// A sink that DIES leaves its session without one until a replacement is
/// attached: a process spawn plus one round trip, retried with backoff by
/// its owner. During that window the session's terminals still have their
/// foreign panes filtered off, so a pane no terminal is watching can stop
/// being read — the very state the sink prevents. It is bounded (by the
/// owner's backoff cap) rather than eliminated, and eliminating it would
/// take a second sink standing by at all times, which trades a permanent
/// cost for a rare one. The honest statement of the guarantee is therefore
/// "except across a sink respawn", and SPEC_impl.md says so too.
///
/// A cheaper shape was measured and rejected: a client left permanently
/// `-f no-output` ALSO kept every pane readable in the same audit (6 of 6
/// trials per version), at zero delivery cost, because tmux treats a
/// no-output client as one that can never fall behind. That is an
/// implementation detail of tmux's flow control rather than anything its
/// documentation promises — the documented rule is about clients being
/// able to CONSUME — so relying on it would make this module's isolation
/// guarantee hostage to an unstated behavior. The sink pays one copy of
/// the session's traffic to rest on the documented rule instead; the pane
/// filter it enables removes N-1 copies in exchange (see [`super::OutputStream`]).
pub struct SessionSink {
    /// The independent command path used to disable this client's output.
    /// Test-only process doubles have no tmux driver and need no transition.
    driver: Option<TmuxDriver>,
    /// tmux's stable name for a control client spawned as this OS process.
    client_target: String,
    /// Set only after the external command has acknowledged `no-output`.
    output_disabled: bool,
    /// Reaped explicitly by orderly shutdown. `kill_on_drop` remains the
    /// cancellation and unwind fallback, tying the process to this value
    /// even when its owner cannot await teardown.
    child: Child,
    /// Held open for the sink's lifetime because stdin EOF detaches a control
    /// client. Normal operation never writes to it. Orderly shutdown disables
    /// output through the independent driver, then closes this pipe.
    stdin: Option<ChildStdin>,
    reader: BufReader<tokio::process::ChildStdout>,
    line: Vec<u8>,
}

impl SessionSink {
    /// Read one command-reply block during the attach exchange.
    ///
    /// The `own_pane` argument of [`read_command_block`] is deliberately
    /// empty here: that parameter exists so a client can tell "live output
    /// for MY pane arrived before the cutover" (an ordering violation)
    /// apart from "a neighbour is busy" (ordinary chatter). The sink
    /// speaks for no pane and every pane's output is chatter to it, so no
    /// pane id can be the wrong one to see — and an empty id matches
    /// nothing tmux can emit, since every pane id starts with `%`.
    async fn read_block(
        &mut self,
        deadline: tokio::time::Instant,
        purpose: &str,
    ) -> anyhow::Result<()> {
        read_command_block(&mut self.reader, &mut self.line, deadline, purpose, "").await?;
        Ok(())
    }

    /// The sink client's process id, or `None` once it has been reaped.
    ///
    /// Exists for the supervisor's own logging and for the test that kills
    /// the sink out from under a live attachment to prove it self-heals:
    /// the pid is the only handle on a process that a test can `kill -9`,
    /// and a respawn is only observable as the pid changing.
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// A sink backed by an arbitrary child process, for tests that need to
    /// drive the SUPERVISING LOOP rather than tmux.
    ///
    /// `run_session_sink`'s policy — when to retry, how long to wait,
    /// whether a run counted as healthy — is decided entirely by how long
    /// `drain` takes to return, and pinning it against a real tmux would
    /// mean killing real clients and hoping the timing lands. A `cat`
    /// whose stdin the test holds is the same thing without the hope: it
    /// drains until the test decides it should end.
    #[cfg(test)]
    pub(crate) fn from_child_for_tests(mut child: Child) -> SessionSink {
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        SessionSink {
            driver: None,
            client_target: String::new(),
            output_disabled: true,
            child,
            stdin: Some(stdin),
            reader: BufReader::new(stdout),
            line: Vec::new(),
        }
    }

    /// Read and discard this client's output until it ends.
    ///
    /// Returns when the client exits or its stream fails — the sink has no
    /// other terminal state, and both cases mean the same thing to the
    /// caller: this session no longer has a sink and needs a new one. The
    /// bytes are genuinely thrown away; nothing about a session's panes is
    /// learned here, and the read exists only so tmux never has an unread
    /// client to apply flow control against.
    ///
    /// Reads in fixed-size chunks rather than lines on purpose. A control
    /// client's output is line-oriented, but the sink parses none of it,
    /// and a line-based read would make an enormous single notification
    /// (a pane emitting a megabyte with no newline) an allocation the sink
    /// has no reason to make.
    pub async fn drain(&mut self) {
        let mut scratch = [0u8; 64 * 1024];
        loop {
            match self.reader.read(&mut scratch).await {
                Ok(0) => return,
                Ok(_) => {}
                Err(e) => {
                    warn!(error = %e, "session-sink control client read failed");
                    return;
                }
            }
        }
    }

    /// Disable this sink's output before detaching its control client.
    async fn disable_output_before_shutdown(&mut self) -> anyhow::Result<()> {
        if self.output_disabled
            || self
                .child
                .try_wait()
                .context("checking whether the session-sink client already exited")?
                .is_some()
        {
            return Ok(());
        }
        let Some(driver) = &self.driver else {
            // Process doubles used by the supervising-loop tests are not tmux
            // clients and have no pane queue to invalidate.
            self.output_disabled = true;
            return Ok(());
        };
        if let Err(error) = driver
            .disable_control_client_output(&self.client_target)
            .await
        {
            if self
                .child
                .try_wait()
                .context("rechecking whether the session-sink client exited")?
                .is_some()
            {
                return Ok(());
            }
            return Err(error).context("disabling session-sink output before shutdown");
        }
        self.output_disabled = true;
        Ok(())
    }

    /// Detach this control client and confirm that the process has exited.
    ///
    /// `kill_on_drop` remains the cancellation fallback, but it cannot
    /// establish a handoff boundary: dropping requests termination without
    /// waiting for the old tmux client to disappear. More importantly, a
    /// process kill can invalidate output tmux has already queued for this
    /// client and crash the server; [`shutdown_output_control_client`] closes
    /// stdin and drains the tail instead. Orderly last-owner teardown uses
    /// this path before a queued reattach may create its replacement.
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.disable_output_before_shutdown().await?;
        shutdown_output_control_client(
            &mut self.child,
            self.stdin.take(),
            &mut self.reader,
            "session-sink control client",
        )
        .await
    }
}

/// Own a sink client until its attach handshake commits or cleanup finishes.
///
/// `open_session_sink` spawns the process before its first awaited reply. This
/// guard makes cancellation hand the client to the runtime, which retries the
/// output-off boundary without keeping the request future alive.
struct SessionSinkOpenCandidate {
    sink: Option<SessionSink>,
    runtime: tokio::runtime::Handle,
}

impl SessionSinkOpenCandidate {
    fn new(sink: SessionSink) -> Self {
        Self {
            sink: Some(sink),
            runtime: tokio::runtime::Handle::current(),
        }
    }

    fn sink_mut(&mut self) -> &mut SessionSink {
        self.sink.as_mut().expect("session-sink candidate is live")
    }

    fn install(mut self) -> SessionSink {
        self.sink.take().expect("session-sink candidate is live")
    }

    async fn shutdown(mut self) {
        if let Some(mut sink) = self.sink.take() {
            reap_session_sink_candidate(&mut sink).await;
        }
    }
}

impl Drop for SessionSinkOpenCandidate {
    fn drop(&mut self) {
        let Some(mut sink) = self.sink.take() else {
            return;
        };
        self.runtime.spawn(async move {
            reap_session_sink_candidate(&mut sink).await;
        });
    }
}

/// Retry an uncommitted sink's safe boundary without flooding logs or forks.
async fn reap_session_sink_candidate(sink: &mut SessionSink) {
    let mut failures = 0u32;
    loop {
        match sink.shutdown().await {
            Ok(()) => return,
            Err(error) => {
                failures = failures.saturating_add(1);
                if failures.is_power_of_two() {
                    warn!(
                        error = %format!("{error:#}"),
                        failures,
                        "uncommitted session-sink cleanup is not yet safe; retrying"
                    );
                }
            }
        }
        tokio::time::sleep(control_cleanup_retry_delay(failures)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;

    /// The sink is what keeps a pane every terminal has filtered off from
    /// being frozen by tmux — asserted as a MECHANISM, both directions.
    ///
    /// Stream tests prove that existing and newly created foreign panes are
    /// filtered. This one pins the separate safety requirement, in both
    /// directions: once every terminal client has turned a pane off, the
    /// sink must keep tmux reading it. The busy pane is turned off on the
    /// ONLY terminal client, and its progress is watched from outside tmux.
    ///
    /// The negative half runs first and is what gives the positive half
    /// its meaning: with no sink, the same configuration must
    /// freeze the pane. Without that control, "the pane kept running"
    /// could just mean the filter never took effect.
    #[farhelm_testtrace::test]
    async fn only_the_sink_keeps_a_filtered_pane_readable() {
        let server = ScratchServer::start().await;
        let progress = server.dir.path().join("busy-progress");
        let agent = server
            .driver
            .create_session("fh-sink-mech", "/", 80, 24, &[], &ticking_pane("AGENT"))
            .await
            .expect("session");
        server
            .driver
            .new_window("fh-sink-mech", "/", &[], &bursting_pane("BUSY", &progress))
            .await
            .expect("a busy window");

        // NO sink: the terminal client's filter is the only opinion tmux
        // has about the busy pane, and the man page says that stops the
        // read. Measured, not assumed.
        let (_modes, _prefill, mut stream) = server
            .driver
            .open_replay_stream("fh-sink-mech", &agent)
            .await
            .expect("replay stream");
        pump_own_pane_ticks(&server, &mut stream, 5, 30).await;
        let before = read_progress(&progress);
        pump_own_pane_ticks(&server, &mut stream, 25, 60).await;
        // At most ONE, not zero. The producer writes its counter and THEN
        // starts the burst that fills the pty, so a pane frozen by this
        // filter can still be one step past where it was when the filter
        // landed — it gets to announce the cycle it then blocks inside.
        // What it cannot do is keep cycling, which is what the sinked half
        // below shows it doing.
        let frozen_at = read_progress(&progress);
        assert!(
            frozen_at <= before + 1,
            "test premise: with no sink, a pane filtered off by every client must freeze — it \
             advanced from {before} to {frozen_at}. If this ever stops holding, the sink's \
             whole justification needs re-auditing"
        );

        // Now the sink, on the same already-frozen pane: attaching a
        // client that wants everything must set it running again.
        let mut sink = server
            .driver
            .open_session_sink("fh-sink-mech")
            .await
            .expect("session sink");
        let sink_task = tokio::spawn(async move { sink.drain().await });
        pump_own_pane_ticks(&server, &mut stream, 25, 60).await;
        let running_at = read_progress(&progress);
        assert!(
            running_at >= frozen_at + 2,
            "the sink did not restart the filtered pane's reads: {frozen_at} -> {running_at} \
             over a window in which an unblocked producer completes several cycles"
        );
        assert!(
            !sink_task.is_finished(),
            "the sink stopped draining, so the recovery above proves nothing"
        );
        shutdown_test_stream(stream).await;
    }
}
