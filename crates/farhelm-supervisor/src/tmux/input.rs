//! Confirmed input delivery over a dedicated no-output control client.

use super::control_codec::read_command_block;
use super::{TmuxDriver, pane_in_session};
use anyhow::Context as _;
use std::fmt::Write as _;
use std::process::Stdio;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};

impl TmuxDriver {
    /// Open a control-mode client dedicated to carrying input into `pane`.
    ///
    /// Attaches with `-f no-output` PERMANENTLY, unlike the replay
    /// client's transient use of the same flag: this client must never
    /// see a pane-output notification — from THIS pane or from any other
    /// window's, since a control client attached to a session receives
    /// every pane on it — because [`InputClient::send`] reads exactly one
    /// command-reply block per chunk it writes, and an interleaved
    /// pane-output line would desynchronize that read from the write that
    /// produced it. See [`InputClient`] for why input gets its own client
    /// at all rather than riding the replay client's stdin.
    pub async fn open_input_client(
        &self,
        session: &str,
        pane: &str,
    ) -> anyhow::Result<InputClient> {
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        let mut child = self
            .command()
            .arg("-C")
            .arg("attach")
            .arg("-f")
            .arg("no-output")
            .arg("-t")
            .arg(pane)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("spawning tmux input-control client")?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = Vec::with_capacity(8192);
        // Mirrors `open_replay_stream`'s handshake: the very first thing
        // this client's stdout carries is the reply to its own implicit
        // `attach-session`, which must be drained before any `send-keys`
        // reply can be read cleanly.
        read_command_block(
            &mut reader,
            &mut line,
            deadline,
            "input-client attach",
            pane,
        )
        .await?;
        Ok(InputClient {
            child,
            stdin,
            reader,
            line,
            pane: pane.to_string(),
            // Quoted for `replay_command_group`'s reason: the pairing
            // contains a `:`, and input must not be able to reach another
            // session's pane through a stale id any more than replay can.
            target: format!("\"{}\"", pane_in_session(session, pane)),
            delivered_any_bytes: false,
            exchange_timeout: self.exchange_timeout,
        })
    }
}

/// A SECOND control-mode client, dedicated to carrying input into one
/// pane, opened by [`TmuxDriver::open_input_client`].
///
/// This replaced two earlier designs, each broken in its own way.
/// `load-buffer -`/`paste-buffer -d -r` avoided hex-encoded input bytes on
/// a spawned process's argv (world-readable via `/proc/<pid>/cmdline`, and
/// input includes credentials typed at agent prompts — see SPEC_impl.md),
/// but paste-buffer caret-escapes control bytes on its way into the pane —
/// verified empirically against tmux 3.7b, DEL (0x7f) arrives as the two
/// literal characters `^?`, ESC as `^[`, ctrl-C as `^C` — so backspace,
/// arrow keys, and ctrl-C were all silently mangled. `send-keys -H`
/// delivers bytes verbatim and fixed that, but writing it to the shared
/// `OutputStream` client's stdin (the very next design) opened a
/// different hole: that call returned once `write_all`/`flush` proved the
/// OS pipe accepted the bytes, which is not the same as tmux having
/// executed them. Two failure modes hid behind that gap — an `%error`
/// reply had nowhere to go, because `OutputStream::next_output` discards
/// every notification it has no use for (command replies, layout-change
/// chatter, `%exit`), so a rejected `send-keys` vanished
/// silently instead of surfacing as dropped input; and a takeover could
/// kill the shared client after `send` returned `Ok` but before tmux
/// processed the buffered command, losing input that had already been
/// reported delivered.
///
/// A dedicated client closes both: nothing else ever writes to or reads
/// from it, so each `send-keys` command's `%begin`/`%end`/`%error` reply
/// can be read synchronously, in-line, via the same
/// `read_command_block` machinery `OutputStream` uses for its own
/// one-shot command group. `send` returning `Ok` now means tmux actually
/// executed the command — not merely that the pipe accepted the bytes.
/// The alternative — correlating replies on the shared output stream —
/// would need an actor owning that stream's stdout so a concurrent reader
/// could hand back the right reply; a second no-output client gets the
/// same synchronous request/reply property for free from machinery that
/// already exists.
///
/// Chunked at [`InputClient::MAX_CHUNK`] bytes per `send-keys` line
/// because tmux rejects a command carrying on the order of ~1000
/// arguments as "command too long", and each input byte becomes one hex
/// argument; 256 stays far below that ceiling with comfortable margin.
/// Chunks are written and their replies read strictly one at a time —
/// there is no pipelining — which is fine because nothing else ever
/// contends for this client's stdin/stdout: unlike `OutputStream`'s
/// former shared-stdin design, there is no concurrent-caller reordering
/// hazard here to guard against.
pub struct InputClient {
    /// Never read directly — `#[allow(dead_code)]` documents that this is
    /// deliberate, not an oversight. Kept alive purely so dropping this
    /// value (from any teardown path — takeover, detach, connection loss,
    /// or a failed `send`) kills the process via `kill_on_drop`, exactly
    /// like `OutputStream`'s client. Unlike `OutputStream`, there is no
    /// long-lived task driving this client's stdout in a loop, so
    /// `kill_on_drop` alone is enough; there is no
    /// cancelled-task-vs-clean-shutdown distinction to make, and so no
    /// explicit `shutdown` method that would read this field either.
    #[allow(dead_code)]
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
    line: Vec<u8>,
    /// The pane this client addresses, for the block reader's own
    /// filtering.
    pane: String,
    /// The quoted `=<session>:.<pane>` target every `send-keys` names —
    /// see [`pane_in_session`] for why input is session-paired rather than
    /// addressed by bare pane id.
    target: String,
    /// Whether tmux has ever CONFIRMED executing a `send-keys` carrying at
    /// least one byte through this client.
    ///
    /// Exists for PLAN_M3.md item 8's correlator: conversation capture
    /// anchors on the moment input actually reached the pane, because that
    /// is the last moment before the agent's record can exist. Confirmed
    /// delivery — a chunk whose `%end` came back — is the only reading that
    /// supports that, which is why this is set here rather than inferred
    /// from `send` returning `Ok`: a send that failed part-way still
    /// delivered the chunks it had confirmed, and one of those may have
    /// carried the prompt's newline.
    ///
    /// Deliberately never reset. It answers "has anything ever landed",
    /// and the correlator it feeds is itself write-once.
    delivered_any_bytes: bool,
    /// This client's copy of [`TmuxDriver::exchange_timeout`], consulted by
    /// [`Self::send`] for every chunk write, flush, and reply read. Stored
    /// rather than passed in per call because `send` computes a fresh
    /// per-reply deadline of its own well after this client was opened —
    /// the same reason `OutputStream` carries its own copy.
    exchange_timeout: std::time::Duration,
}

impl InputClient {
    /// Bytes of input per `send-keys -H` command line. See the struct
    /// docs for why this exists and why 256 was chosen.
    const MAX_CHUNK: usize = 256;

    /// Commands in flight before replies are drained. See `send` for why
    /// this bound is deadlock avoidance, not tuning: 64 replies ≈ 2 KiB,
    /// comfortably inside the ~64 KiB pipe capacity that an unbounded
    /// pipeline would wedge against.
    const PIPELINE_BATCH: usize = 64;

    /// The control client's process id, for lifecycle fault-injection tests.
    pub(crate) fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Deliver `bytes` to the pane as keystrokes, verbatim, confirming
    /// every chunk executed before returning.
    ///
    /// Returning `Ok` is the whole point of this type existing: it means
    /// tmux's control-mode protocol closed every chunk's command block
    /// with `%end`, not merely that the OS pipe accepted the writes. A
    /// `%error` reply becomes an `Err` here instead of vanishing the way
    /// it did on the old shared-stdin design.
    ///
    /// Pipelined in bounded batches, not lock-step: up to
    /// [`Self::PIPELINE_BATCH`] chunk commands are written and flushed,
    /// then that batch's replies are read in order, then the next batch.
    /// tmux replies to control-mode commands in submission order, so
    /// pairing the Nth reply with the Nth chunk needs no correlation
    /// beyond counting — and the guarantee is unchanged, since nothing
    /// returns until the final `%end`. Lock-step write/read was measured
    /// at ~70 KB/s on a multi-megabyte paste (one round trip per 256-byte
    /// chunk); batching removes most of that per-chunk latency.
    ///
    /// The batch bound is not an optimization knob, it is deadlock
    /// avoidance: writing every command before reading any reply lets an
    /// unbounded reply volume accumulate against the client's ~64 KiB
    /// stdout pipe. Once that fills, tmux stops relaying, backpressure
    /// reaches this side's writes, and both ends block until the timeout
    /// fires — a large paste would fail instead of being slow. One
    /// batch's replies (~30 bytes each) stay far below the pipe capacity.
    pub async fn send(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        if bytes.is_empty() {
            // Nothing reaches the pane, so nothing is delivered — see
            // `delivered_any_bytes`, whose whole point is that an empty
            // frame must not start conversation capture's clock.
            return Ok(());
        }
        let mut line = String::with_capacity(32 + Self::MAX_CHUNK * 3);
        let chunks: Vec<&[u8]> = bytes.chunks(Self::MAX_CHUNK).collect();
        for batch in chunks.chunks(Self::PIPELINE_BATCH) {
            for chunk in batch {
                line.clear();
                write!(line, "send-keys -t {} -H", self.target)
                    .expect("String write is infallible");
                for byte in *chunk {
                    write!(line, " {byte:02x}").expect("String write is infallible");
                }
                line.push('\n');
                tokio::time::timeout(self.exchange_timeout, self.stdin.write_all(line.as_bytes()))
                    .await
                    .context("timed out writing tmux send-keys command")?
                    .context("writing tmux send-keys command")?;
            }
            tokio::time::timeout(self.exchange_timeout, self.stdin.flush())
                .await
                .context("timed out flushing tmux send-keys commands")?
                .context("flushing tmux send-keys commands")?;
            for _ in 0..batch.len() {
                // One deadline per reply, not one for the whole batch: a
                // wedged tmux on one reply must not inherit the unspent
                // budget of every reply before it.
                let deadline = tokio::time::Instant::now() + self.exchange_timeout;
                read_command_block(
                    &mut self.reader,
                    &mut self.line,
                    deadline,
                    "send-keys input",
                    &self.pane,
                )
                .await?;
                // Marked per CONFIRMED chunk, not once at the end: a send
                // that fails on a later chunk has still delivered this
                // one, and the correlator cares about the earliest byte
                // that landed rather than about the call succeeding.
                self.delivered_any_bytes = true;
            }
        }
        Ok(())
    }

    /// Whether this attachment has ever had input confirmed into its pane.
    /// See [`InputClient::delivered_any_bytes`] for why conversation
    /// capture keys on this rather than on a successful `send`.
    pub fn delivered_any_bytes(&self) -> bool {
        self.delivered_any_bytes
    }
}
