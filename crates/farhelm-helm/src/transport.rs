//! Host transports are the seam between connection actors and local or SSH
//! connections.

use crate::store::{HostId, HostKind, HostRow};
use anyhow::Context as _;
use std::{future::Future, pin::Pin, process::Stdio};
use tokio::io::{AsyncRead, AsyncWrite};
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
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<TransportPair>> + Send + 'a>>;
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
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<TransportPair>> + Send + 'a>> {
        Box::pin(async move {
            match host.kind {
                HostKind::Local => {
                    let stream = farhelm_supervisor::service::connect(&self.state_dir)
                        .await
                        .map_err(classify_local_dial)?;
                    let (r, w) = tokio::io::split(stream);
                    Ok((
                        Box::new(r) as Box<dyn AsyncRead + Send + Unpin>,
                        Box::new(w) as Box<dyn AsyncWrite + Send + Unpin>,
                    ))
                }
                HostKind::Ssh => {
                    let dest = host.destination.as_deref().context(
                        "an ssh registry row has no destination; the schema's CHECK constraint \
                         should have made this impossible",
                    )?;
                    let control_path = self.state_dir.join("ssh-cm-%C");
                    let mut cmd = tokio::process::Command::new("ssh");
                    cmd.args(crate::ssh::ssh_stdio_args(
                        dest,
                        &control_path,
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
                    let mut child = cmd
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .kill_on_drop(true)
                        .spawn()
                        .context("spawning ssh")?;
                    let stdout = child.stdout.take().expect("piped stdout");
                    let stdin = child.stdin.take().expect("piped stdin");
                    let stderr = child.stderr.take().expect("piped stderr");
                    // Drained continuously rather than at exit: a pipe
                    // nobody reads fills up, and a full stderr blocks the
                    // ssh child's writes — which for a chatty remote means
                    // the transport wedges for reasons no log line would
                    // ever explain.
                    tokio::spawn(relay_ssh_stderr(host.id, dest.to_string(), stderr));
                    Ok((
                        Box::new(SshChannel {
                            stdout,
                            _child: child,
                        }) as Box<dyn AsyncRead + Send + Unpin>,
                        Box::new(stdin) as Box<dyn AsyncWrite + Send + Unpin>,
                    ))
                }
            }
        })
    }
}

/// An ssh exec channel's read half WITH the ssh child that produces it,
/// so the child's lifetime is the transport's lifetime and nothing else.
///
/// The child used to be parked in a detached `wait()` task instead, which
/// reaped it but did not own it: closing the pipes only asks ssh to exit,
/// and an ssh (or a remote proxy) that ignores EOF simply kept running —
/// past a cancelled attempt, past a torn-down actor, one survivor per
/// retry for a host that keeps failing late in the handshake. Holding the
/// `Child` here makes teardown structural: dropping the transport pair —
/// which is what a cancelled attempt, a lost connection, and a reconfigured
/// row all do — drops this value, and tokio's `kill_on_drop` both signals
/// the child and hands it to the runtime's orphan reaper, so there is
/// nothing left to leak and no zombie to collect by hand.
///
/// Only the read half is wrapped: the pair is created and dropped
/// together, so one anchor is enough, and the writer stays a plain
/// `ChildStdin` whose close is what asks ssh to exit politely first.
///
/// The child's STDERR is not here — it is piped and drained by a task of
/// its own (see [`relay_ssh_stderr`]), which ends when the child's stderr
/// closes, i.e. when the child this value owns exits.
struct SshChannel {
    stdout: tokio::process::ChildStdout,
    _child: tokio::process::Child,
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
/// Ends when the child's stderr closes, which is when the child exits;
/// there is no separate lifetime to manage, and the task cannot outlive
/// the transport that spawned it by more than the child's own teardown.
async fn relay_ssh_stderr(host: HostId, destination: String, stderr: tokio::process::ChildStderr) {
    let mut reader = tokio::io::BufReader::new(stderr);
    let mut relayed = 0usize;
    while let Some(line) = next_capped_line(&mut reader, SSH_STDERR_LINE_CAP).await {
        relayed += 1;
        // Past the budget the loop keeps READING and stops logging. It
        // must not stop reading: closing this pipe early makes the child's
        // next stderr write fail, which for ssh means the transport dies
        // because the remote was talkative — a far worse outcome than a
        // quiet log.
        match relayed.cmp(&SSH_STDERR_LINE_BUDGET) {
            std::cmp::Ordering::Greater => continue,
            std::cmp::Ordering::Equal => {
                warn!(
                    host,
                    destination = destination.as_str(),
                    budget = SSH_STDERR_LINE_BUDGET,
                    "the ssh child for this host has said enough; dropping the rest of its \
                     stderr for this connection"
                );
                continue;
            }
            std::cmp::Ordering::Less => {}
        }
        let text = String::from_utf8_lossy(&line.bytes);
        warn!(
            host,
            destination = destination.as_str(),
            truncated = line.truncated,
            // Debug-formatted deliberately; see this function's own docs.
            message = ?text,
            "ssh reported a problem for this host"
        );
    }
}

/// One capped line of a stderr stream: at most `cap` bytes of it, plus
/// whether more were thrown away.
struct CappedLine {
    bytes: Vec<u8>,
    truncated: bool,
}

/// Read up to the next newline, RETAINING at most `cap` bytes and
/// discarding the rest as it goes. `None` at end of stream.
///
/// The cap has to be applied while reading, not after. The obvious shape —
/// `BufReader::split(b'\n')` and truncate the segment — allocates the whole
/// segment first, so a remote that writes megabytes without a newline
/// (a binary blob on stderr, a wedged pager, a deliberate flood) makes this
/// process buy every byte of it before any limit is consulted. That is an
/// unbounded allocation driven entirely by the far end of an ssh channel,
/// which for a registered host is a machine the helm does not control.
///
/// `fill_buf`/`consume` is what makes the bound real: bytes past `cap` are
/// consumed and dropped without ever being retained, so an endless
/// newline-free stream costs the reader's fixed buffer and nothing more.
/// Draining rather than stopping is still the rule — see this function's
/// caller for why closing the pipe early would kill the transport.
async fn next_capped_line<R>(reader: &mut R, cap: usize) -> Option<CappedLine>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    use tokio::io::AsyncBufReadExt as _;

    let mut kept: Vec<u8> = Vec::new();
    let mut truncated = false;
    let mut saw_any = false;
    // A read ERROR ends the line the same way end-of-stream does: there is
    // nothing more to relay either way, and the caller's loop stops.
    while let Ok(available) = reader.fill_buf().await {
        if available.is_empty() {
            break;
        }
        saw_any = true;
        match available.iter().position(|byte| *byte == b'\n') {
            Some(newline) => {
                let take = cap.saturating_sub(kept.len()).min(newline);
                kept.extend_from_slice(&available[..take]);
                truncated |= newline > take;
                // The newline itself is consumed but never kept: callers
                // want the line's content, and a trailing newline in a log
                // field would break the line the field is rendered on.
                reader.consume(newline + 1);
                return Some(CappedLine {
                    bytes: kept,
                    truncated,
                });
            }
            None => {
                let take = cap.saturating_sub(kept.len()).min(available.len());
                kept.extend_from_slice(&available[..take]);
                truncated |= available.len() > take;
                let consumed = available.len();
                reader.consume(consumed);
            }
        }
    }
    // A final segment with no trailing newline is still a line; a stream
    // that ended with nothing pending is the end.
    saw_any.then_some(CappedLine {
        bytes: kept,
        truncated,
    })
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

    /// A stderr stream with no newline in it must cost a BOUNDED amount of
    /// memory, however much the far end writes.
    ///
    /// The shape this replaced buffered a whole newline-free segment before
    /// any cap applied, so a remote that wrote megabytes without a newline —
    /// a binary blob on stderr, a wedged pager, a deliberate flood — made
    /// this process allocate every byte of it first. That is unbounded
    /// allocation driven by the far end of an ssh channel, which for a
    /// registered host is a machine the helm does not control.
    ///
    /// Multi-megabyte rather than merely large: the point is that the size
    /// of the input does not appear in the size of what is retained at all.
    #[farhelm_testtrace::test]
    async fn a_newline_free_stderr_flood_stays_bounded() {
        let flood = vec![b'x'; 4 * 1024 * 1024];
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(flood));
        let line = super::next_capped_line(&mut reader, SSH_STDERR_LINE_CAP)
            .await
            .expect("a final segment with no newline is still a line");
        assert_eq!(
            line.bytes.len(),
            SSH_STDERR_LINE_CAP,
            "only the cap is retained, no matter how much arrived"
        );
        assert!(
            line.truncated,
            "and the caller is told the rest was dropped"
        );
    }

    /// Ordinary lines survive intact, the cap applies per LINE, and the
    /// stream ends when it ends.
    ///
    /// The bound above is only useful if the reader is otherwise a correct
    /// line reader: a version that truncated every line, or lost the last
    /// one, or never terminated, would satisfy the flood test and destroy
    /// the diagnostics the relay exists to carry.
    #[farhelm_testtrace::test]
    async fn capped_line_reading_keeps_short_lines_whole() {
        let long = "y".repeat(SSH_STDERR_LINE_CAP + 50);
        let input = format!("first\nsecond\n{long}\nlast-with-no-newline");
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(input.into_bytes()));

        let mut seen: Vec<(String, bool)> = Vec::new();
        while let Some(line) = super::next_capped_line(&mut reader, SSH_STDERR_LINE_CAP).await {
            seen.push((
                String::from_utf8_lossy(&line.bytes).into_owned(),
                line.truncated,
            ));
        }
        assert_eq!(seen.len(), 4, "every line, including the unterminated tail");
        assert_eq!(seen[0], ("first".to_string(), false));
        assert_eq!(seen[1], ("second".to_string(), false));
        assert_eq!(
            (seen[2].0.len(), seen[2].1),
            (SSH_STDERR_LINE_CAP, true),
            "the cap is per line, and truncation is reported per line"
        );
        assert_eq!(seen[3], ("last-with-no-newline".to_string(), false));
    }
}
