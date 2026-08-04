//! The helm's client half of the supervisor protocol.
//!
//! One connection per supervisor, multiplexing concurrent requests
//! (correlated by `req_id`) and any number of terminal attachments
//! (routed by data-channel id). The transport is opaque: a unix socket
//! for the local host, an ssh exec channel for a remote one — handed in
//! as a reader/writer pair so this code cannot tell the difference,
//! which is the SPEC_impl.md transport-blindness made structural.

use anyhow::{Context, bail};
use farhelm_proto::io::{
    FrameReader, FrameWriter, ProgressWrite, handshake, parse_control, write_frame_before_stall,
};
use farhelm_proto::{
    AgentKind, ControlMsg, ErrorKind, Frame, FrameKind, RestartMode, SessionInfo, TabInfo,
    TerminalSelector, UPLOAD_CHUNK_BYTES, UPLOAD_WINDOW_BYTES,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch};
use tokio::time::Instant;
use tracing::warn;

/// Input chunk size. Well under `MAX_FRAME_LEN`, and matched to the
/// supervisor's replay chunking so both directions behave alike.
const INPUT_CHUNK: usize = 32 * 1024;

/// Depth of one attached terminal's [`TermEvent`] queue.
///
/// The helm's half of PLAN_M2_5.md's bounded-queue work, and the hop with
/// the unusual rule: a full queue here DETACHES that terminal instead of
/// backpressuring, because every terminal on this connection shares one
/// multiplexed reader (see [`SupervisorClient::dispatch`]).
///
/// Sized so a WORST-CASE ATTACH REPLAY can never trip the overflow
/// detach, because a healthy attach must not look like a wedged viewer.
/// The arithmetic, from the two constants that actually bound a replay:
///
/// - tmux retains at most `HISTORY_LIMIT` = 12,000 lines (the SPEC.md
///   replay floor the supervisor configures).
/// - A captured line is a terminal row plus its `capture-pane -e` escape
///   sequences; 256 bytes is a generous ceiling for the 80–200 column
///   rows real agents draw. 12,000 × 256 B ≈ 3 MiB.
/// - The supervisor chunks that at its `REPLAY_CHUNK` (32 KiB), so a
///   full replay is ≈ 96 frames.
///
/// 256 leaves ~2.5× headroom over that for unusually wide or heavily
/// styled panes — comfortably enough to also absorb the one extra event
/// PLAN_M5.md item 4's `ReplayComplete` marker adds to a replay's count,
/// without a dedicated slot of its own. Item ordering matters as much as
/// the number: the
/// supervisor now enqueues `Attached` BEFORE spawning the forwarder, so
/// `attach()` returns and the consumer starts draining while the replay is
/// still being written — without that, no queue depth is safe, because
/// nothing is allowed to read until the reply lands.
///
/// Beyond a replay this is a pure backstop. Real flow control happens two
/// hops away — the browser's watermark pause/resume and the supervisor's
/// bounded writer — so a healthy terminal never approaches this in steady
/// state; it only fills when the consumer has genuinely stopped.
///
/// Honest caveat: this bounds EVENTS, not bytes, and the two only line up
/// when frames are large. Live pane output arrives as whatever tmux
/// notification sizes the producer happens to generate — often a few
/// hundred bytes, not `REPLAY_CHUNK` — so for small frames the real
/// ceiling is closer to a hundred kilobytes than to the 8 MiB the count
/// would suggest, and a consumer that stops draining trips the detach
/// below quickly. That is the right bias for what this bound exists to
/// catch (a genuinely wedged viewer), and it is why the number cannot be
/// read as a memory budget. A byte-accounted bound would be the honest fix
/// if this ever needs to double as one; `mpsc` offers no such thing, and a
/// side counter is not worth it while the watermark upstream governs the
/// steady state.
const TERM_EVENT_QUEUE: usize = 256;

/// Depth of the single outbound queue to the supervisor.
///
/// Deliberately small: this direction is dominated by terminal INPUT
/// (keystrokes and pastes, already chunked at [`INPUT_CHUNK`]) and
/// control messages (requests, resizes, pause/resume), all of them tiny
/// and latency-critical — SPEC_impl.md keeps input off the flow-control
/// path entirely — so a deep queue would buy nothing but a longer window
/// in which a dead supervisor still looks alive. Like every `mpsc` bound
/// here this counts MESSAGES, not bytes; a single paste chunk is up to
/// `INPUT_CHUNK`, so the ceiling is 64 of those.
///
/// Attachment uploads are the one BULK producer sharing this queue, and
/// they are rationed rather than accommodated: [`UPLOAD_ENQUEUE_FRAMES`]
/// caps how much of it upload data may occupy at once. That is what lets
/// this bound stay sized for the interactive traffic it exists to serve
/// instead of growing to fit a megabyte-scale producer that would then
/// sit in front of every keystroke.
const SUPERVISOR_WRITER_QUEUE: usize = 64;

/// How long the writer task may make NO byte progress before declaring
/// the supervisor gone.
///
/// The helm's mirror of the supervisor's own writer bound, needed for the
/// same reason: once the outbound queue is bounded, a supervisor that
/// stops consuming parks this task and — through the queue — every request
/// and every WebSocket handler behind it, with no error to report and no
/// EOF to notice. Sixty seconds is generous by construction; the residual
/// is only that a transport accepting not one byte for a full minute is
/// called gone, which is indistinguishable from gone at this layer.
const WRITER_STALL_TIMEOUT: Duration = Duration::from_secs(60);

/// How long an upload may sit on a closed credit window without the
/// supervisor's cumulative `UploadAck` ADVANCING before the sender gives
/// the transfer up as stalled.
///
/// This is the supervisor-facing leg of PLAN_M4.md item 4's per-hop
/// progress timeout, and the proto names the evidence it watches for us:
/// `ControlMsg::UploadAck`'s docs make an advancing ack the receiver's
/// obligation and "a window that stays open with no advancing ack" the
/// definition of a stalled transfer. Without it a peer that keeps the
/// connection open but stops writing bytes parks the relay — and the HTTP
/// handler behind it — forever, because the browser-facing body timeout
/// is not running while this hop is the one that is blocked.
///
/// Only a STRICTLY advancing ack rearms it. A receiver repeating its last
/// cumulative count is reporting no progress, so treating a duplicate as
/// liveness would let a peer hold a transfer open indefinitely with acks
/// that never move. Sixty seconds matches [`WRITER_STALL_TIMEOUT`] and the
/// helm's browser-facing body timeout, so every hop of one transfer is
/// declared stalled on the same generous timescale.
const UPLOAD_ACK_STALL_TIMEOUT: Duration = Duration::from_secs(60);

/// How many upload data frames this connection may have sitting in the
/// shared writer queue at once, across every concurrent upload.
///
/// The credit window is not enough on its own, and `UPLOAD_CHUNK_BYTES`'s
/// own docs say why: "the credit window bounds transit, the sender bounds
/// queue occupancy". A sender that only respected the window could hand
/// the writer a full 4 MiB — sixteen back-to-back bulk frames — and every
/// keystroke, resize and control reply enqueued after that would wait
/// behind megabytes of attachment data on a queue that carries them all.
///
/// The bound is global rather than per-upload so that concurrent uploads
/// cannot multiply their way back to the same problem, and it is enforced
/// with a semaphore whose permit rides the queued frame (see [`Outbound`]),
/// so an allowance is returned only once the writer has actually WRITTEN
/// that frame — occupancy of the shared queue, not merely bytes accepted
/// by this client. `tokio::sync::Semaphore` hands out permits in FIFO
/// order, which is what keeps several uploads sharing the allowance fairly
/// instead of one starving the rest.
///
/// Four is small on purpose: one frame in the writer's hands plus a few
/// queued keeps the transport busy on a fast link, while capping how much
/// bulk data an input frame can ever find ahead of it at about a megabyte.
const UPLOAD_ENQUEUE_FRAMES: usize = 4;

/// A supervisor-side request failure, carried through as a distinct type
/// (rather than a bare string `anyhow` error) so callers above this client
/// — the HTTP layer's `http_error`, in particular — can recover `kind`
/// without parsing `message`. `request()` is the one place this gets
/// constructed, from the `kind` a `ControlMsg::Error` reply already
/// carries; from there it rides the ordinary `anyhow::Error` chain, so a
/// caller downcasts with `error.downcast_ref::<SupervisorError>()` —
/// `anyhow`'s own `downcast_ref` searches the root cause and every
/// `.context(...)` layer above it, so this finds a `SupervisorError`
/// whatever later callers stack on top, whether it was attached as the
/// root or as context. `Display` is just `message`, matching the existing
/// contract that a supervisor error reaches the user's terminal or HTTP
/// body verbatim.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct SupervisorError {
    pub kind: ErrorKind,
    pub message: String,
}

/// `list_sessions`'s return value: the sessions themselves plus the
/// `SessionList` reply's count/truncation metadata (PLAN_M2.md's "Proto
/// growth").
///
/// A struct rather than a bare `Vec<SessionInfo>` specifically so `total`
/// and `truncated` survive this call — the HTTP surface (PR6, PLAN_M2.md
/// step 6) needs both to tell a user "showing N of M" instead of quietly
/// truncating the list with no indication anything was cut. `GET
/// /api/sessions` now serializes this whole struct as its JSON body
/// (`sessions`/`total`/`truncated`), which is why `Serialize` is derived
/// below: the field names are the wire contract, not a private detail.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SessionListing {
    pub sessions: Vec<SessionInfo>,
    /// The supervisor's full session count before any truncation — see
    /// `ControlMsg::SessionList`'s own docs for why this can differ from
    /// `sessions.len()`.
    pub total: u64,
    /// Whether `sessions` is missing entries the supervisor held back
    /// (PLAN_M2.md's list cap/byte-budget). `false` for a supervisor built
    /// before this field existed, exactly like a fresh-default `total`.
    ///
    /// As of PLAN_M6.md item 1 this is a REST-facing field synthesized,
    /// not a value read directly off the wire the way `total` still is —
    /// the wire itself dropped its own `truncated` bool in the same bump
    /// `next_cursor` arrived in (see that field's own docs). The
    /// synthesis is `next_cursor.is_some() || sessions.len() < total`,
    /// not a bare `next_cursor.is_some()`: this build's `next_cursor` is
    /// unconditionally `None` even on a cut reply (`handle_list_sessions`'s
    /// interim contract), so the disjunction's second half is what still
    /// catches a cap- or byte-budget-truncated page — see the
    /// construction site in `list_sessions` for the full argument.
    /// Keeping THIS struct's shape and name unchanged is deliberate:
    /// `list_sessions`'s docs explain why the REST/UI JSON body's TOP-LEVEL
    /// pagination fields (`sessions`/`total`/`truncated`) stay
    /// byte-for-byte identical through this PR even though the wire
    /// underneath it changed — that parity does not extend to each
    /// session's own JSON, which gained `created_at` in this same bump.
    pub truncated: bool,
}

/// What an attached terminal receives from the supervisor side.
#[derive(Debug)]
pub enum TermEvent {
    /// Raw terminal bytes (replay first, then live output).
    Data(Vec<u8>),
    /// This attach's INITIAL catch-up is over: every byte the supervisor
    /// replays for THIS attach's own catch-up phase precedes this event in
    /// the queue, and live output follows (PLAN_M5.md item 4;
    /// `farhelm_proto::ControlMsg::ReplayComplete`'s docs carry the full
    /// contract — exactly once per attach that completes its catch-up,
    /// never owed to an attach a takeover/detach/stall ended early). The
    /// qualifier is load-bearing, mirroring that same doc's own boundary:
    /// M2.5's flow-control recovery after a tmux `%pause` can replay
    /// retained history into this SAME attachment LATER, mid-stream, with
    /// no marker of its own — so "live output follows" describes what
    /// comes after THIS marker, not a promise that history can never
    /// reappear on the channel again.
    ///
    /// Deliberately rides the SAME bounded queue as `Data`, unlike
    /// `Detached`'s out-of-band watch (see [`TerminalHandle`]'s docs for
    /// why detach needs the opposite treatment). A detach must be
    /// deliverable even when the queue is full — that is precisely the
    /// stalled-viewer case — but this marker means nothing except its
    /// POSITION between the replay bytes before it and the live bytes
    /// after: pulling it out of order onto its own channel would let it
    /// race ahead of data it is supposed to follow, which would make the
    /// marker actively wrong instead of merely late. If the queue is
    /// backed up, the marker waits behind the very data it describes —
    /// that wait is the feature, not a cost of reusing the queue.
    ReplayComplete,
    /// The attachment ended: taken over by another client, or the
    /// session's terminal went away.
    Detached(String),
}

/// One attached terminal, as the client holds it: the bounded event queue
/// (`Data` and, as of PLAN_M5.md item 4, `ReplayComplete`) plus an
/// out-of-band detach signal.
///
/// The detach reason travels on its own `watch` rather than through the
/// event queue, and that separation is what makes teardown always possible.
/// The queue can be full at exactly the moment a terminal must be told it
/// is finished — that is, definitionally, the stalled-terminal case — so a
/// notice that needed queue capacity could not be delivered to the one
/// consumer that most needs it, and the sender would sit pinned behind a
/// browser that has stopped reading. A watch never blocks and never needs
/// capacity.
struct TerminalHandle {
    events: mpsc::Sender<TermEvent>,
    detach: watch::Sender<Option<String>>,
}

/// The receiving half of one attachment: bounded terminal events plus the
/// out-of-band detach signal (see [`TerminalHandle`]).
///
/// Exposes `recv()` with the obvious semantics — events in order, then a
/// final `Detached`, then `None` — while keeping the detach signal
/// separately observable via [`Self::detach_signal`] so a consumer parked
/// on something else (a WebSocket write to a browser that stopped reading)
/// can be woken and torn down without that write ever completing.
#[derive(Debug)]
pub struct TermStream {
    events: mpsc::Receiver<TermEvent>,
    detach: watch::Receiver<Option<String>>,
    ended: bool,
}

impl TermStream {
    /// Next event, or `None` once the attachment has ended and its
    /// backlog is drained.
    ///
    /// Buffered data is preferred over the detach signal, deliberately:
    /// a takeover or a dead terminal must not truncate output the
    /// supervisor already delivered, so the reason is reported only once
    /// there is genuinely nothing left to hand over. The one case that
    /// discards a backlog is the overflow detach, and there the backlog is
    /// exactly the content the consumer already proved it was not reading.
    pub async fn recv(&mut self) -> Option<TermEvent> {
        if self.ended {
            return None;
        }
        loop {
            match self.events.try_recv() {
                Ok(event) => return Some(event),
                Err(mpsc::error::TryRecvError::Empty) => {}
                // The queue closed. That happens at the same moment the
                // handle is dropped, which is the same moment the reason
                // was published — so the reason must be reported here
                // rather than collapsing to a bare end-of-stream. Getting
                // this wrong is exactly a silent disappearance: the client
                // would see its terminal stop with no explanation.
                Err(mpsc::error::TryRecvError::Disconnected) => return self.end_with_reason(),
            }
            if let Some(reason) = self.detach.borrow_and_update().clone() {
                self.ended = true;
                return Some(TermEvent::Detached(reason));
            }
            tokio::select! {
                event = self.events.recv() => match event {
                    Some(event) => return Some(event),
                    None => return self.end_with_reason(),
                },
                changed = self.detach.changed() => {
                    if changed.is_err() {
                        return self.end_with_reason();
                    }
                }
            }
        }
    }

    /// End the stream, reporting a detach reason if one was published.
    ///
    /// The `None` answer means the attachment ended without anyone ever
    /// saying why — which the client's own paths do not do, so it stands
    /// for "the whole client was dropped mid-flight" rather than a normal
    /// detach.
    fn end_with_reason(&mut self) -> Option<TermEvent> {
        self.ended = true;
        self.detach
            .borrow_and_update()
            .clone()
            .map(TermEvent::Detached)
    }

    /// Take an event only if one is already buffered.
    ///
    /// Deliberately narrower than `recv`: it never reports the detach
    /// signal, because its callers use it to sweep up data still queued
    /// behind a detach they have ALREADY observed. `Err` means nothing is
    /// buffered right now, not that the stream has ended.
    pub fn try_recv(&mut self) -> Result<TermEvent, mpsc::error::TryRecvError> {
        self.events.try_recv()
    }

    /// A separate view of the detach signal, for consumers that must be
    /// able to abandon an unrelated await when the attachment ends. Yields
    /// the reason; never resolves while the attachment is live.
    pub fn detach_signal(&self) -> TermDetachSignal {
        TermDetachSignal(self.detach.clone())
    }
}

/// Standalone view of one attachment's detach signal — see
/// [`TermStream::detach_signal`].
#[derive(Debug)]
pub struct TermDetachSignal(watch::Receiver<Option<String>>);

impl TermDetachSignal {
    /// Resolve with the detach reason once the attachment ends, or with
    /// `None` if the client itself disappeared first.
    pub async fn detached(&mut self) -> Option<String> {
        loop {
            if let Some(reason) = self.0.borrow_and_update().clone() {
                return Some(reason);
            }
            if self.0.changed().await.is_err() {
                return None;
            }
        }
    }
}

/// One item on its way to the shared writer: the frame, plus whatever
/// queue allowance its producer had to take out to enqueue it.
///
/// The permit exists to be DROPPED at the right moment. It is released
/// when the writer task drops this item, which happens only after the
/// frame has been written — so an upload's allowance measures occupancy
/// of the shared queue rather than "frames this client accepted", which
/// is the distinction [`UPLOAD_ENQUEUE_FRAMES`] exists to enforce.
/// Everything else on this connection (input, control, replies) converts
/// from a bare `Frame` and carries no permit: only bulk upload data is
/// rationed, because it is the only producer that can put megabytes
/// ahead of a keystroke.
struct Outbound {
    frame: Frame,
    _allowance: Option<OwnedSemaphorePermit>,
}

impl From<Frame> for Outbound {
    fn from(frame: Frame) -> Self {
        Self {
            frame,
            _allowance: None,
        }
    }
}

/// One attachment upload's flow-control and outcome state, shared between
/// the demultiplexer (which publishes acks and terminal outcomes) and the
/// single task relaying that upload (PLAN_M4.md item 5). Carried over a
/// `watch` exactly like [`TerminalHandle`]'s detach signal, and for the
/// identical reason: a sender only ever cares about the LATEST cumulative
/// ack and whether the transfer has ended, never the individual events
/// that produced them, and `watch::Sender::send_modify` never blocks —
/// essential since [`SupervisorClient::dispatch`] is the only writer and
/// must never park on one upload's consumer.
///
/// `declared` and `sent` live here rather than in the relaying task's own
/// stack because ack VALIDATION happens on the demux side: `UploadAck`'s
/// contract ("monotonic, never exceeds the bytes actually sent, never
/// exceeds `BeginUpload`'s declared `size`") can only be checked against
/// numbers the demultiplexer can see.
#[derive(Debug, Clone)]
struct UploadProgress {
    /// `BeginUpload`'s declared size — the ceiling any valid ack must
    /// respect, fixed for the upload's lifetime.
    declared: u64,
    /// Bytes this side has handed to the writer for this channel.
    /// Recorded BEFORE the frame is enqueued, so it is always at least
    /// what the supervisor could possibly have received; an ack past it
    /// is a protocol violation rather than a benign race.
    sent: u64,
    /// Highest cumulative byte count the supervisor has acknowledged
    /// (`ControlMsg::UploadAck::received`), after validation. Outstanding
    /// bytes are `sent - received`, which is what the credit window
    /// bounds.
    received: u64,
    /// Why this upload is over, once it is: the supervisor's verbatim
    /// `UploadAborted` reason, this connection's death, or a local
    /// give-up (stall, protocol violation) whose `AbortUpload` has
    /// already been handed to an independent task.
    ///
    /// `Some` is terminal — the first reason wins and is never rewritten,
    /// because the user is shown the cause that actually ended the
    /// transfer, not whatever happened next.
    ///
    /// What makes the reason survivable is that [`UploadGuard`] holds a
    /// `watch::Receiver` for the upload's whole life: an abort landing
    /// while the relay is parked on its HTTP body (the common case — no
    /// credit wait is subscribed then) is still readable at the next send
    /// or at commit. A design that published the reason only to whoever
    /// happened to be listening at that instant would degrade a precise
    /// supervisor message into a generic "no longer active", which is
    /// exactly the failure this field's lifetime is shaped to avoid.
    ///
    /// It also doubles as the "an abort is already accounted for" flag:
    /// every local path sets it only after arranging delivery, so
    /// `UploadGuard`'s drop can tell an upload that still owes the
    /// supervisor an `AbortUpload` from one that does not.
    ended: Option<String>,
}

/// The optional fields a `CreateSession` may carry beyond the four that
/// describe the session itself.
///
/// Grouped rather than added as three more parameters because they share
/// a property that is easy to lose sight of: all three are OPTIONAL and
/// all three participate in the create's idempotency fingerprint, so a
/// retry that changes any of them is a different request and is refused
/// as a key reuse rather than merged (PLAN_M3.md item 6). `Default` is
/// the pre-M3 create in every respect.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreateExtras {
    /// See [`SupervisorClient::create_session_with_key`].
    pub intent_key: Option<String>,
    /// Force (or forbid) an agent integration the invocation's basename
    /// would not produce on its own; `None` lets the supervisor derive it.
    pub agent_kind: Option<AgentKind>,
    /// Override the resume invocation, as an argv vector. An integrated
    /// kind's template must contain a `{conversation}` element; the
    /// supervisor refuses the create otherwise.
    pub resume_template: Option<Vec<String>>,
}

/// A live connection to one supervisor, shared by every request in flight.
///
/// All methods take `&self` and the type is `Send + Sync`: axum handlers
/// hit it concurrently from many tasks, and multiplexing is what makes
/// that safe — requests are correlated by `req_id`, terminals by data
/// channel, and every outbound frame funnels through one writer task.
///
/// There is no reconnect. A lost connection is terminal for this value:
/// pending requests fail, terminals get a `Detached`, and later calls fail
/// fast rather than queueing onto a corpse. Reconnection with bounded
/// retries (SPEC.md's Errors section) arrives with the host registry.
pub struct SupervisorClient {
    writer_tx: mpsc::Sender<Outbound>,
    pending: Mutex<Pending>,
    terminals: Mutex<HashMap<u32, TerminalHandle>>,
    /// In-flight attachment uploads, keyed by the same connection-unique
    /// data-channel ids `terminals` uses — the two maps share
    /// `next_channel`'s allocator (an upload and a terminal can never
    /// collide on one id) but not their entries, since an upload is a
    /// send-direction attachment while a terminal is receive-direction.
    ///
    /// An entry outlives the transfer it describes: it is created before
    /// `BeginUpload` goes out and removed only by the [`UploadGuard`] that
    /// owns it, never by the demultiplexer. Single ownership of the
    /// removal is what keeps the two sides from having to agree about
    /// timing — the demux only ever publishes into an entry it knows is
    /// there, including the terminal outcome, and the owner decides when
    /// the upload is finished with.
    uploads: Mutex<HashMap<u32, watch::Sender<UploadProgress>>>,
    /// The connection-wide ration of upload frames allowed to occupy the
    /// shared writer queue at once — see [`UPLOAD_ENQUEUE_FRAMES`].
    upload_enqueue: Arc<Semaphore>,
    /// How long a credit wait may go without an advancing ack before the
    /// transfer is declared stalled ([`UPLOAD_ACK_STALL_TIMEOUT`]),
    /// injectable so tests can observe the give-up without waiting out a
    /// production minute.
    upload_stall: Duration,
    next_req: AtomicU64,
    /// Wider than the wire's u32 on purpose: ids are never recycled (see
    /// `allocate_channel`), so the counter must be able to walk past the
    /// u32 range and fail allocation instead of wrapping back into it.
    next_channel: AtomicU64,
}

/// In-flight requests plus the connection-dead flag, under one mutex on
/// purpose: `fail_all` sets `closed` and drains the map in a single lock
/// hold, and `request` checks the flag and inserts under that same lock.
/// Anything looser leaves a window where a request observes "not closed",
/// is preempted by the drain, and then inserts a sender nobody will ever
/// complete — a permanently hung HTTP handler. Keeping the flag inside
/// the guarded state makes that invariant structural instead of a
/// comment on an atomic that was only ever touched under the lock anyway.
#[derive(Default)]
struct Pending {
    map: HashMap<u64, oneshot::Sender<ControlMsg>>,
    /// Set when either half of the connection dies (read EOF/error in the
    /// demux loop, or a write failure in the writer task). Requests made
    /// afterwards fail immediately rather than queueing onto a connection
    /// that will never answer.
    closed: bool,
}

/// Hand out the next terminal channel id. Ids are never recycled within a
/// connection's lifetime; allocation fails once the u32 wire range is spent.
///
/// Recycling — even "carefully", skipping ids still present in `terminals`
/// — is unsound, because absence from that map does not mean an id is
/// retired end-to-end. Callers keep the raw number after their map entry is
/// gone: every cleanup path calls `detach(channel)` unconditionally,
/// including after a server-side `Detached` already removed the entry, and
/// frames addressed to a dead channel can still be in flight on the wire.
/// A recycled id would let that stale cleanup tear down the new owner, or
/// route a stale frame into it. Making ids unique for the connection's
/// lifetime closes the whole class.
///
/// The cost is exhaustion after `u32::MAX` attachments on one connection,
/// which no real deployment approaches; if it ever happens, a clean attach
/// error (reconnect recovers, since ids are per-connection) beats silent
/// cross-attachment corruption. The backing counter is u64, so it does not
/// itself wrap; once past the u32 range every later call keeps failing.
///
/// Id 0 is never produced — the counter starts at 1, because 0 is the
/// control channel and the supervisor rejects an Attach naming it.
fn allocate_channel(next_channel: &AtomicU64) -> anyhow::Result<u32> {
    let id = next_channel.fetch_add(1, Ordering::Relaxed);
    u32::try_from(id)
        .map_err(|_| anyhow::anyhow!("terminal channel ids exhausted on this connection"))
}

/// Tell one terminal it is finished, without ever blocking and without
/// ever needing queue capacity.
///
/// Publishing on the handle's out-of-band watch rather than pushing a
/// `TermEvent` is what makes this always possible. Every caller is on the
/// shared demux path — the multiplexed reader loop, or `fail_all` running
/// from either connection half — where blocking on one terminal's bounded
/// queue would stall every OTHER terminal, request, and control reply on
/// the connection (see `dispatch`'s head-of-line note). Yet the notice
/// must not be dropped either: a client that never learns it was detached
/// shows a live-looking terminal that has silently stopped.
///
/// The first reason wins. A terminal is detached once, and a later cause
/// (the connection dying after a takeover already landed) must not rewrite
/// the specific reason the user is shown.
fn signal_detached(handle: &TerminalHandle, reason: String) {
    handle.detach.send_if_modified(|current| {
        if current.is_some() {
            return false;
        }
        *current = Some(reason);
        true
    });
}

/// Publish one upload's terminal outcome, without ever blocking and
/// without removing the entry that carries it.
///
/// `signal_detached`'s upload twin, down to the first-reason-wins rule and
/// the reason it exists: the callers are the shared demux path and
/// `fail_all`, where blocking on anything one relay owns would stall every
/// other transfer, terminal and reply on the connection.
///
/// The first reason wins because it is the one that actually ended the
/// transfer — a connection death arriving after the supervisor already
/// explained itself must not overwrite "disk full" with "connection lost".
/// The retained entry (see `UploadProgress::ended`) is what lets a relay
/// that was not watching at this instant still read the reason later.
fn end_upload(progress: &watch::Sender<UploadProgress>, reason: String) {
    progress.send_if_modified(|p| {
        if p.ended.is_some() {
            return false;
        }
        p.ended = Some(reason);
        true
    });
}

impl SupervisorClient {
    /// Perform the hello handshake and start the demux loop. Fails fast
    /// on protocol-version mismatch — the SPEC.md skew rule fires here,
    /// at connection time, not on first use.
    pub async fn start<R, W>(r: R, w: W) -> anyhow::Result<Arc<SupervisorClient>>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self::start_with_stall_timeout(r, w, WRITER_STALL_TIMEOUT).await
    }

    /// Like [`Self::start`], but with the writer's no-progress window
    /// supplied explicitly — the seam tests use to observe a wedged-peer
    /// teardown without waiting out a full production minute.
    pub async fn start_with_stall_timeout<R, W>(
        r: R,
        w: W,
        writer_stall: Duration,
    ) -> anyhow::Result<Arc<SupervisorClient>>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self::start_with_stall_timeouts(r, w, writer_stall, UPLOAD_ACK_STALL_TIMEOUT).await
    }

    /// Like [`Self::start_with_stall_timeout`], but also overriding the
    /// upload credit wait's no-progress window
    /// ([`UPLOAD_ACK_STALL_TIMEOUT`]).
    ///
    /// The two bounds are separate parameters because they catch different
    /// peers: `writer_stall` catches a transport that accepts no bytes at
    /// all, `upload_stall` a supervisor that is reading happily and simply
    /// never acknowledges an upload. A test for one wants the other left
    /// at its production value, so that a bug in the bound under test
    /// cannot be masked by the other firing first.
    pub async fn start_with_stall_timeouts<R, W>(
        r: R,
        w: W,
        writer_stall: Duration,
        upload_stall: Duration,
    ) -> anyhow::Result<Arc<SupervisorClient>>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        // Byte-level write progress, so the writer task below can tell a
        // slow supervisor from one that has stopped consuming.
        let (w, bytes_written) = ProgressWrite::new(w);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "helm").await?;

        let (writer_tx, mut writer_rx) = mpsc::channel::<Outbound>(SUPERVISOR_WRITER_QUEUE);
        let (connection_done, _) = watch::channel(false);

        let client = Arc::new(SupervisorClient {
            writer_tx,
            pending: Mutex::new(Pending::default()),
            terminals: Mutex::new(HashMap::new()),
            uploads: Mutex::new(HashMap::new()),
            upload_enqueue: Arc::new(Semaphore::new(UPLOAD_ENQUEUE_FRAMES)),
            upload_stall,
            next_req: AtomicU64::new(1),
            // Channel 0 is the control channel; attachments start at 1.
            next_channel: AtomicU64::new(1),
        });

        // A `Weak`, deliberately: the client owns `writer_tx`, so a
        // strong handle here would be a cycle — `writer_rx.recv()` only
        // returns `None` once every sender is gone, and the task would
        // be holding one alive through its own `Arc`. The task would
        // then park forever after the read side died, leaking itself,
        // the channel, and the whole client. Upgrading only to report a
        // failure keeps the client droppable — which requires the demux
        // task below to be weak too, or the cycle just moves there.
        let wclient = Arc::downgrade(&client);
        let writer_done = connection_done.clone();
        let mut writer_cancel = connection_done.subscribe();
        tokio::spawn(async move {
            loop {
                let item = tokio::select! {
                    _ = writer_cancel.changed() => break,
                    item = writer_rx.recv() => item,
                };
                // Held across the write, not just up to it: an upload's
                // queue allowance is returned when this value drops, and
                // returning it early would let the next bulk frame take
                // the slot while this one is still occupying the writer.
                let Some(item) = item else {
                    break;
                };
                let frame = &item.frame;
                // Logged, not swallowed: a write failure here (broken
                // ssh pipe, dead socket) is the one diagnostic that
                // explains why every later request starts failing. A
                // supervisor that stops consuming entirely is treated the
                // same way and for the same reason as the supervisor's own
                // writer treats a stalled helm — see
                // `write_frame_before_stall`: without a bound, bounding
                // the outbound queue would let a wedged peer park this
                // task and every producer behind it forever.
                if let Err(e) =
                    write_frame_before_stall(&mut writer, &bytes_written, frame, writer_stall).await
                {
                    warn!(error = %e, "frame write to supervisor failed");
                    // The write half dying must fail waiters too: a
                    // half-broken pipe (remote stops reading, keeps
                    // writing) never EOFs the read half, so without this
                    // a request already in `pending` when the write broke
                    // would hang its HTTP handler forever.
                    if let Some(client) = wclient.upgrade() {
                        client.fail_all("supervisor connection lost").await;
                    }
                    let _ = writer_done.send(true);
                    return;
                }
            }
            // Closing this half is also terminal for the connection. Tell
            // the demux to drop its read future rather than leaving a
            // generic split stream alive behind the closed writer.
            let _ = writer_done.send(true);
            // The channel closes when the last external client handle
            // disappears. Explicit shutdown matters for generic split
            // streams, whose read half may otherwise keep the underlying
            // transport—and therefore the peer's read—alive.
            let _ = writer.shutdown().await;
        });

        // Weak here too, and for the same reason as the writer task: a
        // strong handle would keep `writer_tx` alive, which keeps the
        // writer task parked, which holds the transport's write half
        // open — so dropping every external handle to a HEALTHY client
        // would tear nothing down, the peer would never see a close, and
        // (over ssh) the child process would never be reaped. With both
        // tasks weak, the last external drop closes the write half, the
        // peer EOFs, and this loop unwinds.
        // The upgrade happens per frame, never across the read: holding
        // a strong handle while parked in `read_frame` — where this loop
        // spends essentially all its time — would defeat the point.
        let demux = Arc::downgrade(&client);
        let reader_done = connection_done;
        let mut reader_cancel = reader_done.subscribe();
        tokio::spawn(async move {
            loop {
                let frame = tokio::select! {
                    _ = reader_cancel.changed() => return,
                    frame = reader.read_frame() => frame,
                };
                match frame {
                    Ok(Some(frame)) => match demux.upgrade() {
                        Some(client) => {
                            if let Err(e) = client.dispatch(frame).await {
                                warn!(error = %e, "invalid frame from supervisor");
                                break;
                            }
                        }
                        None => break,
                    },
                    Ok(None) => break,
                    Err(e) => {
                        warn!(error = %e, "supervisor connection lost");
                        break;
                    }
                }
            }
            if let Some(client) = demux.upgrade() {
                client.fail_all("supervisor connection lost").await;
            }
            let _ = reader_done.send(true);
        });

        Ok(client)
    }

    /// Declare the connection dead: detach every terminal, abort every
    /// in-flight upload, fail every pending request, and make later
    /// requests fail fast. Idempotent, and called from BOTH halves — the
    /// demux loop on read EOF/error, the writer task on write failure —
    /// because either half dying alone (a half-broken ssh pipe) leaves
    /// the other alive and waiters hung.
    ///
    /// Terminals and uploads each get an explicit out-of-band notice
    /// (`signal_detached`, `end_upload` on the upload's watch); pending
    /// requests are failed by dropping their oneshot senders, which makes
    /// `request()` return the "connection closed" error instead of
    /// hanging an HTTP handler.
    ///
    /// Terminals are DRAINED here while uploads are only marked: a
    /// terminal's consumer owns its receiver and learns the reason from
    /// it, whereas an upload's reason lives in the map entry until the
    /// [`UploadGuard`] that owns the transfer retires it. Draining
    /// uploads would throw that reason away in exactly the case it is
    /// most needed — a relay parked between body chunks when the
    /// supervisor died.
    async fn fail_all(&self, reason: &str) {
        let mut terms = self.terminals.lock().await;
        for (_, handle) in terms.drain() {
            signal_detached(&handle, reason.to_string());
        }
        drop(terms);
        let uploads = self.uploads.lock().await;
        for progress in uploads.values() {
            end_upload(progress, reason.to_string());
        }
        drop(uploads);
        // Flag and drain in one lock hold; see `Pending` for why.
        let mut pending = self.pending.lock().await;
        pending.closed = true;
        pending.map.clear();
    }

    /// Route one inbound frame to whoever is waiting for it: data frames
    /// to their terminal, replies to the request that carries their
    /// `req_id`, `Detached` to the terminal it names, `ReplayComplete`
    /// (PLAN_M5.md item 4) into that SAME terminal's data queue rather
    /// than its detach watch (see [`TermEvent::ReplayComplete`]'s docs for
    /// why), and — PLAN_M4.md item 5's upload vocabulary —
    /// `UploadAck`/`UploadAborted` to the upload it names by `channel`,
    /// the same channel-correlated, no-`req_id` shape `Detached` already
    /// uses.
    ///
    /// A frame for a channel or request that no longer exists is dropped:
    /// that is the normal outcome of a detach racing in-flight output.
    /// Malformed control JSON is different. Framing has no
    /// resynchronization or way to recover the `req_id`, so keeping the
    /// connection alive could strand the corresponding request forever;
    /// it is returned as a fatal protocol error.
    ///
    /// The `req_id != 0` guard is the protocol's "not tied to any
    /// request" rule (see `ControlMsg::Error`): request ids start at 1, so
    /// an `Error` carrying 0 is unsolicited and falls through to the log
    /// rather than completing somebody's request.
    ///
    /// # Why this hop detaches instead of backpressuring
    ///
    /// Every other bounded hop on the terminal path answers a full queue
    /// by blocking, which propagates backpressure upstream until it
    /// reaches tmux's own `pause-after`. This one must not, and the
    /// asymmetry is deliberate (PLAN_M2_5.md calls it the one failure this
    /// hop must never have): every terminal, every pending request, and
    /// the control channel itself are multiplexed over the SINGLE reader
    /// loop that calls this function. Awaiting capacity on one terminal's
    /// channel would stall all of them — one wedged browser tab freezing
    /// every other session's output and every control reply on the
    /// connection.
    ///
    /// So a full per-terminal queue is treated as that terminal being
    /// stalled: `try_send` never blocks, the upstream attachment is torn
    /// down with a `Detach`, and the terminal gets a final
    /// [`TermEvent::Detached`] carrying the same
    /// `DETACH_REASON_STALLED` string the supervisor's own stall detach
    /// uses (they must be identical — that is why the constant exists).
    /// Losing the tail of a wedged terminal's output is the accepted cost;
    /// it is bounded, visible to the user as a detach banner, and
    /// recoverable by reattaching, whereas the alternative is an
    /// unbounded, invisible stall of everything else.
    async fn dispatch(&self, frame: Frame) -> anyhow::Result<()> {
        match frame.kind {
            FrameKind::Data => {
                self.route_terminal_event(frame.channel, TermEvent::Data(frame.body))
                    .await;
            }
            FrameKind::Control => {
                let msg = parse_control(&frame)?;
                match &msg {
                    ControlMsg::SessionCreated { req_id, .. }
                    | ControlMsg::SessionList { req_id, .. }
                    | ControlMsg::SessionStopped { req_id, .. }
                    | ControlMsg::SessionDeleted { req_id, .. }
                    | ControlMsg::SessionRestarted { req_id, .. }
                    | ControlMsg::SessionRenamed { req_id, .. }
                    | ControlMsg::Attached { req_id, .. }
                    | ControlMsg::TabOpened { req_id, .. }
                    | ControlMsg::TabClosed { req_id, .. }
                    | ControlMsg::UploadStarted { req_id, .. }
                    | ControlMsg::UploadCommitted { req_id, .. }
                    | ControlMsg::Error { req_id, .. }
                        if *req_id != 0 =>
                    {
                        if let Some(tx) = self.pending.lock().await.map.remove(req_id) {
                            let _ = tx.send(msg);
                        }
                    }
                    ControlMsg::Detached { channel, reason } => {
                        if let Some(handle) = self.terminals.lock().await.remove(channel) {
                            signal_detached(&handle, reason.clone());
                            // Acknowledge upstream, so the SUPERVISOR can
                            // retire the connection-local routing this
                            // channel still has (its `input_routes` entry,
                            // which pins the `SessionEntry` it resolved).
                            // A server-initiated detach — a delete, a
                            // restart, a stall — otherwise leaves that
                            // mapping for the life of the connection: a
                            // leak per event, and a stale one, since the
                            // entry it pins describes a session (or a run)
                            // that is gone. The supervisor tolerates a
                            // `Detach` for an attachment it has already
                            // dropped, which is exactly what this is.
                            self.release_upstream(*channel);
                        }
                    }
                    // Unsolicited, channel-correlated like `Detached`
                    // above but routed into the DATA queue instead of the
                    // detach watch (PLAN_M5.md item 4) — see
                    // [`TermEvent::ReplayComplete`]'s docs for why. An
                    // unknown channel (the attachment already ended, or
                    // this connection never knew about it) is discarded
                    // exactly like a stray data frame: `route_terminal_event`
                    // is a no-op when the channel is not in `terminals`.
                    ControlMsg::ReplayComplete { channel } => {
                        self.route_terminal_event(*channel, TermEvent::ReplayComplete)
                            .await;
                    }
                    // Unsolicited progress: extends the sender's credit
                    // window (see `UploadGuard::wait_for_credit`), and is
                    // the liveness evidence its stall deadline watches. A
                    // channel this client no longer knows about is a
                    // normal race (the relay already retired the upload)
                    // and is silently dropped, exactly like a stray data
                    // frame for a dead terminal above.
                    //
                    // Validation is the protocol's, not this client's
                    // invention: `UploadAck`'s docs make `received`
                    // monotonic, bounded by the bytes actually sent and by
                    // the declared size, and make a violating ack an abort
                    // — so a peer cannot manufacture credit it was never
                    // owed, nor push the window arithmetic anywhere it
                    // could overflow.
                    ControlMsg::UploadAck { channel, received } => {
                        let mut violation = None;
                        if let Some(progress) = self.uploads.lock().await.get(channel) {
                            progress.send_if_modified(|p| {
                                if p.ended.is_some() {
                                    return false;
                                }
                                if *received < p.received {
                                    violation = Some(format!(
                                        "supervisor upload ack regressed from {} to {received}",
                                        p.received
                                    ));
                                } else if *received > p.sent {
                                    violation = Some(format!(
                                        "supervisor acknowledged {received} upload bytes but only \
                                         {} were sent",
                                        p.sent
                                    ));
                                } else if *received > p.declared {
                                    violation = Some(format!(
                                        "supervisor acknowledged {received} upload bytes past the \
                                         declared size {}",
                                        p.declared
                                    ));
                                }
                                if let Some(reason) = &violation {
                                    p.ended = Some(reason.clone());
                                    return true;
                                }
                                // A repeat of the current count is legal
                                // but reports no progress, so it must not
                                // wake the credit wait: doing so would let
                                // a peer hold a transfer open forever with
                                // acks that never move (see
                                // `UPLOAD_ACK_STALL_TIMEOUT`).
                                if *received == p.received {
                                    return false;
                                }
                                p.received = *received;
                                true
                            });
                        }
                        // Outside the map lock, and fire-and-forget: the
                        // demux loop must never park on the writer queue.
                        if let Some(reason) = violation {
                            warn!(channel, %reason, "aborting upload on an invalid ack");
                            self.enqueue_abort(*channel);
                        }
                    }
                    // Unsolicited: the supervisor gave up on this upload.
                    // Unlike `Detached` above, this does NOT remove the
                    // entry — the guard that owns the upload does that,
                    // once it has acted on the outcome. The reason itself
                    // reaches the relay whether or not it is watching
                    // right now, because the guard holds a receiver for
                    // the transfer's whole life (see
                    // `UploadProgress::ended`); leaving the entry in place
                    // keeps the "only the owner retires it" rule intact
                    // rather than making removal a race between two sides.
                    ControlMsg::UploadAborted { channel, reason } => {
                        if let Some(progress) = self.uploads.lock().await.get(channel) {
                            end_upload(progress, reason.clone());
                        }
                    }
                    other => warn!(?other, "unexpected control message at helm"),
                }
            }
        }
        Ok(())
    }

    /// Push one event onto `channel`'s bounded queue, applying `dispatch`'s
    /// overflow-is-stall-detach rule uniformly for both queue producers:
    /// raw `Data` off the wire and the `ReplayComplete` marker synthesized
    /// from a `ControlMsg` (PLAN_M5.md item 4). A channel `dispatch` does
    /// not know about — the attachment already ended, or never existed on
    /// this connection — is a silent no-op, the same normal race a stray
    /// data frame for a dead terminal already tolerates.
    ///
    /// `entry` rather than get-then-remove so the overflow arm removes the
    /// very entry it just observed, under the SAME lock hold: releasing
    /// first would let a concurrent `detach`/`attach` interleave and leave
    /// this tearing down a channel that is no longer the one that
    /// overflowed.
    async fn route_terminal_event(&self, channel: u32, event: TermEvent) {
        let mut terms = self.terminals.lock().await;
        if let std::collections::hash_map::Entry::Occupied(entry) = terms.entry(channel) {
            match entry.get().events.try_send(event) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    let handle = entry.remove();
                    signal_detached(&handle, farhelm_proto::DETACH_REASON_STALLED.to_string());
                    self.release_upstream(channel);
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    // The consumer dropped its receiver without detaching
                    // (a task cancelled mid-flight, say). Nobody is
                    // listening, so there is no local notice to deliver —
                    // but the SUPERVISOR still holds an attachment for
                    // this channel, and leaving it there would pin a
                    // control client, an input client, and a forwarder
                    // for the life of the connection.
                    entry.remove();
                    self.release_upstream(channel);
                }
            }
        }
    }

    /// Enqueue a `Detach` for `channel` upstream WITHOUT awaiting.
    ///
    /// Called only from the demultiplexer, which is why it must not await:
    /// `detach()` blocks on the bounded writer queue, and blocking there
    /// on the shared reader loop is precisely the head-of-line failure the
    /// per-terminal detach rule exists to avoid — one wedged tab would
    /// stall every other terminal, every pending request, and the control
    /// channel, via the very path meant to protect them from it. Spawning
    /// keeps the loop free; the message is never dropped, because the task
    /// owns a sender clone and outlives this call.
    ///
    /// The local terminal entry is always removed by the caller before
    /// this runs, so nothing routes to the channel while the `Detach` is
    /// still in flight.
    fn release_upstream(&self, channel: u32) {
        let writer_tx = self.writer_tx.clone();
        tokio::spawn(async move {
            let _ = writer_tx
                .send(Frame::control(&ControlMsg::Detach { channel }).into())
                .await;
        });
    }

    /// Enqueue an `AbortUpload` for `channel` from a task this caller does
    /// not own, and never wait for it.
    ///
    /// Every abort in this client goes out this way, and the ownership is
    /// the point. The writer queue is bounded, so enqueueing can park; an
    /// abort awaited inline would be lost exactly when it matters most —
    /// a cancelled relay (its handler's future dropped mid-send) or the
    /// demux loop, which must never block on one upload at all. A task
    /// holding its own sender clone cannot be cancelled by whatever
    /// happened upstream, so the supervisor learns to drop the transfer
    /// and clean its temp file even when nothing is left here to care.
    ///
    /// Delivering an abort more than once, or for a channel the supervisor
    /// never accepted, is harmless by `ControlMsg::AbortUpload`'s own
    /// idempotence contract — which is what lets every teardown path call
    /// this without first proving who won a race.
    fn enqueue_abort(&self, channel: u32) {
        let writer_tx = self.writer_tx.clone();
        tokio::spawn(async move {
            let _ = writer_tx
                .send(Frame::control(&ControlMsg::AbortUpload { channel }).into())
                .await;
        });
    }

    /// Send a request and await its correlated reply.
    ///
    /// `req_id` is passed separately from `msg` because it lives inside
    /// the message too — caller and registry must agree on the same value,
    /// so it is minted once by `req_id()` and threaded through both.
    ///
    /// A supervisor `Error` reply becomes an `Err` wrapping a
    /// [`SupervisorError`], which carries both the message verbatim (so a
    /// remote precondition failure still reaches the user as prose) and
    /// its `kind` (so `http_error` can pick a status code without parsing
    /// that prose). There is no timeout: the connection dying is what
    /// unblocks a waiter, and inventing a deadline here would abandon
    /// slow-but-fine operations on a loaded host.
    async fn request(&self, req_id: u64, msg: ControlMsg) -> anyhow::Result<ControlMsg> {
        // Writer capacity is reserved BEFORE the pending entry is
        // registered, and that ordering is the whole point. The queue is
        // bounded, so sending can park; if it parks with the entry already
        // registered and this future is then cancelled — an axum handler
        // whose client disconnected, a `select!` losing a race — the entry
        // is orphaned in a map nothing ever cleans, for the life of the
        // process. With the reservation first, the only await before
        // registration cannot leave anything behind, and the send itself
        // becomes infallible and instant.
        let permit = self
            .writer_tx
            .reserve()
            .await
            .map_err(|e| anyhow::Error::new(e).context("supervisor connection closed"))?;
        let (tx, rx) = oneshot::channel();
        {
            // Check-and-insert under one lock hold; see `Pending` for
            // why splitting them hangs requests.
            let mut pending = self.pending.lock().await;
            if pending.closed {
                bail!("supervisor connection closed");
            }
            pending.map.insert(req_id, tx);
        }
        permit.send(Frame::control(&msg).into());
        let reply = rx.await.context("supervisor connection closed")?;
        // Matched by value, not `if let ... = &reply`: an owned `message`
        // moves straight into `SupervisorError` instead of a borrow forcing
        // a clone here for no reason (the reply is not used afterwards
        // either way).
        match reply {
            ControlMsg::Error { message, kind, .. } => {
                Err(anyhow::Error::new(SupervisorError { kind, message }))
            }
            reply => Ok(reply),
        }
    }

    fn req_id(&self) -> u64 {
        self.next_req.fetch_add(1, Ordering::Relaxed)
    }

    /// Create and launch a session on this supervisor.
    ///
    /// Success means the session exists, not that the agent is running.
    /// M1 keeps the terminal available when the later `exec` fails, so
    /// its diagnostic remains visible, but does not yet expose structured
    /// launch status. An `Err` here is a precondition failure that left
    /// nothing behind and carries the supervisor's message for display.
    ///
    /// No idempotency: every call is its own create. That is the right
    /// default for the callers this wrapper has — a CLI invocation the user
    /// typed again IS a second intent, and SPEC.md explicitly sanctions two
    /// sessions with identical parameters, so a key derived from the fields
    /// would block something the product allows. Intent identity belongs to
    /// the surface that can distinguish a retry from a new request, which
    /// is the GUI; see [`SupervisorClient::create_session_with_key`].
    pub async fn create_session(
        &self,
        cwd: &str,
        invocation: &str,
        title: Option<String>,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<SessionInfo> {
        self.create_session_with_key(cwd, invocation, title, cols, rows, None)
            .await
    }

    /// [`SupervisorClient::create_session`] carrying a client-supplied
    /// idempotency key (PLAN_M3.md item 6).
    ///
    /// The key belongs to whoever can RETRY — the browser, ultimately —
    /// which is why this parameter is threaded through from the HTTP
    /// request body rather than minted here: a key the helm invented would
    /// be a fresh one on every retry the browser made, deduplicating
    /// nothing. `None` is the pre-M3 path, where each request is its own
    /// create.
    ///
    /// Same key with the same request replays whatever that intent
    /// resolved to: the session it created, an explicit gone-error if that
    /// session has since been deleted, or the ORIGINAL error — including
    /// one from a precondition the request never got past, which is
    /// replayed rather than re-evaluated against a filesystem that may
    /// have changed. Same key with a DIFFERENT request is refused with
    /// `ErrorKind::Conflict`, which `http_error` renders as a 409. A key
    /// whose create is still being reconciled can also come back as an
    /// `Internal` failure that says so; retrying it is the intended
    /// response.
    pub async fn create_session_with_key(
        &self,
        cwd: &str,
        invocation: &str,
        title: Option<String>,
        cols: u16,
        rows: u16,
        intent_key: Option<String>,
    ) -> anyhow::Result<SessionInfo> {
        self.create_session_with_extras(
            cwd,
            invocation,
            title,
            cols,
            rows,
            CreateExtras {
                intent_key,
                ..CreateExtras::default()
            },
        )
        .await
    }

    /// [`SupervisorClient::create_session`] carrying PLAN_M3.md item 7's
    /// snapshot overrides as well as item 6's idempotency key.
    ///
    /// The overrides have no UI caller and are not expected to gain one
    /// before M6.75's profiles: the UI sends nothing and lets the supervisor
    /// derive the kind from the invocation's basename. This entry point
    /// exists because the API and the tests ARE the consumers in the
    /// meantime — a wrapper script or `env claude` classifies as generic
    /// by design, and there has to be a way to say otherwise.
    pub async fn create_session_with_extras(
        &self,
        cwd: &str,
        invocation: &str,
        title: Option<String>,
        cols: u16,
        rows: u16,
        extras: CreateExtras,
    ) -> anyhow::Result<SessionInfo> {
        let req_id = self.req_id();
        match self
            .request(
                req_id,
                ControlMsg::CreateSession {
                    req_id,
                    cwd: cwd.to_string(),
                    invocation: invocation.to_string(),
                    title,
                    cols,
                    rows,
                    intent_key: extras.intent_key,
                    agent_kind: extras.agent_kind,
                    resume_template: extras.resume_template,
                },
            )
            .await?
        {
            ControlMsg::SessionCreated { session, .. } => Ok(session),
            other => bail!("unexpected reply to create_session: {other:?}"),
        }
    }

    /// Every session this supervisor holds (subject to its list cap AND
    /// byte budget — either one can drop entries, independently of the
    /// other, see `build_list_reply`'s docs), in no defined order, plus
    /// the full count and whether either cut actually truncated anything.
    /// Always a live round trip — the helm caches no session state,
    /// because SPEC.md makes supervisors the authority.
    ///
    /// `cursor`/`limit` (PLAN_M6.md item 1's wire vocabulary) are sent as
    /// `None` unconditionally: this PR ships the vocabulary, not helm-side
    /// paging, so there is nothing yet to resume FROM and no reason to ask
    /// for anything but the server's default page. `SessionListing`'s own
    /// `truncated` — this method's REST-facing surface, unlike the wire's
    /// `next_cursor` — keeps its pre-8 meaning by construction (see that
    /// field's own docs).
    pub async fn list_sessions(&self) -> anyhow::Result<SessionListing> {
        let req_id = self.req_id();
        match self
            .request(
                req_id,
                ControlMsg::ListSessions {
                    req_id,
                    cursor: None,
                    limit: None,
                },
            )
            .await?
        {
            ControlMsg::SessionList {
                sessions,
                total,
                next_cursor,
                ..
            } => {
                // Normalized against `sessions.len()`: an older
                // `PROTOCOL_VERSION` 3 supervisor built before `total`
                // existed omits the field entirely, which
                // `#[serde(default)]` decodes as 0 (see
                // `ControlMsg::SessionList`'s own docs) — even though its
                // `sessions` vec is complete and non-empty. Reporting a
                // raw `total: 0` alongside a populated list would be
                // actively misleading to a caller displaying "showing N
                // of M" (PLAN_M2.md's UI contract): `total` must never be
                // smaller than the number of sessions actually in hand.
                let total = total.max(sessions.len() as u64);
                Ok(SessionListing {
                    // PLAN_M6.md item 1: the wire's `truncated` bool became
                    // `next_cursor` (absence means exhaustion), but this PR
                    // is vocabulary only — every supervisor this build can
                    // reach still answers `next_cursor: None`
                    // unconditionally, whether or not a cut actually
                    // happened (`build_list_reply`'s own docs). A bare
                    // `next_cursor.is_some()` would therefore report
                    // `false` for a count-capped or byte-budget-cut list
                    // that v7 honestly flagged `truncated: true` — the one
                    // REST-visible regression this synthesis exists to
                    // avoid. `sessions.len() < total` IS that cut, by
                    // `total`'s own definition (the full count before any
                    // cut), so the disjunction keeps the REST body faithful
                    // both now and after PLAN_M6.md item 2 gives
                    // `next_cursor` a real Some/None distinction.
                    truncated: next_cursor.is_some() || (sessions.len() as u64) < total,
                    total,
                    sessions,
                })
            }
            other => bail!("unexpected reply to list_sessions: {other:?}"),
        }
    }

    /// Kill the agent's whole process tree while leaving the session and
    /// its terminal in place (SPEC.md's "stop"). Idempotent: stopping an
    /// already-stopped or never-live session still succeeds.
    ///
    /// `Err` covers two different things, not one: the connection to the
    /// supervisor itself failing (a dead transport, surfaced as a plain
    /// `anyhow::Error`), or the supervisor answering with an `Error` reply
    /// — which itself can mean either the request was rejected outright
    /// (an unknown `id`) or that it was accepted but the kill sweep could
    /// not be confirmed complete (both surfaced via `SupervisorError`,
    /// `downcast_ref`-able from the returned error). `Ok` is the only
    /// outcome that means the sweep actually ran to completion.
    pub async fn stop_session(&self, id: &str) -> anyhow::Result<()> {
        let req_id = self.req_id();
        match self
            .request(
                req_id,
                ControlMsg::StopSession {
                    req_id,
                    session_id: id.to_string(),
                },
            )
            .await?
        {
            ControlMsg::SessionStopped { .. } => Ok(()),
            other => bail!("unexpected reply to stop_session: {other:?}"),
        }
    }

    /// Relaunch a session's agent (SPEC.md's restart, PLAN_M3.md item 9),
    /// returning the session's freshly recomputed state.
    ///
    /// `mode` must match the session's CURRENT `restart_offer`, which the
    /// supervisor recomputes at handling time rather than trusting the
    /// caller's cached copy: a mismatch comes back as a
    /// [`SupervisorError`] with `ErrorKind::Conflict` naming what the offer
    /// is now, and the caller's correct response is to refresh the session
    /// and re-present that offer — never to retry the same request (see
    /// `ControlMsg::RestartSession`'s staleness contract).
    ///
    /// `stop_if_running` carries the user's explicit consent to stop a
    /// still-running agent first. Without it, a restart against an agent
    /// the supervisor finds alive is refused with the same `Conflict`
    /// shape, which is what keeps a stale client-side "it looked exited"
    /// from silently killing a live process.
    pub async fn restart_session(
        &self,
        id: &str,
        mode: RestartMode,
        stop_if_running: bool,
    ) -> anyhow::Result<SessionInfo> {
        let req_id = self.req_id();
        match self
            .request(
                req_id,
                ControlMsg::RestartSession {
                    req_id,
                    session_id: id.to_string(),
                    mode,
                    stop_if_running,
                },
            )
            .await?
        {
            ControlMsg::SessionRestarted { session, .. } => Ok(session),
            other => bail!("unexpected reply to restart_session: {other:?}"),
        }
    }

    /// Rename a session (PLAN_M5.md item 4; SPEC.md's v1 rename verb),
    /// returning the session's freshly recomputed `SessionInfo` — built the
    /// way `list_sessions` builds one, not a stale row with the title
    /// spliced in (see `ControlMsg::SessionRenamed`'s docs).
    ///
    /// `title` travels to the supervisor VERBATIM: this method neither
    /// trims nor validates it. The supervisor is the sole authority on
    /// what title is acceptable (`ControlMsg::RenameSession`'s
    /// control-character refusal and size cap), and duplicating that rule
    /// here would only give it a second place to drift from the real one.
    /// A refusal — an unknown `id`, or a title the supervisor rejects —
    /// arrives as a [`SupervisorError`] the caller can downcast for its
    /// `ErrorKind` and message, exactly like every other request on this
    /// client.
    pub async fn rename_session(&self, id: &str, title: &str) -> anyhow::Result<SessionInfo> {
        let req_id = self.req_id();
        match self
            .request(
                req_id,
                ControlMsg::RenameSession {
                    req_id,
                    session_id: id.to_string(),
                    title: title.to_string(),
                },
            )
            .await?
        {
            ControlMsg::SessionRenamed { session, .. } => Ok(session),
            other => bail!("unexpected reply to rename_session: {other:?}"),
        }
    }

    /// Remove a session and all its stored state, in any state (SPEC.md's
    /// "delete"). After this returns `Ok`, the session no longer appears
    /// in `list_sessions` and `attach` against its id fails as unknown.
    ///
    /// `Err` is not only "the supervisor rejected the request" (an
    /// unknown `id`): the supervisor fails the whole delete, row and map
    /// entry intact, if the process-tree kill, the tmux teardown, or the
    /// launch-artifact cleanup could not be confirmed complete — removing
    /// the last handle on a possibly-running agent (or leaving a
    /// credential-bearing launch spec behind with nothing left to clean
    /// it up) is the outcome delete must never risk (see
    /// lore/2026-07-27-m2-process-tree-stop.md). A transport failure
    /// (the connection itself going away) is a third, distinct cause,
    /// surfaced as a plain `anyhow::Error` rather than `SupervisorError`.
    /// Only `Ok` means the session and its state are actually gone.
    pub async fn delete_session(&self, id: &str) -> anyhow::Result<()> {
        let req_id = self.req_id();
        match self
            .request(
                req_id,
                ControlMsg::DeleteSession {
                    req_id,
                    session_id: id.to_string(),
                },
            )
            .await?
        {
            ControlMsg::SessionDeleted { .. } => Ok(()),
            other => bail!("unexpected reply to delete_session: {other:?}"),
        }
    }

    /// Open a terminal tab on a session (PLAN_M4.md item 2), returning the
    /// supervisor-minted tab identity to attach it by.
    ///
    /// Exists ahead of the helm's own REST plumbing (item 5) for
    /// [`Self::attach_terminal`]'s reason: the supervisor's tab behavior is
    /// only reachable from the integration tests through this client, and
    /// a second, test-only encoding of the same request would be a second
    /// place for the wire shape to drift.
    ///
    /// `Err` distinguishes nothing the caller has to branch on but does
    /// carry the supervisor's own refusal verbatim via [`SupervisorError`]:
    /// a vanished working directory, a session whose tmux is gone (restart
    /// it first), and a shell already dead by reply time all arrive here as
    /// errors naming what happened, with the session and its other
    /// terminals untouched in every case.
    pub async fn open_tab(&self, session_id: &str) -> anyhow::Result<TabInfo> {
        let req_id = self.req_id();
        match self
            .request(
                req_id,
                ControlMsg::OpenTab {
                    req_id,
                    session_id: session_id.to_string(),
                },
            )
            .await?
        {
            ControlMsg::TabOpened { tab, .. } => Ok(tab),
            other => bail!("unexpected reply to open_tab: {other:?}"),
        }
    }

    /// Close a terminal tab: kill its shell and everything that shell left
    /// behind, then drop its window.
    ///
    /// `Ok` means the reap ran to completion AND the window is gone —
    /// `SessionStopped`'s honesty rule applied per tab. It carries the
    /// same guarantee [`Self::stop_session`] does and no more: the sweep
    /// confirmed that everything it can find is gone, which on a host
    /// with a systemd user manager includes the tab's whole cgroup, and
    /// on a host without one excludes a descendant that both double-forked
    /// and scrubbed its environment (SPEC_impl.md's recorded boundary —
    /// the guarantee covers accidental daemonization, not an adversary).
    /// An unknown tab id is a `NotFound` [`SupervisorError`]; a tab whose
    /// shell had already exited still closes successfully.
    pub async fn close_tab(&self, session_id: &str, tab_id: &str) -> anyhow::Result<()> {
        let req_id = self.req_id();
        match self
            .request(
                req_id,
                ControlMsg::CloseTab {
                    req_id,
                    session_id: session_id.to_string(),
                    tab_id: tab_id.to_string(),
                },
            )
            .await?
        {
            ControlMsg::TabClosed { .. } => Ok(()),
            other => bail!("unexpected reply to close_tab: {other:?}"),
        }
    }

    /// Attach to a session's agent terminal under no lease — what every
    /// attach meant before M4, and what the helm's own request path still
    /// means until PLAN_M4.md item 5 gives the helm its tab and lease
    /// plumbing.
    ///
    /// The returned receiver yields the replay (history + mode
    /// re-synthesis) followed by live output, and finally a `Detached` if
    /// the attachment ends server-side.
    pub async fn attach(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<(u32, TermStream)> {
        self.attach_terminal(session_id, cols, rows, TerminalSelector::default(), "")
            .await
    }

    /// Attach naming the terminal and the lease explicitly.
    ///
    /// Exists ahead of the helm's own use of it so the supervisor's
    /// session-scoped takeover (PLAN_M4.md item 3) is reachable from the
    /// integration tests that drive a real supervisor through this
    /// client — those semantics are only observable to a caller that can
    /// mint two distinct leases, which [`Self::attach`] deliberately
    /// cannot. `lease` must be high-entropy and non-empty to group
    /// several terminals into one client; the empty lease means the
    /// pre-M4 singleton reading (see `ControlMsg::Attach`).
    pub async fn attach_terminal(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
        terminal: TerminalSelector,
        lease: &str,
    ) -> anyhow::Result<(u32, TermStream)> {
        let channel = allocate_channel(&self.next_channel)?;
        let (events_tx, events_rx) = mpsc::channel(TERM_EVENT_QUEUE);
        let (detach_tx, detach_rx) = watch::channel(None);
        let stream = TermStream {
            events: events_rx,
            detach: detach_rx,
            ended: false,
        };
        // Register before sending Attach: replay data frames may arrive
        // before the Attached reply is processed. The insert cannot clobber
        // anything — `allocate_channel` never hands out an id twice on this
        // connection.
        self.terminals.lock().await.insert(
            channel,
            TerminalHandle {
                events: events_tx,
                detach: detach_tx,
            },
        );

        let req_id = self.req_id();
        let result = self
            .request(
                req_id,
                ControlMsg::Attach {
                    req_id,
                    session_id: session_id.to_string(),
                    channel,
                    cols,
                    rows,
                    terminal,
                    lease: lease.to_string(),
                },
            )
            .await;
        if result.is_err() {
            self.terminals.lock().await.remove(&channel);
        }
        // The reply variant is checked like every other request's: a
        // mis-correlated success reply must not read as a live
        // attachment, or the caller waits on a terminal nobody is
        // feeding.
        match result? {
            ControlMsg::Attached { .. } => Ok((channel, stream)),
            other => {
                self.terminals.lock().await.remove(&channel);
                bail!("unexpected reply to attach: {other:?}")
            }
        }
    }

    /// Give up an attachment. Safe to call on a channel already torn down
    /// by a takeover, so every exit path can call it unconditionally.
    ///
    /// The local sender is removed from `terminals` before the message
    /// goes out, which discards output still in flight instead of
    /// delivering it to a caller that has stopped listening.
    pub async fn detach(&self, channel: u32) {
        self.terminals.lock().await.remove(&channel);
        let _ = self
            .writer_tx
            .send(Frame::control(&ControlMsg::Detach { channel }).into())
            .await;
    }

    /// Forward terminal input, chunked below the protocol's frame cap.
    ///
    /// Chunking is not an optimization: a browser can deliver a whole
    /// clipboard paste as one WebSocket message, and a single frame over
    /// `MAX_FRAME_LEN` is a fatal decode error on the supervisor side —
    /// one oversized paste would kill the shared connection and every
    /// session on it.
    ///
    /// Async since the outbound queue became bounded
    /// ([`SUPERVISOR_WRITER_QUEUE`]). Waiting here is the right answer for
    /// input specifically: keystrokes must never be silently dropped, and
    /// the only thing this can wait on is the socket itself draining.
    pub async fn send_input(&self, channel: u32, bytes: Vec<u8>) {
        for chunk in bytes.chunks(INPUT_CHUNK) {
            if self
                .writer_tx
                .send(Frame::data(channel, chunk.to_vec()).into())
                .await
                .is_err()
            {
                break;
            }
        }
    }

    /// Set the session's terminal size. Fire-and-forget by design: a
    /// browser emits these on every drag frame, so there is nothing to
    /// await and nothing to report. `channel` is the attach-time channel:
    /// the supervisor drops the resize unless that channel still holds
    /// the session's attachment, so a resize in flight from a client
    /// that just lost a takeover cannot reflow the winner's terminal.
    pub async fn resize(&self, session_id: &str, channel: u32, cols: u16, rows: u16) {
        let _ = self
            .writer_tx
            .send(
                Frame::control(&ControlMsg::Resize {
                    session_id: session_id.to_string(),
                    channel,
                    cols,
                    rows,
                })
                .into(),
            )
            .await;
    }

    /// Ask the supervisor to stop sending this attachment's output until
    /// a matching [`Self::resume_output`].
    ///
    /// The helm end of PLAN_M2_5.md's watermark flow control: the browser
    /// decides (from its own unflushed `term.write()` backlog) and this
    /// forwards that decision verbatim. Deliberately no state is kept
    /// here — the helm does not model whether a terminal "is" paused,
    /// because the supervisor already ignores a pause for a channel the
    /// sender no longer owns, and a second source of truth could only
    /// disagree with it.
    ///
    /// Fire-and-forget like `resize`, and for the same reason: there is
    /// no reply to correlate and nothing useful a caller could do with a
    /// failure that the connection dying will not already tell it.
    pub async fn pause_output(&self, channel: u32) {
        let _ = self
            .writer_tx
            .send(Frame::control(&ControlMsg::PauseOutput { channel }).into())
            .await;
    }

    /// Tell the supervisor this attachment's client has drained below its
    /// low-water mark and output may flow again. See [`Self::pause_output`].
    pub async fn resume_output(&self, channel: u32) {
        let _ = self
            .writer_tx
            .send(Frame::control(&ControlMsg::ResumeOutput { channel }).into())
            .await;
    }

    /// Start an attachment upload into a session's attachments directory
    /// (PLAN_M4.md item 5 — the helm's half of the pinned attachment REST
    /// contract), returning the [`UploadGuard`] that owns the transfer
    /// from here on.
    ///
    /// The guard, not a bare channel number, is what callers get, because
    /// an accepted upload is an OBLIGATION: the supervisor is holding a
    /// temp file and admission capacity until this side either commits or
    /// aborts, and "the relay's future was dropped" is a perfectly
    /// ordinary way for that side to end (a browser resetting the
    /// connection mid-body cancels the axum handler outright, running no
    /// error branch of any kind). Ownership makes the abort structural:
    /// dropping the guard enqueues one, whatever the reason for the drop.
    ///
    /// The guard exists BEFORE the request goes out — it is created
    /// alongside the `uploads` entry and covers this method's own await —
    /// so a cancellation between registering and hearing `UploadStarted`
    /// cleans up and aborts too, rather than leaking an entry for a
    /// transfer the supervisor may well have accepted.
    ///
    /// # Errors
    ///
    /// Two distinguishable shapes, and callers that map to HTTP care about
    /// the difference:
    ///
    /// - The supervisor REFUSED the begin — an unknown session, channel 0,
    ///   a channel in use, its admission cap — which arrives as a
    ///   [`SupervisorError`] carrying the message verbatim and a `kind` to
    ///   map to a status. Nothing exists on disk (`ControlMsg::
    ///   UploadStarted`'s own words), so nothing is aborted.
    /// - The exchange never completed: a dead connection, or a reply that
    ///   is not an `UploadStarted` for this channel. These are plain
    ///   `anyhow` errors with no `kind` to recover, and map to a 500.
    ///
    /// A filename is never in either set: per `ControlMsg::BeginUpload`,
    /// a proposed name is only ever sanitized or replaced.
    pub async fn begin_upload(
        self: &Arc<Self>,
        session_id: &str,
        filename: &str,
        size: u64,
    ) -> anyhow::Result<UploadGuard> {
        let channel = allocate_channel(&self.next_channel)?;
        let (progress_tx, progress_rx) = watch::channel(UploadProgress {
            declared: size,
            sent: 0,
            received: 0,
            ended: None,
        });
        self.uploads.lock().await.insert(channel, progress_tx);
        // Armed from this line on: every exit below either disarms the
        // guard explicitly or lets it drop, and a drop is an abort.
        let mut guard = UploadGuard {
            client: Arc::clone(self),
            channel,
            progress: progress_rx,
            retired: false,
        };

        let req_id = self.req_id();
        let result = self
            .request(
                req_id,
                ControlMsg::BeginUpload {
                    req_id,
                    session_id: session_id.to_string(),
                    channel,
                    filename: filename.to_string(),
                    size,
                },
            )
            .await;
        let reply = match result {
            Ok(reply) => reply,
            // A refusal created nothing upstream, and a transport failure
            // leaves nothing an abort could reach, so both retire quietly
            // instead of enqueueing an abort for a transfer that does not
            // exist.
            Err(e) => {
                guard.retire().await;
                return Err(e);
            }
        };
        match reply {
            // The echoed channel is checked, not assumed: it is what
            // `UploadStarted` grants credit for, so a mismatch would have
            // this side streaming bytes onto a channel the supervisor
            // never opened while the transfer it DID open sat idle. The
            // guard's drop aborts the channel this side asked for, and no
            // data has been sent.
            ControlMsg::UploadStarted {
                channel: started, ..
            } if started == channel => Ok(guard),
            ControlMsg::UploadStarted {
                channel: started, ..
            } => {
                bail!("supervisor started upload on channel {started}, not the requested {channel}")
            }
            other => bail!("unexpected reply to begin_upload: {other:?}"),
        }
    }
}

/// One in-flight attachment upload, owned by the single task relaying it.
///
/// The send-direction counterpart of `TermStream`, with one extra job that
/// shapes the whole type: it is the upload's CANCELLATION-SAFE owner. Every
/// way this transfer can end — commit, an explicit abort, a supervisor-side
/// abort, a stall, or simply the owning future being dropped — retires the
/// local bookkeeping, and every ending that leaves the supervisor still
/// holding a temp file sends an `AbortUpload`. That is why the relay holds
/// this value rather than a channel id: a channel id has no destructor.
///
/// Terminal state is read through a persistent `watch::Receiver` held for
/// the upload's whole life, which is what lets an abort that arrived while
/// the relay was parked somewhere else (awaiting the next body chunk, or a
/// commit reply) still be reported with the supervisor's own words.
pub struct UploadGuard {
    client: Arc<SupervisorClient>,
    channel: u32,
    progress: watch::Receiver<UploadProgress>,
    /// Set once the local entry has been removed by an owner-driven path,
    /// so `Drop` knows there is nothing left to clean up or abort.
    retired: bool,
}

/// Hand-written because the client behind an upload is not `Debug` and
/// would be noise anyway: what identifies a guard is its channel and how
/// far the transfer has got.
impl std::fmt::Debug for UploadGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadGuard")
            .field("channel", &self.channel)
            .field("progress", &*self.progress.borrow())
            .field("retired", &self.retired)
            .finish()
    }
}

impl UploadGuard {
    /// The data channel this upload streams on — for the relay's own
    /// logging; the guard's methods never need it passed back.
    pub fn channel(&self) -> u32 {
        self.channel
    }

    /// The upload's terminal reason if one has already been published,
    /// without awaiting. Synchronous because `watch::Receiver::borrow`
    /// is, which is what lets `Drop` consult it.
    fn terminal_reason(&self) -> Option<String> {
        self.progress.borrow().ended.clone()
    }

    /// Resolve once this upload has ended for any reason, with the reason
    /// verbatim; never resolves while the transfer is healthy.
    ///
    /// The relay selects on this while reading its HTTP body so a
    /// supervisor-side abort ends the request promptly instead of after
    /// the browser's next chunk (or, for a browser that has gone quiet,
    /// after the body stall timeout). Cancel-safe: dropping the returned
    /// future loses nothing, since the watch's version counter, not this
    /// call, is what tracks what has been observed.
    pub async fn ended(&mut self) -> String {
        wait_ended(&mut self.progress).await
    }

    /// Wait until `additional` more bytes fit inside the credit window,
    /// or until the transfer ends.
    ///
    /// Two independent ways out besides success: a terminal outcome
    /// (reported verbatim), and the per-hop stall deadline
    /// ([`UPLOAD_ACK_STALL_TIMEOUT`]) that fires when the window stays
    /// shut with no ADVANCING ack. Only a strictly advancing ack rearms
    /// the deadline — the demultiplexer does not even wake this wait for a
    /// repeated count — so a peer cannot keep a transfer alive by
    /// restating progress it already reported.
    ///
    /// The arithmetic is deliberately subtractive: `sent - received` is
    /// the outstanding count, which cannot overflow because ack validation
    /// keeps `received <= sent`, whereas the equivalent `received +
    /// window` comparison would put peer-supplied numbers on the growing
    /// side of an addition.
    async fn wait_for_credit(&mut self, additional: u64) -> Result<(), String> {
        let mut deadline = Instant::now() + self.client.upload_stall;
        let mut last_received = self.progress.borrow().received;
        loop {
            let snapshot = self.progress.borrow_and_update().clone();
            if let Some(reason) = snapshot.ended {
                return Err(reason);
            }
            let outstanding = snapshot.sent.saturating_sub(snapshot.received);
            if outstanding.saturating_add(additional) <= UPLOAD_WINDOW_BYTES {
                return Ok(());
            }
            // The select's value is taken out before anything acts on it:
            // its arms hold a borrow of `self.progress`, and the stall
            // path needs `&mut self` back to abort.
            let woke = tokio::select! {
                _ = tokio::time::sleep_until(deadline) => None,
                changed = self.progress.changed() => Some(changed),
            };
            match woke {
                None => {
                    let stalled = farhelm_proto::UPLOAD_ABORT_REASON_STALLED.to_string();
                    warn!(
                        channel = self.channel,
                        "upload stalled: no advancing supervisor ack"
                    );
                    self.abort(stalled.clone()).await;
                    return Err(stalled);
                }
                Some(Err(_)) => return Err("supervisor connection lost".to_string()),
                Some(Ok(())) => {
                    let received = self.progress.borrow().received;
                    if received > last_received {
                        last_received = received;
                        deadline = Instant::now() + self.client.upload_stall;
                    }
                }
            }
        }
    }

    /// Forward attachment bytes, rechunked to at most
    /// [`UPLOAD_CHUNK_BYTES`] regardless of the caller's own chunk
    /// boundaries — rechunking is the PROTOCOL-FRAME SENDER's job
    /// (`UPLOAD_CHUNK_BYTES`'s own docs), so the helm relay must split at
    /// this size no matter how its HTTP body stream happened to arrive.
    ///
    /// Two bounds apply per chunk, and they are not the same bound. The
    /// credit window caps unacknowledged bytes in TRANSIT
    /// ([`UPLOAD_WINDOW_BYTES`]); the connection-wide enqueue allowance
    /// ([`UPLOAD_ENQUEUE_FRAMES`]) caps how many upload frames may sit in
    /// the shared writer queue, which is what actually keeps a keystroke
    /// from queueing behind megabytes of attachment data. The window is
    /// waited on first so an allowance is never held while parked on
    /// credit.
    ///
    /// The running byte count lives in the upload's shared state rather
    /// than a caller-threaded total, because ack validation on the demux
    /// side has to see it; callers just hand over bytes. On `Ok` every
    /// byte has been enqueued; on `Err` the reason is user-legible (the
    /// supervisor's abort verbatim, a stall, or this connection's death)
    /// and the transfer is over — it never partially succeeds silently.
    pub async fn send_upload_chunk(&mut self, bytes: &[u8]) -> Result<(), String> {
        for chunk in bytes.chunks(UPLOAD_CHUNK_BYTES) {
            self.wait_for_credit(chunk.len() as u64).await?;
            let allowance = Arc::clone(&self.client.upload_enqueue)
                .acquire_owned()
                .await
                .map_err(|_| "supervisor connection lost".to_string())?;
            // Counted before the frame is enqueued, so `sent` is never
            // behind what the supervisor could already have acknowledged
            // — an ack past it has to stay a genuine protocol violation
            // rather than a race this side loses.
            self.publish(|p| p.sent += chunk.len() as u64).await;
            let frame = Outbound {
                frame: Frame::data(self.channel, chunk.to_vec()),
                _allowance: Some(allowance),
            };
            if self.client.writer_tx.send(frame).await.is_err() {
                return Err("supervisor connection lost".to_string());
            }
        }
        Ok(())
    }

    /// All bytes sent; ask the supervisor to publish the file, returning
    /// the raw absolute host path `ControlMsg::UploadCommitted::path`
    /// carries.
    ///
    /// Consumes the guard, which is what makes the commit itself
    /// cancellation-safe: if the awaiting future is dropped before the
    /// reply lands, the guard drops with it and the supervisor gets its
    /// `AbortUpload` — rather than a permanently pending transfer and a
    /// local entry nothing would ever remove.
    ///
    /// An abort takes precedence over the commit exchange, before it and
    /// during it. A supervisor that gave up mid-transfer has already
    /// explained why, and that reason is what the user must see; the
    /// correlated commit error for a channel that no longer carries an
    /// upload would say only that the commit failed. Any ANSWERED commit
    /// — published, or refused for a size mismatch or storage failure —
    /// is terminal by `ControlMsg::CommitUpload`'s docs, so the guard
    /// retires either way.
    pub async fn commit(mut self) -> anyhow::Result<String> {
        if let Some(reason) = self.terminal_reason() {
            self.retire().await;
            return Err(anyhow::Error::new(SupervisorError {
                kind: ErrorKind::Internal,
                message: reason,
            }));
        }
        let client = Arc::clone(&self.client);
        let channel = self.channel;
        let req_id = client.req_id();
        // A cloned receiver rather than `self.progress`, so the select
        // below borrows nothing that the retire/abort calls in its arms
        // need back.
        let mut progress = self.progress.clone();
        // The reply is polled FIRST, and the ordering matters in one
        // specific race: a supervisor that answers the commit and then
        // closes the connection publishes a terminal "connection lost" on
        // the way down, and preferring that over a reply already sitting
        // in hand would turn a published upload into a spurious failure.
        // An abort still wins whenever it is the reason the commit has no
        // answer, which is the case this arm exists for — it reached the
        // demultiplexer before any reply could.
        let reply = tokio::select! {
            biased;
            reply = client.request(req_id, ControlMsg::CommitUpload { req_id, channel }) => reply,
            reason = wait_ended(&mut progress) => {
                self.retire().await;
                return Err(anyhow::Error::new(SupervisorError {
                    kind: ErrorKind::Internal,
                    message: reason,
                }));
            }
        };
        self.retire().await;
        match reply? {
            ControlMsg::UploadCommitted { path, .. } => Ok(path),
            other => bail!("unexpected reply to commit_upload: {other:?}"),
        }
    }

    /// Give the transfer up from this side, telling the supervisor to drop
    /// it and clean its temp file.
    ///
    /// `reason` never reaches the wire — `ControlMsg::AbortUpload` carries
    /// no reason field, since the abandoning side has nothing to explain
    /// to the receiver — but it is recorded locally so any later look at
    /// this upload reports why it ended rather than that it merely
    /// vanished.
    ///
    /// Delivery is handed to an independent task (see
    /// [`SupervisorClient::enqueue_abort`]) rather than awaited here, so
    /// an abort cannot be lost to the cancellation that prompted it.
    pub async fn abort(&mut self, reason: String) {
        self.publish(|p| {
            if p.ended.is_none() {
                p.ended = Some(reason);
            }
        })
        .await;
        self.client.enqueue_abort(self.channel);
        self.retire().await;
    }

    /// Mutate this upload's shared state, if the entry is still there.
    ///
    /// A retired upload has no entry, which is not an error: the
    /// bookkeeping only exists to be observed by this guard and the
    /// demultiplexer, and both are done with it by then.
    async fn publish(&self, update: impl FnOnce(&mut UploadProgress)) {
        if let Some(progress) = self.client.uploads.lock().await.get(&self.channel) {
            progress.send_modify(update);
        }
    }

    /// Retire the local bookkeeping and disarm the drop-time abort.
    ///
    /// Called only from paths that have established the supervisor needs
    /// nothing further: a published or refused commit, a refused begin, or
    /// an abort already handed to its own task.
    async fn retire(&mut self) {
        self.client.uploads.lock().await.remove(&self.channel);
        self.retired = true;
    }
}

/// Wait for an upload's terminal reason on a receiver the caller holds.
///
/// A free function rather than only a method because [`UploadGuard::commit`]
/// needs to watch for the outcome on a CLONED receiver: its other select arm
/// wants the guard back, and borrowing `self.progress` there would pin it for
/// the whole race. [`UploadGuard::ended`] is the same wait for callers that
/// can borrow the guard.
///
/// Cancel-safe: dropping the returned future loses nothing, since the
/// watch's version counter — not this call — is what tracks what has been
/// observed. A sender dropped without publishing an outcome is only
/// reachable if the whole client went away (the guard's `Arc` normally
/// prevents it), and is reported as the connection loss it is rather than
/// hanging forever.
async fn wait_ended(progress: &mut watch::Receiver<UploadProgress>) -> String {
    loop {
        if let Some(reason) = progress.borrow_and_update().ended.clone() {
            return reason;
        }
        if progress.changed().await.is_err() {
            return progress
                .borrow()
                .ended
                .clone()
                .unwrap_or_else(|| "supervisor connection lost".to_string());
        }
    }
}

impl Drop for UploadGuard {
    /// The whole reason this type exists: an upload whose owner went away
    /// without saying so still gets torn down at both ends.
    ///
    /// The cleanup runs on a spawned task because a destructor cannot
    /// await — and because the abort MUST NOT be tied to whatever is being
    /// dropped, which by definition may already have been cancelled. An
    /// upload that already ended (the supervisor aborted it, this side
    /// aborted it, the connection died) owes no further message; anything
    /// else does, and `AbortUpload` is idempotent, so sending one for a
    /// transfer the supervisor never accepted is harmless.
    fn drop(&mut self) {
        if self.retired {
            return;
        }
        let needs_abort = self.terminal_reason().is_none();
        let client = Arc::clone(&self.client);
        let channel = self.channel;
        // Guards normally drop inside a task, but a destructor can run
        // anywhere; without a runtime there is nothing to clean up on
        // either — the connection is going away with the process.
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        handle.spawn(async move {
            if needs_abort {
                let _ = client
                    .writer_tx
                    .send(Frame::control(&ControlMsg::AbortUpload { channel }).into())
                    .await;
            }
            client.uploads.lock().await.remove(&channel);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use std::io;
    use std::pin::Pin;
    use std::sync::atomic::AtomicBool;
    use std::task::Poll;
    use tokio::time::timeout;

    /// A write half whose next frame can fail while its read peer stays
    /// open, matching the asymmetric failure possible over ssh.
    struct ToggleWriteFailure<W> {
        inner: W,
        fail_writes: Arc<AtomicBool>,
    }

    impl<W: AsyncWrite + Unpin> AsyncWrite for ToggleWriteFailure<W> {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            if self.fail_writes.load(Ordering::SeqCst) {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "injected client write failure",
                )));
            }
            Pin::new(&mut self.inner).poll_write(cx, buf)
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_flush(cx)
        }

        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_shutdown(cx)
        }
    }

    /// Register a terminal on `client` directly, returning the receiving
    /// half a real `attach()` caller would have got.
    ///
    /// Bypasses `attach` on purpose: these tests drive scripted peers that
    /// would otherwise have to play out a full attach exchange per
    /// channel, which is irrelevant to what any of them is about.
    async fn register_terminal(
        client: &SupervisorClient,
        channel: u32,
        capacity: usize,
    ) -> TermStream {
        let (events_tx, events_rx) = mpsc::channel(capacity);
        let (detach_tx, detach_rx) = watch::channel(None);
        client.terminals.lock().await.insert(
            channel,
            TerminalHandle {
                events: events_tx,
                detach: detach_tx,
            },
        );
        TermStream {
            events: events_rx,
            detach: detach_rx,
            ended: false,
        }
    }

    /// Register an upload on `client` directly, returning the same
    /// [`UploadGuard`] a real `begin_upload` would have handed back.
    ///
    /// Bypasses `begin_upload`'s own `BeginUpload`/`UploadStarted`
    /// exchange — the send-side sibling of `register_terminal`, for the
    /// same reason: the tests that use this are about rechunking,
    /// credit-window, stall, ack-validation and teardown behavior, not
    /// about the begin handshake, so scripting that handshake in every one
    /// of them would only obscure what each test is actually pinning.
    ///
    /// `declared` is `BeginUpload`'s size, which ack validation checks
    /// against, so a test that means to send N bytes must declare at least
    /// N or its own acks become protocol violations.
    async fn register_upload(
        client: &Arc<SupervisorClient>,
        channel: u32,
        declared: u64,
    ) -> UploadGuard {
        let (progress_tx, progress_rx) = watch::channel(UploadProgress {
            declared,
            sent: 0,
            received: 0,
            ended: None,
        });
        client.uploads.lock().await.insert(channel, progress_tx);
        UploadGuard {
            client: Arc::clone(client),
            channel,
            progress: progress_rx,
            retired: false,
        }
    }

    /// Wait for `client` to have no upload bookkeeping left, or fail.
    ///
    /// Teardown after a cancellation is deliberately asynchronous — an
    /// `UploadGuard`'s destructor hands the work to its own task so the
    /// abort cannot be lost with whatever was cancelled — so "the entry is
    /// gone" is a property that becomes true shortly after the drop, not
    /// at it. Polling with a bound turns that into an assertion that fails
    /// loudly instead of a sleep long enough to be flaky.
    async fn await_no_uploads(client: &SupervisorClient) {
        timeout(Duration::from_secs(5), async {
            while !client.uploads.lock().await.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("upload bookkeeping was never retired");
    }

    fn session(id: &str) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            title: id.into(),
            created_at: 1_700_000_000,
            cwd: format!("/{id}"),
            invocation: "agent".into(),
            status: farhelm_proto::SessionStatus::Alive,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
        }
    }

    /// A healthy connection must close when its final external client
    /// handle is dropped. Capturing a strong Arc in either background
    /// task creates a cycle that keeps the peer open forever.
    #[tokio::test]
    async fn dropping_the_last_client_handle_closes_the_transport() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            reader.read_frame().await.unwrap()
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        drop(client);

        assert!(
            timeout(Duration::from_secs(2), peer)
                .await
                .expect("peer did not observe EOF after client drop")
                .unwrap()
                .is_none()
        );
    }

    /// Request correlation is keyed by req_id, not arrival order. Two
    /// reversed replies must still complete the matching futures.
    #[tokio::test]
    async fn concurrent_requests_accept_replies_in_reverse_order() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let first = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let second = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let req_id = |message: ControlMsg| match message {
                ControlMsg::ListSessions { req_id, .. } => req_id,
                other => panic!("unexpected request: {other:?}"),
            };
            let first = req_id(first);
            let second = req_id(second);
            for req_id in [second, first] {
                writer
                    .write_control(&ControlMsg::SessionList {
                        req_id,
                        sessions: vec![session(&req_id.to_string())],
                        total: 1,
                        next_cursor: None,
                    })
                    .await
                    .unwrap();
            }
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let first = client.request(
            101,
            ControlMsg::ListSessions {
                req_id: 101,
                cursor: None,
                limit: None,
            },
        );
        let second = client.request(
            202,
            ControlMsg::ListSessions {
                req_id: 202,
                cursor: None,
                limit: None,
            },
        );
        let (first, second) = tokio::join!(first, second);

        assert!(matches!(
            first.unwrap(),
            ControlMsg::SessionList { req_id: 101, sessions, .. }
                if sessions[0].id == "101"
        ));
        assert!(matches!(
            second.unwrap(),
            ControlMsg::SessionList { req_id: 202, sessions, .. }
                if sessions[0].id == "202"
        ));
        peer.await.unwrap();
    }

    /// `truncated` synthesizes from a disjunction —
    /// `next_cursor.is_some() || sessions.len() < total` (see that call
    /// site's own docs) — and this case isolates the FIRST disjunct: a
    /// `next_cursor: Some(_)` with `total == sessions.len()`, so the count
    /// half of the disjunction is false and cannot be the one making
    /// `truncated` true. A regression that dropped `next_cursor.is_some()`
    /// from the synthesis, keeping only the count comparison, would
    /// compute `false` here and go uncaught by a test that let both
    /// conditions hold at once.
    #[tokio::test]
    async fn list_sessions_reports_truncated_from_next_cursor_alone() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::ListSessions { req_id, .. } = request else {
                panic!("unexpected request: {request:?}");
            };
            writer
                .write_control(&ControlMsg::SessionList {
                    req_id,
                    sessions: vec![session("only-one")],
                    total: 1,
                    next_cursor: Some("opaque-cursor-value".to_string()),
                })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let listing = client.list_sessions().await.unwrap();
        assert_eq!(listing.sessions.len(), 1);
        assert_eq!(listing.total, 1);
        assert!(
            listing.truncated,
            "next_cursor alone must synthesize truncated: true even when total == sessions.len()"
        );
        peer.await.unwrap();
    }

    /// `truncated` synthesizes from a disjunction —
    /// `next_cursor.is_some() || sessions.len() < total` (see that call
    /// site's own docs) — and this case isolates the SECOND disjunct: a
    /// `next_cursor: None` with a `total` (sentinel `42`, deliberately far
    /// from `sessions.len()`) that proves an honest, larger `total` a
    /// truncating supervisor sent on purpose survives untouched, rather
    /// than being recomputed or dropped. A regression that dropped
    /// `sessions.len() < total` from the synthesis, keeping only
    /// `next_cursor.is_some()`, would compute `false` here and go
    /// uncaught by a test that let both conditions hold at once.
    ///
    /// This is deliberately NOT a claim that every `total` value is
    /// preserved verbatim: `total.max(sessions.len())` (see that call
    /// site's own docs, for the case where an old supervisor's `total`
    /// UNDER-reports by omitting the field) rewrites a `total` smaller
    /// than the list actually in hand — a different test
    /// (`list_sessions_reports_a_populated_legacy_reply_with_a_real_total`)
    /// pins that rewrite specifically.
    #[tokio::test]
    async fn list_sessions_reports_truncated_from_a_larger_total_alone() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::ListSessions { req_id, .. } = request else {
                panic!("unexpected request: {request:?}");
            };
            writer
                .write_control(&ControlMsg::SessionList {
                    req_id,
                    sessions: vec![session("only-one")],
                    total: 42,
                    next_cursor: None,
                })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let listing = client.list_sessions().await.unwrap();
        assert_eq!(listing.sessions.len(), 1);
        assert_eq!(
            listing.total, 42,
            "an honest, larger total than sessions.len() must survive unchanged"
        );
        assert!(
            listing.truncated,
            "a larger total alone must synthesize truncated: true even when next_cursor is None"
        );
        peer.await.unwrap();
    }

    /// The other half of the normalization at `list_sessions`'s call
    /// site: an older `PROTOCOL_VERSION` 3 supervisor, built before
    /// `total` existed, sends a `SessionList` with the field simply
    /// ABSENT — `#[serde(default)]` decodes that as `total: 0` (see
    /// `ControlMsg::SessionList`'s own docs) — even though its `sessions`
    /// vec is complete and non-empty. Forwarding that raw `0` would be
    /// actively misleading ("showing 0 of 0" next to a visibly populated
    /// list), so `total.max(sessions.len())` must rewrite it up to the
    /// real count. This is the scripted-peer complement to the pure
    /// `total.max(...)` arithmetic itself: it pins that the client's
    /// `list_sessions` call site actually applies the rewrite to a wire
    /// reply shaped exactly like a real legacy sender's, not just to a
    /// value constructed directly in a unit test.
    ///
    /// It doubles as the BOTH-FALSE case of `truncated`'s disjunction
    /// (`next_cursor.is_some() || sessions.len() < total`): `next_cursor`
    /// is absent (decodes to `None`) and, after the rewrite above,
    /// `total == sessions.len()`. A regression that made either disjunct
    /// unconditionally `true` — reporting a cut that never happened —
    /// would flip `listing.truncated` here and be caught by the assertion
    /// below.
    #[tokio::test]
    async fn list_sessions_reports_a_populated_legacy_reply_with_a_real_total() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::ListSessions { req_id, .. } = request else {
                panic!("unexpected request: {request:?}");
            };
            // Write the JSON by hand, WITHOUT `total`/`next_cursor` at all —
            // constructing a `ControlMsg::SessionList` in Rust and just
            // not setting them is not possible (the fields are required
            // in this build's own type); omitting them from the wire is
            // exactly what an actual older build's serializer would
            // produce, which is the scenario under test.
            let body = serde_json::json!({
                "type": "session_list",
                "req_id": req_id,
                "sessions": [
                    { "id": "a", "title": "a", "cwd": "/a", "invocation": "agent" },
                    { "id": "b", "title": "b", "cwd": "/b", "invocation": "agent" },
                ],
            });
            writer
                .write_frame(&Frame {
                    kind: FrameKind::Control,
                    channel: 0,
                    body: serde_json::to_vec(&body).unwrap(),
                })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let listing = client.list_sessions().await.unwrap();
        assert_eq!(listing.sessions.len(), 2);
        assert_eq!(
            listing.total, 2,
            "an absent (defaulted-to-0) total must be rewritten up to the real session \
             count, not forwarded as a false '0 sessions' claim"
        );
        assert!(!listing.truncated);
        peer.await.unwrap();
    }

    /// A broken write half must fail pending requests and detach
    /// terminals even while the peer deliberately keeps its write half
    /// open. Read-side EOF cannot rescue this path.
    #[tokio::test]
    async fn writer_failure_fails_waiters_on_a_half_broken_transport() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let fail_writes = Arc::new(AtomicBool::new(false));
        let w = ToggleWriteFailure {
            inner: w,
            fail_writes: Arc::clone(&fail_writes),
        };
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, _peer_writer) = peer.await.unwrap();
        let mut term_rx = register_terminal(&client, 7, TERM_EVENT_QUEUE).await;

        fail_writes.store(true, Ordering::SeqCst);
        let request = timeout(Duration::from_secs(2), client.list_sessions())
            .await
            .expect("request hung after writer failure")
            .expect_err("request must fail after writer failure");
        assert!(request.to_string().contains("connection closed"));
        assert!(matches!(
            timeout(Duration::from_secs(2), term_rx.recv())
                .await
                .expect("terminal did not detach"),
            Some(TermEvent::Detached(reason)) if reason.contains("connection lost")
        ));
        assert!(
            timeout(Duration::from_secs(2), peer_reader.read_frame())
                .await
                .expect("client demux survived writer failure")
                .unwrap()
                .is_none()
        );
    }

    /// Read EOF must cancel the parked writer even while an external
    /// client handle keeps its sender alive. Otherwise a half-closed
    /// supervisor leaves the write task and transport descriptor parked
    /// for the lifetime of AppState.
    #[tokio::test]
    async fn reader_eof_cancels_the_writer_transport() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let _client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();

        peer_writer.shutdown().await.unwrap();

        assert!(
            timeout(Duration::from_secs(2), peer_reader.read_frame())
                .await
                .expect("client writer survived read EOF")
                .unwrap()
                .is_none()
        );
    }

    /// Malformed control JSON is a fatal protocol violation. Ignoring it
    /// loses the req_id and leaves the matching request parked forever.
    #[tokio::test]
    async fn malformed_control_frame_fails_pending_requests() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let _request = reader.read_frame().await.unwrap().unwrap();
            writer
                .write_frame(&Frame {
                    kind: FrameKind::Control,
                    channel: 0,
                    body: b"{".to_vec(),
                })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        timeout(Duration::from_secs(2), client.list_sessions())
            .await
            .expect("request hung after malformed control frame")
            .expect_err("malformed control frame must fail the request");
        peer.await.unwrap();
    }

    /// The single most important test of the helm's bounded-queue rule:
    /// one terminal whose consumer has stopped draining must be detached,
    /// and must NOT stall the other terminals sharing the connection.
    ///
    /// This is the head-of-line failure PLAN_M2_5.md says this hop must
    /// never have. Every terminal, every request, and the control channel
    /// itself are multiplexed over one reader task, so the obvious
    /// implementation — await capacity on the per-terminal channel — turns
    /// a single wedged browser tab into a supervisor-wide outage for that
    /// helm. The assertion that catches it is the LAST one: a second,
    /// healthy terminal still receiving data after the first has
    /// overflowed. The first two assertions (a `Detach` reaches the
    /// supervisor, and the stalled terminal is told why with the shared
    /// [`farhelm_proto::DETACH_REASON_STALLED`] string) pin the rest of
    /// the contract: the wedged attachment is genuinely released rather
    /// than merely abandoned locally, and its user is told rather than
    /// left staring at a terminal that silently stopped.
    ///
    /// Drives the real demux loop through a scripted peer rather than
    /// calling `dispatch` directly, so the overflow is discovered exactly
    /// where production discovers it — inside the shared reader — and a
    /// refactor that moved the send off that path could not pass this
    /// while reintroducing the block.
    #[tokio::test]
    async fn a_full_terminal_queue_detaches_that_terminal_without_blocking_the_others() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();

        // Registered directly rather than through `attach`, which would
        // need the scripted peer to also play out an attach exchange for
        // two channels — irrelevant to what this test is about. Registered
        // BEFORE any frame is written, since the demux drops frames for
        // channels it does not know.
        let mut stalled_rx = register_terminal(&client, 1, TERM_EVENT_QUEUE).await;
        let mut healthy_rx = register_terminal(&client, 2, TERM_EVENT_QUEUE).await;

        // Flood the stalled channel well past its bound, then send a
        // single frame on the healthy one. The healthy frame goes LAST on
        // purpose: it can only arrive if the reader loop got past the
        // overflow without waiting.
        for _ in 0..(TERM_EVENT_QUEUE * 2) {
            peer_writer
                .write_frame(&Frame::data(1, b"flood".to_vec()))
                .await
                .unwrap();
        }
        peer_writer
            .write_frame(&Frame::data(2, b"healthy".to_vec()))
            .await
            .unwrap();

        // `stalled_rx` is deliberately never drained: it stands in for a
        // browser that has stopped consuming entirely.
        let detach = timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("the overflowing terminal never sent a Detach upstream")
            .unwrap()
            .expect("connection closed before the Detach arrived");
        assert!(
            matches!(
                parse_control(&detach).unwrap(),
                ControlMsg::Detach { channel: 1 }
            ),
            "the stalled terminal's attachment must be released upstream, not merely dropped \
             locally: {detach:?}"
        );

        assert!(
            matches!(
                timeout(Duration::from_secs(5), healthy_rx.recv()).await,
                Ok(Some(TermEvent::Data(bytes))) if bytes == b"healthy"
            ),
            "a terminal sharing the connection stopped receiving because another one \
             overflowed — this is the head-of-line block the detach-not-block rule exists to \
             prevent"
        );

        // Drain the backlog the stalled terminal did accumulate; its
        // final event must be the stall detach, which is delivered on its
        // own task precisely so it survives the queue having been full.
        let final_event = timeout(Duration::from_secs(5), async {
            loop {
                match stalled_rx.recv().await {
                    Some(TermEvent::Data(_)) => {}
                    other => return other,
                }
            }
        })
        .await
        .expect("the stalled terminal never received its detach notice");
        assert!(
            matches!(final_event, Some(TermEvent::Detached(reason))
                if reason == farhelm_proto::DETACH_REASON_STALLED),
            "the stalled terminal must be told why it stopped, using the same reason string \
             the supervisor's own stall detach emits"
        );
    }

    /// The marker shares `Data`'s bounded-queue pressure rule rather than
    /// a separate one (PLAN_M5.md item 4, `route_terminal_event`'s whole
    /// point): with the queue already at EXACT capacity from plain data,
    /// a `ReplayComplete` — not a `Data` frame — is what trips the
    /// overflow here, and it must produce the identical stall-detach/
    /// release contract the test above pins for `Data`, without
    /// disturbing a healthy terminal sharing the connection. A refactor
    /// that split the marker onto its own path — awaiting queue capacity,
    /// dropping it silently outside the detach rule, or bypassing the
    /// bound entirely — would still pass every OTHER marker test in this
    /// file (none of them fill the queue) and would only be caught here.
    #[tokio::test]
    async fn a_replay_complete_marker_against_a_full_queue_gets_the_same_stall_detach_as_data() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();

        let mut stalled_rx = register_terminal(&client, 1, TERM_EVENT_QUEUE).await;
        let mut healthy_rx = register_terminal(&client, 2, TERM_EVENT_QUEUE).await;

        // Fill the stalled channel's queue to EXACTLY capacity with plain
        // data — every one of these must be accepted, unlike the flood
        // test above which overshoots deliberately. The marker sent next
        // is then the one event that finds no capacity left, so it (and
        // not a `Data` frame) is what must trip the overflow.
        for _ in 0..TERM_EVENT_QUEUE {
            peer_writer
                .write_frame(&Frame::data(1, b"x".to_vec()))
                .await
                .unwrap();
        }
        peer_writer
            .write_control(&ControlMsg::ReplayComplete { channel: 1 })
            .await
            .unwrap();
        // Sent after the marker, on a different channel: it can only
        // arrive if the reader loop got past the marker's overflow
        // without ever waiting on the full queue's capacity.
        peer_writer
            .write_frame(&Frame::data(2, b"healthy".to_vec()))
            .await
            .unwrap();

        let detach = timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("the overflowing marker never sent a Detach upstream")
            .unwrap()
            .expect("connection closed before the Detach arrived");
        assert!(
            matches!(
                parse_control(&detach).unwrap(),
                ControlMsg::Detach { channel: 1 }
            ),
            "a marker against a full queue must release the attachment upstream, not merely \
             drop it locally: {detach:?}"
        );

        assert!(
            matches!(
                timeout(Duration::from_secs(5), healthy_rx.recv()).await,
                Ok(Some(TermEvent::Data(bytes))) if bytes == b"healthy"
            ),
            "a healthy terminal sharing the connection must keep receiving even while another \
             one's marker is overflowing — the marker must never await queue capacity"
        );

        // Drain the capacity-filled backlog; the final event must be the
        // stall detach, delivered on its own task precisely so it
        // survives the queue having been full.
        let final_event = timeout(Duration::from_secs(5), async {
            loop {
                match stalled_rx.recv().await {
                    Some(TermEvent::Data(_)) => {}
                    other => return other,
                }
            }
        })
        .await
        .expect("the stalled terminal never received its detach notice");
        assert!(
            matches!(final_event, Some(TermEvent::Detached(reason))
                if reason == farhelm_proto::DETACH_REASON_STALLED),
            "a marker overflow must detach with the same shared reason a Data overflow uses, \
             not a different one that would let a client distinguish the two causes"
        );
    }

    /// The whole point of putting the marker on the DATA queue instead of
    /// the detach watch (PLAN_M5.md item 4, `TermEvent::ReplayComplete`'s
    /// own docs): a scripted peer plays out replay bytes, the marker, then
    /// live bytes, and the client must yield them back in that exact
    /// order. A marker delivered on its own out-of-band channel — the
    /// `Detached` shape — could race ahead of queued data and land before
    /// the replay it is supposed to follow; only sharing the queue rules
    /// that out structurally.
    #[tokio::test]
    async fn replay_complete_marker_is_ordered_between_replay_and_live_data_in_the_queue() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (_peer_reader, mut peer_writer) = peer.await.unwrap();

        let mut term_rx = register_terminal(&client, 5, TERM_EVENT_QUEUE).await;

        peer_writer
            .write_frame(&Frame::data(5, b"replay".to_vec()))
            .await
            .unwrap();
        peer_writer
            .write_control(&ControlMsg::ReplayComplete { channel: 5 })
            .await
            .unwrap();
        peer_writer
            .write_frame(&Frame::data(5, b"live".to_vec()))
            .await
            .unwrap();

        let replay = timeout(Duration::from_secs(5), term_rx.recv())
            .await
            .expect("never received the replay data");
        assert!(matches!(replay, Some(TermEvent::Data(bytes)) if bytes == b"replay"));

        let marker = timeout(Duration::from_secs(5), term_rx.recv())
            .await
            .expect("never received the marker");
        assert!(matches!(marker, Some(TermEvent::ReplayComplete)));

        let live = timeout(Duration::from_secs(5), term_rx.recv())
            .await
            .expect("never received the live data");
        assert!(
            matches!(live, Some(TermEvent::Data(bytes)) if bytes == b"live"),
            "live output must follow the marker, not race ahead of it"
        );
    }

    /// A marker for a channel this connection no longer knows about — the
    /// attachment already ended, or this was always a stale/foreign id —
    /// must be silently discarded, exactly like a stray `Data` frame for a
    /// dead terminal, and must not disturb any OTHER terminal multiplexed
    /// over the same connection. Pins `route_terminal_event`'s no-op path
    /// for `ReplayComplete` specifically, since it shares the helper with
    /// `Data` but is reached through a different `dispatch` arm
    /// (`ControlMsg::ReplayComplete`, not `FrameKind::Data`).
    #[tokio::test]
    async fn replay_complete_for_an_unknown_channel_is_discarded_without_disturbing_others() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (_peer_reader, mut peer_writer) = peer.await.unwrap();

        // Channel 9 is never registered, standing in for a marker that
        // arrived for an attachment already torn down locally.
        let mut healthy_rx = register_terminal(&client, 2, TERM_EVENT_QUEUE).await;

        peer_writer
            .write_control(&ControlMsg::ReplayComplete { channel: 9 })
            .await
            .unwrap();
        peer_writer
            .write_frame(&Frame::data(2, b"still healthy".to_vec()))
            .await
            .unwrap();

        assert!(
            matches!(
                timeout(Duration::from_secs(5), healthy_rx.recv()).await,
                Ok(Some(TermEvent::Data(bytes))) if bytes == b"still healthy"
            ),
            "a stray marker for an unknown channel must not disturb a healthy terminal sharing \
             the connection"
        );
    }

    /// `rename_session` must forward `title` VERBATIM — control characters,
    /// leading/trailing whitespace, all of it — because the supervisor is
    /// the sole authority on what a title may contain (PLAN_M5.md item 4)
    /// and any local trimming or rewriting would silently alter caller
    /// data the supervisor never asked this client to launder. The
    /// scripted peer asserts the exact bytes it received, not merely that
    /// SOME title arrived.
    ///
    /// This also pins `SessionRenamed`'s demux classification — the same
    /// concern `session_restarted_reply_resolves_the_pending_request` and
    /// `tab_opened_reply_resolves_the_pending_request` pin for their own
    /// messages: PLAN_M5.md item 1 added this variant to `ControlMsg`
    /// without adding it to `dispatch`'s req_id-classification match,
    /// exactly the miss the `TabOpened`/`TabClosed` precedent documents
    /// (PLAN_M4.md) — an unclassified reply falls into the `other =>
    /// warn!(...)` arm, leaving the pending caller's oneshot unresolved
    /// and this call hanging forever. Folded into this test rather than
    /// kept as its own low-level `.request()`-driven check: a missing arm
    /// hangs THIS public-method call exactly as it would a bare
    /// `.request()`, so a separate test pinned nothing this one does not.
    #[tokio::test]
    async fn rename_session_forwards_the_title_verbatim() {
        const TITLE: &str = "  weird \u{7}title\twith\ncontrol chars  ";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::RenameSession {
                req_id,
                session_id,
                title,
            } = request
            else {
                panic!("expected RenameSession, got {request:?}");
            };
            assert_eq!(session_id, "sess-1");
            assert_eq!(
                title, TITLE,
                "the title must reach the supervisor byte-for-byte unchanged"
            );
            writer
                .write_control(&ControlMsg::SessionRenamed {
                    req_id,
                    session: session("sess-1"),
                })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let session = client.rename_session("sess-1", TITLE).await.unwrap();
        assert_eq!(session.id, "sess-1");
        peer.await.unwrap();
    }

    /// Push fire-and-forget messages until one blocks, returning the
    /// widths that were actually accepted.
    ///
    /// Counting rather than assuming a number: what fills first is the
    /// bounded queue PLUS whatever the transport buffer happens to
    /// absorb, and only the former is a constant. `resize` carries a
    /// distinguishable payload and needs no reply, so the accepted
    /// prefix is directly checkable against what the peer later reads.
    ///
    /// A cancelled `send` is not enqueued (tokio's `mpsc::Sender::send`
    /// is cancel-safe that way), so the returned widths are exactly the
    /// messages in flight — which is what makes the ordering assertion
    /// below exact rather than approximate.
    async fn saturate_outbound(client: &SupervisorClient) -> Vec<u16> {
        let mut accepted = Vec::new();
        for cols in 0..2000u16 {
            if timeout(Duration::from_millis(50), client.resize("s", 1, cols, 24))
                .await
                .is_err()
            {
                return accepted;
            }
            accepted.push(cols);
        }
        panic!(
            "2000 sends completed against a transport nobody is draining — the outbound queue \
             is not bounded at all"
        );
    }

    /// The outbound queue must behave as a BOUND: sends stop completing
    /// once it is full against a transport that is not draining, and
    /// everything accepted lands in order when capacity returns.
    ///
    /// Pins the bound itself rather than a symptom. Without this, swapping
    /// the bounded channel back to an unbounded one — the exact debt this
    /// milestone closed — passes every other test in this file, because
    /// nothing else can observe the difference until memory runs out.
    /// Order matters as much as the bound: a fix that dropped or reordered
    /// under pressure would still "not block".
    #[tokio::test]
    async fn the_outbound_queue_blocks_at_capacity_and_drains_in_order() {
        // A small transport so the writer task parks quickly instead of
        // absorbing the whole burst into a socket buffer.
        let (client_side, peer_side) = tokio::io::duplex(1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            reader
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let mut peer_reader = peer.await.unwrap();

        let accepted = saturate_outbound(&client).await;
        assert!(
            accepted.len() >= SUPERVISOR_WRITER_QUEUE,
            "sends stopped after only {} messages, fewer than the queue's own capacity — the \
             bound is tighter than it claims",
            accepted.len()
        );

        // Draining the peer releases the writer, and everything accepted
        // arrives in the order it was sent.
        let mut widths = Vec::new();
        while widths.len() < accepted.len() {
            let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("peer read hung")
                .unwrap()
                .expect("connection closed mid-drain");
            if let ControlMsg::Resize { cols, .. } = parse_control(&frame).unwrap() {
                widths.push(cols);
            }
        }
        assert_eq!(
            widths, accepted,
            "queued messages must drain in send order once capacity returns"
        );
    }

    /// A request cancelled while parked on a full outbound queue must
    /// leave no pending entry behind.
    ///
    /// `request` registers its `req_id` so a reply can be correlated, and
    /// that entry is only ever removed by a reply arriving. Registering it
    /// BEFORE awaiting queue capacity means a cancellation in that window
    /// — an axum handler whose browser disconnected, a `select!` losing a
    /// race — orphans it in a map nothing sweeps, for the life of the
    /// process. Reserving capacity first is what closes that, and this is
    /// the only test that can tell the two orderings apart.
    #[tokio::test]
    async fn a_request_cancelled_on_a_full_queue_leaks_no_pending_entry() {
        let (client_side, peer_side) = tokio::io::duplex(1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let _peer = peer.await.unwrap();

        saturate_outbound(&client).await;

        let cancelled = timeout(Duration::from_millis(300), client.list_sessions()).await;
        assert!(
            cancelled.is_err(),
            "test premise: the request must still be parked on the full queue when cancelled"
        );
        assert!(
            client.pending.lock().await.map.is_empty(),
            "a request cancelled while waiting for queue capacity left its pending entry \
             behind; it would never be reaped"
        );
    }

    /// A terminal whose receiver was dropped must be released UPSTREAM,
    /// not merely forgotten locally.
    ///
    /// The consumer disappearing without detaching is ordinary — a task
    /// cancelled mid-flight, a handler that returned early — and the
    /// supervisor cannot see it. Left registered, the entry keeps routing
    /// frames nowhere while the supervisor holds a control client, an
    /// input client, and a forwarder open for the life of the connection.
    /// Nothing else in this file covers the closed (as opposed to full)
    /// case, and the two need different handling: there is no local notice
    /// to deliver here, because nobody is listening.
    #[tokio::test]
    async fn a_terminal_whose_receiver_was_dropped_is_released_upstream() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();

        let stream = register_terminal(&client, 3, TERM_EVENT_QUEUE).await;
        drop(stream);

        peer_writer
            .write_frame(&Frame::data(3, b"output for nobody".to_vec()))
            .await
            .unwrap();

        let detach = timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("no Detach reached the supervisor for a dropped terminal")
            .unwrap()
            .expect("connection closed before the Detach arrived");
        assert!(
            matches!(
                parse_control(&detach).unwrap(),
                ControlMsg::Detach { channel: 3 }
            ),
            "a dropped receiver must release its attachment upstream: {detach:?}"
        );
        assert!(
            !client.terminals.lock().await.contains_key(&3),
            "the dead terminal must also be unregistered locally"
        );
    }

    /// The common case: allocation is a plain monotonic counter starting at
    /// 1 (0 is the control channel). This pins the never-recycle contract's
    /// happy half — each call hands out a new id and advances.
    #[test]
    fn allocate_channel_hands_out_sequential_ids() {
        let next_channel = AtomicU64::new(1);

        assert_eq!(allocate_channel(&next_channel).unwrap(), 1);
        assert_eq!(allocate_channel(&next_channel).unwrap(), 2);
    }

    /// The never-recycle contract's hard half: once the u32 wire range is
    /// spent, allocation must fail — permanently — rather than wrap back
    /// into ids that may still be referenced by detached-but-uncleaned
    /// callers or in-flight frames. Wrapping here is exactly the
    /// cross-attachment corruption bug this allocator exists to prevent.
    #[test]
    fn allocate_channel_fails_instead_of_recycling() {
        let next_channel = AtomicU64::new(u64::from(u32::MAX));

        // The last id in the wire range is still valid...
        assert_eq!(allocate_channel(&next_channel).unwrap(), u32::MAX);
        // ...and everything past it errors, on every subsequent call.
        assert!(allocate_channel(&next_channel).is_err());
        assert!(allocate_channel(&next_channel).is_err());
    }

    /// `attach()` must surface exhaustion as a clean error before touching
    /// any shared state or the wire: no Attach frame sent, no terminals
    /// entry leaked. This exercises exhaustion through the real attach path
    /// so a refactor that stops routing allocation through
    /// `allocate_channel` cannot silently reintroduce id reuse while the
    /// unit tests above stay green.
    ///
    /// The three assertions are ordered from cheapest to prove to
    /// strongest, and the last is the one with a prerequisite: proving
    /// nothing was WRITTEN requires closing the connection first, because
    /// only then does the peer's read resolve — to `None` if the attach
    /// was truly silent, or to the stray frame if it was not.
    #[tokio::test]
    async fn attach_fails_cleanly_when_channel_ids_exhausted() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            reader
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let mut peer_reader = peer.await.unwrap();

        client
            .next_channel
            .store(u64::from(u32::MAX) + 1, Ordering::Relaxed);

        let error = timeout(Duration::from_secs(2), client.attach("s", 80, 24))
            .await
            .expect("attach hung on exhausted channel ids")
            .expect_err("attach must fail once channel ids are exhausted");
        assert!(error.to_string().contains("exhausted"));
        assert!(client.terminals.lock().await.is_empty());

        // The failed attach must not have written anything: dropping the
        // client closes the connection, and the peer must see clean EOF
        // with no frames in between.
        drop(client);
        assert!(
            timeout(Duration::from_secs(2), peer_reader.read_frame())
                .await
                .expect("peer read hung")
                .unwrap()
                .is_none()
        );
    }

    /// `SessionRestarted` must resolve its `RestartSession` request through
    /// the demux exactly like every other req_id-bearing reply
    /// (`concurrent_requests_accept_replies_in_reverse_order` above pins the
    /// same mechanism for `SessionList`). PLAN_M3 review batch item 5: this
    /// message was added to `ControlMsg` alongside the reply-classification
    /// match in `run_reader`'s demux, and without that addition a scripted
    /// peer sending `SessionRestarted` would fall into the `other =>
    /// warn!(...)` arm — the pending caller's oneshot would never resolve,
    /// hanging forever, since unlike `Detached` this reply exists to answer
    /// a specific waiting request. Deliberately kept at the demux level
    /// even now that [`SupervisorClient::restart_session`] exists: what it
    /// pins is the reply CLASSIFICATION, which a refactor could break for
    /// this message alone while the public method's own tests stay green.
    #[tokio::test]
    async fn session_restarted_reply_resolves_the_pending_request() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::RestartSession { req_id, .. } = request else {
                panic!("expected RestartSession, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::SessionRestarted {
                    req_id,
                    session: session("restarted"),
                })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let reply = client
            .request(
                7,
                ControlMsg::RestartSession {
                    req_id: 7,
                    session_id: "restarted".to_string(),
                    mode: farhelm_proto::RestartMode::Resume,
                    stop_if_running: false,
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            reply,
            ControlMsg::SessionRestarted { req_id: 7, session } if session.id == "restarted"
        ));
        peer.await.unwrap();
    }

    /// `TabOpened` must resolve its `OpenTab` request through the demux
    /// exactly like every other req_id-bearing reply
    /// (`session_restarted_reply_resolves_the_pending_request` above pins
    /// the same mechanism for `RestartSession`). PLAN_M4.md item 1 added
    /// this variant to `ControlMsg` without adding it to `dispatch`'s
    /// reply-classification match, so a scripted peer sending it fell
    /// into the `other => warn!(...)` arm — the pending caller's oneshot
    /// would never resolve, hanging `request()` forever.
    #[tokio::test]
    async fn tab_opened_reply_resolves_the_pending_request() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::OpenTab { req_id, .. } = request else {
                panic!("expected OpenTab, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::TabOpened {
                    req_id,
                    tab: farhelm_proto::TabInfo {
                        id: "t1".to_string(),
                    },
                })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let reply = client
            .request(
                20,
                ControlMsg::OpenTab {
                    req_id: 20,
                    session_id: "s1".to_string(),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            reply,
            ControlMsg::TabOpened { req_id: 20, ref tab } if tab.id == "t1"
        ));
        peer.await.unwrap();
    }

    /// `TabClosed`'s sibling case for `CloseTab` — see
    /// `tab_opened_reply_resolves_the_pending_request`'s doc comment for
    /// why this specific miss mattered.
    #[tokio::test]
    async fn tab_closed_reply_resolves_the_pending_request() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CloseTab { req_id, .. } = request else {
                panic!("expected CloseTab, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::TabClosed { req_id })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let reply = client
            .request(
                21,
                ControlMsg::CloseTab {
                    req_id: 21,
                    session_id: "s1".to_string(),
                    tab_id: "t1".to_string(),
                },
            )
            .await
            .unwrap();
        assert!(matches!(reply, ControlMsg::TabClosed { req_id: 21 }));
        peer.await.unwrap();
    }

    /// `UploadStarted`'s sibling case for `BeginUpload` — see
    /// `tab_opened_reply_resolves_the_pending_request`'s doc comment for
    /// why this specific miss mattered.
    #[tokio::test]
    async fn upload_started_reply_resolves_the_pending_request() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, channel, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let reply = client
            .request(
                22,
                ControlMsg::BeginUpload {
                    req_id: 22,
                    session_id: "s1".to_string(),
                    channel: 4,
                    filename: "a.txt".to_string(),
                    size: 10,
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            reply,
            ControlMsg::UploadStarted {
                req_id: 22,
                channel: 4
            }
        ));
        peer.await.unwrap();
    }

    /// `UploadCommitted`'s sibling case for `CommitUpload` — see
    /// `tab_opened_reply_resolves_the_pending_request`'s doc comment for
    /// why this specific miss mattered.
    #[tokio::test]
    async fn upload_committed_reply_resolves_the_pending_request() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::CommitUpload { req_id, .. } = request else {
                panic!("expected CommitUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadCommitted {
                    req_id,
                    path: "/tmp/a.txt".to_string(),
                })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let reply = client
            .request(
                23,
                ControlMsg::CommitUpload {
                    req_id: 23,
                    channel: 4,
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            reply,
            ControlMsg::UploadCommitted { req_id: 23, ref path } if path == "/tmp/a.txt"
        ));
        peer.await.unwrap();
    }

    /// `begin_upload` sends the caller's fields verbatim and, on a
    /// successful `UploadStarted`, hands back a guard whose channel is
    /// registered in `uploads` for the rest of the transfer to use — the
    /// register-before-request half of its own doc comment.
    ///
    /// The scripted peer returns its reader/writer instead of letting them
    /// drop at the end of its async block: dropping them here would close
    /// this side of the duplex right after the reply, which can race the
    /// demux loop into observing EOF and calling `fail_all` — clearing
    /// EVERY upload, including the one just registered — before this
    /// test's own assertion runs. Keeping the peer's halves alive (held,
    /// unused, in the joined result) until after the assertion is what
    /// makes the test observe begin_upload's OWN postcondition rather than
    /// an unrelated connection-teardown race.
    #[tokio::test]
    async fn begin_upload_sends_the_declared_fields_and_registers_the_channel() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id,
                session_id,
                channel,
                filename,
                size,
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            assert_eq!(session_id, "s1");
            assert_eq!(filename, "screenshot.png");
            assert_eq!(size, 999);
            writer
                .write_control(&ControlMsg::UploadStarted { req_id, channel })
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let upload = client
            .begin_upload("s1", "screenshot.png", 999)
            .await
            .unwrap();
        assert!(
            client.uploads.lock().await.contains_key(&upload.channel()),
            "a successful begin_upload must leave the channel registered for later calls"
        );
        let _peer = peer.await.unwrap();
    }

    /// `send_upload_chunk` must rechunk a single caller-supplied buffer at
    /// exactly [`UPLOAD_CHUNK_BYTES`], never at whatever size the caller
    /// happened to pass in — the "one big body chunk => multiple
    /// UPLOAD_CHUNK_BYTES frames" contract the helm's HTTP relay depends
    /// on, since an HTTP body stream's own chunk boundaries have nothing
    /// to do with the protocol's frame-size discipline. The content itself
    /// (a byte pattern derived from position) is checked too, not just the
    /// lengths, so a rechunking bug that reordered or duplicated bytes
    /// would also fail this.
    #[tokio::test]
    async fn send_upload_chunk_rechunks_a_single_buffer_at_upload_chunk_bytes() {
        let (client_side, peer_side) = tokio::io::duplex(4 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            reader
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let channel = 5;

        // Two full chunks plus a remainder — comfortably inside the
        // credit window, so this test is purely about rechunking, not
        // flow control.
        let remainder = 777usize;
        let total = UPLOAD_CHUNK_BYTES * 2 + remainder;
        let mut upload = register_upload(&client, channel, total as u64).await;
        let bytes: Vec<u8> = (0..total).map(|i| (i % 256) as u8).collect();
        upload.send_upload_chunk(&bytes).await.unwrap();

        let mut peer_reader = peer.await.unwrap();
        let mut reassembled = Vec::new();
        let mut got_lens = Vec::new();
        loop {
            let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("timed out reading a rechunked upload frame")
                .unwrap();
            let Some(frame) = frame else { break };
            assert_eq!(frame.channel, channel);
            assert!(matches!(frame.kind, FrameKind::Data));
            got_lens.push(frame.body.len());
            reassembled.extend_from_slice(&frame.body);
            if reassembled.len() == total {
                break;
            }
        }
        assert_eq!(
            got_lens,
            vec![UPLOAD_CHUNK_BYTES, UPLOAD_CHUNK_BYTES, remainder],
            "rechunking must split at exactly UPLOAD_CHUNK_BYTES regardless of the caller's own \
             buffer size, with only the final piece short"
        );
        assert_eq!(
            reassembled, bytes,
            "rechunked frames must reassemble to exactly the original bytes, in order"
        );
    }

    /// The credit window bounds outstanding (unacknowledged) bytes at
    /// [`UPLOAD_WINDOW_BYTES`] — PLAN_M4.md item 1's flow control for the
    /// upload direction. A sender offered more than the window must stall
    /// after putting exactly the window's worth on the wire, and resume
    /// only once an `UploadAck` extends the credit baseline.
    #[tokio::test]
    async fn send_upload_chunk_stalls_at_the_credit_window_then_progresses_after_an_ack() {
        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();
        let channel = 9;

        // UPLOAD_WINDOW_BYTES is an exact multiple of UPLOAD_CHUNK_BYTES
        // (4 MiB / 256 KiB = 16), so "one chunk past the window" lands on
        // a clean chunk boundary and the assertions below need no
        // rounding.
        let total = UPLOAD_WINDOW_BYTES as usize + UPLOAD_CHUNK_BYTES;
        let mut upload = register_upload(&client, channel, total as u64).await;
        let bytes = vec![7u8; total];
        let send = tokio::spawn(async move {
            let result = upload.send_upload_chunk(&bytes).await;
            // The guard is kept alive until the send is observed: dropping
            // it here would abort the very transfer the test is waiting to
            // see finish.
            (result, upload)
        });

        // Drain exactly the window's worth. The sender must not put more
        // than this on the wire before any ack — the property under test.
        let mut received = 0u64;
        while received < UPLOAD_WINDOW_BYTES {
            let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("timed out waiting for the sender's initial window")
                .unwrap()
                .expect("connection closed mid-window");
            received += frame.body.len() as u64;
        }
        assert_eq!(received, UPLOAD_WINDOW_BYTES);

        // Now genuinely stalled: neither another frame nor the send's own
        // completion arrives within a short, generous bound.
        assert!(
            timeout(Duration::from_millis(300), peer_reader.read_frame())
                .await
                .is_err(),
            "the sender put more than UPLOAD_WINDOW_BYTES on the wire before any ack"
        );
        assert!(
            !send.is_finished(),
            "the send must still be parked on credit, not finished"
        );

        // Ack the whole window; credit reopens and the final chunk must
        // now arrive.
        peer_writer
            .write_control(&ControlMsg::UploadAck {
                channel,
                received: UPLOAD_WINDOW_BYTES,
            })
            .await
            .unwrap();

        let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("the final chunk never arrived after the ack")
            .unwrap()
            .expect("connection closed after the ack");
        assert_eq!(frame.body.len(), UPLOAD_CHUNK_BYTES);
        let (result, _upload) = send.await.expect("send task panicked");
        result.expect("send_upload_chunk must succeed once every chunk is delivered");
    }

    /// An `UploadAborted` arriving while a sender is parked waiting for
    /// credit must wake it immediately with the reason, rather than
    /// leaving it waiting for an ack that will now never come.
    #[tokio::test]
    async fn send_upload_chunk_reports_the_reason_when_aborted_while_waiting_for_credit() {
        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();
        let channel = 11;

        let total = UPLOAD_WINDOW_BYTES as usize + 10;
        let mut upload = register_upload(&client, channel, total as u64).await;
        let bytes = vec![1u8; total];
        let send = tokio::spawn(async move { upload.send_upload_chunk(&bytes).await });

        let mut received = 0u64;
        while received < UPLOAD_WINDOW_BYTES {
            let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("timed out waiting for the sender's initial window")
                .unwrap()
                .expect("connection closed mid-window");
            received += frame.body.len() as u64;
        }

        const SENTINEL: &str = "SENTINEL-abort-mid-wait";
        peer_writer
            .write_control(&ControlMsg::UploadAborted {
                channel,
                reason: SENTINEL.to_string(),
            })
            .await
            .unwrap();

        let result = timeout(Duration::from_secs(5), send)
            .await
            .expect("send_upload_chunk hung after an UploadAborted arrived")
            .expect("send task panicked");
        assert_eq!(result, Err(SENTINEL.to_string()));
    }

    /// The connection dying while a sender is parked on credit must also
    /// wake it — `fail_all`'s upload half, mirroring how it already wakes
    /// a parked terminal detach. Without this an upload whose supervisor
    /// vanished mid-transfer would hang its HTTP handler forever.
    #[tokio::test]
    async fn a_dead_connection_wakes_a_sender_parked_on_credit() {
        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();
        let channel = 13;

        let total = UPLOAD_WINDOW_BYTES as usize + UPLOAD_CHUNK_BYTES;
        let mut upload = register_upload(&client, channel, total as u64).await;
        let bytes = vec![2u8; total];
        let send = tokio::spawn(async move { upload.send_upload_chunk(&bytes).await });

        let mut received = 0u64;
        while received < UPLOAD_WINDOW_BYTES {
            let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("timed out waiting for the sender's initial window")
                .unwrap()
                .expect("connection closed mid-window");
            received += frame.body.len() as u64;
        }

        // Kill the connection instead of acking: shutting down the peer's
        // write half is what the reader-EOF tests elsewhere in this file
        // use to trigger `fail_all` from the demux side.
        peer_writer.shutdown().await.unwrap();

        let result = timeout(Duration::from_secs(5), send)
            .await
            .expect("send_upload_chunk hung after the connection died")
            .expect("send task panicked");
        assert!(
            result.is_err(),
            "a dead connection must fail a sender parked on credit, not hang it forever"
        );
    }

    /// `commit` clears the upload's local bookkeeping whichever way the
    /// supervisor answers — `channel` is spent once a commit has been
    /// ANSWERED, and both answers are terminal by `ControlMsg::
    /// CommitUpload`'s docs — and passes each outcome through unchanged:
    /// the published path on success, the supervisor's own
    /// [`SupervisorError`] (message AND kind) on a refusal, since a size
    /// mismatch has to reach the browser as a 400 with the supervisor's
    /// words rather than a locally invented failure.
    ///
    /// Both arms keep the scripted peer's halves ALIVE past the
    /// assertions. Dropping them would close the connection, and
    /// `fail_all` clearing everything would make a leaked entry look
    /// cleaned up — the failure this test exists to catch.
    #[tokio::test]
    async fn commit_upload_passes_the_outcome_through_and_clears_bookkeeping_either_way() {
        const SENTINEL: &str = "SENTINEL-commit-mismatch-4f1b: declared 10 bytes, received 7";
        // (channel, what the peer answers, what the caller must see)
        enum Answer {
            Committed,
            Refused,
        }
        for (channel, answer) in [(6u32, Answer::Committed), (8u32, Answer::Refused)] {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::CommitUpload {
                    req_id,
                    channel: committed,
                } = request
                else {
                    panic!("expected CommitUpload, got {request:?}");
                };
                assert_eq!(committed, channel);
                let reply = match answer {
                    Answer::Committed => ControlMsg::UploadCommitted {
                        req_id,
                        path: "/tmp/pub.png".to_string(),
                    },
                    Answer::Refused => ControlMsg::Error {
                        req_id,
                        message: SENTINEL.to_string(),
                        kind: ErrorKind::InvalidRequest,
                    },
                };
                writer.write_control(&reply).await.unwrap();
                (reader, writer)
            });
            let (r, w) = tokio::io::split(client_side);
            let client = SupervisorClient::start(r, w).await.unwrap();
            let upload = register_upload(&client, channel, 10).await;

            match upload.commit().await {
                Ok(path) => assert_eq!(path, "/tmp/pub.png"),
                Err(e) => {
                    let supervisor = e
                        .downcast_ref::<SupervisorError>()
                        .expect("a commit refusal must stay a SupervisorError, kind and all");
                    assert_eq!(supervisor.message, SENTINEL);
                    assert_eq!(supervisor.kind, ErrorKind::InvalidRequest);
                }
            }
            assert!(
                !client.uploads.lock().await.contains_key(&channel),
                "commit must clear the upload's local bookkeeping on every answered outcome"
            );
            let _peer = peer.await.unwrap();
        }
    }

    /// `UploadGuard::abort` is fire-and-forget like `detach`: the local
    /// entry goes away and an `AbortUpload` reaches the supervisor.
    #[tokio::test]
    async fn abort_upload_sends_the_control_message_and_clears_local_bookkeeping() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            reader
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let mut upload = register_upload(&client, 7, 100).await;

        upload.abort("gave up".to_string()).await;
        assert!(!client.uploads.lock().await.contains_key(&7));

        let mut peer_reader = peer.await.unwrap();
        let frame = timeout(Duration::from_secs(2), peer_reader.read_frame())
            .await
            .expect("no AbortUpload reached the supervisor")
            .unwrap()
            .expect("connection closed before AbortUpload arrived");
        assert!(matches!(
            parse_control(&frame).unwrap(),
            ControlMsg::AbortUpload { channel: 7 }
        ));
    }

    /// A `BeginUpload` the supervisor REFUSES must leave nothing behind —
    /// no `uploads` entry, and the refusal preserved as a
    /// [`SupervisorError`] with its kind intact.
    ///
    /// The peer is kept ALIVE past the assertions, which is the whole
    /// point of this test existing separately from the route-level
    /// refusal coverage: a peer dropped after replying closes the
    /// connection, and `fail_all` then clears every upload — so a leaked
    /// entry would look cleaned up and the test would pass over the bug.
    ///
    /// No `AbortUpload` may follow either. `UploadStarted`'s docs say a
    /// refused begin created nothing on disk, so aborting would name a
    /// transfer that never existed.
    #[tokio::test]
    async fn begin_upload_refusal_leaves_no_bookkeeping_and_sends_no_abort() {
        const SENTINEL: &str = "SENTINEL-begin-refused-91cd: no such session";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload { req_id, .. } = request else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::Error {
                    req_id,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::NotFound,
                })
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let error = client
            .begin_upload("missing", "x.png", 5)
            .await
            .expect_err("a refused begin must not yield an upload");
        let supervisor = error
            .downcast_ref::<SupervisorError>()
            .expect("a begin refusal must stay a SupervisorError");
        assert_eq!(supervisor.message, SENTINEL);
        assert_eq!(supervisor.kind, ErrorKind::NotFound);
        assert!(
            client.uploads.lock().await.is_empty(),
            "a refused begin must leave no upload bookkeeping behind"
        );

        let (mut peer_reader, _peer_writer) = peer.await.unwrap();
        assert!(
            timeout(Duration::from_millis(200), peer_reader.read_frame())
                .await
                .is_err(),
            "a refused begin must send nothing further, least of all an AbortUpload for a \
             transfer that was never accepted"
        );
    }

    /// An `UploadStarted` naming a DIFFERENT channel than the one
    /// `BeginUpload` asked for is a protocol error, not a channel
    /// reassignment: `UploadStarted` grants credit for the channel it
    /// names, so streaming onto the requested one would push bytes at a
    /// channel the supervisor never opened while the transfer it did open
    /// sat idle. The begin must fail, send no data, and leave no local
    /// bookkeeping — with an `AbortUpload` released for the channel this
    /// side asked for, since the supervisor did accept SOMETHING.
    #[tokio::test]
    async fn begin_upload_rejects_an_upload_started_for_another_channel() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::BeginUpload {
                req_id, channel, ..
            } = request
            else {
                panic!("expected BeginUpload, got {request:?}");
            };
            writer
                .write_control(&ControlMsg::UploadStarted {
                    req_id,
                    channel: channel + 100,
                })
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let error = client
            .begin_upload("s1", "x.png", 5)
            .await
            .expect_err("a mismatched UploadStarted channel must fail the begin");
        assert!(
            error.to_string().contains("not the requested"),
            "the error must name the mismatch: {error}"
        );
        await_no_uploads(&client).await;

        let (mut peer_reader, _peer_writer) = peer.await.unwrap();
        let frame = timeout(Duration::from_secs(2), peer_reader.read_frame())
            .await
            .expect("no frame followed the mismatched UploadStarted")
            .unwrap()
            .expect("connection closed before the abort arrived");
        assert!(
            matches!(
                parse_control(&frame).unwrap(),
                ControlMsg::AbortUpload { .. }
            ),
            "the only frame after a mismatch must be the abort — never upload data"
        );
    }

    /// Dropping the future that owns an upload — an HTTP handler cancelled
    /// by a client that reset its connection — must still release the
    /// supervisor's half. This is the failure no explicit branch can
    /// cover: a cancelled future runs none of them, so without ownership
    /// the supervisor holds a temp file and admission capacity until its
    /// own timeout fires.
    ///
    /// Cancelled at the hardest moment, parked on credit with the window
    /// closed, and both halves of the contract are asserted: EXACTLY one
    /// `AbortUpload` (a second would mean two teardown paths fired), and
    /// no local bookkeeping left behind.
    #[tokio::test]
    async fn dropping_an_upload_parked_on_credit_aborts_it_exactly_once() {
        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, _peer_writer) = peer.await.unwrap();
        let channel = 21;

        let total = UPLOAD_WINDOW_BYTES as usize + UPLOAD_CHUNK_BYTES;
        let mut upload = register_upload(&client, channel, total as u64).await;
        let bytes = vec![8u8; total];
        let send = tokio::spawn(async move { upload.send_upload_chunk(&bytes).await });

        // Let the whole window reach the wire, so the task is genuinely
        // parked on credit rather than merely started.
        let mut received = 0u64;
        while received < UPLOAD_WINDOW_BYTES {
            let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("timed out waiting for the sender's initial window")
                .unwrap()
                .expect("connection closed mid-window");
            received += frame.body.len() as u64;
        }

        send.abort();
        await_no_uploads(&client).await;

        let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("a cancelled upload never released the supervisor's half")
            .unwrap()
            .expect("connection closed before the abort arrived");
        assert!(matches!(
            parse_control(&frame).unwrap(),
            ControlMsg::AbortUpload { channel: c } if c == channel
        ));
        assert!(
            timeout(Duration::from_millis(200), peer_reader.read_frame())
                .await
                .is_err(),
            "a cancelled upload must abort exactly once"
        );
    }

    /// The same cancellation, one step later: dropped while awaiting the
    /// commit reply. `commit` consumes its guard precisely so this case
    /// behaves — the guard dies with the future, so the supervisor still
    /// hears about it and the local entry is still retired, instead of
    /// both sides waiting on a caller that no longer exists.
    #[tokio::test]
    async fn dropping_an_upload_awaiting_its_commit_reply_aborts_and_retires_it() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, _peer_writer) = peer.await.unwrap();
        let channel = 23;
        let upload = register_upload(&client, channel, 0).await;

        // Never answered: the commit request is left pending on purpose,
        // which is exactly the window this test cancels inside.
        let commit = tokio::spawn(async move { upload.commit().await });
        let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("the commit never reached the supervisor")
            .unwrap()
            .unwrap();
        assert!(matches!(
            parse_control(&frame).unwrap(),
            ControlMsg::CommitUpload { channel: c, .. } if c == channel
        ));

        commit.abort();
        await_no_uploads(&client).await;

        let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("a cancelled commit never released the supervisor's half")
            .unwrap()
            .expect("connection closed before the abort arrived");
        assert!(matches!(
            parse_control(&frame).unwrap(),
            ControlMsg::AbortUpload { channel: c } if c == channel
        ));
    }

    /// A supervisor that keeps the connection open but stops
    /// ACKNOWLEDGING must not park an upload forever: the credit wait has
    /// its own per-hop progress deadline (`UPLOAD_ACK_STALL_TIMEOUT`),
    /// whose expiry aborts the transfer and reports the shared
    /// `UPLOAD_ABORT_REASON_STALLED` string.
    ///
    /// The duplicate ack in the middle is the sharp edge, not decoration:
    /// a receiver repeating its last cumulative count reports NO progress,
    /// so an implementation that rearmed the deadline on any ack — or that
    /// woke the wait for one — would let a peer hold a transfer open
    /// indefinitely. The injected timeout is short so the give-up is
    /// observable, while the writer's own stall bound stays at its
    /// production value so it cannot be what fires.
    #[tokio::test]
    async fn a_supervisor_that_stops_acking_stalls_the_upload_and_aborts_it() {
        let (client_side, peer_side) = tokio::io::duplex(16 * 1024 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start_with_stall_timeouts(
            r,
            w,
            WRITER_STALL_TIMEOUT,
            Duration::from_millis(300),
        )
        .await
        .unwrap();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();
        let channel = 25;

        let total = UPLOAD_WINDOW_BYTES as usize + UPLOAD_CHUNK_BYTES;
        let mut upload = register_upload(&client, channel, total as u64).await;
        let bytes = vec![6u8; total];
        let send = tokio::spawn(async move { upload.send_upload_chunk(&bytes).await });

        let mut received = 0u64;
        while received < UPLOAD_WINDOW_BYTES {
            let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("timed out waiting for the sender's initial window")
                .unwrap()
                .expect("connection closed mid-window");
            received += frame.body.len() as u64;
        }

        // A cumulative count of zero: perfectly valid, and by definition
        // no progress. These keep coming for far longer than the deadline
        // — an implementation that rearmed on any ack, rather than only
        // on an advancing one, would therefore never give up at all and
        // fail this test by timing out rather than by asserting.
        let acker = tokio::spawn(async move {
            loop {
                if peer_writer
                    .write_control(&ControlMsg::UploadAck {
                        channel,
                        received: 0,
                    })
                    .await
                    .is_err()
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });

        let result = timeout(Duration::from_secs(5), send)
            .await
            .expect("the sender never gave up on a peer that stopped acking")
            .expect("send task panicked");
        assert_eq!(
            result,
            Err(farhelm_proto::UPLOAD_ABORT_REASON_STALLED.to_string())
        );

        let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("a stalled upload never released the supervisor's half")
            .unwrap()
            .expect("connection closed before the abort arrived");
        assert!(matches!(
            parse_control(&frame).unwrap(),
            ControlMsg::AbortUpload { channel: c } if c == channel
        ));
        acker.abort();
    }

    /// `UploadAck`'s validity rules are the protocol's, and a violation
    /// aborts the transfer rather than being clamped away: an ack that
    /// regresses, one past the bytes actually sent, or one past the
    /// declared size all mean the peer is not describing this transfer.
    /// Silently tolerating any of them would hand out credit that was
    /// never earned — the sender's only defence against a receiver
    /// claiming progress it did not make.
    ///
    /// The `u64::MAX` case additionally pins the arithmetic: the window
    /// comparison must not be reachable with a value that would overflow
    /// it, which is why the check is subtractive rather than
    /// `received + UPLOAD_WINDOW_BYTES`.
    #[tokio::test]
    async fn an_invalid_upload_ack_aborts_the_transfer() {
        // (label, declared size, the ack the peer sends after 1 KiB was sent)
        let cases: [(&str, u64, u64); 4] = [
            ("regressing", 4096, 0),
            ("ahead of sent", 4096, 2048),
            ("ahead of declared", 4096, 8192),
            ("near u64::MAX", u64::MAX, u64::MAX),
        ];
        for (label, declared, bad_ack) in cases {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                (reader, writer)
            });
            let (r, w) = tokio::io::split(client_side);
            let client = SupervisorClient::start(r, w).await.unwrap();
            let (mut peer_reader, mut peer_writer) = peer.await.unwrap();
            let channel = 31;
            let mut upload = register_upload(&client, channel, declared).await;

            // 1 KiB sent, then acked in full — so the regressing case has
            // something to regress FROM and the others have a sent
            // frontier to exceed.
            upload.send_upload_chunk(&vec![1u8; 1024]).await.unwrap();
            let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(frame.body.len(), 1024, "{label}");
            peer_writer
                .write_control(&ControlMsg::UploadAck {
                    channel,
                    received: 1024,
                })
                .await
                .unwrap();

            peer_writer
                .write_control(&ControlMsg::UploadAck {
                    channel,
                    received: bad_ack,
                })
                .await
                .unwrap();

            let ended = timeout(Duration::from_secs(5), upload.ended())
                .await
                .unwrap_or_else(|_| panic!("an invalid ack ({label}) did not end the transfer"));
            assert!(
                ended.contains("acknowledged") || ended.contains("regressed"),
                "the recorded reason must name the ack violation ({label}): {ended}"
            );
            assert_eq!(
                upload.send_upload_chunk(&[9u8; 8]).await,
                Err(ended),
                "a transfer aborted for a protocol violation must not keep sending ({label})"
            );

            let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .unwrap_or_else(|_| panic!("no abort followed an invalid ack ({label})"))
                .unwrap()
                .unwrap();
            assert!(
                matches!(
                    parse_control(&frame).unwrap(),
                    ControlMsg::AbortUpload { channel: c } if c == channel
                ),
                "an invalid ack ({label}) must abort the transfer",
            );
        }
    }

    /// An `UploadAborted` that arrives while NOBODY is watching must still
    /// be reported verbatim later.
    ///
    /// This is the normal case, not an exotic one: the relay spends most
    /// of a transfer awaiting its next HTTP body chunk, with no credit
    /// wait subscribed at all. An implementation that dropped the upload's
    /// entry when the abort landed would answer the next send with a
    /// generic "no longer active" and lose the supervisor's actual reason
    /// — the one thing the user needed to see.
    #[tokio::test]
    async fn an_abort_arriving_between_sends_is_retained_for_the_next_one() {
        const SENTINEL: &str = "SENTINEL-abort-between-sends: disk full";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, mut peer_writer) = peer.await.unwrap();
        let channel = 33;
        let mut upload = register_upload(&client, channel, 4096).await;

        upload.send_upload_chunk(&[1u8; 512]).await.unwrap();
        timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .unwrap()
            .unwrap()
            .unwrap();

        peer_writer
            .write_control(&ControlMsg::UploadAborted {
                channel,
                reason: SENTINEL.to_string(),
            })
            .await
            .unwrap();
        // Observed through `ended()` first, which is how the relay learns
        // of an abort while parked on its body stream.
        let reason = timeout(Duration::from_secs(5), upload.ended())
            .await
            .expect("the abort was never observable");
        assert_eq!(reason, SENTINEL);

        assert_eq!(
            upload.send_upload_chunk(&[2u8; 512]).await,
            Err(SENTINEL.to_string()),
            "a send after an abort must report the supervisor's reason, not a generic failure"
        );
    }

    /// The same retention, at the other end of the transfer: an abort that
    /// lands just before `commit` must surface as the abort's reason, not
    /// as whatever correlated error the commit would collect for a channel
    /// the supervisor has already torn down. The user needs the cause
    /// ("disk full"), not the symptom ("commit failed").
    #[tokio::test]
    async fn an_abort_just_before_commit_surfaces_its_reason() {
        const SENTINEL: &str = "SENTINEL-abort-before-commit: session deleted mid-transfer";

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (peer_reader, mut peer_writer) = peer.await.unwrap();
        let channel = 35;
        let mut upload = register_upload(&client, channel, 0).await;

        peer_writer
            .write_control(&ControlMsg::UploadAborted {
                channel,
                reason: SENTINEL.to_string(),
            })
            .await
            .unwrap();
        timeout(Duration::from_secs(5), upload.ended())
            .await
            .expect("the abort was never observable");

        let error = upload
            .commit()
            .await
            .expect_err("committing an aborted transfer must fail");
        let supervisor = error
            .downcast_ref::<SupervisorError>()
            .expect("an abort must reach the caller as a mappable SupervisorError");
        assert_eq!(supervisor.message, SENTINEL);
        let _peer = (peer_reader, peer_writer);
    }

    /// A bulk upload must not push latency-sensitive traffic to the back
    /// of the shared writer queue.
    ///
    /// `UPLOAD_CHUNK_BYTES`'s docs are explicit that the credit window
    /// alone does not deliver this: a sender obeying only the window may
    /// hand the writer a whole 4 MiB — sixteen frames — and everything
    /// enqueued afterwards waits behind all of it. With the enqueue
    /// allowance in place, a control frame sent DURING an upload burst
    /// finds at most [`UPLOAD_ENQUEUE_FRAMES`] of bulk data ahead of it.
    ///
    /// The peer is deliberately slow: a small duplex buffer parks the
    /// writer, which is what lets the queue fill at all. Reading one frame
    /// first proves the burst is genuinely underway before the control
    /// frame is sent, so the measured position is contention and not a
    /// race with a transfer that had not started.
    #[tokio::test]
    async fn a_control_frame_does_not_queue_behind_a_whole_upload_burst() {
        let (client_side, peer_side) = tokio::io::duplex(16 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        let (mut peer_reader, _peer_writer) = peer.await.unwrap();
        let channel = 41;

        // A full window's worth: enough to fill the writer queue with bulk
        // frames if nothing bounded how many may be enqueued at once.
        let total = UPLOAD_WINDOW_BYTES as usize;
        let mut upload = register_upload(&client, channel, total as u64).await;
        let bytes = vec![4u8; total];
        let send = tokio::spawn(async move { upload.send_upload_chunk(&bytes).await });

        let first = timeout(Duration::from_secs(5), peer_reader.read_frame())
            .await
            .expect("the upload burst never started")
            .unwrap()
            .unwrap();
        assert!(matches!(first.kind, FrameKind::Data));

        timeout(Duration::from_secs(5), client.pause_output(7))
            .await
            .expect("a control frame could not even be enqueued during an upload burst");

        let mut bulk_ahead = 0usize;
        loop {
            let frame = timeout(Duration::from_secs(5), peer_reader.read_frame())
                .await
                .expect("the control frame never arrived")
                .unwrap()
                .expect("connection closed before the control frame arrived");
            if matches!(frame.kind, FrameKind::Control) {
                break;
            }
            bulk_ahead += 1;
        }
        // Two ceilings, deliberately. The first says the allowance was
        // honored; the second is an absolute number the allowance cannot
        // move, so raising `UPLOAD_ENQUEUE_FRAMES` to something that no
        // longer protects interactive traffic fails this test instead of
        // relaxing it along with the constant.
        let window_frames = (UPLOAD_WINDOW_BYTES as usize) / UPLOAD_CHUNK_BYTES;
        assert!(
            bulk_ahead <= UPLOAD_ENQUEUE_FRAMES + 2 && bulk_ahead < window_frames / 2,
            "a control frame waited behind {bulk_ahead} upload frames; the enqueue allowance is \
             {UPLOAD_ENQUEUE_FRAMES}, and with no bound at all a whole {window_frames}-frame \
             window sits ahead of it"
        );
        send.abort();
    }
}
