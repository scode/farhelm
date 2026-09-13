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
/// Printable ASCII uses one quoted `send-keys -l` argument instead: the
/// per-byte hex argument parsing is expensive enough to hold a large paste
/// ahead of the helm's control requests. Control bytes and non-ASCII data
/// retain `-H`, since literal-string key handling is not byte-transparent
/// for those values. Both forms preserve the same command boundaries.
/// Chunks are pipelined in bounded batches before their ordered replies are
/// read. Nothing else contends for this client's stdin/stdout, so reply order
/// is enough to keep each confirmation paired with its command; the bound
/// prevents tmux's replies from filling the client pipe while writes are
/// still in progress. See [`InputClient::send`] for the full contract.
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
    /// This client's copy of [`TmuxDriver::exchange_timeout`], used by
    /// [`Self::send`] to establish one deadline for the whole input frame.
    /// Stored rather than passed in per call because the timeout is selected
    /// when the client is opened — the same reason `OutputStream` carries its
    /// own copy.
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
    /// One whole call has one [`TmuxDriver::exchange_timeout`] budget shared
    /// by every write, flush, and reply read across all batches. This bounds
    /// how long the caller can hold the attachments lock: the shipped helm
    /// chunks input at 32 KiB, so ordinary frames fit with wide margin, while
    /// an oversized frame that cannot finish in one budget fails explicitly
    /// instead of extending that lock hold once per chunk.
    pub async fn send(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + self.exchange_timeout;
        send_input(
            bytes,
            SendInputContext {
                stdin: &mut self.stdin,
                reader: &mut self.reader,
                line: &mut self.line,
                target: &self.target,
                pane: &self.pane,
                deadline,
                delivered_any_bytes: &mut self.delivered_any_bytes,
            },
        )
        .await
    }
}

/// Send one frame through a dedicated control client under one absolute
/// deadline. The generic I/O parameters keep the timing contract testable
/// with a scripted duplex stream without changing the production client's
/// process ownership.
struct SendInputContext<'a, S, R> {
    stdin: &'a mut S,
    reader: &'a mut BufReader<R>,
    line: &'a mut Vec<u8>,
    target: &'a str,
    pane: &'a str,
    deadline: tokio::time::Instant,
    delivered_any_bytes: &'a mut bool,
}

async fn send_input<S, R>(bytes: &[u8], context: SendInputContext<'_, S, R>) -> anyhow::Result<()>
where
    S: tokio::io::AsyncWrite + Unpin,
    R: tokio::io::AsyncRead + Unpin,
{
    let SendInputContext {
        stdin,
        reader,
        line,
        target,
        pane,
        deadline,
        delivered_any_bytes,
    } = context;
    if bytes.is_empty() {
        // Nothing reaches the pane, so nothing is delivered — see
        // `delivered_any_bytes`, whose whole point is that an empty
        // frame must not start conversation capture's clock.
        return Ok(());
    }
    let mut command = String::with_capacity(32 + InputClient::MAX_CHUNK * 3);
    let chunks: Vec<&[u8]> = bytes.chunks(InputClient::MAX_CHUNK).collect();
    for (batch_index, batch) in chunks.chunks(InputClient::PIPELINE_BATCH).enumerate() {
        let batch_start = batch_index * InputClient::PIPELINE_BATCH;
        for (chunk_offset, chunk) in batch.iter().enumerate() {
            let chunk_index = batch_start + chunk_offset;
            command.clear();
            if chunk.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
                // Printable ASCII has the same pane bytes under -l and
                // -H. Keep it in one argument to avoid parsing hundreds
                // of separate hex arguments. This is tmux syntax, not
                // shell syntax: even a leading tilde in double quotes
                // expands unless escaped. Quoting does not stop option
                // parsing, so -- also protects chunks beginning with -
                // from being consumed as flags or overriding the target.
                // Payload never enters a process's argv.
                write!(command, "send-keys -t {} -l -- \"", target)
                    .expect("String write is infallible");
                for &byte in *chunk {
                    if matches!(byte, b'\\' | b'"' | b'$' | b'~') {
                        command.push('\\');
                    }
                    command.push(char::from(byte));
                }
                command.push('"');
            } else {
                write!(command, "send-keys -t {} -H", target).expect("String write is infallible");
                for byte in *chunk {
                    write!(command, " {byte:02x}").expect("String write is infallible");
                }
            }
            command.push('\n');
            tokio::time::timeout_at(deadline, stdin.write_all(command.as_bytes()))
                .await
                .map_err(|_| {
                    anyhow::anyhow!(
                        "input send exceeded its whole-call budget at chunk {chunk_index}"
                    )
                })?
                .context("writing tmux send-keys command")?;
        }
        tokio::time::timeout_at(deadline, stdin.flush())
            .await
            .map_err(|_| {
                anyhow::anyhow!("input send exceeded its whole-call budget at chunk {batch_start}")
            })?
            .context("flushing tmux send-keys commands")?;
        for (reply_offset, _) in batch.iter().enumerate() {
            // A wedged tmux on one reply no longer gets a fresh budget.
            // Sharing the deadline means the whole call, and therefore
            // the attachments lock held by its caller, cannot outlive
            // one exchange timeout.
            if let Err(error) =
                read_command_block(reader, line, deadline, "send-keys input", pane).await
            {
                if error
                    .downcast_ref::<tokio::time::error::Elapsed>()
                    .is_some()
                {
                    anyhow::bail!(
                        "input send exceeded its whole-call budget at chunk {}",
                        batch_start + reply_offset
                    );
                }
                return Err(error);
            }
            // Marked per CONFIRMED chunk, not once at the end: a send
            // that fails on a later chunk has still delivered this
            // one, and the correlator cares about the earliest byte
            // that landed rather than about the call succeeding.
            *delivered_any_bytes = true;
        }
    }
    Ok(())
}

impl InputClient {
    /// Whether this attachment has ever had input confirmed into its pane.
    /// See [`InputClient::delivered_any_bytes`] for why conversation
    /// capture keys on this rather than on a successful `send`.
    pub fn delivered_any_bytes(&self) -> bool {
        self.delivered_any_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::{InputClient, SendInputContext, send_input};
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, duplex, split};

    /// Answer each scripted command after a fixed virtual delay, modelling a
    /// tmux server that is slow but still answers every exchange.
    async fn scripted_tmux(
        mut reader: BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
        mut writer: tokio::io::WriteHalf<tokio::io::DuplexStream>,
        reply_delay: Duration,
    ) {
        let mut command = Vec::new();
        for id in 0..4 {
            command.clear();
            if reader.read_until(b'\n', &mut command).await.is_err() {
                return;
            }
            if !reply_delay.is_zero() {
                // sleep-ok: this is the scripted tmux response latency under the paused clock.
                tokio::time::sleep(reply_delay).await;
            }
            let reply = format!("%begin {id} 0 1\n%end {id} 0 1\n");
            if writer.write_all(reply.as_bytes()).await.is_err() {
                return;
            }
        }
    }

    /// A multi-chunk frame still succeeds when every scripted reply is
    /// immediate; the shared deadline must not change the normal batched
    /// confirmation path.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn send_confirms_a_fast_multi_chunk_frame() {
        let (client, server) = duplex(8192);
        let (client_reader, mut client_writer) = split(client);
        let (server_reader, server_writer) = split(server);
        let responder = tokio::spawn(scripted_tmux(
            BufReader::new(server_reader),
            server_writer,
            Duration::ZERO,
        ));
        let mut reader = BufReader::new(client_reader);
        let mut line = Vec::new();
        let mut delivered = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        send_input(
            &vec![b'x'; InputClient::MAX_CHUNK * 4],
            SendInputContext {
                stdin: &mut client_writer,
                reader: &mut reader,
                line: &mut line,
                target: "\"session:.0\"",
                pane: "%0",
                deadline,
                delivered_any_bytes: &mut delivered,
            },
        )
        .await
        .expect("fast scripted replies should confirm every chunk");
        assert!(delivered);
        responder.await.expect("scripted tmux task must not panic");
    }

    /// Four replies that each arrive just before a one-second exchange
    /// timeout must not buy four seconds of attachment-lock ownership. The
    /// error identifies exhaustion of the frame budget rather than a tmux
    /// command failure.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn send_bounds_slow_multi_chunk_frame_to_one_budget() {
        let (client, server) = duplex(8192);
        let (client_reader, mut client_writer) = split(client);
        let (server_reader, server_writer) = split(server);
        let responder = tokio::spawn(scripted_tmux(
            BufReader::new(server_reader),
            server_writer,
            Duration::from_millis(900),
        ));
        let sender = tokio::spawn(async move {
            let mut reader = BufReader::new(client_reader);
            let mut line = Vec::new();
            let mut delivered = false;
            let start = tokio::time::Instant::now();
            let result = send_input(
                &vec![b'x'; InputClient::MAX_CHUNK * 4],
                SendInputContext {
                    stdin: &mut client_writer,
                    reader: &mut reader,
                    line: &mut line,
                    target: "\"session:.0\"",
                    pane: "%0",
                    deadline: start + Duration::from_secs(1),
                    delivered_any_bytes: &mut delivered,
                },
            )
            .await;
            (start, result, delivered)
        });

        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(Duration::from_millis(900)).await;
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;

        let (start, result, delivered) = sender.await.expect("send task must not panic");
        let error = result.expect_err("the whole-call budget must expire");
        assert!(
            error
                .to_string()
                .contains("input send exceeded its whole-call budget"),
            "unexpected timeout error: {error:#}"
        );
        assert!(error.to_string().contains("chunk 1"));
        assert!(delivered);
        assert_eq!(
            tokio::time::Instant::now().duration_since(start),
            Duration::from_secs(1)
        );
        responder.abort();
    }
}
