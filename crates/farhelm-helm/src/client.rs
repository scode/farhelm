//! The helm's client half of the supervisor protocol.
//!
//! One connection per supervisor, multiplexing concurrent requests
//! (correlated by `req_id`) and any number of terminal attachments
//! (routed by data-channel id). The transport is opaque: a unix socket
//! for the local host, an ssh exec channel for a remote one — handed in
//! as a reader/writer pair so this code cannot tell the difference,
//! which is the SPEC_impl.md transport-blindness made structural.

use anyhow::bail;
use farhelm_proto::io::{
    FrameReader, FrameWriter, ProgressWrite, handshake, parse_control, write_frame_before_stall,
};
use farhelm_proto::{
    AgentKind, ControlMsg, ErrorKind, Frame, FrameKind, ProfileSnapshot, RestartMode, SessionInfo,
    TabInfo, TerminalSelector, UPLOAD_CHUNK_BYTES, UPLOAD_WINDOW_BYTES,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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

/// How many agent upcalls one connection will answer at a time.
///
/// An admission bound, applied BEFORE the answering task is spawned, and
/// the connection is the right unit for it: a supervisor forwards agent
/// requests for every session on its host, so one wedged or hostile host
/// must not be able to conscript the helm's runtime, its database, and its
/// memory on behalf of the whole fleet. Each answer walks the merged
/// listing and materializes a reply that can reach megabytes, so the
/// interesting resource is not the task but what the task allocates.
///
/// Four rather than one because these are read-only questions that mostly
/// wait on helm.db, so a little concurrency costs nothing and keeps a slow
/// listing from serializing an unrelated fast one — and rather than dozens
/// because there is no user waiting on the fifth: the overflow answer is an
/// immediate `Unavailable` telling the agent to retry, which is a better
/// outcome than a queue that turns into the upcall timeout.
///
/// ## What the slot covers, and what it therefore bounds
///
/// A permit is held from admission until the WRITER has transmitted the
/// answer's frame, not merely until the frame was accepted onto the queue
/// (it rides the queued [`Outbound`], exactly as an upload's allowance
/// does). So the bound is on queued answer BYTES as well as on concurrent
/// work: at most `AGENT_ANSWER_SLOTS × MAX_FRAME_LEN` of agent replies can
/// occupy the shared writer queue at any moment, whatever the peer does.
///
/// That is a count of frames, not a byte-weighted admission: four answers
/// that happen to be near the frame limit still qualify, so the ceiling is
/// coarse — about 32 MiB — even though a realistic listing is a tiny
/// fraction of one frame. Weighting admission by encoded size (and giving
/// interactive control traffic priority over bulk answers) is a possible
/// follow-up; it was deliberately not built here, because choosing those
/// weights is a policy decision and the count already closes the unbounded
/// case this constant exists for.
const AGENT_ANSWER_SLOTS: usize = 4;

/// Source of [`SupervisorClient::connection_id`] values.
///
/// Process-wide and never reused, which is the whole requirement: the id
/// exists so a request that travelled up one connection can be checked
/// against whatever connection the manager currently publishes for that
/// host, and a recycled number would make a dead connection's request look
/// live again.
static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

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

/// A request that produced no USABLE reply, split by the one fact that
/// changes what a caller may safely do next: whether the frame had already
/// been handed to the writer queue when the answer was lost.
///
/// The split is two-phase and the variants are four, because the post-send
/// half has three shapes that look nothing alike and mean exactly the same
/// thing to a caller: no reply, a reply of the wrong variant, and a reply
/// of the right variant this side had to throw away. They stay separate so
/// the sentence a human reads names what actually happened.
///
/// A typed error rather than the bare `anyhow` string this replaces,
/// because the two halves of that split are indistinguishable in prose and
/// the distinction is exactly the one an agent's `Rename`/`Stop`/`Archive`
/// turns on. Everything above this client used to see both endings as an
/// unclassified failure, which [`crate::error_kind`] reads as `Internal` —
/// a kind that says nothing about retrying, for a situation where "may I
/// send this again?" is the whole question. See
/// [`crate::agent_requests`]'s `transport_outcome` for who consults it and
/// what each variant then means to an agent.
///
/// The supervisor's own relay makes the SAME split one hop further out
/// (`service::agent_relay::connection_lost_after_queueing`), and for the
/// same reason: a rename that was handed to a peer that then died may
/// already have taken effect, and telling its caller "nothing happened" is
/// an invitation to apply it twice.
///
/// The two connection-loss `Display` strings keep the words "connection
/// closed" the previous untyped errors carried, since callers and tests
/// across two crates match on that phrase to recognize a dead transport.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SupervisorTransportError {
    /// The connection was already gone when the request was made: the
    /// writer queue refused it, or the pending registry was closed. Nothing
    /// was sent, so nothing happened, so a retry is free.
    #[error("the supervisor connection closed before the request was sent")]
    NotSent,
    /// The frame reached the writer queue and the connection died before a
    /// reply came back. The supervisor may have performed the request and
    /// lost only the answer.
    #[error("the supervisor connection closed after the request was sent")]
    SentUnanswered,
    /// The frame reached the writer queue, the supervisor answered, and the
    /// answer was not the reply this request's own wrapper accepts — a
    /// correlated frame of the wrong variant.
    ///
    /// A third phase rather than a plain protocol error because the FACTS
    /// about the request are the same ones `SentUnanswered` records: it went
    /// out, and nothing came back that says what became of it. A peer buggy
    /// or hostile enough to answer a `StopSession` with a `SessionRenamed`
    /// may well have performed the stop, so a caller that reads this as
    /// "something is broken, try again" can kill an agent somebody restarted
    /// in between — and one that answers a `CreateSession` with anything but
    /// a `SessionCreated` may well have started a session whose id the
    /// caller can now never be told. Only the wrappers for verbs that travel
    /// the agent relay AND change something construct it — the three
    /// lifecycle ones and the three creates; everywhere else a wrong reply
    /// stays the untyped protocol error it has always been, because
    /// `Internal` is the right reading when nothing durable was at stake.
    #[error("the supervisor answered {request} with an unexpected {reply}")]
    SentWrongReply {
        /// The request this client sent, named as its `ControlMsg` variant.
        request: &'static str,
        /// What came back instead, as its `ControlMsg` VARIANT NAME and
        /// nothing else.
        ///
        /// Bounded on purpose, and it used to be the whole `{reply:?}`
        /// rendering. Two things were wrong with that, and both are
        /// realized by one legal frame — a near-limit `SessionList`
        /// answering a `StopSession`. It is unbounded: this string is
        /// re-encoded into the agent's own reply frame, so an oversized
        /// wrong reply pushed that frame past `MAX_FRAME_LEN` and the size
        /// backstop in [`agent_response_frame`] replaced the whole outcome
        /// — turning the `Timeout`-plus-remedy vocabulary this variant
        /// exists to produce back into the bare `Internal` it exists to
        /// avoid. And it is indiscriminate: a `SessionInfo` carries a raw
        /// invocation argv and cwd, which the agent-facing surfaces redact
        /// and which a wrong-variant diagnostic has no business restoring.
        /// The variant name is the whole of what a caller can act on.
        reply: &'static str,
    },
    /// The frame reached the writer queue, the supervisor answered with the
    /// RIGHT reply variant, and the payload inside it was unusable — the
    /// ingress rules refused it, so this client threw the answer away.
    ///
    /// The same post-send phase [`Self::SentWrongReply`] records, reached
    /// one step later, and it is here for exactly that reason: the facts a
    /// caller can act on are identical. The request went out, something
    /// came back, and nothing that came back says what became of it. A
    /// `SessionCreated` whose id is empty, over the ingress cap, or
    /// carrying control characters is the case this exists for — the target
    /// has in all likelihood STARTED the session, and the one thing that
    /// could address it afterwards is the id this client just refused. A
    /// caller told "internal error" retries an unkeyed create and gets a
    /// second real session; a caller told "outcome unknown, go look" does
    /// not.
    ///
    /// Refusing the payload rather than sanitizing it is
    /// [`created_session`]'s decision and its docs carry the reasoning;
    /// this variant only carries the classification consequence of that
    /// refusal.
    #[error("the supervisor answered {request} with an unusable {reply}: {problem}")]
    SentInvalidReply {
        /// The request this client sent, named as its `ControlMsg` variant.
        request: &'static str,
        /// The reply variant that carried the unusable payload, named the
        /// same way. Not "wrong" — it is the variant this request asked
        /// for, which is what separates this from [`Self::SentWrongReply`].
        reply: &'static str,
        /// WHICH rule the payload broke, as a fixed phrase.
        ///
        /// A `&'static str` for [`Self::SentWrongReply::reply`]'s reason:
        /// this text is re-encoded into the asking agent's own reply frame,
        /// so anything derived from the peer's bytes could push that frame
        /// past `MAX_FRAME_LEN` and cost the caller the very
        /// outcome-unknown vocabulary this variant exists to deliver. The
        /// refused id itself is therefore never quoted, and neither is its
        /// length: which rule it broke is the whole of what a reader can do
        /// anything with, and the value that broke it belongs to the peer.
        problem: &'static str,
    },
}

/// Bound and sanity-check a session a supervisor says it just created,
/// before it is cached, projected, or printed.
///
/// The same ingress rule `manager::drain_sessions` applies to every id in a
/// LISTING, applied to the one id that never travels through a listing.
/// Nothing before this point checks a `SessionCreated` reply's id at all,
/// and the value goes on to three places that each assume it is well
/// formed: the helm's own cache and REST paths (an id near the frame
/// limit produces a URL no client could send), a REST body, and — through
/// the agent relay — a CLI that prints it on stdout as its machine-readable
/// answer.
/// That last one is why the control-character rule is here and not only the
/// length one: an id carrying a newline forges a second line of output in
/// whatever captured it, and an ESC reaches the terminal that captured it.
///
/// Refused rather than sanitized. A truncated or scrubbed id is not the
/// session's id, and every later use of it — stopping it, naming it as a
/// parent — would address something that does not exist. A create whose
/// reply cannot be trusted is a create that failed, and the session it
/// nonetheless started shows up in the next listing, where the same rules
/// apply to it.
///
/// The refusal is a TYPED post-send failure
/// ([`SupervisorTransportError::SentInvalidReply`]) rather than the bare
/// string error it started as, because "the session it nonetheless started"
/// is the whole problem: this is a create that reached its target, and the
/// agent relay has to say "outcome unknown, look before you retry" rather
/// than the `Internal` an unclassified error becomes. See that variant and
/// `agent_requests::transport_outcome` for the vocabulary.
fn created_session(session: SessionInfo) -> anyhow::Result<SessionInfo> {
    use crate::manager::MAX_SESSION_ID_BYTES;
    let refuse = |problem: &'static str| {
        anyhow::Error::new(SupervisorTransportError::SentInvalidReply {
            request: "CreateSession",
            reply: "SessionCreated",
            problem,
        })
    };
    if session.id.is_empty() {
        return Err(refuse("the session id is empty"));
    }
    if session.id.len() > MAX_SESSION_ID_BYTES {
        return Err(refuse("the session id is past the ingress cap"));
    }
    if session.id.chars().any(char::is_control) {
        return Err(refuse("the session id contains control characters"));
    }
    Ok(session)
}

/// `list_sessions`'s return value: one host's whole session list, and
/// whether the supervisor's cap cut it.
///
/// A struct rather than a bare `Vec<SessionInfo>` specifically so
/// `truncated` survives this call — a caller needs it to say "could not
/// read to the end" instead of quietly presenting a cut list as a whole
/// one (SPEC.md's Session list section).
///
/// This is ONE HOST's answer, and it is not what `GET /api/sessions`
/// serializes: that body is the merged, multi-host list
/// `crate::aggregate::SessionListBody` describes, built from the cache
/// rather than from a live call. What still reaches this type at serving
/// time is the per-session detail route's live lookup, which asks the
/// owning host directly (a reachable host's detail must never come from the
/// cache — see `crate::get_session`), and the manager's own refresh drain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionListing {
    pub sessions: Vec<SessionInfo>,
    /// Whether `sessions` is missing entries the supervisor held back at
    /// its cap (`farhelm_proto::LIST_SESSIONS_CAP`), straight off the wire.
    pub truncated: bool,
}

/// The peer's own hello, retained from the handshake that opened this
/// connection.
///
/// Kept because the handshake is the ONLY moment these values cross the
/// wire — `ControlMsg::Hello` is exchanged once, at connection setup, and
/// never repeated — while the consumer that needs them runs much later.
/// PLAN_M6.md item 4's connection manager decides a host's identity
/// disposition (first contact, mismatch, duplicate) from `host_identity`
/// after the connection is already live, and its diagnostic trail names
/// `build_version` on every reconnection. Without this the manager would
/// have to run its own handshake beside the client's, which is exactly the
/// duplication that lets two hellos drift apart.
///
/// Only the two fields a consumer actually acts on are kept. The hello's
/// `protocol_version` and `role` are not among them: a version this side
/// cannot speak has already failed the handshake by the time this value
/// exists (the skew refusal carries both versions itself), and `role` is
/// diagnostic-only per the wire contract. Retaining them "in case" would
/// invite a future reader to believe the connection's compatibility is
/// still an open question here.
///
/// `host_identity` is `Option` because the wire's is: a supervisor whose
/// construction has no standing to mint an identity legitimately reports
/// none (see `ControlMsg::Hello::host_identity`). `None` here means
/// exactly that and is never papered over with a synthesized value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerHello {
    pub build_version: String,
    pub host_identity: Option<String>,
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
///
/// Two producers ration themselves this way, and for the same reason:
/// bulk upload data ([`UPLOAD_ENQUEUE_FRAMES`]) and answered agent
/// upcalls ([`AGENT_ANSWER_SLOTS`]), the only two that can put megabytes
/// ahead of a keystroke. Everything else on this connection (input,
/// control, replies, and the small agent REFUSALS, which hold no
/// admission at all) converts from a bare `Frame` and carries no permit.
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
    /// The helm-resolved profile identity. This travels with the invocation
    /// so the supervisor can persist provenance without owning a catalog.
    pub source_profile: Option<ProfileSnapshot>,
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
    /// What the peer said about itself when this connection came up. Fixed
    /// for the connection's whole life — a hello is exchanged once and
    /// never repeated — which is what makes retaining it here correct
    /// rather than a cache that could go stale. See [`PeerHello`].
    peer: PeerHello,
    /// The connection-is-dead flag both background halves publish to (see
    /// [`Self::closed`]). A RECEIVER, never a sender: holding a sender here
    /// would keep the channel alive past the tasks and, worse, invert the
    /// weak-handle discipline the two tasks are built on.
    closed: watch::Receiver<bool>,
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
    /// Who answers a request the SUPERVISOR sends up this connection
    /// (`ControlMsg::AgentRequest`), and which host to answer it as.
    ///
    /// `None` for a connection nobody wired a handler into — every test
    /// double, and any future caller that has no fleet to describe. Such a
    /// connection still answers, with a refusal: a supervisor that
    /// forwarded a request is holding an agent's process open, and silence
    /// costs that agent the whole upcall budget.
    agent: Option<AgentUpcalls>,
    /// This connection's identity, for the life of the process — see
    /// [`SupervisorClient::connection_id`].
    connection_id: u64,
    /// Answering tasks this connection currently owns, so connection death
    /// can end work being done on its behalf.
    ///
    /// A `std::sync::Mutex` rather than the tokio one because every use is
    /// a push or a drain with no await inside — and because the pushing
    /// side ([`SupervisorClient::spawn_agent_answer`]) is called from the
    /// demultiplexer, which must not acquire an async lock that some other
    /// task could be holding across an await.
    agent_tasks: std::sync::Mutex<AgentTasks>,
    /// A test-only interruption between spawning an answer's work task and
    /// registering its abort handle, so the one interval retirement could
    /// once have slipped through can be produced on demand rather than
    /// raced for.
    ///
    /// See [`SupervisorClient::spawn_agent_answer`]'s "Registered before it
    /// can run" section for the property it exists to pin, and
    /// `an_answer_spawned_into_a_retirement_never_runs` for the fixture.
    #[cfg(test)]
    agent_spawn_seam: std::sync::Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    /// Whether this connection has already logged an agent refusal it could
    /// not enqueue — see [`SupervisorClient::refuse_agent_request`], whose
    /// failure mode floods by nature and is worth exactly one line.
    refusal_undeliverable_logged: AtomicBool,
    /// The connection-is-done signal, held so [`SupervisorClient::retire`]
    /// can end both background halves at a moment of the manager's
    /// choosing.
    ///
    /// A SENDER here, unlike `closed`'s receiver, and the two are the same
    /// channel: the tasks publish death into it, and this end injects it.
    /// The client holding one does not resurrect the weak-handle discipline
    /// the tasks are built on — a `watch` sender keeps no task alive and
    /// holds no transport — but it does mean nothing observes "all senders
    /// dropped" while the client lives, which is why both halves set the
    /// flag explicitly on their way out rather than relying on that.
    shutdown: watch::Sender<bool>,
}

/// The answering work one connection owns, plus the fact that decides
/// whether any MORE of it may start.
///
/// The flag is what makes "retirement cancels this connection's answering
/// work" a property of the code rather than of the scheduler. Registration
/// and the question "has this connection been retired?" have to be answered
/// under ONE lock hold, because they are the two halves of a single
/// decision: a task registered after the last drain would be a task nothing
/// is left to abort, doing a fleet listing — or routing a mutation to
/// another host — on behalf of a peer that is provably gone. See
/// [`SupervisorClient::spawn_agent_answer`] for the start gate that makes
/// the decision reachable before the work has run.
///
/// Terminal, deliberately: a retired connection is never revived (the
/// manager builds a new one), so nothing ever clears the flag.
#[derive(Default)]
struct AgentTasks {
    /// The WORK tasks' abort handles — not their owners'; see
    /// [`SupervisorClient::abort_agent_tasks`].
    handles: Vec<tokio::task::AbortHandle>,
    /// Whether [`SupervisorClient::abort_agent_tasks`] has run, i.e. this
    /// connection has been failed or retired.
    retired: bool,
}

/// What one connection needs to answer an agent's question: the shared
/// handler slot, who to answer as, and this connection's ration of
/// concurrent answers.
///
/// The ORIGIN is here because it is the thing the handler cannot work out
/// for itself. An upcall arrives on exactly one host actor's connection,
/// and that host is by construction the asking session's own — which is how
/// `current` gets answered (see [`crate::agent_requests`]'s module docs) —
/// while the connection id is what lets the handler check that this
/// connection is still the one that host is served by.
#[derive(Clone)]
struct AgentUpcalls {
    handler: crate::agent_requests::AgentRequestSlot,
    origin: crate::agent_requests::AgentOrigin,
    /// [`AGENT_ANSWER_SLOTS`] permits, shared by every answer on this
    /// connection.
    permits: Arc<Semaphore>,
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

/// The refusal an agent gets when this connection cannot take its question
/// right now.
///
/// [`farhelm_proto::ErrorKind::Unavailable`] rather than `Internal`,
/// because every case it covers is transient and the caller's correct
/// response is to try again: a helm that has not finished starting will
/// have finished in a moment, a connection at its concurrent-answer limit
/// will have a slot shortly, and a connection with no handler belongs to a
/// caller that is not serving a fleet at all. Each message names its own
/// remedy, because "retry" is right for the first two and "there is nothing
/// here to ask" for the third.
fn not_ready(message: &str) -> farhelm_proto::AgentOutcome {
    farhelm_proto::AgentOutcome::Err {
        kind: farhelm_proto::ErrorKind::Unavailable,
        message: message.to_string(),
    }
}

/// The error for a MUTATING request answered with a correlated reply of
/// the wrong variant — see [`SupervisorTransportError::SentWrongReply`].
///
/// A function rather than an inline construction at each site because the
/// phase claim it makes is the load-bearing part and must be made
/// identically by every wrapper that makes it: `stop`, `rename`, `archive`
/// and the three creates are the verbs an agent can drive across two hops,
/// so each of them is a place where a wrong answer has to keep the
/// request's own "it was sent" fact rather than degrading into an untyped
/// protocol complaint. A create is the sharpest case of it — a peer that
/// answered `CreateSession` with something other than `SessionCreated` may
/// still have started a session, whose id the caller now has no way to
/// learn. Verbs whose wrong replies stay untyped (`list_sessions`,
/// `restart`, the tab and upload calls) are deliberately not routed here;
/// nothing above them turns the distinction into advice.
///
/// BOTH sides are `ControlMsg` variant NAMES rather than messages, because
/// this string is rendered into an agent-facing error chain and re-encoded
/// into the agent's own reply frame: a message carries session ids, raw
/// invocations and cwds, and a reply message carries however many megabytes
/// the peer chose to send. See [`SupervisorTransportError::SentWrongReply`]'s
/// `reply` field for what the unbounded rendering actually cost.
fn wrong_reply(request: &'static str, reply: &ControlMsg) -> anyhow::Error {
    anyhow::Error::new(SupervisorTransportError::SentWrongReply {
        request,
        reply: reply.variant_name(),
    })
}

/// The answer an upcall gets when the task that was preparing it DIED —
/// panicked, or ended in any way that produced no outcome of its own.
///
/// It exists because "the answer task is gone" is not a state the rest of
/// the relay can observe. The connection stays healthy, so nothing calls
/// `fail_all`; the supervisor's pending entry stays in a LIVE link's map,
/// so its answer-budget expiry retains the mutation's delete fence and then
/// waits for a resolution that can no longer come. The asking session would
/// get a timeout and the asker's session id would stay fenced against
/// deletion for the rest of the connection's life. So the supervising task
/// answers on the dead one's behalf, which both frees the fence and tells
/// the asker something true.
///
/// A MUTATION gets [`farhelm_proto::ErrorKind::Timeout`] and the
/// check-before-retrying remedy for the same reason every other
/// delivered-outcome-unknown ending does: the handler had already begun,
/// and this side cannot know whether it got as far as renaming, stopping or
/// archiving the target before it died. A listing gets the ordinary
/// retry-safe refusal, having changed nothing whatever it did.
fn panic_fallback(is_mutation: bool) -> farhelm_proto::AgentOutcome {
    if !is_mutation {
        return not_ready("the helm failed while answering this request; retry");
    }
    farhelm_proto::AgentOutcome::Err {
        kind: farhelm_proto::ErrorKind::Timeout,
        message: format!(
            "the helm failed while performing this request, so the outcome is unknown — {}",
            farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY
        ),
    }
}

/// Encode one `AgentResponse`, substituting a small refusal for a reply
/// that would not fit in a frame.
///
/// The size check is what keeps ONE oversized answer from costing the whole
/// connection. Frames are encoded by the writer task, which discovers a
/// too-large body only as a write error it cannot attribute — and treats it
/// exactly like a broken transport, tearing down the multiplexed connection
/// that carries every terminal, upload and request for this host. Checking
/// here lets the request that caused it fail alone (see
/// [`Frame::exceeds_max_len`], which exists for this).
///
/// It is a BACKSTOP, not the bound: the listing handler holds its own reply
/// under a byte allowance well below the frame limit and the CLI caps its
/// column widths, so a well-behaved helm never reaches this. It is here
/// because "well-behaved" is an assumption about code on the other side of
/// a trait object, and the cost of the assumption being wrong is a
/// connection rather than a request.
///
/// `Internal` rather than `Unavailable`: nothing about retrying or waiting
/// changes the answer, since the same question would produce the same
/// oversized reply.
///
/// ## The one outcome the backstop must not flatten
///
/// A replacement that always said `Internal` could DOWNGRADE the answer it
/// was replacing, and the case is not hypothetical: an outcome that already
/// carries [`farhelm_proto::ErrorKind::Timeout`] is the relay's
/// "delivered, outcome unknown" verdict on a mutation, built by
/// [`crate::agent_requests`]'s `transport_outcome` out of an error chain
/// this side does not control the length of. Rewriting that to `Internal`
/// tells the asking agent "this should not happen" about a mutation — a
/// rename/stop/archive, or a create/clone — that may well have taken
/// effect: the exact substitution the mutation vocabulary exists to
/// prevent, arrived at through a size check.
/// So the kind and the check-before-retrying remedy survive the
/// replacement; only the oversized prose is dropped.
///
/// Keyed on the KIND rather than on a `mutating` flag threaded down from
/// the caller, because the kind is the claim: `Timeout` on this path means
/// outcome-unknown wherever it came from, and a size backstop has no
/// business deciding that a claim it cannot fit is a claim it can revoke.
fn agent_response_frame(req_id: u64, outcome: farhelm_proto::AgentOutcome) -> Frame {
    // Read before `outcome` moves into the frame; nothing is cloned.
    let outcome_unknown = matches!(
        &outcome,
        farhelm_proto::AgentOutcome::Err {
            kind: farhelm_proto::ErrorKind::Timeout,
            ..
        }
    );
    let frame = Frame::control(&ControlMsg::AgentResponse { req_id, outcome });
    if frame.exceeds_max_len() {
        warn!(
            req_id,
            bytes = frame.encoded_len(),
            outcome_unknown,
            "an agent reply exceeded the protocol's frame limit and was replaced by a refusal"
        );
        let replacement = if outcome_unknown {
            farhelm_proto::AgentOutcome::Err {
                kind: farhelm_proto::ErrorKind::Timeout,
                message: format!(
                    "the reply to this request is too large to send, so the outcome is unknown \
                     — {}",
                    farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY
                ),
            }
        } else {
            farhelm_proto::AgentOutcome::Err {
                kind: farhelm_proto::ErrorKind::Internal,
                message: "the reply to this request is too large to send".to_string(),
            }
        };
        return Frame::control(&ControlMsg::AgentResponse {
            req_id,
            outcome: replacement,
        });
    }
    frame
}

/// Enqueue one answered `AgentResponse`, carrying its admission permit onto
/// the writer queue.
///
/// The permit is MOVED into the queued item rather than dropped when this
/// returns, which is what makes [`AGENT_ANSWER_SLOTS`] bound queued bytes
/// as well as concurrent work: the allowance comes back when the writer has
/// written the frame, so a connection can never hold more than its slots'
/// worth of near-frame-sized answers in the shared queue at once. Releasing
/// it at enqueue time — the shape this replaces — let each answer free its
/// slot the moment the frame was accepted, so a slow-but-progressing
/// transport could accumulate a queue of megabyte replies ahead of every
/// keystroke on the connection. It is the same discipline uploads already
/// use; see [`Outbound`].
async fn send_agent_outcome(
    writer_tx: &mpsc::Sender<Outbound>,
    req_id: u64,
    outcome: farhelm_proto::AgentOutcome,
    allowance: OwnedSemaphorePermit,
) {
    let frame = agent_response_frame(req_id, outcome);
    let _ = writer_tx
        .send(Outbound {
            frame,
            _allowance: Some(allowance),
        })
        .await;
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
        Self::start_with_seams(r, w, writer_stall, upload_stall, None).await
    }

    /// Like [`Self::start`], but able to answer the supervisor's agent
    /// upcalls — the shape the connection manager uses for every real host
    /// (PROTOCOL_VERSION 13).
    ///
    /// The handler arrives as a SLOT rather than a value, and reading it
    /// per request rather than capturing it here is what closes a startup
    /// window: actors begin dialling before the helm's `AppState` (and so
    /// the handler) exists, and a connection that captured `None` at that
    /// moment would answer "not ready" for the rest of its life. See
    /// [`crate::agent_requests::AgentRequestSlot`].
    pub async fn start_for_host<R, W>(
        r: R,
        w: W,
        handler: crate::agent_requests::AgentRequestSlot,
        host: crate::store::HostId,
    ) -> anyhow::Result<Arc<SupervisorClient>>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self::start_with_seams(
            r,
            w,
            WRITER_STALL_TIMEOUT,
            UPLOAD_ACK_STALL_TIMEOUT,
            Some((handler, host)),
        )
        .await
    }

    /// Every knob the constructors above vary, in one place, so that the
    /// handshake and task wiring exist exactly once.
    ///
    /// `agent` arrives as the raw pair rather than a built [`AgentUpcalls`]
    /// because the connection's own id is minted HERE, and the origin an
    /// upcall is answered under has to carry it.
    async fn start_with_seams<R, W>(
        r: R,
        w: W,
        writer_stall: Duration,
        upload_stall: Duration,
        agent: Option<(
            crate::agent_requests::AgentRequestSlot,
            crate::store::HostId,
        )>,
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
        // Retained rather than discarded: the peer's identity and build
        // reach this process exactly once, here. See [`PeerHello`].
        // `handshake` guarantees the reply is a `Hello` (anything else is
        // already an `Err` above), so the `else` arm below is unreachable
        // in practice — it is written as a hard failure rather than a
        // silent default so a future change to that guarantee cannot
        // quietly hand every host a `None` identity.
        let peer = match handshake(&mut reader, &mut writer, "helm").await? {
            ControlMsg::Hello {
                build_version,
                host_identity,
                ..
            } => PeerHello {
                build_version,
                host_identity,
            },
            other => bail!("handshake returned a non-hello reply: {other:?}"),
        };

        let (writer_tx, mut writer_rx) = mpsc::channel::<Outbound>(SUPERVISOR_WRITER_QUEUE);
        let (connection_done, _) = watch::channel(false);

        let connection_id = NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed);
        let agent = agent.map(|(handler, host)| AgentUpcalls {
            handler,
            origin: crate::agent_requests::AgentOrigin {
                host,
                connection: connection_id,
            },
            permits: Arc::new(Semaphore::new(AGENT_ANSWER_SLOTS)),
        });

        let client = Arc::new(SupervisorClient {
            peer,
            closed: connection_done.subscribe(),
            writer_tx,
            pending: Mutex::new(Pending::default()),
            terminals: Mutex::new(HashMap::new()),
            uploads: Mutex::new(HashMap::new()),
            upload_enqueue: Arc::new(Semaphore::new(UPLOAD_ENQUEUE_FRAMES)),
            upload_stall,
            next_req: AtomicU64::new(1),
            // Channel 0 is the control channel; attachments start at 1.
            next_channel: AtomicU64::new(1),
            agent,
            connection_id,
            agent_tasks: std::sync::Mutex::new(AgentTasks::default()),
            #[cfg(test)]
            agent_spawn_seam: std::sync::Mutex::new(None),
            refusal_undeliverable_logged: AtomicBool::new(false),
            shutdown: connection_done.clone(),
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
                    // `break`, not `return`: cancellation is the manager
                    // withdrawing this connection ([`SupervisorClient::retire`])
                    // or the writer half having already died, and BOTH leave
                    // requests sitting in `pending` whose frames the writer
                    // queue accepted. Returning here skipped the `fail_all()`
                    // tail below and left every one of them parked on a
                    // `oneshot` nothing would ever complete — for an agent's
                    // rename/stop/archive, a supervisor-side delete fence held
                    // against the asking session for the life of the process
                    // while its host reconnected happily on a new connection.
                    // Every ending of this loop must drain, so they all leave
                    // it the same way.
                    _ = reader_cancel.changed() => break,
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
    /// the other alive and waiters hung. The demux loop also arrives here
    /// by way of the shutdown signal, which is how a manager-driven
    /// [`Self::retire`] drains what the connection was carrying instead of
    /// abandoning it.
    ///
    /// Terminals and uploads each get an explicit out-of-band notice
    /// (`signal_detached`, `end_upload` on the upload's watch); pending
    /// requests are failed by dropping their oneshot senders, which makes
    /// `request()` return [`SupervisorTransportError::SentUnanswered`]
    /// instead of hanging an HTTP handler. That variant rather than the
    /// other one is the whole point of draining here: every entry in this
    /// map belongs to a frame the writer queue already accepted, so a
    /// caller must be told its request may have been acted on.
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
        drop(pending);
        // Agent answers are the one kind of work this connection does on
        // somebody ELSE's behalf, so they are the one kind that has to be
        // stopped explicitly rather than merely failed: the supervisor that
        // asked has already been told (its own teardown fails every upcall
        // it was carrying), and what remains is a fleet listing being
        // assembled — a database walk and a reply that can reach megabytes
        // — for a peer that can no longer receive it.
        self.abort_agent_tasks();
    }

    /// Stop every answer this connection is assembling.
    ///
    /// Shared by the two endings that must not leave one running: the
    /// connection dying ([`Self::fail_all`]) and the manager withdrawing
    /// the connection ([`Self::retire`]). Dropping an `AbortHandle` does
    /// not abort anything, so nothing but this call ends that work.
    ///
    /// These handles are the WORK tasks, not their supervisors (see
    /// [`Self::spawn_agent_answer`]). Each supervisor observes its task's
    /// cancellation, sends nothing, and releases the admission slot — the
    /// one termination that is deliberately not answered, because this is
    /// exactly the case where the peer is already gone.
    ///
    /// ABORTING A MUTATION DOES NOT UNDO IT, and nothing here pretends
    /// otherwise. A `Rename`/`Stop`/`Archive` task aborted at an await
    /// point may already have sent its mutation to the TARGET host — a
    /// different connection from this one, which this abort does not touch
    /// — so the durable change can land after the asking side has been told
    /// the request ended. There is no way to know from here which side of
    /// that line an aborted task was on, so the honest vocabulary is
    /// applied where the answer is reported instead: the supervisor's relay
    /// gives a mutating verb whose connection died an "outcome unknown"
    /// ending rather than a retry-safe one (`service::agent_relay`'s
    /// `connection_lost_after_queueing`). Aborting anyway is still right —
    /// the alternative is a listing being assembled for a peer that cannot
    /// receive it — but it is a cancellation of the ANSWER, not of the act.
    ///
    /// Marks the connection retired in the SAME lock hold that drains the
    /// handles, which is what closes the door behind it: an answer whose
    /// task was spawned but not yet registered would otherwise be inserted
    /// into a list this call had already emptied and run on unabortably.
    /// See [`AgentTasks`] and [`Self::spawn_agent_answer`].
    fn abort_agent_tasks(&self) {
        let handles = {
            let mut tasks = self.agent_tasks.lock().expect("agent task list poisoned");
            tasks.retired = true;
            std::mem::take(&mut tasks.handles)
        };
        for handle in handles {
            handle.abort();
        }
    }

    /// End this connection because the manager has stopped publishing it —
    /// a retarget, an adoption, a reconnect, or a retired host entry.
    ///
    /// Synchronous, and callable from inside a `watch::send_modify`
    /// closure, because that is where a client is withdrawn (see
    /// `manager::HostActor::publish_refresh`). It does exactly two things,
    /// both of which the last `Arc` drop CANNOT be trusted to do:
    ///
    /// - aborts the answering tasks, which is what stops a fleet listing
    ///   being assembled for a peer nobody will accept an answer from;
    /// - signals both background halves, which shuts the write half and
    ///   lets the transport (an ssh child, a socket) actually close.
    ///
    /// Signalling the halves is also what FAILS this connection's pending
    /// requests, and that is a requirement rather than a side effect: the
    /// demux loop answers the signal by breaking into [`Self::fail_all`],
    /// so a request whose frame the writer queue had already accepted comes
    /// back as [`SupervisorTransportError::SentUnanswered`] instead of
    /// waiting on a connection nobody is reading any more. A retirement is
    /// ordinary — a reconnect, a retarget, an adoption — so the waiter is
    /// typically an agent's rename/stop/archive being relayed to this host,
    /// and the supervisor that asked holds a delete fence until it hears
    /// something back. Retiring without draining strands that fence for the
    /// life of the process, on a fleet that has otherwise recovered.
    /// Asynchronous only in the sense that the drain happens on the demux
    /// task rather than under this call.
    ///
    /// The drop was never sufficient for either. An in-flight agent task
    /// holds a `writer_tx` clone, so the writer channel stays open, so the
    /// writer task stays parked, so the transport stays alive — and when
    /// the listing finishes it writes its answer onto a connection the
    /// registry replaced, which for a host listing means naming the OLD
    /// connection's row as the asking session's `current` host after that
    /// row's machine has already changed. Retiring explicitly makes the
    /// withdrawal and the teardown the same event.
    ///
    /// Idempotent: a second call re-signals a flag that is already set and
    /// drains an empty task list.
    pub(crate) fn retire(&self) {
        self.abort_agent_tasks();
        let _ = self.shutdown.send(true);
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
    /// A reply correlated to a `req_id` this connection NEVER ISSUED is
    /// fatal for the same reason and not for the "no longer exists" one —
    /// see the arm itself, which is where the distinction between an
    /// impossible correlation and an ordinary late answer is drawn.
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
                // WHICH messages are replies is the protocol's question,
                // not this client's: [`ControlMsg::reply_req_id`] answers
                // it beside the enum, so a reply variant added there cannot
                // arrive here unrouted and hang its caller forever. It
                // deliberately answers `None` for the REQUESTS that carry a
                // `req_id` too — a peer echoing one back must never be
                // delivered as that request's answer.
                //
                // `req_id` 0 means "tied to no request" (an unsolicited
                // `Error`); it has no pending entry to complete and falls
                // through to the arms below, which log it.
                if let Some(req_id) = msg.reply_req_id().filter(|req_id| *req_id != 0) {
                    // A reply naming an id this connection has NEVER handed
                    // out is impossible rather than merely late, and the
                    // two must be told apart here because only one of them
                    // is survivable. See `Self::request`'s own docs: a
                    // waiter on this map has no deadline, so a peer that
                    // answers under an id nobody is waiting for — and never
                    // under the real one — parks that waiter for the life
                    // of a connection that stays perfectly healthy, along
                    // with the agent-answer permit and, one hop out, the
                    // asking session's delete fence. Killing the connection
                    // converts it into the one ending the whole relay
                    // already knows how to report (`fail_all`, hence
                    // `SentUnanswered` for everything already queued).
                    //
                    // "Issued" is exactly `< next_req`, and `request()`
                    // maintains that for every id it sends rather than
                    // trusting callers to have minted theirs with
                    // `req_id()` — see the `fetch_max` there. The relaxed
                    // load is ordered by causality rather than by the
                    // atomic: a peer can only name an id whose request
                    // reached the wire, which is after that update and
                    // after the writer queue's own synchronization.
                    if req_id >= self.next_req.load(Ordering::Relaxed) {
                        anyhow::bail!(
                            "the supervisor answered with req_id {req_id}, which this connection \
                             never issued"
                        );
                    }
                    // An id that WAS issued and is no longer in the map is
                    // the ordinary late answer — a cancelled HTTP handler,
                    // a request whose caller went away — and is dropped in
                    // silence, exactly as before. Nothing here can tell a
                    // late answer from a peer re-answering a completed id,
                    // so that residual case is bounded only by the
                    // connection's own life; the supervisor's retained
                    // fence is bounded independently
                    // (`service::agent_relay::HelmLink::upcall`).
                    if let Some(tx) = self.pending.lock().await.map.remove(&req_id) {
                        let _ = tx.send(msg);
                    }
                    return Ok(());
                }
                match &msg {
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
                    // The one message on this protocol that arrives here as
                    // a REQUEST rather than a reply or an event: an agent
                    // inside a session asked the helm something, and its
                    // supervisor forwarded the question up the connection
                    // the helm itself opened. See
                    // [`crate::agent_requests`] for why the question
                    // travels this way at all.
                    //
                    // Answered from a SPAWNED task, without exception. This
                    // is the shared demultiplexer for every terminal,
                    // reply, and upload on the connection, so answering
                    // inline would put the whole fleet's traffic behind one
                    // listing — and the listing reads helm.db, which is
                    // exactly the kind of work that must not sit on this
                    // path (see `dispatch`'s head-of-line note above).
                    //
                    // THE TRUST BOUNDARY IS THIS CONNECTION, NOT THIS
                    // MESSAGE. `session_id` and the claim that this
                    // connection's host is that session's host are accepted
                    // without re-verification: the helm never sees the
                    // per-session credential (only the supervisor can check
                    // it, and does, before forwarding), and the supervisor
                    // on the far end of a full-authority connection is the
                    // helm's own provisioned install with complete
                    // authority over every session on its host. A helm that
                    // could not trust it could not route an operation to it
                    // either. See SPEC_impl.md's version-13 paragraph.
                    ControlMsg::AgentRequest {
                        req_id,
                        session_id,
                        request,
                    } => {
                        self.spawn_agent_answer(*req_id, session_id.clone(), request.clone());
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

    /// This connection's identity within this process — minted with the
    /// connection, never reused, never recycled.
    ///
    /// It exists for one consumer: the agent-request handler, which is
    /// handed a host's registry id and has to decide whether the connection
    /// that carried the request is still the one that host is served by
    /// (see [`crate::agent_requests::AgentOrigin`]). Registry ids survive a
    /// retarget and a machine swap; this does not.
    pub fn connection_id(&self) -> u64 {
        self.connection_id
    }

    /// Answer one agent upcall off the demultiplexer's thread and send the
    /// response back up the same connection.
    ///
    /// Answers on every path it can — no handler wired in, no admission
    /// slot free, a reply too large to send. The supervisor that forwarded
    /// the request is holding an agent's `farhelm` process open until
    /// something arrives, so silence here is not a dropped message: it is a
    /// user watching a command hang for the whole upcall budget and then
    /// fail with a timeout that names the wrong cause.
    ///
    /// The single exception is a refusal that will not fit on a writer
    /// queue that is already full: it is neither queued nor spawned, and
    /// the CONNECTION ends instead, so the supervisor's own teardown is what
    /// tells the asking session — see [`Self::refuse_agent_request`] for why
    /// an accepted request may never be left with no terminal response at
    /// all.
    ///
    /// ## Admission before work
    ///
    /// The permit is taken with `try_acquire` BEFORE anything is spawned,
    /// because what needs bounding is not the task but everything the task
    /// then allocates — a merged fleet listing and a reply that can reach
    /// megabytes. Acquiring inside the task would bound concurrency while
    /// leaving an unbounded queue of tasks waiting to become expensive. A
    /// connection with no slot free gets an immediate refusal naming the
    /// retry, which beats a queue that resolves as the supervisor's
    /// timeout.
    ///
    /// ## Refusals spawn NOTHING
    ///
    /// The two refusal paths — no handler on this connection, no slot free
    /// — enqueue with a non-blocking [`Self::refuse_agent_request`] instead
    /// of a task of their own. A spawned refusal is unbounded by
    /// construction: it holds no permit (it was refused for want of one),
    /// so a peer that sends requests faster than it drains replies parks a
    /// new task on the full writer queue for every one, none of them owned
    /// by this connection's teardown. That is the same unbounded growth
    /// admission control exists to prevent, arrived at through the path
    /// that was supposed to enforce it.
    ///
    /// ## Owned, not merely spawned — and SUPERVISED
    ///
    /// The handle is retained so [`Self::fail_all`] can abort it. A
    /// connection's death means nobody is left to receive the answer, and
    /// an answer in progress is a database walk and a multi-megabyte
    /// allocation being done for a dead peer.
    ///
    /// Two tasks rather than one: the work task computes an outcome, and a
    /// small owner task awaits it and is what actually sends. That shape
    /// exists so that EVERY way the work can end produces a response —
    /// including the one nothing else in this system can see. A panic
    /// anywhere in the handler leaves the connection perfectly healthy, so
    /// no teardown runs and no `fail_all` fires; the supervisor's pending
    /// entry sits in a live link's map, its answer budget expires, and for a
    /// mutation the delete fence it retained is then held against the
    /// asker's session id until the connection eventually dies. Every later
    /// mutation and any delete of that session blocks behind it. Answering
    /// from outside the task is the only place that ending can be caught;
    /// see [`panic_fallback`] for what is said. A CANCELLED task is
    /// deliberately silent — that is this connection's own teardown, where
    /// the answer has nowhere to go.
    ///
    /// ## Registered before it can run
    ///
    /// A spawned task starts when the runtime says so, which on a
    /// multithreaded runtime can be before the next line of this function
    /// executes. Storing the abort handle afterwards therefore used to leave
    /// a real interval — spawn to registration — in which
    /// [`Self::abort_agent_tasks`] could drain an empty list and return,
    /// after which the handle was inserted into a connection that had
    /// already been torn down. The escaped task kept walking the database
    /// for a peer nobody would accept an answer from, and a mutation whose
    /// entry check had already passed kept routing to its target, both
    /// outside the boundary these docs claim owns them.
    ///
    /// So the work task is spawned PARKED, behind a one-shot start gate, and
    /// the gate is opened only after the handle is stored — under the same
    /// lock hold that asks whether the connection has been retired (see
    /// [`AgentTasks`]). Retired, and the task is aborted at the gate instead
    /// of registered: nothing of the handler ever runs, and the owner
    /// observes an ordinary cancellation. The ordering is then a property of
    /// the code rather than of how the scheduler felt.
    ///
    /// ## The origin is checked twice — but ONLY for a read-only verb
    ///
    /// The handler checks it on the way in (that is where `current` is
    /// computed from), and for `Hosts`/`Sessions` this checks it again with
    /// [`crate::agent_requests::AgentRequestHandler::origin_is_live`]
    /// immediately before an ANSWER is enqueued. The listing between the
    /// two awaits on the database and the manager, and a host that was
    /// retargeted, adopted or reconnected in that window has a registry row
    /// whose machine has changed underneath the answer being assembled for
    /// it. Sending it anyway would cross that boundary — the reply names
    /// the old connection's host as `current` while the row now belongs to
    /// someone else — so the answer is dropped for a refusal the agent can
    /// retry.
    ///
    /// A MUTATING verb that came back `Ok` skips this second check
    /// entirely, and that is not an oversight: by the time `Ok` reaches
    /// here the mutation has already happened, non-retriably, at its
    /// target. Reporting it as [`not_ready`]'s `Unavailable` — which
    /// callers read as "never happened, safe to repeat" — would be false
    /// for an action that just took effect; a retry could re-rename,
    /// re-stop, or re-target the wrong session on the strength of a lie
    /// this connection told about its own liveness, and for a
    /// `Create`/`Clone` it would strand a session that is already running
    /// under an id nobody was told. The ENTRY check inside
    /// `handle` already refused the mutation outright if the connection
    /// was stale before any of it ran (see that function's own docs); there
    /// is no honest "undo" once it has run, so the exit check is skipped
    /// rather than made to lie. This also sidesteps the one thing the exit
    /// check could still have caught for these verbs — a
    /// `Rename`/`Archive`/`Create`/`Clone` reply's own host name going
    /// stale in the same window — because
    /// `agent_requests::agent_row_of_mutation` (behind both
    /// `agent_session_reply` and `agent_created_reply`) pins that name (and
    /// marks the row `stale` when it cannot) against the SAME incarnation
    /// the mutation itself routed through, rather than a fresh, unchecked
    /// lookup this check would otherwise be guarding.
    ///
    /// Fire-and-forget on the writer queue: a response that cannot be
    /// enqueued means the connection is going away, and the supervisor's
    /// own teardown already fails every upcall it was carrying.
    fn spawn_agent_answer(
        &self,
        req_id: u64,
        session_id: String,
        request: farhelm_proto::AgentVerb,
    ) {
        let Some(agent) = self.agent.clone() else {
            self.refuse_agent_request(req_id, "this helm connection cannot answer agent requests");
            return;
        };
        let Ok(permit) = Arc::clone(&agent.permits).try_acquire_owned() else {
            self.refuse_agent_request(
                req_id,
                "too many agent requests in flight on this host; retry",
            );
            return;
        };
        // Captured before `request` is moved into `handler.handle` below —
        // see this method's own docs ("The origin is checked twice") for
        // why a MUTATING verb's successful outcome must not be re-judged
        // against the origin's liveness the way a listing's is. The
        // classification itself belongs to the verb, not to this file: the
        // supervisor's relay asks the identical question about the identical
        // set, and two hand-maintained lists would eventually disagree.
        let is_mutation = request.is_mutating();
        let writer_tx = self.writer_tx.clone();
        // The start gate — see "Registered before it can run" above. The
        // sender is released only once this task's abort handle is stored,
        // so everything below the gate is inside the cancellation boundary
        // rather than racing it.
        let (start, started) = oneshot::channel::<()>();
        // The work task ANSWERS rather than sends: what reaches the wire is
        // decided by its owner below, which is what lets a death that
        // produced no answer still produce one.
        let answer = tokio::spawn(async move {
            // The gate is dropped rather than sent on only if registration
            // refused this task, which also aborts it — so this arm is
            // unreachable and is written as a refusal rather than as work,
            // so that a future change which drops the gate WITHOUT aborting
            // cannot turn it into an unowned answer.
            if started.await.is_err() {
                return not_ready("the host connection was replaced; retry");
            }
            match agent.handler.get() {
                Some(handler) => {
                    let outcome = handler.handle(agent.origin, &session_id, request).await;
                    // Re-checked here rather than left to the handler's own
                    // entry check: the lookup that produced this answer has
                    // been awaiting all along, and the question that matters
                    // is whether the connection is current NOW, one step
                    // before the frame is queued. Skipped for a completed
                    // mutation — see this method's docs.
                    match outcome {
                        farhelm_proto::AgentOutcome::Ok { .. }
                            if !is_mutation && !handler.origin_is_live(agent.origin) =>
                        {
                            not_ready("the host connection was replaced; retry")
                        }
                        outcome => outcome,
                    }
                }
                None => not_ready("the helm is still starting up; retry in a moment"),
            }
        });
        let abort = answer.abort_handle();
        tokio::spawn(async move {
            let outcome = match answer.await {
                Ok(outcome) => outcome,
                // Cancelled means THIS CONNECTION is being torn down
                // (`abort_agent_tasks`, from `fail_all` or `retire`), which
                // is the one ending that must stay silent: there is nobody
                // left to receive an answer, the supervisor's own teardown
                // has already failed every upcall it was carrying, and the
                // queue this would push onto is going away with the rest.
                Err(join) if join.is_cancelled() => return,
                Err(_) => {
                    warn!(
                        req_id,
                        is_mutation,
                        "an agent answer task died; the asking session is being told the outcome \
                         is unknown"
                    );
                    panic_fallback(is_mutation)
                }
            };
            // The permit goes ONTO the queue with the frame; see
            // `send_agent_outcome`. It rides in this task rather than in the
            // one above so a panic cannot take the admission slot's whole
            // purpose with it: the fallback answer is queued under the same
            // allowance a real one would have been.
            send_agent_outcome(&writer_tx, req_id, outcome, permit).await;
        });
        // The interval this seam interrupts is exactly the one the gate
        // above exists to make survivable; see the field's docs.
        #[cfg(test)]
        if let Some(seam) = self
            .agent_spawn_seam
            .lock()
            .expect("agent spawn seam poisoned")
            .as_ref()
        {
            seam();
        }
        {
            let mut tasks = self.agent_tasks.lock().expect("agent task list poisoned");
            if tasks.retired {
                // The connection was withdrawn while this answer was being
                // set up. Aborting BEFORE the gate opens is what makes the
                // cancellation boundary structural: the work task has not
                // been polled past its first await, so the handler never
                // runs, and the owner sees a cancelled join and stays
                // silent exactly as it does for a task aborted mid-flight.
                abort.abort();
                return;
            }
            // Pruned on the way in rather than by each task on its way out: a
            // finishing task would have to reach back into this list to remove
            // itself, and the list can never hold more than the permits allow
            // plus whatever has finished since the last request.
            tasks.handles.retain(|handle| !handle.is_finished());
            // The WORK task's handle, not its owner's: aborting the work is
            // what stops a fleet listing being assembled for a peer that
            // cannot receive it, and the owner then observes the cancellation
            // and releases the admission slot. Storing the owner's instead
            // would leave the work running with nothing left to notice.
            tasks.handles.push(abort);
        }
        // Registered, so it may run. Nothing waits on this: the receiver is
        // held by a task that cannot have finished, so the send cannot fail.
        let _ = start.send(());
    }

    /// Enqueue one small `Unavailable` refusal for an upcall this
    /// connection will not do any work for, without blocking and without
    /// spawning — and RETIRE the connection if even that will not fit.
    ///
    /// Called from the demultiplexer, so it must not await — but neither
    /// may it hand the refusal to a task, which is what makes `try_send`
    /// the whole point rather than an optimization. A refusal is issued
    /// precisely when admission was DENIED, so a task carrying one holds no
    /// permit and nothing bounds how many of them a peer can create by
    /// sending faster than it reads.
    ///
    /// ## Why a full queue ends the connection
    ///
    /// The refusal is the SOLE terminal response to a request the
    /// supervisor has already accepted on the asking session's behalf, and
    /// dropping it used to be treated as harmless on the grounds that the
    /// supervisor's own budget would expire. That reasoning holds for a
    /// listing and fails for a mutation: a rename/stop/archive whose answer
    /// budget expires does not END there, because the budget expiring says
    /// nothing about whether the helm is still working, so the supervisor
    /// RETAINS the asking session's delete fence until the request resolves
    /// or the link dies (`service::agent_relay::HelmLink::upcall`). A queue
    /// that later drains on a connection that never closes gives it
    /// neither, and every subsequent mutation from that agent — and any
    /// delete of its session — blocks for the rest of an otherwise healthy
    /// connection's life.
    ///
    /// So the ending is the link's, not the request's: [`Self::retire`]
    /// signals both halves, the write half closes, and the supervisor's own
    /// teardown resolves every upcall it was carrying with the post-queue
    /// vocabulary. That vocabulary is conservative rather than exact — a
    /// mutation refused for want of an admission slot provably did NOT run,
    /// yet arrives as "delivered, outcome unknown" — which is the right way
    /// round: a bounded, honest overstatement of doubt beats an unbounded
    /// silent hold.
    ///
    /// Retiring is chosen over the alternative of reserving refusal capacity
    /// before admitting a request because it needs no new bookkeeping to go
    /// wrong: this path is only reachable on a connection whose bounded
    /// writer queue is already full, which is a connection failing to keep
    /// up with its own peer, so closing it costs a reconnect that the
    /// no-progress timeouts were heading towards anyway.
    ///
    /// Logged once per connection, not once per failure: `retire` is
    /// idempotent and the frames behind this one produce the same ending,
    /// so the first line has said everything the thousandth would.
    fn refuse_agent_request(&self, req_id: u64, message: &str) {
        let frame = agent_response_frame(req_id, not_ready(message));
        if self.writer_tx.try_send(frame.into()).is_err() {
            if !self
                .refusal_undeliverable_logged
                .swap(true, Ordering::Relaxed)
            {
                warn!(
                    req_id,
                    host = self.agent.as_ref().map(|agent| agent.origin.host),
                    "the writer queue was full and an agent refusal could not be enqueued; \
                     retiring the connection so the asking session is told something (logged \
                     once per connection)"
                );
            }
            self.retire();
        }
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
    ///
    /// That makes the ONLY terminal event a connection loss, which is why
    /// [`Self::dispatch`] treats a reply naming a never-issued `req_id` as
    /// fatal rather than dropping it: without a deadline of its own, a
    /// waiter whose answer arrived under an impossible id would otherwise
    /// sit here for the life of a connection that is behaving normally in
    /// every other respect.
    ///
    /// A connection that dies instead of answering becomes a
    /// [`SupervisorTransportError`], and WHICH variant depends on how far
    /// the request had got — the two failure sites before the enqueue say
    /// `NotSent`, the wait after it says `SentUnanswered`. That phase split
    /// is load-bearing rather than descriptive: an agent's lifecycle verb
    /// travels this leg, and the difference between "never left this
    /// process" and "the target may have done it and lost the answer" is
    /// the difference between a free retry and one that can stop a session
    /// somebody has since restarted. Both used to be one untyped string.
    async fn request(&self, req_id: u64, msg: ControlMsg) -> anyhow::Result<ControlMsg> {
        // Record `req_id` as ISSUED before anything can reach the wire.
        //
        // [`Self::dispatch`] rejects a reply naming an id past `next_req` as
        // an impossible correlation, which is only sound if this counter
        // covers every id that ever went out. `req_id()` is where production
        // callers get theirs, but this method ACCEPTS one — the id lives
        // inside `msg` too, so the caller has to mint it — and an id that
        // arrived any other way would otherwise be answered into a
        // connection kill. Maintaining the claim here, at the one place a
        // request is sent, makes it a property of the registry rather than
        // of caller discipline. A no-op on the ordinary path, since
        // `req_id()` has already moved the counter past its own id.
        //
        // Saturating rather than wrapping: an id of `u64::MAX` would have
        // nothing above it to reserve, and pinning the counter there costs
        // only that one unreachable id's answer.
        self.next_req
            .fetch_max(req_id.saturating_add(1), Ordering::Relaxed);
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
            .map_err(|_| anyhow::Error::new(SupervisorTransportError::NotSent))?;
        let (tx, rx) = oneshot::channel();
        {
            // Check-and-insert under one lock hold; see `Pending` for
            // why splitting them hangs requests.
            let mut pending = self.pending.lock().await;
            if pending.closed {
                return Err(anyhow::Error::new(SupervisorTransportError::NotSent));
            }
            pending.map.insert(req_id, tx);
        }
        permit.send(Frame::control(&msg).into());
        // Everything from here on is POST-ENQUEUE: the sender being dropped
        // means `fail_all` ran, or the whole connection went, with this
        // frame already in the writer's hands.
        let reply = rx
            .await
            .map_err(|_| anyhow::Error::new(SupervisorTransportError::SentUnanswered))?;
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

    /// What the supervisor said about itself at handshake time — in
    /// particular its host identity, which PLAN_M6.md item 4's connection
    /// manager runs every first-contact/mismatch/duplicate decision from.
    ///
    /// Immutable for this connection's lifetime by construction: a hello is
    /// exchanged once and never repeated, so a host whose identity changed
    /// is necessarily a DIFFERENT connection, which is precisely why
    /// identity handling belongs on the connect path rather than on a
    /// refresh.
    pub fn peer_hello(&self) -> &PeerHello {
        &self.peer
    }

    /// Resolves once this connection is dead — either half having failed,
    /// or both background tasks having exited — and stays resolved.
    ///
    /// The HTTP surface never needed this: a request against a dead
    /// connection fails on its own, immediately, which is all a one-shot
    /// handler wants. A connection ACTOR wants the opposite shape
    /// (PLAN_M6.md item 4). It has to notice loss while it is doing
    /// nothing — between refreshes, an idle host is idle for whole
    /// cadence periods — because the reconnect clock starts at the loss,
    /// not at the next thing that happened to fail. Deriving it from a
    /// failed request instead would delay every reconnection by up to one
    /// refresh interval and would make an idle host's outage invisible in
    /// the diagnostic trail until something asked it a question.
    ///
    /// Both a `true` value and the senders being dropped mean the same
    /// thing here, so both end the wait: the flag is set by whichever half
    /// died first (see `start_with_stall_timeouts`), and the senders can
    /// only disappear once both tasks have exited, which is itself the end
    /// of the connection.
    pub async fn closed(&self) {
        let mut rx = self.closed.clone();
        let _ = rx.wait_for(|done| *done).await;
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

    /// [`SupervisorClient::create_session`] carrying a fully resolved launch
    /// bundle as well as PLAN_M3.md item 6's idempotency key.
    ///
    /// Raw creates leave the profile snapshot absent and may let the
    /// supervisor infer the agent kind. Profile-backed creates use this same
    /// entry point after the helm has resolved the catalog row, because the
    /// supervisor deliberately has no catalog of its own.
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
                    parent: None,
                    profile_name: None,
                    cwd: cwd.to_string(),
                    invocation: Some(invocation.to_string()),
                    title,
                    cols,
                    rows,
                    intent_key: extras.intent_key,
                    agent_kind: extras.agent_kind,
                    resume_template: extras.resume_template,
                    source_profile: extras.source_profile,
                },
            )
            .await?
        {
            ControlMsg::SessionCreated { session, .. } => created_session(session),
            other => Err(wrong_reply("CreateSession", &other)),
        }
    }

    /// Every session this supervisor holds, in one reply, cut only at the
    /// wire's cap (`farhelm_proto::LIST_SESSIONS_CAP`, flagged by
    /// `truncated`). Always a live round trip: THIS call never consults a
    /// cache, and answers only what the supervisor said just now. That is a
    /// claim about this method, not about the helm — the helm keeps a
    /// durable last-known session cache per host, which is what serves the
    /// stale list for a host that is down. Supervisors remain the authority
    /// (SPEC.md); the cache is what "last we knew" means once there is
    /// nobody to ask.
    ///
    /// The one wire read behind both the manager's refresh drain
    /// (`crate::manager::drain_sessions`) and the per-session detail
    /// lookup: there is no page-walk primitive beside it, because the wire
    /// has no pages.
    pub async fn list_sessions(&self) -> anyhow::Result<SessionListing> {
        let req_id = self.req_id();
        match self
            .request(req_id, ControlMsg::ListSessions { req_id })
            .await?
        {
            ControlMsg::SessionList {
                sessions,
                truncated,
                ..
            } => Ok(SessionListing {
                sessions,
                truncated,
            }),
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
    ///
    /// A correlated reply of the WRONG variant is a third thing again, and
    /// comes back as [`SupervisorTransportError::SentWrongReply`] rather
    /// than as an untyped protocol complaint: the stop was sent, so an agent
    /// relaying this must be told its outcome is unknown. See
    /// [`wrong_reply`].
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
            other => Err(wrong_reply("StopSession", &other)),
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
    /// client. A correlated reply of the wrong VARIANT does not: it is
    /// [`SupervisorTransportError::SentWrongReply`], which keeps the fact
    /// that the rename was sent (see [`wrong_reply`]).
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
            other => Err(wrong_reply("RenameSession", &other)),
        }
    }

    /// Archive a session, returning its retained post-teardown metadata.
    ///
    /// Success means the agent, tabs, and terminal are gone and the durable
    /// row is marked archived; attachments deliberately remain available to
    /// a later restart. Repeating the request is successful and returns the
    /// same current state, which lets a caller recover from an ambiguous
    /// transport failure without guessing whether the first request landed.
    ///
    /// A correlated reply of the wrong variant is
    /// [`SupervisorTransportError::SentWrongReply`], not an untyped protocol
    /// error, for the reason [`wrong_reply`] gives — even though this verb
    /// is the one whose repeat is harmless, because the vocabulary is the
    /// same across all three lifecycle verbs by design.
    pub async fn archive_session(&self, id: &str) -> anyhow::Result<SessionInfo> {
        let req_id = self.req_id();
        match self
            .request(
                req_id,
                ControlMsg::ArchiveSession {
                    req_id,
                    session_id: id.to_string(),
                },
            )
            .await?
        {
            ControlMsg::SessionArchived { session, .. } => Ok(session),
            other => Err(wrong_reply("ArchiveSession", &other)),
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
        // Displacing, like every pre-M6 attach: this wrapper exists for
        // callers with no lease and no reconnect flow, and a refusal is
        // something only an automatic retry knows what to do with.
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
        self.attach_with_policy(session_id, cols, rows, terminal, lease, false)
            .await
    }

    /// Attach only if no OTHER client holds this session, refusing rather
    /// than displacing one that does (`ControlMsg::Attach::if_unowned`).
    ///
    /// The counterpart to [`Self::attach_terminal`], and a separate entry
    /// point rather than a flag on it because the difference is not a
    /// parameter a caller tunes — it is which of two DIFFERENT operations
    /// is being asked for, and the wrong one is a session taken from
    /// someone who is using it. A bare `false` at forty call sites says
    /// nothing at any of them; a name says which contract each one wants.
    ///
    /// Refused with [`farhelm_proto::ATTACH_REFUSED_TAKEN_OVER`] and
    /// `ErrorKind::Conflict`, and nothing is installed on a refusal — the
    /// channel this named stays unattached.
    pub async fn attach_terminal_if_unowned(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
        terminal: TerminalSelector,
        lease: &str,
    ) -> anyhow::Result<(u32, TermStream)> {
        self.attach_with_policy(session_id, cols, rows, terminal, lease, true)
            .await
    }

    /// The shared body of the two attach entry points above; `if_unowned`
    /// is the one thing they differ in and it goes straight onto the wire.
    async fn attach_with_policy(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
        terminal: TerminalSelector,
        lease: &str,
        if_unowned: bool,
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
                    if_unowned,
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
            parent: None,
            archived: false,
            id: id.into(),
            title: id.into(),
            created_at: 1_700_000_000,
            last_activity_at: 1_700_000_000,
            creation_seq: None,
            cwd: format!("/{id}"),
            invocation: "agent".into(),
            status: farhelm_proto::SessionStatus::Running,
            annotation: None,
            restart_offer: farhelm_proto::RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
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

    /// An `Error` carrying `req_id` 0 is tied to NO request
    /// (`ControlMsg::Error`'s own contract), and the demultiplexer's whole
    /// treatment of that case is one `.filter(|req_id| *req_id != 0)` —
    /// small enough to lose in a refactor, and invisible when lost. Without
    /// it the message is classified as a reply, looked up in a pending map
    /// that has no entry 0, and dropped in silence: the supervisor said
    /// something went wrong and nobody, not even the log, ever hears it.
    ///
    /// So the assertion is on the LOG, not just on the request. Both
    /// directions have to hold, and only together do they pin the guard:
    /// the unsolicited error must reach the unhandled-message arm and be
    /// warned about, and the real reply that follows it must still complete
    /// the waiting request. Ordering is deterministic rather than raced —
    /// one reader loop dispatches frames in the order the peer wrote them,
    /// so by the time the request resolves the error has already been
    /// handled one way or the other.
    ///
    /// The capture buffer is process-global (see `test_capture`), hence the
    /// sentinel: it is what distinguishes this test's event from every
    /// other test's concurrent noise.
    #[tokio::test]
    async fn an_unsolicited_error_is_logged_rather_than_routed_as_a_reply() {
        const SENTINEL: &str = "unsolicited-error-a41c7f";
        let events = crate::test_capture::install();

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
                panic!("expected a ListSessions, got {request:?}");
            };
            // The unsolicited error goes FIRST, so a demultiplexer that
            // mistook it for a reply would have consumed it before the real
            // one arrived — the order in which the bug does the most damage.
            writer
                .write_control(&ControlMsg::Error {
                    req_id: 0,
                    message: SENTINEL.to_string(),
                    kind: ErrorKind::Internal,
                })
                .await
                .unwrap();
            writer
                .write_control(&ControlMsg::SessionList {
                    req_id,
                    sessions: vec![session("s1")],
                    truncated: false,
                })
                .await
                .unwrap();
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let reply = client
            .request(101, ControlMsg::ListSessions { req_id: 101 })
            .await
            .expect("the real reply must still complete the request");
        assert!(
            matches!(
                &reply,
                ControlMsg::SessionList { req_id: 101, sessions, .. } if sessions[0].id == "s1"
            ),
            "the request must be completed by its own reply, not by the unsolicited error: \
             {reply:?}"
        );
        peer.await.unwrap();

        let warned = crate::test_capture::matching(&events, "unexpected control message at helm")
            .into_iter()
            .any(|event| {
                event
                    .field("other")
                    .is_some_and(|other| other.contains(SENTINEL))
            });
        assert!(
            warned,
            "an error tied to no request must fall through to the unhandled arm and be logged, \
             not be swallowed by the reply lookup"
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
                        truncated: false,
                    })
                    .await
                    .unwrap();
            }
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let first = client.request(101, ControlMsg::ListSessions { req_id: 101 });
        let second = client.request(202, ControlMsg::ListSessions { req_id: 202 });
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

    /// The wire's `truncated` reaches the caller as it was sent, in both
    /// directions. Nothing is synthesized from counts any more (there is
    /// no `total` to compare against), so this pins that a cut a
    /// supervisor reports is neither dropped nor invented on the way to
    /// the REST body that tells the user the list could not be read to the
    /// end.
    #[tokio::test]
    async fn list_sessions_passes_the_supervisors_truncated_flag_through() {
        for sent in [true, false] {
            let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
            let peer = tokio::spawn(async move {
                let (r, w) = tokio::io::split(peer_side);
                let mut reader = FrameReader::new(r);
                let mut writer = FrameWriter::new(w);
                handshake(&mut reader, &mut writer, "supervisor")
                    .await
                    .unwrap();
                let request = parse_control(&reader.read_frame().await.unwrap().unwrap()).unwrap();
                let ControlMsg::ListSessions { req_id } = request else {
                    panic!("unexpected request: {request:?}");
                };
                writer
                    .write_control(&ControlMsg::SessionList {
                        req_id,
                        sessions: vec![session("a")],
                        truncated: sent,
                    })
                    .await
                    .unwrap();
            });
            let (r, w) = tokio::io::split(client_side);
            let client = SupervisorClient::start(r, w).await.unwrap();

            let listing = client.list_sessions().await.unwrap();
            assert_eq!(listing.sessions.len(), 1);
            assert_eq!(
                listing.truncated, sent,
                "the flag must pass through unchanged"
            );
            peer.await.unwrap();
        }
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

    /// Spec: a supervisor that READS a request and then dies without
    /// answering fails it as [`SupervisorTransportError::SentUnanswered`].
    ///
    /// The phase this records is the whole reason the error is typed. The
    /// verb driven here is a `StopSession`, which is what an agent's
    /// `farhelm agent stop` becomes on this leg: the target supervisor may
    /// have killed the agent and lost only the reply, so the failure that
    /// comes back must not read as "nothing happened". Everything above
    /// this client used to see one untyped string for both phases, which
    /// `error_kind` classifies as `Internal` — a kind that says nothing
    /// about retrying at all. `agent_requests::transport_outcome` is what
    /// turns this variant into the agent's outcome-unknown ending.
    ///
    /// The peer READING the frame is what makes the phase provable: the
    /// enqueue happened, observably, before the connection went.
    #[tokio::test]
    async fn a_peer_that_reads_a_request_and_dies_reports_it_as_sent() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let frame = reader
                .read_frame()
                .await
                .unwrap()
                .expect("the helm sent nothing to read");
            parse_control(&frame).expect("decode the request")
            // Both halves drop here: the supervisor dies holding a request
            // it has already taken off the wire.
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let stop = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.stop_session("s1").await }
        });
        let read_by_the_peer = timeout(Duration::from_secs(5), peer)
            .await
            .expect("the peer never read the request")
            .expect("peer task");
        assert!(matches!(read_by_the_peer, ControlMsg::StopSession { .. }));

        let error = timeout(Duration::from_secs(5), stop)
            .await
            .expect("the request hung after its peer died")
            .expect("request task")
            .expect_err("a peer that never answers cannot succeed");
        assert!(
            matches!(
                error.downcast_ref::<SupervisorTransportError>(),
                Some(SupervisorTransportError::SentUnanswered)
            ),
            "a request the peer had already taken must be reported as sent: {error:#}"
        );
    }

    /// Spec: RETIRING a connection drains its pending requests exactly as a
    /// dead one does — [`SupervisorTransportError::SentUnanswered`] for a
    /// frame the writer queue had already taken — while a replacement
    /// connection to the same peer keeps working.
    ///
    /// Retirement is not a failure; it is the manager withdrawing a
    /// connection on a reconnect, a retarget, an adoption or a retired host
    /// row, and the peer on the other side is usually perfectly healthy.
    /// That is exactly what made the missing drain so quiet: the demux
    /// loop's cancellation arm used to RETURN, skipping the `fail_all` tail
    /// that every other ending of that loop goes through, so a request on
    /// the old connection sat on a `oneshot` nobody would ever complete
    /// while the fleet visibly recovered around it. For an agent's
    /// rename/stop/archive that is a supervisor-side delete fence held
    /// against the asking session for the life of the process — the far end
    /// of the chain this drain feeds, pinned in the supervisor's own crate
    /// by `agent_relay::tests::a_mutations_fence_outlives_the_answer_budget`
    /// (the `fail_all` half), since the fence is not observable from here.
    ///
    /// The second connection is what makes "stranded" the right word rather
    /// than "delayed": nothing about the retirement stops the host being
    /// served again immediately, which is why nothing else would ever have
    /// noticed the hung waiter.
    #[tokio::test]
    async fn retiring_a_connection_drains_a_request_the_queue_already_took() {
        /// One live client plus the peer's frame reader, with the hello
        /// already exchanged and the peer's writer parked in a task the
        /// caller can answer through.
        ///
        /// Returned rather than inlined twice because this test's whole
        /// point is the SECOND connection: a fixture written once cannot
        /// accidentally differ between the retired connection and its
        /// replacement.
        async fn connected() -> (
            Arc<SupervisorClient>,
            FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
            FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
        ) {
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
            let (reader, writer) = peer.await.expect("peer task");
            (client, reader, writer)
        }

        let (retired, mut retired_reader, _retired_writer) = connected().await;
        let stop = tokio::spawn({
            let client = Arc::clone(&retired);
            async move { client.stop_session("s1").await }
        });
        // Read by the peer, which is what makes the phase provable: the
        // frame left the queue before anything was retired.
        let frame = timeout(Duration::from_secs(5), retired_reader.read_frame())
            .await
            .expect("the peer never saw the request")
            .unwrap()
            .expect("the helm sent nothing to read");
        assert!(matches!(
            parse_control(&frame).expect("decode the request"),
            ControlMsg::StopSession { .. }
        ));

        // The manager withdrawing the connection. The peer is untouched and
        // still perfectly able to answer — it simply never will, because
        // nothing is left listening for it.
        retired.retire();
        let error = timeout(Duration::from_secs(5), stop)
            .await
            .expect("a retired connection left its request hanging")
            .expect("request task")
            .expect_err("a retired connection cannot answer");
        assert!(
            matches!(
                error.downcast_ref::<SupervisorTransportError>(),
                Some(SupervisorTransportError::SentUnanswered)
            ),
            "a retired connection's queued request must be reported as sent: {error:#}"
        );

        // And the replacement works, which is the state the fleet is really
        // in while a stranded waiter would still be waiting.
        let (replacement, mut reader, mut writer) = connected().await;
        let stop = tokio::spawn({
            let client = Arc::clone(&replacement);
            async move { client.stop_session("s1").await }
        });
        let frame = timeout(Duration::from_secs(5), reader.read_frame())
            .await
            .expect("the replacement peer never saw the request")
            .unwrap()
            .expect("the helm sent nothing to read");
        let ControlMsg::StopSession { req_id, .. } =
            parse_control(&frame).expect("decode the request")
        else {
            panic!("the replacement must send a StopSession");
        };
        writer
            .write_control(&ControlMsg::SessionStopped { req_id })
            .await
            .expect("answer the replacement");
        timeout(Duration::from_secs(5), stop)
            .await
            .expect("the replacement's request hung")
            .expect("request task")
            .expect("a healthy replacement connection answers");
    }

    /// Spec: a request made on a connection that is ALREADY dead fails as
    /// [`SupervisorTransportError::NotSent`].
    ///
    /// The other side of the phase split, and it needs its own test because
    /// nothing structural keeps the two apart: one `map_err` copied to the
    /// wrong side of the enqueue collapses them, and every message they
    /// produce still says the connection closed. Collapsed toward `NotSent`
    /// a lost mutation reply becomes "retry freely" — the exact lie the
    /// split exists to prevent; collapsed the other way, every request that
    /// never left the process starts telling its caller to go and inspect
    /// the fleet before trying again.
    #[tokio::test]
    async fn a_request_on_an_already_dead_connection_reports_that_nothing_was_sent() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            // And immediately goes away, before anything is asked of it.
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();
        peer.await.expect("peer task");
        // The connection is dead the moment the demux loop sees EOF, which
        // is a scheduling step away rather than instant; waiting for the
        // flag is what makes this test about the pre-enqueue path rather
        // than a race against it.
        timeout(Duration::from_secs(5), async {
            while !client.pending.lock().await.closed {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the client never noticed its peer was gone");

        let error = timeout(Duration::from_secs(5), client.stop_session("s1"))
            .await
            .expect("a request on a dead connection must fail fast")
            .expect_err("a dead connection cannot answer");
        assert!(
            matches!(
                error.downcast_ref::<SupervisorTransportError>(),
                Some(SupervisorTransportError::NotSent)
            ),
            "nothing was queued, so the failure must say so: {error:#}"
        );
    }

    /// Spec: a LIFECYCLE request answered with a correlated reply of the
    /// wrong variant keeps the fact that it was sent, as
    /// [`SupervisorTransportError::SentWrongReply`].
    ///
    /// The phase, not the protocol violation, is what this pins. A peer
    /// that answers a `StopSession` with a `SessionRenamed` is broken or
    /// hostile either way, but the request still went out and the target may
    /// still have stopped the agent before answering nonsense — so an
    /// untyped "unexpected reply" error, which `error_kind` reads as
    /// `Internal`, tells an agent nothing about the one question it has.
    /// `agent_requests::transport_outcome` turns this variant into the same
    /// outcome-unknown ending a dead connection gets, pinned there beside
    /// the other phases.
    ///
    /// `stop` is the verb because it is the one whose repeat is destructive
    /// in the plainest way; the three lifecycle wrappers share the shape.
    #[tokio::test]
    async fn a_lifecycle_reply_of_the_wrong_variant_is_reported_as_sent() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let frame = reader
                .read_frame()
                .await
                .unwrap()
                .expect("the helm sent nothing to read");
            let ControlMsg::StopSession { req_id, .. } =
                parse_control(&frame).expect("decode the request")
            else {
                panic!("expected a StopSession");
            };
            // Correlated, well formed, and the answer to a question nobody
            // asked.
            writer
                .write_control(&ControlMsg::SessionRenamed {
                    req_id,
                    session: session("s1"),
                })
                .await
                .expect("answer with the wrong variant");
            // Held so the connection stays alive: the point is that this
            // failure is NOT a connection loss.
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let error = timeout(Duration::from_secs(5), client.stop_session("s1"))
            .await
            .expect("the request hung on a wrong reply")
            .expect_err("a wrong reply cannot succeed");
        assert!(
            matches!(
                error.downcast_ref::<SupervisorTransportError>(),
                Some(SupervisorTransportError::SentWrongReply { request, .. })
                    if *request == "StopSession"
            ),
            "a correlated wrong reply must keep the request's own phase: {error:#}"
        );
        let _peer = peer.await.expect("peer task");
    }

    /// Spec: a wrong-variant reply is reported by VARIANT NAME, so a reply
    /// that legally fills most of a frame still produces a small error.
    ///
    /// The small `SessionRenamed` fixture above cannot see this, and the
    /// case it misses is the one that broke the vocabulary the sibling test
    /// pins. `SentWrongReply` used to carry the whole `{reply:?}` rendering,
    /// which for a near-limit `SessionList` is megabytes; that string
    /// travels into `agent_requests::transport_outcome`'s message, which is
    /// re-encoded into the agent's own `AgentResponse` frame, which then
    /// tripped `agent_response_frame`'s size backstop and was replaced
    /// wholesale — so the `Timeout`-plus-remedy answer a mutation is
    /// supposed to get arrived as a bare `Internal`. Size is not the only
    /// harm: the same rendering carries a `SessionInfo`'s raw invocation
    /// and cwd into an agent-facing error chain, which the agent surfaces
    /// deliberately redact.
    ///
    /// The reply is built near the frame limit rather than merely "big"
    /// because that is the legal shape a real supervisor can produce — a
    /// listing answering a `StopSession` — and because a bound that holds
    /// for a kilobyte and not for a megabyte is not a bound.
    #[tokio::test]
    async fn a_near_frame_limit_wrong_reply_still_reports_a_small_error() {
        // ~7 MiB of encoded sessions: comfortably under `MAX_FRAME_LEN` (so
        // the peer can actually send it) and comfortably over it once
        // Debug-rendered and re-encoded inside another frame, which is what
        // the old shape did.
        let sessions: Vec<SessionInfo> = (0..116)
            .map(|n| {
                let mut info = session(&format!("s{n}"));
                info.title = "t".repeat(60_000);
                info.invocation = "claude --dangerously-skip-permissions".to_string();
                info.cwd = "/home/someone/secret-project".to_string();
                info
            })
            .collect();
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let frame = reader
                .read_frame()
                .await
                .unwrap()
                .expect("the helm sent nothing to read");
            let ControlMsg::StopSession { req_id, .. } =
                parse_control(&frame).expect("decode the request")
            else {
                panic!("expected a StopSession");
            };
            writer
                .write_control(&ControlMsg::SessionList {
                    req_id,
                    sessions,
                    truncated: false,
                })
                .await
                .expect("a near-limit listing is a legal frame");
            // Held so the connection stays alive; this failure is not a
            // connection loss.
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let error = timeout(Duration::from_secs(30), client.stop_session("s1"))
            .await
            .expect("the request hung on a wrong reply")
            .expect_err("a wrong reply cannot succeed");
        assert!(
            matches!(
                error.downcast_ref::<SupervisorTransportError>(),
                Some(SupervisorTransportError::SentWrongReply { request, reply })
                    if *request == "StopSession" && *reply == "SessionList"
            ),
            "the wrong reply must be named by variant: {error:#}"
        );
        let rendered = format!("{error:#}");
        assert!(
            rendered.len() < 1024,
            "a megabyte reply must not produce a megabyte error ({} bytes)",
            rendered.len()
        );
        assert!(
            !rendered.contains("secret-project") && !rendered.contains("skip-permissions"),
            "the error must not carry fields the agent surfaces redact: {rendered}"
        );
        let _peer = peer.await.expect("peer task");
    }

    /// Spec: a CREATE answered with a correlated reply of the wrong variant
    /// keeps the fact that it was sent, as
    /// [`SupervisorTransportError::SentWrongReply`] naming `CreateSession`.
    ///
    /// The lifecycle sibling above pins the same shape, and this one is not
    /// redundant with it: a create is the mutation whose lost outcome is
    /// least recoverable. A stop that may or may not have landed can be
    /// settled by looking at the session; a create that may or may not have
    /// landed leaves a session running on some host under an id the asking
    /// agent was never told, and the only kind that says so is the
    /// outcome-unknown one `agent_requests::transport_outcome` derives from
    /// this variant. An untyped "unexpected reply" here reads as `Internal`
    /// and would invite the retry that starts the second session.
    ///
    /// All three create wrappers send `ControlMsg::CreateSession` and share
    /// this arm, so the raw-invocation one stands in for the profile ones.
    #[tokio::test]
    async fn a_create_reply_of_the_wrong_variant_is_reported_as_sent() {
        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            let frame = reader
                .read_frame()
                .await
                .unwrap()
                .expect("the helm sent nothing to read");
            let ControlMsg::CreateSession { req_id, .. } =
                parse_control(&frame).expect("decode the request")
            else {
                panic!("expected a CreateSession");
            };
            // Correlated and well formed, and it says nothing about whether
            // a session was started.
            writer
                .write_control(&ControlMsg::SessionRenamed {
                    req_id,
                    session: session("s1"),
                })
                .await
                .expect("answer with the wrong variant");
            // Held so the connection stays alive: this failure is not a
            // connection loss.
            (reader, writer)
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        let error = timeout(
            Duration::from_secs(5),
            client.create_session("/tmp", "claude", None, 80, 24),
        )
        .await
        .expect("the request hung on a wrong reply")
        .expect_err("a wrong reply cannot succeed");
        assert!(
            matches!(
                error.downcast_ref::<SupervisorTransportError>(),
                Some(SupervisorTransportError::SentWrongReply { request, reply })
                    if *request == "CreateSession" && *reply == "SessionRenamed"
            ),
            "a correlated wrong reply to a create must keep the request's own phase: {error:#}"
        );
        let _peer = peer.await.expect("peer task");
    }

    /// Spec: a reply under a `req_id` this connection never issued ends the
    /// connection, while one under an id it HAS issued is dropped in
    /// silence.
    ///
    /// [`SupervisorClient::request`] has no deadline on purpose, so a
    /// connection loss is its only terminal event. That makes a dropped
    /// correlation unsurvivable in one specific case: a peer that answers
    /// under an id nobody is waiting for, and never under the real one,
    /// leaves the waiter parked forever on a transport that stays healthy —
    /// and with it the connection's agent-answer permit and, one hop out,
    /// the asking session's delete fence. An id past this connection's own
    /// counter cannot be a late answer, so it is treated as the protocol
    /// violation it is.
    ///
    /// The benign half is asserted first and is not a formality: retiring
    /// on EVERY unrecognized id would tear the connection down on ordinary
    /// traffic, since a request whose caller went away leaves exactly such
    /// an id behind. Its evidence is that the second request still reaches
    /// the peer at all — a connection retired by the duplicate would have
    /// failed it as `NotSent` instead.
    #[tokio::test]
    async fn a_reply_under_a_never_issued_req_id_ends_the_connection() {
        /// The next `StopSession`'s `req_id`, since this fixture reads two.
        async fn next_stop<R: tokio::io::AsyncRead + Unpin>(reader: &mut FrameReader<R>) -> u64 {
            let frame = reader
                .read_frame()
                .await
                .unwrap()
                .expect("the helm sent nothing to read");
            match parse_control(&frame).expect("decode the request") {
                ControlMsg::StopSession { req_id, .. } => req_id,
                other => panic!("expected a StopSession, got {other:?}"),
            }
        }

        let (client_side, peer_side) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();

            let first = next_stop(&mut reader).await;
            writer
                .write_control(&ControlMsg::SessionStopped { req_id: first })
                .await
                .expect("answer the first request");
            // The same id a second time: issued, no longer pending, and
            // indistinguishable from a late answer.
            writer
                .write_control(&ControlMsg::SessionStopped { req_id: first })
                .await
                .expect("answer it again");

            let second = next_stop(&mut reader).await;
            // Correlated to nothing this connection ever sent.
            writer
                .write_control(&ControlMsg::SessionStopped {
                    req_id: second + 1_000,
                })
                .await
                .expect("answer under an impossible id");
            writer
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start(r, w).await.unwrap();

        timeout(Duration::from_secs(5), client.stop_session("s1"))
            .await
            .expect("the first request hung")
            .expect("a healthy peer answers");

        let error = timeout(Duration::from_secs(5), client.stop_session("s2"))
            .await
            .expect("the request hung on an impossible correlation")
            .expect_err("an answer nobody can be waiting for is not an answer");
        assert!(
            matches!(
                error.downcast_ref::<SupervisorTransportError>(),
                Some(SupervisorTransportError::SentUnanswered)
            ),
            "the request was queued before the violation, and a `NotSent` here would also mean \
             the harmless duplicate had wrongly killed the connection: {error:#}"
        );
        timeout(Duration::from_secs(5), client.closed())
            .await
            .expect("an impossible correlation must end the connection");
        let _peer = peer.await.expect("peer task");
    }

    /// Spec: the oversized-answer backstop keeps an outcome-unknown verdict
    /// outcome-unknown; only its prose is dropped.
    ///
    /// The backstop exists to stop one huge reply costing the whole
    /// connection, and the obvious implementation — replace anything that
    /// does not fit with a small `Internal` — quietly changes what the
    /// answer CLAIMS. `Timeout` on this path is the relay's
    /// "delivered, outcome unknown" verdict on a rename/stop/archive, built
    /// out of an error chain whose length this side does not control.
    /// Rewriting it to `Internal` tells the asking agent "this should not
    /// happen" about a mutation that may have taken effect, which is the
    /// exact substitution the mutation vocabulary exists to prevent —
    /// arrived at through a size check rather than through any judgement
    /// about the request.
    ///
    /// Both directions, because the fix is only correct if it is narrow: an
    /// oversized answer that was NOT outcome-unknown must still collapse to
    /// `Internal`, which is the honest reading when nothing durable is at
    /// stake and no retry changes anything.
    #[test]
    fn the_oversized_answer_backstop_preserves_an_unknown_outcome() {
        let huge = "x".repeat(farhelm_proto::MAX_FRAME_LEN as usize + 1);
        let decode = |frame: Frame| match parse_control(&frame).expect("decode the response") {
            ControlMsg::AgentResponse { outcome, .. } => outcome,
            other => panic!("expected an AgentResponse, got {other:?}"),
        };

        let replaced = agent_response_frame(
            7,
            farhelm_proto::AgentOutcome::Err {
                kind: farhelm_proto::ErrorKind::Timeout,
                message: huge.clone(),
            },
        );
        assert!(
            !replaced.exceeds_max_len(),
            "the replacement must be the thing that fits"
        );
        match decode(replaced) {
            farhelm_proto::AgentOutcome::Err { kind, message } => {
                assert_eq!(
                    kind,
                    farhelm_proto::ErrorKind::Timeout,
                    "a delivered-outcome-unknown verdict must survive its own size"
                );
                assert!(
                    message.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
                    "and must keep telling the agent what to do before retrying: {message}"
                );
            }
            other => panic!("the backstop cannot turn a refusal into a success: {other:?}"),
        }

        let collapsed = agent_response_frame(
            8,
            farhelm_proto::AgentOutcome::Err {
                kind: farhelm_proto::ErrorKind::Internal,
                message: huge,
            },
        );
        match decode(collapsed) {
            farhelm_proto::AgentOutcome::Err { kind, message } => {
                assert_eq!(
                    kind,
                    farhelm_proto::ErrorKind::Internal,
                    "nothing durable was at stake, so the honest kind is unchanged"
                );
                assert!(
                    !message.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
                    "and the mutation remedy must not be spread to a class that cannot need it: \
                     {message}"
                );
            }
            other => panic!("the backstop cannot turn a refusal into a success: {other:?}"),
        }
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

    // ---------------------------------------------------------------
    // Agent upcalls: the one request that travels UP this connection.
    // ---------------------------------------------------------------

    /// A helm-side handler the test drives step by step: it announces every
    /// call on `entered` and then waits for a permit before answering.
    ///
    /// Both halves are needed to test the demultiplexer's behavior at all.
    /// The announcement is what lets a test know an upcall is genuinely
    /// IN the handler rather than still in flight, and the gate is what
    /// keeps it there while the test does something else on the same
    /// connection.
    struct GatedHandler {
        entered: mpsc::Sender<()>,
        gate: Arc<Semaphore>,
        reply: farhelm_proto::AgentReply,
    }

    impl GatedHandler {
        /// A handler parked until the test releases it, one call per
        /// [`Self::release`].
        fn parked() -> (Arc<GatedHandler>, mpsc::Receiver<()>, Arc<Semaphore>) {
            let (entered, calls) = mpsc::channel(8);
            let gate = Arc::new(Semaphore::new(0));
            (
                Arc::new(GatedHandler {
                    entered,
                    gate: Arc::clone(&gate),
                    reply: farhelm_proto::AgentReply::Hosts { hosts: Vec::new() },
                }),
                calls,
                gate,
            )
        }

        /// A handler that answers immediately with `reply`.
        fn answering(reply: farhelm_proto::AgentReply) -> Arc<GatedHandler> {
            let (entered, _calls) = mpsc::channel(8);
            Arc::new(GatedHandler {
                entered,
                gate: Arc::new(Semaphore::new(Semaphore::MAX_PERMITS)),
                reply,
            })
        }
    }

    #[async_trait::async_trait]
    impl crate::agent_requests::AgentRequestHandler for GatedHandler {
        async fn handle(
            &self,
            _origin: crate::agent_requests::AgentOrigin,
            _session_id: &str,
            _verb: farhelm_proto::AgentVerb,
        ) -> farhelm_proto::AgentOutcome {
            let _ = self.entered.send(()).await;
            let _permit = self.gate.acquire().await.expect("the gate is never closed");
            farhelm_proto::AgentOutcome::Ok {
                reply: self.reply.clone(),
            }
        }
    }

    /// The supervisor's end of a connection, with the hello already
    /// exchanged — frames written and read by hand, which is what these
    /// tests need: the shapes involved travel UP the connection, so there
    /// is no client method that sends them.
    struct AgentPeer {
        reader: FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
        writer: FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
    }

    impl AgentPeer {
        async fn ask(&mut self, req_id: u64, verb: farhelm_proto::AgentVerb) {
            self.writer
                .write_control(&ControlMsg::AgentRequest {
                    req_id,
                    session_id: "s1".to_string(),
                    request: verb,
                })
                .await
                .expect("send the agent request");
        }

        /// The next control message, with a deadline so a regression that
        /// answers nothing fails instead of hanging the suite.
        async fn next_control(&mut self) -> ControlMsg {
            let frame = timeout(Duration::from_secs(5), self.reader.read_frame())
                .await
                .expect("the helm sent nothing")
                .expect("read a frame")
                .expect("the helm closed instead of answering");
            parse_control(&frame).expect("decode the helm's frame")
        }

        /// The outcome of the next message, insisting it is the response to
        /// `req_id`.
        async fn outcome(&mut self, req_id: u64) -> farhelm_proto::AgentOutcome {
            match self.next_control().await {
                ControlMsg::AgentResponse {
                    req_id: got,
                    outcome,
                } if got == req_id => outcome,
                other => panic!("expected an AgentResponse for {req_id}, got {other:?}"),
            }
        }
    }

    /// A connection wired for agent upcalls against `slot`, with the peer
    /// handed back so the test can play the supervisor.
    ///
    /// The peer's handshake runs in its own task because the client's
    /// constructor performs the other half of the same exchange and neither
    /// completes without the other.
    async fn agent_connection(
        slot: crate::agent_requests::AgentRequestSlot,
    ) -> (Arc<SupervisorClient>, AgentPeer) {
        agent_connection_buffered(slot, 1 << 20).await
    }

    /// [`agent_connection`] with an explicit transport buffer.
    ///
    /// A SMALL buffer is a fixture in its own right: it makes the writer
    /// task block partway through one frame as soon as the peer stops
    /// reading, which is the only way to hold answers in the writer's hands
    /// and in its queue long enough to observe what an admission slot
    /// covers. The generous default is what every other test wants, since
    /// nothing else here is about backpressure.
    ///
    /// It must still comfortably exceed a HELLO, in both directions: the
    /// two sides of the handshake each write before either reads, so a
    /// buffer too small to hold one hello deadlocks the connection before
    /// the test starts.
    async fn agent_connection_buffered(
        slot: crate::agent_requests::AgentRequestSlot,
        buffer: usize,
    ) -> (Arc<SupervisorClient>, AgentPeer) {
        let (client_side, peer_side) = tokio::io::duplex(buffer);
        let peer = tokio::spawn(async move {
            let (r, w) = tokio::io::split(peer_side);
            let mut reader = FrameReader::new(r);
            let mut writer = FrameWriter::new(w);
            handshake(&mut reader, &mut writer, "supervisor")
                .await
                .expect("peer handshake");
            AgentPeer { reader, writer }
        });
        let (r, w) = tokio::io::split(client_side);
        let client = SupervisorClient::start_for_host(r, w, slot, 1)
            .await
            .expect("client handshake");
        (client, peer.await.expect("peer task"))
    }

    /// A connection that reaches "live" before the helm's `AppState` exists
    /// must refuse with "still starting up" and then start answering once
    /// the SAME slot is filled — without reconnecting.
    ///
    /// This is the whole reason the handler is a shared `OnceLock` read per
    /// request rather than a value captured at connection time. Production
    /// starts the connection manager before `AppState`, so a connection CAN
    /// become live while the slot is empty; an implementation that captured
    /// the empty state once would answer "not ready" for the rest of that
    /// connection's life, and every existing test that fills the slot first
    /// would still pass.
    #[tokio::test]
    async fn an_empty_handler_slot_refuses_and_then_answers_once_filled() {
        let slot: crate::agent_requests::AgentRequestSlot = Arc::new(std::sync::OnceLock::new());
        let (_client, mut peer) = agent_connection(Arc::clone(&slot)).await;

        peer.ask(1, farhelm_proto::AgentVerb::Hosts {}).await;
        match peer.outcome(1).await {
            farhelm_proto::AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, farhelm_proto::ErrorKind::Unavailable);
                assert!(
                    message.contains("starting up"),
                    "the refusal must name the transient cause, got: {message}"
                );
            }
            other => panic!("an empty slot must refuse, got {other:?}"),
        }

        // The SAME slot, on the SAME connection: no reconnect happens here.
        slot.set(GatedHandler::answering(farhelm_proto::AgentReply::Hosts {
            hosts: vec![farhelm_proto::AgentHost {
                name: "this machine".to_string(),
                kind: "local".to_string(),
                state: "connected".to_string(),
                current: true,
            }],
        }))
        .ok()
        .expect("a fresh slot is empty");

        peer.ask(2, farhelm_proto::AgentVerb::Hosts {}).await;
        match peer.outcome(2).await {
            farhelm_proto::AgentOutcome::Ok {
                reply: farhelm_proto::AgentReply::Hosts { hosts },
            } => assert_eq!(hosts.len(), 1),
            other => panic!("a filled slot must answer, got {other:?}"),
        }
    }

    /// A handler that records whether it was ever entered and answers
    /// immediately.
    ///
    /// The flag is the whole fixture: the cancellation-boundary tests are
    /// about work that must NOT begin, and "the handler was never called" is
    /// the only observation that distinguishes an aborted task from one that
    /// ran and had its answer discarded.
    struct RecordingHandler {
        entered: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl crate::agent_requests::AgentRequestHandler for RecordingHandler {
        async fn handle(
            &self,
            _origin: crate::agent_requests::AgentOrigin,
            _session_id: &str,
            _verb: farhelm_proto::AgentVerb,
        ) -> farhelm_proto::AgentOutcome {
            self.entered.store(true, Ordering::SeqCst);
            farhelm_proto::AgentOutcome::Ok {
                reply: farhelm_proto::AgentReply::Stopped {},
            }
        }
    }

    /// Spec: an answer whose connection is retired between spawning its work
    /// task and registering it never runs the handler at all.
    ///
    /// The interval is real and used to be unowned. A spawned task starts
    /// when the runtime says so, so on a multithreaded runtime the work
    /// could begin — and `abort_agent_tasks` could drain an empty list and
    /// return — before the abort handle was stored. The handle then landed
    /// in a torn-down connection's list, and the escaped task kept walking
    /// the database for a peer nobody would accept an answer from; worse,
    /// for a mutation whose entry check had already passed, it kept routing
    /// a `stop` to its target on the authority of a connection the manager
    /// had withdrawn. Every docstring in this file claims retirement OWNS
    /// that work, and this is what makes the claim structural rather than a
    /// statement about scheduling luck.
    ///
    /// The seam fires precisely between the spawn and the registration,
    /// which is the only place the race was ever reachable from; a refactor
    /// that reorders those two must move the seam with it or this test stops
    /// pinning anything. `Stop` is the verb because a mutation escaping the
    /// boundary is the consequential half — a listing merely wastes work.
    #[tokio::test]
    async fn an_answer_spawned_into_a_retirement_never_runs() {
        let entered = Arc::new(AtomicBool::new(false));
        let slot: crate::agent_requests::AgentRequestSlot =
            Arc::new(std::sync::OnceLock::from(Arc::new(RecordingHandler {
                entered: Arc::clone(&entered),
            })
                as Arc<dyn crate::agent_requests::AgentRequestHandler>));
        let (client, mut peer) = agent_connection(slot).await;

        // A `Weak`, so the seam cannot keep the client alive past the test.
        let retiring = Arc::downgrade(&client);
        *client
            .agent_spawn_seam
            .lock()
            .expect("agent spawn seam poisoned") = Some(Box::new(move || {
            if let Some(client) = retiring.upgrade() {
                client.retire();
            }
        }));

        peer.ask(1, farhelm_proto::AgentVerb::Stop { session_id: None })
            .await;
        timeout(Duration::from_secs(5), client.closed())
            .await
            .expect("the seam's retirement never took effect");

        assert!(
            !entered.load(Ordering::SeqCst),
            "the handler ran for a connection that had already been retired"
        );
        assert!(
            client
                .agent_tasks
                .lock()
                .expect("agent task list poisoned")
                .handles
                .is_empty(),
            "a retired connection must register nothing after its drain"
        );
        // Silence is the contract for a cancelled answer (see
        // `spawn_agent_answer`), so the peer sees the connection end rather
        // than an `AgentResponse`.
        let ending = timeout(Duration::from_secs(5), peer.reader.read_frame())
            .await
            .expect("the connection neither answered nor closed")
            .expect("read the connection's ending");
        assert!(
            ending.is_none(),
            "a cancelled answer must send nothing: {:?}",
            ending.map(|frame| parse_control(&frame))
        );
    }

    /// A handler whose ENTRY check would pass but whose EXIT check always
    /// says the connection is no longer current — the exact shape
    /// `spawn_agent_answer`'s second `origin_is_live` call exists to catch,
    /// isolated from any real retarget/adoption race so it can be produced
    /// on demand.
    struct StaleExitHandler {
        reply: farhelm_proto::AgentReply,
    }

    #[async_trait::async_trait]
    impl crate::agent_requests::AgentRequestHandler for StaleExitHandler {
        async fn handle(
            &self,
            _origin: crate::agent_requests::AgentOrigin,
            _session_id: &str,
            _verb: farhelm_proto::AgentVerb,
        ) -> farhelm_proto::AgentOutcome {
            farhelm_proto::AgentOutcome::Ok {
                reply: self.reply.clone(),
            }
        }

        fn origin_is_live(&self, _origin: crate::agent_requests::AgentOrigin) -> bool {
            false
        }
    }

    /// Spec: a completed READ-ONLY listing whose origin has gone stale by
    /// the time the answer is ready is downgraded to `Unavailable` — the
    /// exit re-check `spawn_agent_answer`'s docs describe, still armed for
    /// the two verbs it protects.
    ///
    /// This is the control for
    /// [`a_stale_origin_does_not_downgrade_a_completed_mutation`]: without
    /// it, a mutating verb skipping the check would be indistinguishable
    /// from BOTH verb classes never being checked at all.
    #[tokio::test]
    async fn a_stale_origin_downgrades_a_completed_listing_to_unavailable() {
        let handler = Arc::new(StaleExitHandler {
            reply: farhelm_proto::AgentReply::Hosts { hosts: Vec::new() },
        });
        let slot: crate::agent_requests::AgentRequestSlot = Arc::new(std::sync::OnceLock::from(
            handler as Arc<dyn crate::agent_requests::AgentRequestHandler>,
        ));
        let (_client, mut peer) = agent_connection(slot).await;

        peer.ask(1, farhelm_proto::AgentVerb::Hosts {}).await;
        match peer.outcome(1).await {
            farhelm_proto::AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, farhelm_proto::ErrorKind::Unavailable);
                assert!(
                    message.contains("replaced"),
                    "the refusal must name the cause, got: {message}"
                );
            }
            other => panic!("a stale-origin listing must be downgraded, got {other:?}"),
        }
    }

    /// Spec: a completed MUTATING verb is reported EXACTLY as the handler
    /// returned it even when the origin has gone stale by the time the
    /// answer is ready.
    ///
    /// This is the regression test for the bug fixed alongside it: before
    /// the fix, `spawn_agent_answer` downgraded ANY `Ok` outcome whose
    /// origin had gone stale to `Unavailable` — a kind callers read as
    /// "never happened, safe to retry". A completed stop, rename or archive
    /// could therefore be reported as though it had not happened, inviting
    /// a retry that re-applies an action already taken.
    ///
    /// ALL THREE verbs are driven rather than one standing in for the
    /// others, and the reason is the shape of the thing being protected:
    /// what decides the skip is a classification list
    /// ([`farhelm_proto::AgentVerb::is_mutating`]) rather than anything
    /// structural about the outcome. A verb dropped from that list would
    /// fail no test that only exercised its neighbors, which is exactly the
    /// failure mode centralizing the list was meant to end — so the test
    /// enumerates the set too. `Stop`, the two `Session`-shaped replies and
    /// the two `Created`-shaped ones are all represented, since they take
    /// different arms of the reply match on the way out.
    ///
    /// The CREATING verbs are the costliest case to get wrong and the
    /// newest, which is why they are here rather than assumed to follow.
    /// Downgrading a completed create to `Unavailable` tells the caller no
    /// session was made while one is already running on some host, and the
    /// id it would have needed to find it was in the answer that was
    /// thrown away.
    #[tokio::test]
    async fn a_stale_origin_does_not_downgrade_a_completed_mutation() {
        let renamed = |id: &str| farhelm_proto::AgentSession {
            id: id.to_string(),
            host: Some("this machine".to_string()),
            title: "t".to_string(),
            cwd: "/w".to_string(),
            agent: "claude".to_string(),
            status: "running".to_string(),
            current: false,
            archived: false,
            stale: false,
        };
        let cases = [
            (
                farhelm_proto::AgentVerb::Stop { session_id: None },
                farhelm_proto::AgentReply::Stopped {},
            ),
            (
                farhelm_proto::AgentVerb::Rename {
                    session_id: None,
                    title: "t".to_string(),
                },
                farhelm_proto::AgentReply::Session {
                    session: renamed("s1"),
                },
            ),
            (
                farhelm_proto::AgentVerb::Archive { session_id: None },
                farhelm_proto::AgentReply::Session {
                    session: renamed("s1"),
                },
            ),
            (
                farhelm_proto::AgentVerb::Create {
                    host: None,
                    cwd: "/w".to_string(),
                    profile_name: None,
                    invocation: Some("sh".to_string()),
                    title: None,
                    intent_key: None,
                },
                farhelm_proto::AgentReply::Created {
                    session: renamed("created-1"),
                },
            ),
            (
                farhelm_proto::AgentVerb::Clone {
                    host: None,
                    cwd: None,
                    title: None,
                    intent_key: None,
                },
                farhelm_proto::AgentReply::Created {
                    session: renamed("created-2"),
                },
            ),
        ];

        for (verb, reply) in cases {
            let handler = Arc::new(StaleExitHandler {
                reply: reply.clone(),
            });
            let slot: crate::agent_requests::AgentRequestSlot =
                Arc::new(std::sync::OnceLock::from(
                    handler as Arc<dyn crate::agent_requests::AgentRequestHandler>,
                ));
            let (_client, mut peer) = agent_connection(slot).await;

            peer.ask(1, verb.clone()).await;
            match peer.outcome(1).await {
                farhelm_proto::AgentOutcome::Ok { reply: got } => assert_eq!(
                    got, reply,
                    "{verb:?} must be reported exactly as the handler answered it"
                ),
                other => panic!(
                    "a completed {verb:?} must be reported truthfully even with a stale origin, \
                     got {other:?}"
                ),
            }
        }
    }

    /// A handler that panics on every call, which is the one ending an
    /// answer task can reach that produces no outcome of its own.
    struct PanickingHandler;

    #[async_trait::async_trait]
    impl crate::agent_requests::AgentRequestHandler for PanickingHandler {
        async fn handle(
            &self,
            _origin: crate::agent_requests::AgentOrigin,
            _session_id: &str,
            _verb: farhelm_proto::AgentVerb,
        ) -> farhelm_proto::AgentOutcome {
            panic!("the agent handler blew up");
        }
    }

    /// Spec: a panicking handler still produces an `AgentResponse` — the
    /// outcome-unknown ending for a mutation, a retry-safe refusal for a
    /// listing — while the connection stays alive and its admission slots
    /// come back.
    ///
    /// This is the ending nothing else in the relay can see, and the reason
    /// the answer task is supervised rather than merely spawned. A panic
    /// leaves the connection HEALTHY, so no teardown runs and the
    /// supervisor's `fail_all` never fires; its pending entry waits out the
    /// answer budget in a live link's map, and for a mutation the delete
    /// fence that budget retains is then held against the asking session
    /// until the connection eventually dies — blocking every later mutation
    /// from that agent and any delete of it, on a link that looks perfectly
    /// well. A response arriving is exactly what releases that fence, which
    /// is why the assertion is "something came back", asserted from the
    /// supervisor's own side of the wire.
    ///
    /// More requests are driven than there are admission slots, and that is
    /// the second half of the claim: the permit rides in the supervising
    /// task, so a dead work task cannot take a slot with it. Were the slot
    /// leaked, the run past [`AGENT_ANSWER_SLOTS`] would come back as "too
    /// many agent requests" instead of as the fallback — a connection
    /// permanently unable to answer any agent at all after four panics.
    #[tokio::test]
    async fn a_panicking_handler_still_answers_and_leaves_the_connection_alive() {
        let slot: crate::agent_requests::AgentRequestSlot = Arc::new(std::sync::OnceLock::from(
            Arc::new(PanickingHandler) as Arc<dyn crate::agent_requests::AgentRequestHandler>,
        ));
        let (client, mut peer) = agent_connection(slot).await;

        for req_id in 1..=(AGENT_ANSWER_SLOTS as u64 + 1) {
            peer.ask(req_id, farhelm_proto::AgentVerb::Stop { session_id: None })
                .await;
            match peer.outcome(req_id).await {
                farhelm_proto::AgentOutcome::Err { kind, message } => {
                    assert_eq!(
                        kind,
                        farhelm_proto::ErrorKind::Timeout,
                        "a mutation whose handler died may already have taken effect: {message}"
                    );
                    assert!(
                        message.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
                        "and must say what to do about it: {message}"
                    );
                }
                other => panic!("a dead answer task must still answer, got {other:?}"),
            }
        }

        // A listing has changed nothing, so it gets the retry-safe refusal
        // rather than the mutation vocabulary.
        let listing_id = AGENT_ANSWER_SLOTS as u64 + 2;
        peer.ask(listing_id, farhelm_proto::AgentVerb::Hosts {})
            .await;
        match peer.outcome(listing_id).await {
            farhelm_proto::AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, farhelm_proto::ErrorKind::Unavailable);
                assert!(
                    !message.contains(farhelm_proto::AGENT_MUTATION_UNKNOWN_REMEDY),
                    "a listing has nothing to check before retrying: {message}"
                );
            }
            other => panic!("a dead answer task must still answer, got {other:?}"),
        }

        // The connection is the other thing being claimed: a panicking
        // handler must not take the host's terminals and requests with it.
        let sessions = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.list_sessions().await }
        });
        let req_id = match peer.next_control().await {
            ControlMsg::ListSessions { req_id, .. } => req_id,
            other => panic!("the connection died with the panicking handler: {other:?}"),
        };
        peer.writer
            .write_control(&ControlMsg::SessionList {
                req_id,
                sessions: Vec::new(),
                truncated: false,
            })
            .await
            .expect("answer the ordinary request");
        timeout(Duration::from_secs(5), sessions)
            .await
            .expect("the connection stopped serving after a handler panic")
            .expect("listing task")
            .expect("listing");
    }

    /// A slow agent handler must not sit on the demultiplexer: ordinary
    /// traffic on the same connection has to keep flowing while one is
    /// parked.
    ///
    /// The property is why `spawn_agent_answer` spawns at all, and no other
    /// test establishes it. The relay's own timeout test looks identical
    /// from the outside whether the handler is awaited inline or not — it
    /// waits out a supervisor-side budget either way — so an implementation
    /// that answered upcalls on the reader loop would freeze every
    /// terminal, upload and reply on the connection and still pass. Here
    /// the ordinary request must complete BEFORE the handler is released,
    /// which is only possible if the two are on different tasks.
    #[tokio::test]
    async fn a_parked_agent_handler_does_not_stall_the_connection() {
        let (handler, mut calls, gate) = GatedHandler::parked();
        let slot: crate::agent_requests::AgentRequestSlot = Arc::new(std::sync::OnceLock::from(
            handler as Arc<dyn crate::agent_requests::AgentRequestHandler>,
        ));
        let (client, mut peer) = agent_connection(slot).await;

        peer.ask(1, farhelm_proto::AgentVerb::Sessions {}).await;
        timeout(Duration::from_secs(5), calls.recv())
            .await
            .expect("the handler was never called")
            .expect("the handler's announcement channel is open");

        // With the handler parked, an ordinary request must still get out
        // and its reply must still be routed back.
        let listing = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.list_sessions().await }
        });
        let req_id = match peer.next_control().await {
            ControlMsg::ListSessions { req_id, .. } => req_id,
            other => panic!("expected the ordinary request to reach the peer, got {other:?}"),
        };
        peer.writer
            .write_control(&ControlMsg::SessionList {
                req_id,
                sessions: vec![session("a")],
                truncated: false,
            })
            .await
            .expect("answer the ordinary request");
        let listing = timeout(Duration::from_secs(5), listing)
            .await
            .expect("the ordinary request never completed while an upcall was parked")
            .expect("listing task")
            .expect("listing");
        assert_eq!(listing.sessions.len(), 1);

        // Only now is the upcall allowed to finish, which proves the
        // ordering above was not an accident of scheduling.
        gate.add_permits(1);
        assert!(matches!(
            peer.outcome(1).await,
            farhelm_proto::AgentOutcome::Ok { .. }
        ));
    }

    /// A reply too large for a frame must fail its own request and leave
    /// the connection alive.
    ///
    /// The writer task encodes frames long after the sender has moved on,
    /// and an oversized body reaches it as an unattributable write error —
    /// which it treats exactly like a broken transport, tearing down the
    /// connection that carries every terminal, upload and request for this
    /// host. So one pathological listing would cost the whole host. The
    /// pre-enqueue size check is a backstop behind the handler's own byte
    /// allowance (that allowance is on the other side of a trait object,
    /// which is precisely why a backstop exists), and this pins both
    /// halves: the request fails, and the next one still succeeds.
    #[tokio::test]
    async fn an_oversized_agent_reply_fails_its_request_and_spares_the_connection() {
        let huge = farhelm_proto::AgentReply::Sessions {
            sessions: vec![farhelm_proto::AgentSession {
                id: "s1".to_string(),
                host: Some("this machine".to_string()),
                title: "x".repeat(farhelm_proto::MAX_FRAME_LEN as usize + 1),
                cwd: "/w".to_string(),
                agent: "claude".to_string(),
                status: "running".to_string(),
                current: true,
                archived: false,
                stale: false,
            }],
            truncated: false,
        };
        let slot: crate::agent_requests::AgentRequestSlot =
            Arc::new(std::sync::OnceLock::from(GatedHandler::answering(huge)
                as Arc<dyn crate::agent_requests::AgentRequestHandler>));
        let (client, mut peer) = agent_connection(slot).await;

        peer.ask(1, farhelm_proto::AgentVerb::Sessions {}).await;
        match peer.outcome(1).await {
            farhelm_proto::AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, farhelm_proto::ErrorKind::Internal);
                assert!(
                    message.contains("too large"),
                    "the refusal must say what was wrong, got: {message}"
                );
            }
            other => panic!("an unsendable reply must be refused, got {other:?}"),
        }

        // The connection is the thing being protected: it must still carry
        // ordinary traffic afterwards.
        let listing = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.list_sessions().await }
        });
        let req_id = match peer.next_control().await {
            ControlMsg::ListSessions { req_id, .. } => req_id,
            other => panic!("the connection died with the oversized reply: {other:?}"),
        };
        peer.writer
            .write_control(&ControlMsg::SessionList {
                req_id,
                sessions: Vec::new(),
                truncated: false,
            })
            .await
            .expect("answer the ordinary request");
        timeout(Duration::from_secs(5), listing)
            .await
            .expect("the connection stopped serving after the oversized reply")
            .expect("listing task")
            .expect("listing");
    }

    /// Past [`AGENT_ANSWER_SLOTS`] concurrent upcalls, the next one is
    /// refused immediately rather than queued.
    ///
    /// Admission has to happen before the answering task starts, because
    /// what needs bounding is what the task allocates: a merged fleet
    /// listing and a reply that can reach megabytes. One host forwarding
    /// requests for every session it runs must not be able to conscript the
    /// helm's runtime and memory on the fleet's behalf, and a queue would
    /// only convert that pressure into upcall timeouts for everyone.
    #[tokio::test]
    async fn concurrent_agent_answers_are_capped_per_connection() {
        let (handler, mut calls, gate) = GatedHandler::parked();
        let slot: crate::agent_requests::AgentRequestSlot = Arc::new(std::sync::OnceLock::from(
            handler as Arc<dyn crate::agent_requests::AgentRequestHandler>,
        ));
        let (_client, mut peer) = agent_connection(slot).await;

        for req_id in 1..=AGENT_ANSWER_SLOTS as u64 {
            peer.ask(req_id, farhelm_proto::AgentVerb::Hosts {}).await;
            timeout(Duration::from_secs(5), calls.recv())
                .await
                .expect("an admitted upcall never reached the handler")
                .expect("the handler's announcement channel is open");
        }

        let overflow = AGENT_ANSWER_SLOTS as u64 + 1;
        peer.ask(overflow, farhelm_proto::AgentVerb::Hosts {}).await;
        match peer.outcome(overflow).await {
            farhelm_proto::AgentOutcome::Err { kind, message } => {
                assert_eq!(kind, farhelm_proto::ErrorKind::Unavailable);
                assert!(
                    message.contains("too many"),
                    "the refusal must name the cause, got: {message}"
                );
            }
            other => panic!("the overflow request must be refused, got {other:?}"),
        }

        gate.add_permits(AGENT_ANSWER_SLOTS);
    }

    /// Spec: a refusal that cannot be enqueued RETIRES the connection —
    /// without ever awaiting, and without a task of its own — so the
    /// request it was the sole answer to still gets an ending, and the
    /// connection's pending requests are drained rather than stranded.
    ///
    /// Two properties in one fixture, because each is unsound without the
    /// other. The refusal must not block or spawn: it is issued from the
    /// demultiplexer, where an await stalls every terminal, upload and reply
    /// on the connection, and it exists precisely because admission was
    /// DENIED, so a task carrying one holds no permit and a peer sending
    /// faster than it reads could spawn one per request. But it must also
    /// not simply vanish, which is what this used to do: the supervisor's
    /// answer budget expiring does not END a mutation, so it RETAINS the
    /// asking session's delete fence until the request resolves or the link
    /// dies (`service::agent_relay::HelmLink::upcall`). A queue that later
    /// drains on a connection that never closes supplies neither, and the
    /// asker's every later mutation blocks behind a fence nothing will
    /// release. Retiring converts a request with no possible answer into a
    /// connection loss, which is an ending the whole relay already knows how
    /// to report.
    ///
    /// The pending `stop_session` is what proves the second half locally:
    /// its drain to `SentUnanswered` is exactly the event that, one hop out,
    /// makes the supervisor's `fail_all` resolve the upcall and drop the
    /// retained fence (pinned there by
    /// `agent_relay::tests::a_mutations_fence_outlives_the_answer_budget`).
    /// The fence itself lives in another crate and cannot be observed from
    /// here.
    #[tokio::test]
    async fn an_undeliverable_refusal_retires_the_connection() {
        let slot: crate::agent_requests::AgentRequestSlot = Arc::new(std::sync::OnceLock::new());
        let (client, _peer) = agent_connection(slot).await;

        // A request the writer queue has already accepted, so the drain has
        // something to prove. Its peer never answers it.
        let stop = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.stop_session("s1").await }
        });
        timeout(Duration::from_secs(5), async {
            while client.pending.lock().await.map.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the stop never reached the pending registry");

        // Fills to the brim, and the writer cannot empty it while this
        // runs: a `try_send` loop never awaits, so on the test runtime's
        // single thread the writer task is not polled between iterations.
        while client
            .writer_tx
            .try_send(Frame::control(&ControlMsg::Detach { channel: 1 }).into())
            .is_ok()
        {}

        // Returns rather than parking, which is the whole contract: this is
        // called from the demultiplexer, where an await would stall every
        // terminal, upload and reply on the connection.
        client.refuse_agent_request(1, "too many agent requests in flight on this host; retry");
        assert!(
            client.refusal_undeliverable_logged.load(Ordering::Relaxed),
            "an undeliverable refusal must be recorded, so the connection logs it exactly once"
        );

        timeout(Duration::from_secs(5), client.closed())
            .await
            .expect("an undeliverable refusal must end the connection");
        let error = timeout(Duration::from_secs(5), stop)
            .await
            .expect("the queued request hung after the connection was retired")
            .expect("request task")
            .expect_err("a retired connection cannot answer");
        assert!(
            matches!(
                error.downcast_ref::<SupervisorTransportError>(),
                Some(SupervisorTransportError::SentUnanswered)
            ),
            "the request was already queued, so its ending must say so: {error:#}"
        );
    }

    /// An answer's admission slot is held until the writer has SENT it, not
    /// merely until the writer queue accepted it.
    ///
    /// The distinction is what makes [`AGENT_ANSWER_SLOTS`] a bound on
    /// queued bytes rather than only on concurrent work. A reply may
    /// legally approach the frame limit; the writer queue counts messages,
    /// not bytes; so with the slot released at enqueue time a peer that
    /// reads slowly — slowly enough to fill the queue, fast enough to avoid
    /// the no-progress timeout — could stack dozens of near-limit replies
    /// ahead of every keystroke on the connection. Holding the permit on
    /// the queued frame caps that at the slots' worth.
    ///
    /// The fixture pins it from the outside: with the peer not reading, the
    /// four admitted answers can get no further than the queue, and the
    /// fifth request must therefore still be refused. Release-at-enqueue
    /// answers it instead, which is exactly the regression.
    #[tokio::test]
    async fn an_answers_slot_is_held_until_the_writer_sends_it() {
        // Several times the transport buffer below, so the writer blocks
        // partway through the first answer it writes and every later one
        // stays in the queue where this test can see it.
        let reply = farhelm_proto::AgentReply::Hosts {
            hosts: (0..128)
                .map(|n| farhelm_proto::AgentHost {
                    name: format!("host-{n}"),
                    kind: "ssh".to_string(),
                    state: "connected".to_string(),
                    current: false,
                })
                .collect(),
        };
        let slot: crate::agent_requests::AgentRequestSlot =
            Arc::new(std::sync::OnceLock::from(GatedHandler::answering(reply)
                as Arc<dyn crate::agent_requests::AgentRequestHandler>));
        let (client, mut peer) = agent_connection_buffered(slot, 4096).await;

        for req_id in 1..=AGENT_ANSWER_SLOTS as u64 {
            peer.ask(req_id, farhelm_proto::AgentVerb::Hosts {}).await;
        }
        // Wait for the answers to reach the queue — as far as they can get
        // with nothing reading — so the overflow request below lands after
        // the moment a release-at-enqueue implementation would have freed
        // their slots, rather than racing it.
        timeout(Duration::from_secs(5), async {
            while SUPERVISOR_WRITER_QUEUE - client.writer_tx.capacity() < AGENT_ANSWER_SLOTS - 1 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the admitted answers never reached the writer queue");

        let overflow = AGENT_ANSWER_SLOTS as u64 + 1;
        peer.ask(overflow, farhelm_proto::AgentVerb::Hosts {}).await;

        // Only now does anything drain, which is what lets the queued
        // answers and the refusal be read at all. Collected by req_id
        // rather than in order: the four answers race each other onto the
        // queue.
        let mut outcomes = std::collections::HashMap::new();
        for _ in 0..=AGENT_ANSWER_SLOTS {
            match peer.next_control().await {
                ControlMsg::AgentResponse { req_id, outcome } => {
                    outcomes.insert(req_id, outcome);
                }
                other => panic!("expected an AgentResponse, got {other:?}"),
            }
        }
        for req_id in 1..=AGENT_ANSWER_SLOTS as u64 {
            assert!(
                matches!(
                    outcomes.get(&req_id),
                    Some(farhelm_proto::AgentOutcome::Ok { .. })
                ),
                "the admitted answers must still be delivered: {outcomes:?}"
            );
        }
        match outcomes.get(&overflow) {
            Some(farhelm_proto::AgentOutcome::Err { kind, message }) => {
                assert_eq!(*kind, farhelm_proto::ErrorKind::Unavailable);
                assert!(
                    message.contains("too many"),
                    "the refusal must name the cause, got: {message}"
                );
            }
            other => panic!(
                "a request arriving while four answers sit in the queue must be refused, got \
                 {other:?}"
            ),
        }
    }
}
