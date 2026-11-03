//! The supervisor's connection to one terminal host: what connecting finds
//! at a socket, and the requests a host terminal's runtime drives.
//!
//! The wire format is the host crate's own codec (`terminal_host::protocol`),
//! and `crates/terminal-host/SPEC.md` is the contract this follows. What the
//! supervisor adds on top of the contract is decided here:
//!
//! - **Outcome classes at connect** ([`ConnectError`]). A missing socket
//!   file means the host is gone. A refused connection means gone only once
//!   refusals have lasted [`REFUSAL_WINDOW`] without a break: on Linux a
//!   refusal means nothing listens, but on macOS a live host whose listen
//!   backlog is full refuses too. A version-1 host never answers a
//!   take-over `Hello` with `Busy`; should one, it is retried until the
//!   deadline and never means gone, since a busy host is alive. Everything
//!   else that answers but cannot be used (a protocol version this
//!   supervisor does not speak, a malformed frame, an error before
//!   `Welcome`, no answer by the deadline) is unavailable: the host may be
//!   running, so it is never treated as an orphan, and only
//!   [`super::process::terminate_verified`] may end it.
//! - **Never gone while a host process serves the socket.** A host that is
//!   stopped or wedged does not accept, so every connection that times out
//!   against it stays queued in its listen backlog (64 deep). Once that is
//!   full, Linux answers new connections with `EAGAIN` and macOS refuses
//!   them, as if nothing listened, and neither names a peer. So before a
//!   connect says gone, or unavailable without a peer, it looks for a live
//!   process of this user whose command line ends with `--socket <that
//!   path>` ([`super::process::socket_process`]); if there is one, the
//!   result is unavailable and names it. A caller that meets an unavailable
//!   host should end it with that peer rather than keep reconnecting: each
//!   attempt adds to the backlog.
//! - **Always take over** (the plan's D-11). Every `Hello` sets `take_over`:
//!   the supervisor holds its state directory's lock, so any other client a
//!   host has is a stale connection of an earlier supervisor (or of a
//!   cleanup tool acting for it), and refusing would lock a restarted
//!   supervisor out of its own terminals until that connection died. The
//!   same holds within one supervisor: a second connection to a host cuts
//!   off the first without notice, ending its stream and leaving its
//!   in-flight input unknown. So each host has exactly one owning
//!   connection inside a supervisor, every request goes through it, and an
//!   owner that sees [`ClientError::Closed`] must allow that someone took
//!   the host over.
//! - **The peer process** is read from the connection's credentials before
//!   anything is sent, and kept with its start identity
//!   ([`HostConnection::peer`], and in [`ConnectError`]), so a host that
//!   cannot be ended through the protocol can still be ended safely.
//! - **One stream per connection.** The host allows one `Read` per
//!   connection; [`HostConnection::start_read`] refuses a second locally.
//!   Replies to `Info` and `Snapshot` arrive in the same byte stream as the
//!   records, so the request methods keep the records they read while
//!   waiting and hand them out through [`HostConnection::next_event`] in
//!   order.
//! - **Bounded unacknowledged input** ([`INPUT_WINDOW`]). The host holds at
//!   most `MAX_PENDING_INPUT` of input and then stops reading the client,
//!   which would let a program that stops reading its terminal wedge the
//!   connection's snapshots, resizes and shutdown behind its input. The
//!   window keeps far below that, so the host always keeps reading control
//!   messages. Input is never resent: when a connection ends, its
//!   [`InputCounts`] say what is known written, known discarded, and
//!   unknown, and the unknown part is the caller's to report, not to retry
//!   (some of it may have reached the program).
//!
//! Every method is cancel-safe at the frame level: bytes read are kept in the
//! connection's buffer, and a frame queued for sending stays queued until a
//! later call writes the rest of it, so dropping a future (a `select!` arm
//! losing, a timeout) never leaves half a frame on the wire.
//!
//! Every request first writes out what an earlier, dropped request left
//! queued, and only then queues its own frame. So the queue never holds more
//! than one request's frame, however often requests are dropped against a
//! host that stopped reading, and each frame's write deadline starts when
//! that frame is queued, not when an older one was. It also decides what a
//! dropped request did:
//!
//! - **Dropped after its frame was queued:** it was sent. Its frame goes out
//!   with whatever call comes next, and its reply, if it has one, is
//!   recognized and skipped later.
//! - **Dropped before its frame was queued:** it was not sent, and left
//!   nothing behind (no request number, no input window cost, no read
//!   stream). That is a request dropped before it was first polled (these
//!   are `async fn`s, which do nothing until polled, so a `select!` arm that
//!   loses before its turn sends nothing), or one dropped while it was
//!   still writing out an earlier frame. The caller re-issues it if it still
//!   wants it; a dropped [`HostConnection::resize`] in particular must be
//!   re-issued, or the host keeps the old size.
//!
//! A request queues its frame on its first poll when nothing is waiting to
//! be written, and only once the leftover is out otherwise
//! ([`HostConnection::write_pending`] says which, but only about bytes
//! already queued: it cannot tell whether a dropped request was ever
//! polled). So a caller that drops requests either awaits the ones it
//! cannot afford to lose outside a droppable `select!` arm, or checks after
//! every cancellation: for input, [`HostConnection::input_counts`]'s `sent`
//! grows only when an `Input` was queued, so an unchanged `sent` means not
//! sent; a cancelled resize is simply re-issued, since a repeated resize is
//! harmless.
//!
//! Waits for a reply, for a queued frame to be written and for `Shutdown`
//! are bounded by [`ClientDeadlines`]; running out of one ends the
//! connection ([`ClientError::Timeout`]), since a host that does not answer
//! or does not read is treated as wedged. A queued frame keeps its deadline
//! when the request that queued it is dropped, so a later
//! [`HostConnection::next_event`] still gives up on it. Waiting for events
//! with nothing left to write is unbounded: a quiet program sends nothing,
//! and that is not a failure.
//!
//! The owner must keep reading events. Every request reads the socket while
//! it waits and keeps what arrives for [`HostConnection::next_event`], up to
//! [`EVENT_QUEUE_LIMIT`]; past that the connection ends
//! ([`ClientError::EventQueueFull`]) rather than grow the supervisor without
//! bound or drop terminal output. And a host whose client stops reading
//! altogether stops acting on its requests once its own outgoing queue
//! fills, so the next request times out.

use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use terminal_host::protocol::{
    ChildExit, ClientMessage, ErrorCode, HostMessage, INPUT_COST, Info, MAX_ERROR_MESSAGE,
    MAX_FRAME, MAX_INPUT, MAX_PENDING_INPUT, OUTGOING_HIGH_WATER, PROTOCOL_VERSION, Position, Size,
    SnapshotRejection, decode_host, encode_client, split_frame,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixStream;
use tokio::time::Instant;

use super::process::{HostProcess, peer_process, socket_process};

// ---- Limits and deadlines ------------------------------------------------------

/// How long a connect may take, retries included. A healthy host answers
/// `Hello` at once; this covers a loaded machine and the refusal window.
pub const CONNECT_DEADLINE: Duration = Duration::from_secs(5);

/// How long a socket must refuse every connection, without anything else in
/// between, before it counts as gone (left by a host that was killed). The
/// goal's "repeated attempts over a few seconds", matching the test-state
/// cleanup's window (`farhelm_teststate::hosts::STALE_REFUSAL_WINDOW`).
pub const REFUSAL_WINDOW: Duration = Duration::from_secs(3);

/// Pause between connection attempts.
pub const RETRY_INTERVAL: Duration = Duration::from_millis(100);

/// How long a request waits for its reply, or for its frame to be written.
/// The host acts on `Info`, `Snapshot`, `Resize` and `Shutdown` as they
/// arrive, never behind input, so this only has to cover a loaded machine
/// and the largest snapshot blob.
///
/// Each frame has this much from when it is queued, and each reply this
/// much from then. A request that first writes out an earlier, dropped
/// request's frame waits for that under the earlier frame's own deadline,
/// so one call can take up to about twice this in all. `shutdown` is the
/// exception: its whole call is bounded by [`SHUTDOWN_DEADLINE`].
pub const REPLY_DEADLINE: Duration = Duration::from_secs(10);

/// How long `shutdown` takes at most, from the call: writing out what an
/// earlier, dropped request left queued, writing `Shutdown`, and waiting
/// for `Goodbye` or the end of the stream. The host ends as soon as its
/// `Goodbye` is written, and gives up writing it after one second
/// (`GOODBYE_FLUSH`). Bounded as a whole, unlike other requests, because it
/// is the step Delete and orderly stops wait on before falling back to a
/// signal.
pub const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(5);

/// The most unacknowledged input a connection has in flight, counted the way
/// the host counts its pending input: each `Input`'s bytes plus
/// `INPUT_COST`. A quarter of the host's `MAX_PENDING_INPUT`, so the host
/// never reaches the bound at which it stops reading the client, and still
/// room for three maximum-size `Input`s, so a paste is never held to one at
/// a time.
pub const INPUT_WINDOW: usize = 256 * 1024;

const _: () = assert!(INPUT_WINDOW * 4 <= MAX_PENDING_INPUT);
const _: () = assert!(3 * (MAX_INPUT + INPUT_COST) <= INPUT_WINDOW);

/// The most a connection keeps of events read and not yet taken by
/// `next_event`, counted as their data plus [`EVENT_COST`] each. While a
/// request waits for its reply, what arrives first is at most what the
/// host had queued for the client when it acted on the request (its
/// `OUTGOING_HIGH_WATER` plus one frame, a snapshot record of up to
/// `MAX_SNAPSHOT`), plus what the socket holds; this leaves room for that
/// twice over, so only an owner that stopped reading events reaches it.
pub const EVENT_QUEUE_LIMIT: usize = 2 * MAX_FRAME + 2 * OUTGOING_HIGH_WATER;

/// What each queued event costs against [`EVENT_QUEUE_LIMIT`] beyond its
/// data, so that a flood of tiny events is bounded too.
pub const EVENT_COST: usize = 64;

/// How much room each read from the socket makes in the receive buffer.
const READ_CHUNK: usize = 64 * 1024;

/// The longest frame a host may send before `Welcome`: an `Error` with the
/// longest message. Anything longer at that point is malformed, so a peer
/// that is not a host cannot make a connect buffer much.
const MAX_BEFORE_WELCOME: usize = 1 + 2 + 4 + MAX_ERROR_MESSAGE;

/// The time limits a connection works within. [`Default`] gives the
/// constants above; tests shorten them to reach each outcome in moments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientDeadlines {
    /// See [`CONNECT_DEADLINE`].
    pub connect: Duration,
    /// See [`REFUSAL_WINDOW`].
    pub refusal_window: Duration,
    /// See [`RETRY_INTERVAL`].
    pub retry_interval: Duration,
    /// See [`REPLY_DEADLINE`].
    pub reply: Duration,
    /// See [`SHUTDOWN_DEADLINE`].
    pub shutdown: Duration,
}

impl Default for ClientDeadlines {
    fn default() -> Self {
        Self {
            connect: CONNECT_DEADLINE,
            refusal_window: REFUSAL_WINDOW,
            retry_interval: RETRY_INTERVAL,
            reply: REPLY_DEADLINE,
            shutdown: SHUTDOWN_DEADLINE,
        }
    }
}

// ---- Outcomes ------------------------------------------------------------------

/// What a connect found instead of a host this supervisor can use.
///
/// The classes decide what callers may do: only [`Self::Gone`] lets a
/// socket be removed or a terminal be recorded as ended; [`Self::Busy`] and
/// [`Self::Unavailable`] mean a host may be running, which is left alone
/// (or, when it must end, ended through
/// [`super::process::terminate_verified`] with the `peer` given here).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectError {
    /// Nothing listens there.
    #[error("no terminal host: {0}")]
    Gone(Gone),
    /// The host answered `Busy` until the deadline: it is alive and
    /// serving another client. A `Hello` with take-over never gets this
    /// from a version-1 host; it is kept as a class so that it can never be
    /// read as gone.
    #[error("the terminal host stayed busy")]
    Busy { peer: Option<HostProcess> },
    /// Something answers that this supervisor cannot use.
    #[error("the terminal host is unavailable: {reason}")]
    Unavailable {
        reason: Unavailable,
        /// The listening process, when the connection got that far.
        peer: Option<HostProcess>,
    },
}

/// Why a socket counts as gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Gone {
    /// The socket file does not exist (a host removes its own as it ends).
    #[error("the socket file is missing")]
    NoSocket,
    /// The socket refused every connection for the refusal window: a host
    /// killed without the chance to remove its file.
    #[error("the socket refused every connection")]
    Refused,
}

/// Why a host that answers cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unavailable {
    /// `Welcome` named a version this supervisor does not speak.
    #[error("it speaks protocol version {0}")]
    UnsupportedVersion(u32),
    /// The host answered `Hello` with `Error`.
    #[error("it refused the connection ({code:?}): {message}")]
    Refused { code: ErrorCode, message: String },
    /// The answer broke the protocol.
    #[error("it sent a malformed answer: {0}")]
    Malformed(String),
    /// No answer, or no connection, by the deadline (a host that is
    /// stopped, wedged, or has a full listen backlog).
    #[error("it did not answer in time")]
    Timeout,
    /// The connection kept closing before `Welcome` (a host that is ending,
    /// or one that refuses this user).
    #[error("it closed the connection before answering")]
    Closed,
    /// The socket takes no connection (its file is missing, or it refuses
    /// or turns away every attempt), but a live process of this user still
    /// serves it by its command line: a host whose backlog filled while it
    /// was stopped or wedged, or whose socket file was removed under it.
    /// The peer is that process.
    #[error("it no longer takes connections, but its process still runs")]
    NotAccepting,
    /// Connecting failed in a way that says nothing about a host, or the
    /// process table could not be searched for one before judging the
    /// socket gone.
    #[error("connecting failed: {0}")]
    Io(String),
}

/// Why a request failed. Every variant but [`Self::ReadAlreadyStarted`],
/// [`Self::InvalidRequest`] and [`Self::InputWindowFull`], which are refused
/// before anything is sent, ends the connection: later calls return the
/// same error, and the caller reconnects.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClientError {
    /// The host closed the connection: it ended, or another client took it
    /// over (which the host does not announce).
    #[error("the terminal host closed the connection")]
    Closed,
    /// The host sent `Error` and closed the connection.
    #[error("the terminal host refused a request ({code:?}): {message}")]
    Host { code: ErrorCode, message: String },
    /// The host sent something the protocol does not allow.
    #[error("the terminal host broke the protocol: {0}")]
    Malformed(String),
    /// No reply, or no room to write, within the deadline.
    #[error("the terminal host did not answer in time")]
    Timeout,
    /// The socket failed.
    #[error("the connection to the terminal host failed: {0}")]
    Io(String),
    /// A second `Read` on one connection. The host allows one; a client
    /// that wants to start over reconnects.
    #[error("this connection already reads the terminal's stream")]
    ReadAlreadyStarted,
    /// The owner left more than [`EVENT_QUEUE_LIMIT`] of events unread.
    #[error("more than EVENT_QUEUE_LIMIT of events went unread")]
    EventQueueFull,
    /// A request the protocol cannot carry (empty or oversized input, a
    /// size out of range, a snapshot larger than a frame).
    #[error("invalid request: {0}")]
    InvalidRequest(&'static str),
    /// The input does not fit in what is left of [`INPUT_WINDOW`]. Nothing
    /// was sent; retry once acknowledgements have made `needed` bytes of
    /// room.
    #[error("{needed} bytes of input do not fit the {room} left in the window")]
    InputWindowFull { needed: usize, room: usize },
}

impl ClientError {
    /// Whether this error ends the connection.
    fn is_fatal(&self) -> bool {
        !matches!(
            self,
            Self::ReadAlreadyStarted | Self::InvalidRequest(_) | Self::InputWindowFull { .. }
        )
    }
}

/// What became of the input sent on one connection, in bytes. Counts are
/// per connection and start at zero on each.
///
/// `written` and `discarded` come from the host's acknowledgements: written
/// reached the pty, discarded did not (the terminal had no reader, or a
/// write failed). The rest of `sent` is [`Self::unknown`]: some of it may
/// have been written. Once the connection has ended that is final, and the
/// caller must not resend it: a paste or a submitted line would arrive
/// twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputCounts {
    pub sent: u64,
    pub written: u64,
    pub discarded: u64,
}

impl InputCounts {
    /// Bytes sent whose fate the host has not reported.
    pub fn unknown(&self) -> u64 {
        self.sent - self.written - self.discarded
    }

    /// Whether the `Input` that `ticket` stands for has been resolved
    /// (written or discarded). Inputs resolve in the order they were sent.
    pub fn resolved(&self, ticket: InputTicket) -> bool {
        self.written + self.discarded >= ticket.end
    }
}

/// One sent `Input`: where it ends in the connection's input, counting all
/// bytes sent before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputTicket {
    pub end: u64,
}

/// Something the host sent on its own, in the order it was sent: the
/// records of the stream started by [`HostConnection::start_read`], and
/// input acknowledgements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEvent {
    /// The stored snapshot, sent when the read position could not be
    /// continued from (see `crates/terminal-host/SPEC.md`, "Reading").
    Snapshot { position: Position, blob: Vec<u8> },
    /// Output at `offset` in the output stream.
    Output { offset: u64, data: Vec<u8> },
    /// The terminal was resized at `offset`; this is resize number `index`.
    Resized { offset: u64, index: u64, size: Size },
    /// Output from `from` to `to` was trimmed; records resume at `to`, with
    /// `resizes` resizes applied and `size` in force.
    Gap {
        from: u64,
        to: u64,
        resizes: u64,
        size: Size,
    },
    /// The child exited, after the output before `offset`.
    Exited { offset: u64, exit: ChildExit },
    /// The host resolved more input; the counts are the connection's
    /// totals now.
    InputAcknowledged(InputCounts),
}

/// The host's answer to a `Snapshot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotReply {
    Accepted(Position),
    Rejected {
        position: Position,
        reason: SnapshotRejection,
    },
}

/// How a `Shutdown` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownOutcome {
    /// Whether the host acknowledged the `Shutdown` with `Goodbye`. When it
    /// did not, the `Shutdown` was written whole but the connection then
    /// ended without an answer: the host may be ending (one that cannot
    /// write `Goodbye` in time ends anyway), or the connection may have
    /// been cut before it read the request. That is not confirmed; the
    /// caller checks the peer process or the socket file before counting
    /// the host as ended.
    pub goodbye: bool,
    /// The connection's final input counts. The host discards pending input
    /// at `Shutdown` without acknowledging it, so what was unacknowledged
    /// stays unknown.
    pub input: InputCounts,
}

// ---- The connection ------------------------------------------------------------

/// A connection to one terminal host, past `Welcome`.
///
/// One owner drives it (the methods take `&mut self`), which is the shape a
/// per-host runtime task has: it `select!`s over [`Self::next_event`] and
/// its own work, and makes requests in between. Dropping it closes the
/// connection; the host then discards whatever input of it was not yet
/// written, so [`Self::input_counts`] is worth reading first.
pub struct HostConnection {
    stream: UnixStream,
    socket: PathBuf,
    peer: Option<HostProcess>,
    deadlines: ClientDeadlines,
    /// Bytes read and not yet decoded.
    inbound: Vec<u8>,
    /// Frames queued and not yet written; their first bytes may already be.
    outbound: Vec<u8>,
    /// When the frame in `outbound` must be written by. A frame is only ever
    /// queued into an empty `outbound` (every request writes out what is
    /// left first), so this is that frame's own deadline: set when it is
    /// queued, cleared once it is written, and kept when the request that
    /// queued it is dropped.
    write_deadline: Option<Instant>,
    /// Events read and not yet handed out by `next_event`.
    events: VecDeque<HostEvent>,
    /// What `events` costs against [`EVENT_QUEUE_LIMIT`].
    queued: usize,
    /// Whether `Read` was sent.
    reading: bool,
    /// Whether `Shutdown` was sent.
    shutting_down: bool,
    /// Whether `Goodbye` arrived.
    goodbye: bool,
    /// Requests sent and replies received, per kind, so a reply can be
    /// matched to its request after an earlier request's future was
    /// dropped.
    info: Replies<Info>,
    snapshots: Replies<SnapshotReply>,
    input: InputWindow,
    /// The error that ended the connection.
    ended: Option<ClientError>,
}

impl std::fmt::Debug for HostConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostConnection")
            .field("socket", &self.socket)
            .field("peer", &self.peer)
            .field("reading", &self.reading)
            .field("input", &self.input.counts())
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

/// Replies to one kind of request, matched by order: the host answers each
/// request once, in the order they were sent.
#[derive(Debug)]
struct Replies<T> {
    sent: u64,
    received: u64,
    /// The latest reply and its request's number.
    last: Option<(u64, T)>,
}

impl<T> Default for Replies<T> {
    fn default() -> Self {
        Self {
            sent: 0,
            received: 0,
            last: None,
        }
    }
}

impl<T> Replies<T> {
    /// Number a request about to be sent.
    fn next(&mut self) -> u64 {
        self.sent += 1;
        self.sent - 1
    }

    fn receive(&mut self, reply: T) -> Result<(), ClientError> {
        if self.received >= self.sent {
            return Err(ClientError::Malformed("a reply to no request".into()));
        }
        self.last = Some((self.received, reply));
        self.received += 1;
        Ok(())
    }

    fn has(&self, request: u64) -> bool {
        matches!(self.last, Some((number, _)) if number == request)
    }
}

/// The unacknowledged input in flight: the [`INPUT_WINDOW`] accounting and
/// the counts the host's acknowledgements give.
#[derive(Debug, Default)]
struct InputWindow {
    counts: InputCounts,
    /// Each unresolved `Input`'s end and its cost against the window, in
    /// the order sent.
    unresolved: VecDeque<(u64, usize)>,
    /// The sum of the unresolved costs.
    cost: usize,
}

impl InputWindow {
    fn counts(&self) -> InputCounts {
        self.counts
    }

    fn room(&self) -> usize {
        INPUT_WINDOW.saturating_sub(self.cost)
    }

    /// Count `len` bytes as sent, if they fit the window.
    fn admit(&mut self, len: usize) -> Result<InputTicket, ClientError> {
        let needed = len + INPUT_COST;
        let room = self.room();
        if needed > room {
            return Err(ClientError::InputWindowFull { needed, room });
        }
        self.counts.sent += len as u64;
        self.unresolved.push_back((self.counts.sent, needed));
        self.cost += needed;
        Ok(InputTicket {
            end: self.counts.sent,
        })
    }

    /// Apply an acknowledgement's running totals. Each one resolves whole
    /// `Input`s, in order, and neither total goes down
    /// (`crates/terminal-host/SPEC.md`, "Input"); anything else is the
    /// host breaking the protocol.
    fn acknowledge(&mut self, written: u64, discarded: u64) -> Result<(), ClientError> {
        let broken = |what: &str| {
            Err(ClientError::Malformed(format!(
                "input acknowledgement {what}"
            )))
        };
        if written < self.counts.written || discarded < self.counts.discarded {
            return broken("went backwards");
        }
        let Some(resolved) = written.checked_add(discarded) else {
            return broken("overflowed");
        };
        if resolved > self.counts.sent {
            return broken("covers more input than was sent");
        }
        let mut boundary = self.counts.written + self.counts.discarded;
        while let Some(&(end, cost)) = self.unresolved.front()
            && end <= resolved
        {
            boundary = end;
            self.cost -= cost;
            self.unresolved.pop_front();
        }
        if boundary != resolved {
            return broken("ends inside an Input");
        }
        self.counts.written = written;
        self.counts.discarded = discarded;
        Ok(())
    }
}

/// What one step of reading or writing the socket did.
enum Step {
    Read(io::Result<usize>),
    Wrote(io::Result<usize>),
}

impl HostConnection {
    /// Connect to the host at `socket` with the default deadlines.
    pub async fn connect(socket: &Path) -> Result<Self, ConnectError> {
        Self::connect_with(socket, ClientDeadlines::default()).await
    }

    /// Connect to the host at `socket`, taking over from any client it has,
    /// and return once it has said `Welcome`.
    ///
    /// Retries refusals (until they have lasted the refusal window, which
    /// may run past the connect deadline so that a socket that only ever
    /// refuses is always judged), `Busy`, a full listen backlog and a
    /// connection closed before any answer, until the connect deadline.
    /// See [`ConnectError`] for what each failure means.
    pub async fn connect_with(
        socket: &Path,
        deadlines: ClientDeadlines,
    ) -> Result<Self, ConnectError> {
        match Self::connect_attempts(socket, deadlines).await {
            Ok(connection) => Ok(connection),
            Err(error) => Err(confirm_with_process(socket, error).await),
        }
    }

    /// The retries behind [`Self::connect_with`], before a gone or
    /// peer-less outcome is checked against the process table.
    async fn connect_attempts(
        socket: &Path,
        deadlines: ClientDeadlines,
    ) -> Result<Self, ConnectError> {
        let deadline = Instant::now() + deadlines.connect;
        let mut refusing_since: Option<Instant> = None;
        // What the latest retried attempt found, reported if the deadline
        // ends the retries.
        let mut last = None;
        loop {
            let found = attempt(socket, deadline).await;
            // A retry that the deadline cut short says nothing new about
            // the host: a busy host whose last attempt timed out is still
            // busy, not unresponsive. Only the pid it may have learned is
            // worth keeping.
            if let Attempt::Retry(error) | Attempt::Final(error) = &found
                && matches!(
                    error,
                    ConnectError::Unavailable {
                        reason: Unavailable::Timeout,
                        ..
                    }
                )
                && Instant::now() >= deadline
                && let Some(earlier) = last.take()
            {
                return Err(with_peer(earlier, peer_of(error)));
            }
            match found {
                Attempt::Welcomed(stream, peer, inbound) => {
                    return Ok(Self {
                        stream,
                        socket: socket.to_path_buf(),
                        peer,
                        deadlines,
                        inbound,
                        outbound: Vec::new(),
                        write_deadline: None,
                        events: VecDeque::new(),
                        queued: 0,
                        reading: false,
                        shutting_down: false,
                        goodbye: false,
                        info: Replies::default(),
                        snapshots: Replies::default(),
                        input: InputWindow::default(),
                        ended: None,
                    });
                }
                Attempt::Final(error) => return Err(error),
                Attempt::Refused => {
                    let since = *refusing_since.get_or_insert_with(Instant::now);
                    if since.elapsed() >= deadlines.refusal_window {
                        return Err(ConnectError::Gone(Gone::Refused));
                    }
                }
                Attempt::Retry(error) => {
                    refusing_since = None;
                    last = Some(error);
                }
            }
            // A run of refusals is always followed until it is judged, even
            // past the deadline (by at most the refusal window), so that a
            // socket that only refuses always comes out gone. Anything else
            // stops at the deadline with the last thing it saw.
            if refusing_since.is_none() && Instant::now() + deadlines.retry_interval > deadline {
                return Err(last.unwrap_or(ConnectError::Unavailable {
                    reason: Unavailable::Timeout,
                    peer: None,
                }));
            }
            tokio::time::sleep(deadlines.retry_interval).await;
        }
    }

    /// The socket this connection was made to.
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// The host process, as its connection's peer credentials and start
    /// identity named it at connect. `None` when the platform did not say
    /// or the process could not be read.
    pub fn peer(&self) -> Option<HostProcess> {
        self.peer
    }

    /// The protocol version in use. Always 1: the only one this supervisor
    /// speaks.
    pub fn version(&self) -> u32 {
        PROTOCOL_VERSION
    }

    /// This connection's input counts so far.
    pub fn input_counts(&self) -> InputCounts {
        self.input.counts()
    }

    /// How much of [`INPUT_WINDOW`] is free, in the window's units (input
    /// bytes plus `INPUT_COST` per `Input`).
    pub fn input_room(&self) -> usize {
        self.input.room()
    }

    /// The error that ended this connection, if one has.
    pub fn ended(&self) -> Option<&ClientError> {
        self.ended.as_ref()
    }

    /// Whether part of an earlier, dropped request's frame is still waiting
    /// to be written: queued bytes, nothing more. While it is, a request
    /// made now writes that out first, and dropped in the meantime it is
    /// not sent; while it is not, a request queues its frame on its first
    /// poll. Either way a request dropped before its first poll is not
    /// sent, which this cannot show, so it is no proof that a cancelled
    /// request was sent (see the module docs for how a caller tells).
    pub fn write_pending(&self) -> bool {
        !self.outbound.is_empty()
    }

    /// Ask for the host's `Info` and wait for it.
    ///
    /// Stream records and acknowledgements that arrive first are kept for
    /// [`Self::next_event`]. The reply's values are the host's state when it
    /// acted on the request, so records still to come can lie below its
    /// `end`.
    pub async fn info(&mut self) -> Result<Info, ClientError> {
        self.check_ended()?;
        self.flush().await?;
        let request = self.info.next();
        self.queue(&ClientMessage::Info);
        let deadline = Instant::now() + self.deadlines.reply;
        self.drive(|c| c.info.has(request), Some(deadline)).await?;
        let (_, info) = self.info.last.take().expect("the reply just arrived");
        Ok(info)
    }

    /// Start the connection's one stream, from `from`: the position the
    /// caller has already processed, or `(0, 0)` for everything. The
    /// records arrive through [`Self::next_event`].
    pub async fn start_read(&mut self, from: Position) -> Result<(), ClientError> {
        self.check_ended()?;
        if self.reading {
            return Err(ClientError::ReadAlreadyStarted);
        }
        self.flush().await?;
        self.reading = true;
        self.queue(&ClientMessage::Read(from));
        self.flush().await
    }

    /// The next stream record or input acknowledgement, waiting as long as
    /// it takes. Cancel-safe.
    ///
    /// Events read before the connection ended are all handed out before
    /// the error that ended it, including those decoded from the same read
    /// as the frame that ended it. A frame still waiting to be written keeps
    /// its deadline here too, so a host that stopped reading cannot hold
    /// this forever; with nothing to write, the wait has no bound.
    pub async fn next_event(&mut self) -> Result<HostEvent, ClientError> {
        if let Some(event) = self.take_event() {
            return Ok(event);
        }
        let driven = self.drive(|c| !c.events.is_empty(), None).await;
        match self.take_event() {
            Some(event) => Ok(event),
            None => Err(driven.expect_err("drive returns once an event is queued")),
        }
    }

    /// Send `data` as one `Input`, if it fits the window.
    ///
    /// `data` is 1 to `MAX_INPUT` bytes. One of at most `ATOMIC_INPUT` bytes
    /// (a key's whole encoding, such as Shift+Enter's ESC CR) is offered to
    /// the pty in one write; a longer one may be split by the host, so a
    /// caller must not split a key across two calls. When the window is
    /// full, [`ClientError::InputWindowFull`] says so and nothing is sent;
    /// [`HostEvent::InputAcknowledged`] tells when room returns.
    ///
    /// The input counts as sent from the moment its frame is queued, which
    /// is once anything an earlier, dropped request left unwritten has gone
    /// out. From then on it stays sent even if the future is dropped before
    /// the write finishes (the rest is written by the next call), and even
    /// if the write then fails: its fate is then unknown, and the caller
    /// must not resend it. A future dropped before that point (before its
    /// first poll, or while an earlier frame was still being written) sent
    /// nothing and cost no window. A caller that cancelled one compares
    /// [`Self::input_counts`]: `sent` grew by the input's length if it was
    /// sent, and is unchanged if not, in which case it may send it again.
    pub async fn send_input(&mut self, data: &[u8]) -> Result<InputTicket, ClientError> {
        self.check_ended()?;
        if data.is_empty() || data.len() > MAX_INPUT {
            return Err(ClientError::InvalidRequest(
                "an input must be 1 to MAX_INPUT bytes",
            ));
        }
        self.flush().await?;
        let ticket = self.input.admit(data.len())?;
        self.queue(&ClientMessage::Input(data.to_vec()));
        self.flush().await?;
        Ok(ticket)
    }

    /// Resize the terminal. The host answers with a resize record in the
    /// stream, not a reply.
    ///
    /// Dropped before its frame was queued (before its first poll, or while
    /// an earlier frame was still being written), it was not sent, and
    /// nothing shows that it was not. A caller that cancelled one re-issues
    /// it unless it knows the frame was queued, which is safe because a
    /// repeated resize is harmless; or it awaits resizes outside a
    /// droppable arm. The host keeps the old size until then.
    pub async fn resize(&mut self, size: Size) -> Result<(), ClientError> {
        self.check_ended()?;
        if !size.is_valid() {
            return Err(ClientError::InvalidRequest(
                "a size must be 1 to MAX_DIMENSION each way",
            ));
        }
        self.flush().await?;
        self.queue(&ClientMessage::Resize(size));
        self.flush().await
    }

    /// Hand the host a snapshot of the terminal as of `position`, and wait
    /// for it to accept or reject it.
    ///
    /// The host judges the blob's size against `MAX_SNAPSHOT` itself
    /// (rejecting a larger one); only a blob too large for any frame is
    /// refused here.
    pub async fn snapshot(
        &mut self,
        position: Position,
        blob: Vec<u8>,
    ) -> Result<SnapshotReply, ClientError> {
        self.check_ended()?;
        // Kind, position and the blob's length field.
        if blob.len() > MAX_FRAME - 1 - 16 - 4 {
            return Err(ClientError::InvalidRequest(
                "a snapshot blob larger than a frame",
            ));
        }
        self.flush().await?;
        let request = self.snapshots.next();
        self.queue(&ClientMessage::Snapshot { position, blob });
        let deadline = Instant::now() + self.deadlines.reply;
        self.drive(|c| c.snapshots.has(request), Some(deadline))
            .await?;
        let (_, reply) = self.snapshots.last.take().expect("the reply just arrived");
        Ok(reply)
    }

    /// Tell the host to end, and wait for its `Goodbye` or the end of the
    /// connection. The whole call, including writing out what an earlier,
    /// dropped request left queued, is bounded by the shutdown deadline.
    ///
    /// The host acts on `Shutdown` even with input pending (which it
    /// discards), and drops replies and records it had not started
    /// writing, so nothing more arrives. `Goodbye` means the host is ending;
    /// confirming that its process has gone is the caller's business
    /// (watch [`Self::peer`], or the socket file, which the host removes).
    /// A connection that ends before the `Shutdown` was written whole is
    /// [`ClientError::Closed`], since the host cannot have read it; one that
    /// ends after it without `Goodbye` is an outcome whose `goodbye` is
    /// false, which is not a confirmation either. On an error the host may
    /// still run: the caller copies [`Self::peer`] out first and may end it
    /// with [`super::process::terminate_verified`].
    pub async fn shutdown(mut self) -> Result<ShutdownOutcome, ClientError> {
        self.check_ended()?;
        // One deadline for the whole call, set before anything waits (see
        // `SHUTDOWN_DEADLINE`).
        let deadline = Instant::now() + self.deadlines.shutdown;
        self.shutting_down = true;
        self.events.clear();
        self.queued = 0;
        self.drive(|c| c.outbound.is_empty(), Some(deadline))
            .await?;
        self.queue(&ClientMessage::Shutdown);
        let goodbye = match self.drive(|c| c.goodbye, Some(deadline)).await {
            Ok(()) => true,
            // Everything queued, the `Shutdown` last, went out before the
            // end: the host may have read it.
            Err(ClientError::Closed) if self.outbound.is_empty() => false,
            Err(error) => return Err(error),
        };
        Ok(ShutdownOutcome {
            goodbye,
            input: self.input.counts(),
        })
    }

    // ---- Internals ---------------------------------------------------------

    fn check_ended(&self) -> Result<(), ClientError> {
        match &self.ended {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    /// Record `error` as what ended the connection, if it does, and return
    /// it.
    fn fail(&mut self, error: ClientError) -> ClientError {
        if error.is_fatal() && self.ended.is_none() {
            self.ended = Some(error.clone());
        }
        error
    }

    /// Queue `message` for writing, with a write deadline of its own. Only
    /// called once `outbound` is empty ([`Self::flush`] first), which is
    /// what bounds the queue to one frame.
    fn queue(&mut self, message: &ClientMessage) {
        debug_assert!(self.outbound.is_empty(), "a frame queued behind another");
        self.write_deadline = Some(Instant::now() + self.deadlines.reply);
        self.outbound.extend(encode_client(message));
    }

    /// Write every queued frame, within the write deadline: a request's own
    /// frame, or what an earlier, dropped request left queued.
    async fn flush(&mut self) -> Result<(), ClientError> {
        self.drive(|c| c.outbound.is_empty(), None).await
    }

    /// Hand out the oldest queued event.
    fn take_event(&mut self) -> Option<HostEvent> {
        let event = self.events.pop_front()?;
        self.queued -= event_cost(&event);
        Some(event)
    }

    /// Queue an event for `next_event`, ending the connection when the
    /// owner has left too much unread.
    fn push_event(&mut self, event: HostEvent) -> Result<(), ClientError> {
        if self.shutting_down {
            return Ok(());
        }
        self.queued += event_cost(&event);
        self.events.push_back(event);
        if self.queued > EVENT_QUEUE_LIMIT {
            return Err(ClientError::EventQueueFull);
        }
        Ok(())
    }

    /// Read, decode and write until `done` holds, the connection ends, or
    /// a deadline passes (which ends the connection): `deadline`, or the
    /// write deadline while anything is queued for writing.
    ///
    /// Reading never pauses while a write waits, and the other way round:
    /// a host stops acting on a client's messages while too much of its own
    /// output waits unread, so a client that only wrote could deadlock with
    /// it.
    async fn drive(
        &mut self,
        done: impl Fn(&Self) -> bool,
        deadline: Option<Instant>,
    ) -> Result<(), ClientError> {
        loop {
            self.check_ended()?;
            if let Err(error) = self.decode_buffered() {
                return Err(self.fail(error));
            }
            if self.outbound.is_empty() {
                self.write_deadline = None;
            }
            if done(self) {
                return Ok(());
            }
            let limit = match (deadline, self.write_deadline) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            // Checked here as well as by the timeout below, which tokio
            // only fires once the read stops being ready: a host that keeps
            // sending must not keep a request alive past its deadline.
            if limit.is_some_and(|limit| Instant::now() >= limit) {
                return Err(self.fail(ClientError::Timeout));
            }
            let step = match limit {
                Some(limit) => tokio::time::timeout_at(limit, self.step())
                    .await
                    .unwrap_or(Err(ClientError::Timeout)),
                None => self.step().await,
            };
            if let Err(error) = step {
                return Err(self.fail(error));
            }
        }
    }

    /// One read from or write to the socket, whichever is ready first.
    /// Cancel-safe: a read lands in the buffer and a write is taken off the
    /// queue only once the socket call has returned.
    async fn step(&mut self) -> Result<(), ClientError> {
        self.inbound.reserve(READ_CHUNK);
        let flushing = !self.outbound.is_empty();
        let step = {
            let (mut reader, mut writer) = self.stream.split();
            tokio::select! {
                read = reader.read_buf(&mut self.inbound) => Step::Read(read),
                wrote = writer.write(&self.outbound), if flushing => Step::Wrote(wrote),
            }
        };
        match step {
            Step::Read(Ok(0)) | Step::Wrote(Ok(0)) => Err(ClientError::Closed),
            Step::Read(Ok(_)) => Ok(()),
            Step::Wrote(Ok(n)) => {
                self.outbound.drain(..n);
                Ok(())
            }
            Step::Read(Err(error)) | Step::Wrote(Err(error)) => Err(io_failure(&error)),
        }
    }

    /// Decode and handle every whole frame in the receive buffer.
    fn decode_buffered(&mut self) -> Result<(), ClientError> {
        let mut used = 0;
        let result = loop {
            let message = match split_frame(&self.inbound[used..], MAX_FRAME) {
                Ok(None) => break Ok(()),
                Err(malformed) => break Err(ClientError::Malformed(malformed.0.into())),
                Ok(Some(frame)) => {
                    used += frame.consumed;
                    decode_host(frame, PROTOCOL_VERSION)
                }
            };
            let handled = match message {
                Ok(message) => self.handle(message),
                Err(malformed) => Err(ClientError::Malformed(malformed.0.into())),
            };
            if let Err(error) = handled {
                break Err(error);
            }
        };
        self.inbound.drain(..used);
        result
    }

    /// Act on one message from the host.
    fn handle(&mut self, message: HostMessage) -> Result<(), ClientError> {
        let event = match message {
            HostMessage::SnapshotRecord { position, blob } => {
                HostEvent::Snapshot { position, blob }
            }
            HostMessage::Output { offset, data } => HostEvent::Output { offset, data },
            HostMessage::Resized {
                offset,
                index,
                size,
            } => HostEvent::Resized {
                offset,
                index,
                size,
            },
            HostMessage::Gap {
                from,
                to,
                resizes,
                size,
            } => HostEvent::Gap {
                from,
                to,
                resizes,
                size,
            },
            HostMessage::Exited { offset, exit } => HostEvent::Exited { offset, exit },
            HostMessage::InfoReply(info) => return self.info.receive(info),
            HostMessage::SnapshotAccepted(position) => {
                return self.snapshots.receive(SnapshotReply::Accepted(position));
            }
            HostMessage::SnapshotRejected { position, reason } => {
                return self
                    .snapshots
                    .receive(SnapshotReply::Rejected { position, reason });
            }
            HostMessage::InputAck { written, discarded } => {
                self.input.acknowledge(written, discarded)?;
                return self.push_event(HostEvent::InputAcknowledged(self.input.counts()));
            }
            HostMessage::Error { code, message } => {
                return Err(ClientError::Host { code, message });
            }
            HostMessage::Goodbye if self.shutting_down => {
                self.goodbye = true;
                return Ok(());
            }
            HostMessage::Goodbye => {
                return Err(ClientError::Malformed("Goodbye without Shutdown".into()));
            }
            HostMessage::Welcome { .. } | HostMessage::Busy => {
                return Err(ClientError::Malformed(
                    "a connection answer after Welcome".into(),
                ));
            }
        };
        // Only records reach here.
        if !self.reading {
            return Err(ClientError::Malformed("a stream record before Read".into()));
        }
        self.push_event(event)
    }
}

/// What `event` costs against [`EVENT_QUEUE_LIMIT`].
fn event_cost(event: &HostEvent) -> usize {
    EVENT_COST
        + match event {
            HostEvent::Snapshot { blob, .. } => blob.len(),
            HostEvent::Output { data, .. } => data.len(),
            _ => 0,
        }
}

/// The client error an I/O failure on the connection means: a peer that
/// went away is [`ClientError::Closed`].
fn io_failure(error: &io::Error) -> ClientError {
    match error.kind() {
        io::ErrorKind::BrokenPipe
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::NotConnected => ClientError::Closed,
        _ => ClientError::Io(error.to_string()),
    }
}

// ---- Connecting ----------------------------------------------------------------

/// The peer an outcome names, if any.
fn peer_of(error: &ConnectError) -> Option<HostProcess> {
    match error {
        ConnectError::Gone(_) => None,
        ConnectError::Busy { peer } | ConnectError::Unavailable { peer, .. } => *peer,
    }
}

/// `error` with `peer` filled in where it names none.
fn with_peer(error: ConnectError, found: Option<HostProcess>) -> ConnectError {
    match error {
        ConnectError::Busy { peer: None } => ConnectError::Busy { peer: found },
        ConnectError::Unavailable { reason, peer: None } => ConnectError::Unavailable {
            reason,
            peer: found,
        },
        other => other,
    }
}

/// Check a gone or peer-less outcome against the process table: a live
/// process of this user serving `socket` makes it unavailable, naming that
/// process (see the module docs, "Never gone while a host process serves
/// the socket").
///
/// A search that cannot run turns gone into unavailable, since gone is the
/// one outcome that lets a caller forget a host; a peer-less outcome stays
/// as it was.
async fn confirm_with_process(socket: &Path, error: ConnectError) -> ConnectError {
    if peer_of(&error).is_some() {
        return error;
    }
    let path = socket.to_path_buf();
    // The search reads every process's command line: off the runtime's
    // threads.
    let found = tokio::task::spawn_blocking(move || socket_process(&path))
        .await
        .unwrap_or_else(|join| Err(format!("searching the process table: {join}")));
    match (error, found) {
        (ConnectError::Gone(_), Ok(Some(host))) => ConnectError::Unavailable {
            reason: Unavailable::NotAccepting,
            peer: Some(host),
        },
        (ConnectError::Gone(_), Err(why)) => ConnectError::Unavailable {
            reason: Unavailable::Io(format!("the socket looks gone, but {why}")),
            peer: None,
        },
        (error, Ok(Some(host))) => with_peer(error, Some(host)),
        (error, _) => error,
    }
}

/// What one connection attempt found.
enum Attempt {
    /// `Welcome` for the version this supervisor speaks, with any bytes
    /// that followed it.
    Welcomed(UnixStream, Option<HostProcess>, Vec<u8>),
    /// A refusal: nothing listens, or (macOS) the backlog is full.
    Refused,
    /// Something worth retrying until the deadline, and what to report if
    /// the deadline comes first.
    Retry(ConnectError),
    /// An answer that retrying cannot change.
    Final(ConnectError),
}

/// Connect once, say `Hello`, and read the answer, all by `deadline`.
async fn attempt(socket: &Path, deadline: Instant) -> Attempt {
    let unavailable = |reason, peer| ConnectError::Unavailable { reason, peer };
    // tokio's connect never blocks in the kernel: a full Linux backlog
    // answers `EAGAIN` at once rather than waiting.
    let mut stream = match tokio::time::timeout_at(deadline, UnixStream::connect(socket)).await {
        Err(_) => return Attempt::Retry(unavailable(Unavailable::Timeout, None)),
        Ok(Ok(stream)) => stream,
        Ok(Err(error)) => {
            return match error.kind() {
                io::ErrorKind::NotFound => Attempt::Final(ConnectError::Gone(Gone::NoSocket)),
                io::ErrorKind::ConnectionRefused => Attempt::Refused,
                io::ErrorKind::WouldBlock => {
                    Attempt::Retry(unavailable(Unavailable::Timeout, None))
                }
                _ => Attempt::Final(unavailable(Unavailable::Io(error.to_string()), None)),
            };
        }
    };
    // Before `Hello`: see `peer_process` for why the order matters.
    let peer = peer_process(&stream);
    let hello = encode_client(&ClientMessage::Hello {
        version: PROTOCOL_VERSION,
        take_over: true,
    });
    let answer = tokio::time::timeout_at(deadline, async {
        stream.write_all(&hello).await?;
        let mut buf = Vec::with_capacity(64);
        loop {
            match split_frame(&buf, MAX_BEFORE_WELCOME) {
                Ok(Some(frame)) => {
                    let consumed = frame.consumed;
                    let message = decode_host(frame, PROTOCOL_VERSION);
                    buf.drain(..consumed);
                    return Ok(Some((message, buf)));
                }
                Ok(None) => {}
                Err(malformed) => return Ok(Some((Err(malformed), buf))),
            }
            if stream.read_buf(&mut buf).await? == 0 {
                return Ok(None);
            }
        }
    })
    .await;
    let (message, rest) = match answer {
        Err(_) => return Attempt::Final(unavailable(Unavailable::Timeout, peer)),
        Ok(Err(error)) => {
            return match io_failure(&error) {
                ClientError::Closed => Attempt::Retry(unavailable(Unavailable::Closed, peer)),
                _ => Attempt::Final(unavailable(Unavailable::Io(error.to_string()), peer)),
            };
        }
        Ok(Ok(None)) => return Attempt::Retry(unavailable(Unavailable::Closed, peer)),
        Ok(Ok(Some(answer))) => answer,
    };
    match message {
        Ok(HostMessage::Welcome { version }) if version == PROTOCOL_VERSION => {
            Attempt::Welcomed(stream, peer, rest)
        }
        // A host answers with the lower of the two versions, so any other
        // is one this supervisor cannot speak.
        Ok(HostMessage::Welcome { version }) => {
            Attempt::Final(unavailable(Unavailable::UnsupportedVersion(version), peer))
        }
        Ok(HostMessage::Busy) => Attempt::Retry(ConnectError::Busy { peer }),
        Ok(HostMessage::Error { code, message }) => {
            Attempt::Final(unavailable(Unavailable::Refused { code, message }, peer))
        }
        Ok(_) => Attempt::Final(unavailable(
            Unavailable::Malformed("a message other than Welcome first".into()),
            peer,
        )),
        Err(malformed) => Attempt::Final(unavailable(
            Unavailable::Malformed(malformed.0.into()),
            peer,
        )),
    }
}

#[cfg(test)]
mod tests;
