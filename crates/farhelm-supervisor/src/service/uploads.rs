//! Attachment upload transfer machinery (PLAN_M4.md items 1 and 4).
//!
//! Each accepted `BeginUpload` gets a task (`run_upload`/`upload_transfer`)
//! that owns one transfer end to end: staging the bytes, crediting the
//! sender, and publishing on `CommitUpload` — because some of the ways a
//! transfer can end (a client disconnecting, a session being deleted, a
//! sender that simply stops) happen where no request handler is present
//! to clean up. `UploadRoute` is the connection-local map entry every other
//! upload control message looks up by channel.

use super::core::{RequestError, Supervisor, truncate_for_error};
use farhelm_proto::{ControlMsg, ErrorKind, Frame};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

/// How many attachment uploads one connection may have in flight at once
/// — `BeginUpload`'s admission decision (that variant's own docs name it
/// as the receiver's, not the protocol's, to make).
///
/// What it bounds is aggregate cost that [`farhelm_proto::UPLOAD_WINDOW_BYTES`]
/// deliberately does not: that constant bounds ONE transfer's in-flight
/// bytes, so N concurrent transfers cost N windows of queued chunks, N
/// staged temp files, and N tasks. Per CONNECTION rather than
/// supervisor-wide because a connection is what a client is, and a
/// generous per-client bound is more useful than a global one that one
/// busy client could exhaust for everybody. Eight is far beyond dropping
/// a handful of files at once and still a real bound against a client
/// that opens transfers without ever finishing them.
pub(crate) const MAX_UPLOADS_PER_CONNECTION: usize = 8;

/// Depth of one upload's chunk queue, between the connection's read loop
/// and the task that writes to disk.
///
/// Sized from the credit window, but note what the window actually
/// bounds: BYTES in flight, not FRAMES. A sender that respects
/// [`farhelm_proto::UPLOAD_WINDOW_BYTES`] and uses full-size chunks
/// cannot fill this queue, but one that respects the window while sending
/// tiny chunks can fill it with frames whose bytes are nowhere near a
/// window — which is legal. So a full queue is not proof of misbehavior
/// on its own; what it IS is the point past which this connection would
/// have to buffer without bound, and the read loop answers it by failing
/// that one transfer rather than by blocking (see the data-frame arm in
/// `handle_connection`). Blocking there would be worse than it sounds: it
/// would stop the loop from reading the very `AbortUpload` that could end
/// the transfer.
///
/// The `+ 1` is for the boundary case where a whole window is in flight
/// and one more chunk is mid-delivery.
pub(crate) const UPLOAD_CHUNK_QUEUE: usize =
    (farhelm_proto::UPLOAD_WINDOW_BYTES as usize).div_ceil(farhelm_proto::UPLOAD_CHUNK_BYTES) + 1;

/// Depth of one upload's out-of-band signal queue.
///
/// Signals are cancellations, and a transfer needs exactly one to end;
/// the slack is for the honest races (a client abort arriving as a delete
/// fires, two deletes) where a second send must not block its sender. Every
/// send is a `try_send` precisely so no signal path can ever wait on a
/// transfer — a full queue already means a cancellation is pending, which
/// is what the sender wanted.
pub(crate) const UPLOAD_SIGNAL_QUEUE: usize = 4;

/// Depth of a connection's prioritized writer queue — see the queue's own
/// comment in `handle_connection` for why uploads do not share the main
/// one.
///
/// Small on purpose: it carries an ack per chunk plus a handful of replies
/// per transfer, all tiny and all latency-sensitive, and its bound is
/// reached only by a client that has stopped reading its socket entirely
/// (which `WRITER_STALL_TIMEOUT` already ends). A deep queue here would
/// buy nothing but a longer line of stale acks.
pub(crate) const UPLOAD_PRIORITY_QUEUE: usize = 32;

/// How many FINISHED transfers' routes one connection keeps as tombstones
/// (see [`UploadRoute::outcome`]).
///
/// A tombstone answers one question — "your commit lost to a session
/// delete" versus "you never had an upload here" — and only until the
/// client reuses that channel. Kept bounded because a client that never
/// reuses a channel number would otherwise accumulate one entry per
/// upload for the life of the connection; the oldest are evicted first,
/// and an evicted tombstone degrades the answer to the generic one rather
/// than breaking anything.
const MAX_UPLOAD_TOMBSTONES: usize = 32;

/// Where a transfer stands, shared between the transfer's own task and
/// the connection that admitted it.
///
/// The connection needs three states, not two, and needs them without
/// taking a lock on the read-loop path: LIVE (the channel is in use and
/// its admission slot is held), ENDED, and ENDED-BECAUSE-THE-SESSION-WAS-
/// DELETED — the last of which is what lets a commit that lost a race
/// with `DeleteSession` be told the truth instead of "no upload is in
/// flight" (PLAN_M4.md item 4: a commit racing deletion fails with the
/// session-gone error).
#[derive(Clone)]
pub(crate) struct UploadOutcome(Arc<AtomicU8>);

/// [`UploadOutcome`]'s codes. A plain atomic rather than an enum behind a
/// mutex: the read loop consults this for every begin and commit, and it
/// is never read together with anything else that would need consistency.
const UPLOAD_LIVE: u8 = 0;
const UPLOAD_ENDED: u8 = 1;
const UPLOAD_ENDED_SESSION_GONE: u8 = 2;

impl UploadOutcome {
    pub(crate) fn live() -> UploadOutcome {
        UploadOutcome(Arc::new(AtomicU8::new(UPLOAD_LIVE)))
    }

    /// Publish this transfer's ending. Called by the task as its LAST
    /// act — after the temp file is cleaned and the client's terminal
    /// event is enqueued — because it is what releases the channel and
    /// the admission slot, and neither may be released while work
    /// attributable to this transfer can still happen.
    fn end(&self, session_gone: bool) {
        self.0.store(
            if session_gone {
                UPLOAD_ENDED_SESSION_GONE
            } else {
                UPLOAD_ENDED
            },
            Ordering::Release,
        );
    }

    fn is_live(&self) -> bool {
        self.0.load(Ordering::Acquire) == UPLOAD_LIVE
    }

    fn session_gone(&self) -> bool {
        self.0.load(Ordering::Acquire) == UPLOAD_ENDED_SESSION_GONE
    }
}

/// One connection's routing entry for a data channel carrying an upload —
/// the upload counterpart of [`InputRoute`], and connection-local for the
/// identical reason: channel ids are unique only within a connection,
/// since every client numbers its channels from 1.
///
/// Outlives its transfer deliberately, in two stages. While the transfer
/// runs, the entry is what routes chunks to it and what holds its channel
/// number and its admission slot. Once it ends, the entry becomes a
/// TOMBSTONE — no longer live, still answering for the channel — until
/// the client reuses that channel or [`MAX_UPLOAD_TOMBSTONES`] evicts it.
pub(crate) struct UploadRoute {
    /// The transfer this channel feeds, for the diagnostic trail and for
    /// naming the entry in `Supervisor::uploads`.
    pub(crate) transfer: u64,
    /// Which session the upload is for, so a commit arriving after the
    /// transfer is already gone can be answered in that session's terms.
    pub(crate) session: String,
    /// Chunks and commits, from the client.
    pub(crate) commands: mpsc::Sender<UploadCommand>,
    /// Cancellation, out of band from the commands above.
    ///
    /// Separate because a cancellation must be able to OVERTAKE a queue
    /// full of chunks: a client that aborts (or a connection that dies)
    /// while its own chunks are still queued must stop the transfer now,
    /// not after every buffered byte has been written to a file nobody
    /// will publish.
    pub(crate) signals: mpsc::Sender<UploadSignal>,
    /// Whether the transfer is still running — see [`UploadOutcome`]. The
    /// route stays in the map either way; this is what distinguishes a
    /// live entry from a tombstone.
    pub(crate) outcome: UploadOutcome,
    /// Whether the client has already sent the one terminal message it
    /// gets per transfer (a commit or an abort).
    ///
    /// Connection-local because it is about what the CLIENT did, not
    /// about what the transfer is doing: a second commit on the same
    /// channel is answered immediately here rather than queued behind
    /// the first one's publication, which is also what stops a pipelined
    /// flood of commits from being admitted as if each freed a slot.
    pub(crate) answered: bool,
    /// Insertion order, for tombstone eviction.
    pub(crate) admitted: u64,
}

impl UploadRoute {
    /// Whether this entry still stands for a running transfer (rather
    /// than being a tombstone).
    pub(crate) fn is_live(&self) -> bool {
        self.outcome.is_live()
    }
}

/// What a connection's read loop asks a running upload to do.
///
/// Deliberately only the two things that arrive from the CLIENT as
/// requests. Cancellation travels by [`UploadSignal`] instead — see
/// [`UploadRoute::signals`].
pub(crate) enum UploadCommand {
    /// One data frame's payload, exactly as it arrived.
    Chunk(Vec<u8>),
    /// The client says the bytes are all sent; verify and publish.
    /// `req_id` is what the eventual `UploadCommitted` or `Error`
    /// correlates to.
    Commit { req_id: u64 },
}

/// An out-of-band instruction to end a transfer now.
///
/// Three senders, one shape: the client (`AbortUpload`), the connection
/// (its read loop ending), and `DeleteSession` (which holds no route at
/// all and reaches transfers through `Supervisor::uploads`). Making them
/// one message rather than three mechanisms is what keeps "stop
/// immediately, clean up, and stay stopped" a single implementation.
pub(crate) struct UploadSignal {
    /// User-legible, rendered verbatim if `tell_client` is set.
    pub(crate) reason: String,
    /// Whether the client is told with an `UploadAborted`. False when the
    /// client is the one who asked (it knows) or is gone (nobody to
    /// tell); true for a supervisor-side end like a session delete.
    pub(crate) tell_client: bool,
    /// Whether this ending is a session deletion, which the channel's
    /// tombstone records so a later commit gets the session-gone error
    /// rather than a generic one.
    pub(crate) session_gone: bool,
}

/// The supervisor-wide registry entry for one in-flight upload — what
/// `DeleteSession` needs to find a transfer it must stop (see
/// [`Supervisor::uploads`]).
pub(crate) struct UploadHandle {
    /// The session whose attachments directory this transfer is writing
    /// into; the key delete selects by.
    pub(crate) session: String,
    /// The same cancellation path the connection uses (see
    /// [`UploadRoute::signals`]), so a delete-driven end and a
    /// client-driven one are the same code inside the task.
    pub(crate) signals: mpsc::Sender<UploadSignal>,
    /// Resolves — with an error, because the task drops the sender — once
    /// the transfer's task has fully finished, including removing its
    /// staging file.
    ///
    /// A completion signal rather than a `JoinHandle` so it can be
    /// registered BEFORE the task is spawned: a handle registered
    /// afterwards leaves a window where a delete sees no transfer in
    /// flight while one is already staging a file into the directory
    /// that delete is about to detach.
    pub(crate) finished: oneshot::Receiver<std::convert::Infallible>,
}

/// Everything one transfer is fixed at `BeginUpload` time.
pub(crate) struct UploadRequest {
    /// The begin's own request id: what `UploadStarted` — or a refusal —
    /// correlates to.
    pub(crate) req_id: u64,
    pub(crate) session_id: String,
    pub(crate) channel: u32,
    /// The RESOLVED, shell-safe publish name — sanitized once at
    /// admission (`crate::attachments::publish_name`), never re-derived.
    ///
    /// Two reasons it is resolved this early rather than at publication.
    /// A proposal that sanitizes to nothing gets a GENERATED name, and
    /// generating it twice would produce two different names, so the
    /// diagnostics would name a file that does not exist. And the raw
    /// proposal is caller-controlled and can be megabytes; keeping only
    /// the bounded result means an in-flight transfer retains a name, not
    /// a frame.
    pub(crate) name: String,
    /// The declared size the commit verifies byte-for-byte.
    pub(crate) size: u64,
    /// This supervisor's own id for the transfer: the key in
    /// [`Supervisor::uploads`] and the identifier every one of this
    /// transfer's diagnostic events carries, so begin, publish, and abort
    /// can be followed as one story in a log.
    pub(crate) transfer: u64,
}

/// Enqueue an upload's control message on the connection's PRIORITY
/// queue.
///
/// Uploads use a separate queue from terminal output for a reason the ack
/// contract states directly: acks must advance promptly and "must not
/// queue behind bulk frames", or a healthy transfer stalls on credit
/// while a busy terminal drains ahead of it. On the shared FIFO an ack
/// enqueued behind a screenful of terminal output waits for all of it —
/// megabytes, on a slow link — which is exactly the condition the
/// sender's own progress timeout would then call a stall.
///
/// Every message of the upload family goes this way, replies included, so
/// their relative order is preserved among themselves; only their order
/// against unrelated terminal traffic changes, and no contract exists
/// between two different channels' frames.
///
/// Separate from [`send_reply`] also because that helper routes through
/// [`reply_frame`], which needs a `req_id` and panics for a message that
/// has none — `UploadAck` and `UploadAborted` correlate by channel.
async fn send_upload(priority: &mpsc::Sender<Frame>, m: &ControlMsg) {
    let _ = priority.send(Frame::control(m)).await;
}

/// Run one attachment upload from `BeginUpload` to its end, then
/// deregister it.
///
/// Owns the whole transfer — its staged temp file, its progress
/// accounting, its replies — because every one of those has to end
/// correctly on paths no request handler is present for: a client
/// disconnecting mid-stream, a session being deleted underneath it, a
/// sender that simply stops. A task per transfer is what makes those
/// endings ordinary control flow rather than cleanup somebody has to
/// remember.
///
/// Deliberately NOT run through [`spawn_admitted`]: the admission
/// semaphore bounds concurrent SLOW REQUESTS (kill sweeps, tmux round
/// trips) at eight supervisor-wide, and a transfer lives as long as the
/// user's file takes to arrive — holding a permit for that would let eight
/// pasted screenshots stall every session's list, stop, and delete. The
/// bound that applies here is [`MAX_UPLOADS_PER_CONNECTION`] instead, and
/// every exit path of this task is bounded by either the client, the
/// progress timeout, or a delete.
pub(crate) async fn run_upload(
    sup: Arc<Supervisor>,
    priority: mpsc::Sender<Frame>,
    request: UploadRequest,
    mut commands: mpsc::Receiver<UploadCommand>,
    mut signals: mpsc::Receiver<UploadSignal>,
    finished: oneshot::Sender<std::convert::Infallible>,
    outcome: UploadOutcome,
) {
    let transfer = request.transfer;
    let session_gone =
        upload_transfer(&sup, &priority, &request, &mut commands, &mut signals).await;
    // Deregistration happens LAST, after every disk operation this
    // transfer will ever perform: an entry removed earlier would let a
    // concurrent delete conclude that nothing was in flight while this
    // task still held an open staging file in the directory it is about
    // to detach.
    sup.uploads.lock().await.remove(&transfer);
    // And the channel's own release is last of all. Until this store, the
    // connection still counts this transfer against its admission bound
    // and still refuses its channel number — which is the point: a client
    // that pipelines commits, or reuses a channel the instant it sees a
    // terminal event, must not find a slot freed while this task is still
    // enqueueing that very event (its ack or abort would otherwise land on
    // the NEXT transfer's channel).
    outcome.end(session_gone);
    // Dropping the sender is the completion signal a waiting delete is
    // parked on; it must outlive everything above for the same reason.
    drop(finished);
}

/// [`run_upload`]'s body: stage, stream, and finish one transfer.
/// Returns whether the transfer ended because its SESSION was deleted,
/// which is what the channel's tombstone records.
///
/// Split out so `run_upload`'s deregistration and channel release cannot
/// be skipped by an early return — every `return` below is inside this
/// function, and the caller's tail always runs.
async fn upload_transfer(
    sup: &Arc<Supervisor>,
    priority: &mpsc::Sender<Frame>,
    request: &UploadRequest,
    commands: &mut mpsc::Receiver<UploadCommand>,
    signals: &mut mpsc::Receiver<UploadSignal>,
) -> bool {
    let UploadRequest {
        req_id,
        session_id,
        channel,
        name,
        size,
        transfer,
    } = request;
    let (channel, size, transfer) = (*channel, *size, *transfer);

    let mut staged = match stage_upload(sup, request, signals).await {
        Ok(staged) => staged,
        Err(e) => {
            // Nothing exists yet, so a refused begin is an ordinary
            // correlated error and the channel simply never carried an
            // upload. It is still part of the transfer trail: a paste that
            // failed before its first byte is exactly as visible to the
            // user as one that failed at commit, and an operator reading
            // the log needs the same identifiers either way.
            warn!(
                session = %session_id, transfer, channel, name = %name,
                declared_bytes = size, received_bytes = 0, reason = %e.message,
                "attachment upload could not be staged"
            );
            let session_gone = e.kind == ErrorKind::NotFound;
            send_upload(
                priority,
                &ControlMsg::Error {
                    req_id: *req_id,
                    message: e.message,
                    kind: e.kind,
                },
            )
            .await;
            return session_gone;
        }
    };
    info!(
        session = %session_id, transfer, channel, name = %name, declared_bytes = size,
        "attachment upload started"
    );
    // This reply IS the sender's initial credit (`UploadStarted`'s own
    // contract), so nothing may be sent before the staging above
    // succeeded: a client granted a window against a transfer that does
    // not exist would stream bytes into a channel with no receiver.
    send_upload(
        priority,
        &ControlMsg::UploadStarted {
            req_id: *req_id,
            channel,
        },
    )
    .await;

    let mut received: u64 = 0;
    let mut deadline = tokio::time::Instant::now() + sup.timeouts.upload_progress;
    loop {
        let command = tokio::select! {
            // The order of these three branches is the stall contract, not
            // a style choice. A cancellation wins over everything: the
            // session is going away (or the client has left) either way,
            // and writing more bytes first is pure waste. The DEADLINE
            // then wins over queued commands, which is what makes the
            // timeout real — polled after `recv`, a sender that keeps a
            // command always ready (a flood of empty chunks, none of which
            // is progress) would be served forever and the expired
            // deadline never observed.
            biased;
            signal = signals.recv() => {
                let signal = signal.unwrap_or_else(cancelled_without_a_reason);
                return end_cancelled(sup, priority, request, staged, commands, signal, received)
                    .await;
            }
            _ = tokio::time::sleep_until(deadline) => {
                // The forever-pending upload this timeout exists for: the
                // connection is fine, the client simply stopped sending.
                fail_upload(
                    sup, priority, request, staged, commands,
                    UploadFailure {
                        reason: farhelm_proto::UPLOAD_ABORT_REASON_STALLED.to_string(),
                        kind: ErrorKind::Internal,
                        received,
                    },
                )
                .await;
                return false;
            }
            command = commands.recv() => command,
        };
        match command {
            // The connection's read loop is gone without having signalled
            // — its own teardown always signals first, so this is the
            // shutdown-ordering tail rather than a case with its own
            // meaning. Treated exactly like a client abort: stop, clean
            // up, tell nobody.
            None => {
                abandon_upload(sup, staged).await;
                info!(
                    session = %session_id, transfer, channel, received_bytes = received,
                    "attachment upload ended with its connection"
                );
                return false;
            }
            Some(UploadCommand::Chunk(bytes)) => {
                // Chunk SIZE is checked before enqueueing, in the read
                // loop (see `handle_connection`), because a frame this
                // task rejects has already cost the memory by the time it
                // gets here. What is checked here is the DECLARATION: a
                // chunk that would carry the transfer past the size it
                // declared is refused BEFORE it is written, which is what
                // keeps `received` — and therefore every ack — truthful
                // rather than capped, and keeps a sender from growing a
                // file that can never publish (the commit's size check is
                // exact).
                let would_be = received.saturating_add(bytes.len() as u64);
                if would_be > size {
                    fail_upload(
                        sup,
                        priority,
                        request,
                        staged,
                        commands,
                        UploadFailure {
                            reason: format!(
                                "this upload declared {size} bytes and the sender is past that \
                                 ({would_be} and counting)"
                            ),
                            kind: ErrorKind::InvalidRequest,
                            received,
                        },
                    )
                    .await;
                    return false;
                }
                let empty = bytes.is_empty();
                let written = bytes.len() as u64;
                staged = match write_upload_chunk(sup, staged, bytes, signals).await {
                    DiskStage::Done(staged) => staged,
                    DiskStage::Cancelled(signal) => {
                        // The stream went with the blocking task; see
                        // `write_upload_chunk` for why that is the safe
                        // way to leave a write that may still be running.
                        return end_cancelled(
                            sup,
                            priority,
                            request,
                            UploadStorage::AlreadyCleaned,
                            commands,
                            signal,
                            received,
                        )
                        .await;
                    }
                    DiskStage::Failed(reason) => {
                        // A full disk lands here (SPEC_impl.md's no-cap
                        // policy: ENOSPC is a failed upload with nothing
                        // published, never a truncated file), and so does
                        // a write that outlived its bound.
                        fail_upload(
                            sup,
                            priority,
                            request,
                            UploadStorage::AlreadyCleaned,
                            commands,
                            UploadFailure {
                                reason: format!("could not write this upload to disk: {reason}"),
                                kind: ErrorKind::Internal,
                                received,
                            },
                        )
                        .await;
                        return false;
                    }
                };
                received = received.saturating_add(written);
                // Only real bytes re-arm the progress window: an empty
                // chunk is not progress, and treating it as such would
                // let a sender hold a transfer open forever with frames
                // that carry nothing.
                if !empty {
                    deadline = tokio::time::Instant::now() + sup.timeouts.upload_progress;
                }
                // Acked per chunk, after the write returned: the ack is
                // both the sender's credit and the evidence its own
                // progress timeout watches, and `received` is defined as
                // bytes SAFELY WRITTEN — never a count of bytes merely
                // accepted, and never past the declaration, which the
                // check above already refused.
                send_upload(priority, &ControlMsg::UploadAck { channel, received }).await;
            }
            Some(UploadCommand::Commit { req_id }) => {
                return commit_upload(sup, priority, request, staged, received, req_id, signals)
                    .await;
            }
        }
    }
}

/// The cancellation a transfer assumes when its signal channel closes
/// without one — a shutdown-ordering tail rather than a real case, since
/// every teardown path signals before dropping its sender.
fn cancelled_without_a_reason() -> UploadSignal {
    UploadSignal {
        reason: "the upload was cancelled".to_string(),
        tell_client: false,
        session_gone: false,
    }
}

/// End a transfer because something cancelled it: clean up, tell the
/// client if the signal says to, and report whether this was a session
/// deletion (which the channel's tombstone records).
///
/// The `tell_client` split is the whole reason cancellation carries a flag
/// rather than always emitting `UploadAborted`. A client that asked for
/// the abort already knows; one whose connection died cannot be told; a
/// session delete is the case where the client is present, healthy, and
/// owed an explanation.
async fn end_cancelled(
    sup: &Arc<Supervisor>,
    priority: &mpsc::Sender<Frame>,
    request: &UploadRequest,
    storage: impl Into<UploadStorage>,
    commands: &mut mpsc::Receiver<UploadCommand>,
    signal: UploadSignal,
    received: u64,
) -> bool {
    if signal.tell_client {
        fail_upload(
            sup,
            priority,
            request,
            storage,
            commands,
            UploadFailure {
                reason: signal.reason,
                kind: if signal.session_gone {
                    ErrorKind::NotFound
                } else {
                    ErrorKind::Internal
                },
                received,
            },
        )
        .await;
        return signal.session_gone;
    }
    if let UploadStorage::Staged(staged) = storage.into() {
        abandon_upload(sup, staged).await;
    }
    info!(
        session = %request.session_id, transfer = request.transfer, channel = request.channel,
        received_bytes = received, reason = %signal.reason,
        "attachment upload cancelled"
    );
    // A commit the client managed to send before abandoning still has a
    // `req_id` outstanding; answering it is what keeps its paste from
    // hanging on a transfer nobody is going to finish.
    answer_queued_commits(
        priority,
        commands,
        &format!("this upload was cancelled: {}", signal.reason),
        ErrorKind::InvalidRequest,
    )
    .await;
    signal.session_gone
}

/// What a failure path still has to clean up: either a live staged stream
/// or nothing, because the write itself already cleaned up after itself.
///
/// A named two-state value rather than an `Option<StagedStream>` so the
/// call sites read as the distinction they are making — "the temp file is
/// still there" versus "the failing write already removed it" — instead
/// of as an absence that could equally mean a bug.
enum UploadStorage {
    Staged(crate::files::StagedStream),
    AlreadyCleaned,
}

impl From<crate::files::StagedStream> for UploadStorage {
    fn from(staged: crate::files::StagedStream) -> UploadStorage {
        UploadStorage::Staged(staged)
    }
}

/// How a transfer ended badly, gathered into one value so [`fail_upload`]
/// does not grow a parameter per detail.
struct UploadFailure {
    /// The user-legible cause, rendered verbatim by clients
    /// (`UploadAborted`'s contract).
    reason: String,
    /// The classification a commit that was ALREADY QUEUED behind this
    /// failure gets. The abort itself carries no `ErrorKind` — deliberately,
    /// per `UploadAborted` — but a commit is a request with a `req_id`
    /// waiting on it, and its answer is an ordinary `Error` that an
    /// HTTP-facing caller still has to map to a status.
    kind: ErrorKind,
    /// Bytes safely written before the failure, for the diagnostic trail.
    received: u64,
}

/// End a transfer that failed after `UploadStarted`: clean up, tell the
/// client on its channel, answer any commit that was already in the
/// queue, and log the abort.
///
/// `UploadAborted` rather than an `Error` for the failure itself, because
/// by this point the begin's `req_id` is already answered and `channel` is
/// the only identity that can name one of several concurrent transfers
/// (`UploadAborted`'s own contract). Cleanup is ATTEMPTED before the
/// notice goes out — see [`abandon_upload`] for what happens when it does
/// not succeed — so a client that reacts by retrying is not racing this
/// transfer's own staging file in the ordinary case.
async fn fail_upload(
    sup: &Arc<Supervisor>,
    priority: &mpsc::Sender<Frame>,
    request: &UploadRequest,
    storage: impl Into<UploadStorage>,
    commands: &mut mpsc::Receiver<UploadCommand>,
    failure: UploadFailure,
) {
    if let UploadStorage::Staged(staged) = storage.into() {
        abandon_upload(sup, staged).await;
    }
    let UploadFailure {
        reason,
        kind,
        received,
    } = failure;
    warn!(
        session = %request.session_id, transfer = request.transfer, channel = request.channel,
        received_bytes = received, declared_bytes = request.size, reason = %reason,
        "attachment upload aborted"
    );
    send_upload(
        priority,
        &ControlMsg::UploadAborted {
            channel: request.channel,
            reason: reason.clone(),
        },
    )
    .await;
    answer_queued_commits(
        priority,
        commands,
        &format!("this upload was aborted before its commit could run: {reason}"),
        kind,
    )
    .await;
}

/// Answer every commit still sitting in a dying transfer's queue.
///
/// Not tidiness: a client that sent its chunks and its commit back to back
/// has a `req_id` outstanding, and a transfer that ended with that commit
/// still queued would leave the request unanswered forever — its paste
/// would hang rather than fail, which is the one outcome SPEC.md's
/// "upload failures must be visible" rules out.
///
/// The receiver is CLOSED first, and that ordering is load-bearing rather
/// than tidy: `close` stops new sends and lets the queue be drained to
/// empty, while a bare drain races the connection's read loop, which may
/// be enqueueing that very commit as the loop below sees an empty queue
/// and stops. Everything else drained is chunks for a file that will never
/// publish, and is dropped.
async fn answer_queued_commits(
    priority: &mpsc::Sender<Frame>,
    commands: &mut mpsc::Receiver<UploadCommand>,
    message: &str,
    kind: ErrorKind,
) {
    commands.close();
    while let Some(command) = commands.recv().await {
        if let UploadCommand::Commit { req_id } = command {
            send_upload(
                priority,
                &ControlMsg::Error {
                    req_id,
                    message: message.to_string(),
                    kind,
                },
            )
            .await;
        }
    }
}

/// Stage one upload's file: the whole of `BeginUpload`'s side-effecting
/// half.
///
/// Runs under the session's LIFECYCLE claim (`Supervisor::lifecycle_locks`)
/// and that is what makes "nothing recreates the attachments directory
/// after a delete removed it" true rather than merely likely. Delete holds
/// the same claim for its entire run, so this either happens wholly before
/// a delete (and is then cancelled and swept by it) or wholly after (and
/// finds no session at all). Without the claim there is an interleaving
/// where this creates the directory in the gap between the delete
/// detaching it and the delete removing the session's row.
///
/// The claim is released the moment the staging file exists: a transfer
/// holds no lifecycle claim while it streams, or a large upload would
/// block that session's stop, restart, and delete for its entire duration.
///
/// ## The publish directory is canonicalized here, once
///
/// `UploadCommitted::path` is a binding promise of the RAW ABSOLUTE
/// host-side path, and the state directory is whatever the operator
/// passed — `--state-dir ./state` is a perfectly ordinary invocation, and
/// resolving it lazily would hand the client a path that is only correct
/// relative to a working directory it knows nothing about. Canonicalizing
/// the directory (which exists by this point) resolves that, plus any
/// symlinks in it, once per transfer rather than per commit.
async fn stage_upload(
    sup: &Arc<Supervisor>,
    request: &UploadRequest,
    signals: &mut mpsc::Receiver<UploadSignal>,
) -> Result<crate::files::StagedStream, RequestError> {
    let session_id = &request.session_id;
    // Waiting for the claim is interruptible, and must be: a delete that
    // is already running holds it and waits for THIS transfer to finish,
    // so a plain await here would deadlock the two against each other.
    let _lifecycle = tokio::select! {
        biased;
        signal = signals.recv() => {
            let session_gone = signal.is_some_and(|signal| signal.session_gone);
            return Err(if session_gone {
                RequestError::new(
                    ErrorKind::NotFound,
                    format!(
                        "session {} was deleted before this upload could start",
                        truncate_for_error(session_id)
                    ),
                )
            } else {
                RequestError::new(
                    ErrorKind::InvalidRequest,
                    "this upload was cancelled before it could start".to_string(),
                )
            });
        }
        claim = sup.lifecycle_locks.claim(session_id) => claim,
    };
    if !sup.sessions.lock().await.contains_key(session_id) {
        return Err(RequestError::new(
            ErrorKind::NotFound,
            format!("no such session: {}", truncate_for_error(session_id)),
        ));
    }
    let dir = crate::attachments::session_dir(&sup.state_dir, session_id);
    crate::attachments::ensure_session_dirs(&sup.state_dir, session_id)
        .await
        .map_err(|e| {
            RequestError::new(
                ErrorKind::Internal,
                format!(
                    "could not prepare this session's attachments directory ({}): {e}",
                    dir.display()
                ),
            )
        })?;
    let publish_dir = tokio::fs::canonicalize(&dir).await.map_err(|e| {
        RequestError::new(
            ErrorKind::Internal,
            format!(
                "could not resolve this session's attachments directory ({}): {e}",
                dir.display()
            ),
        )
    })?;
    // Checked once the real path is known, because the published path is
    // the PRODUCT here: `UploadCommitted::path` is UTF-8 by the protocol's
    // own path contract, and a path that merely resembles the real one is
    // worse than a refusal. It fails BEFORE any file is created.
    //
    // Defence in depth, and known to be: the only way here is a non-UTF-8
    // state directory, and creation already refuses one (the launch spec
    // carries the same constraint), so no session on such a directory
    // exists to upload into — pinned by
    // `a_non_utf8_state_directory_is_refused_before_any_session_can_exist`.
    // Kept anyway because this check is one line and the alternative, if
    // that ever changes, is a client inserting a path to a file that is
    // not there.
    if publish_dir.to_str().is_none() {
        return Err(RequestError::new(
            ErrorKind::Internal,
            format!(
                "this supervisor's attachments directory ({}) is not valid UTF-8, so an \
                 uploaded file's path could not be reported; nothing was stored",
                publish_dir.display()
            ),
        ));
    }
    let staging = publish_dir.join(
        crate::attachments::staging_dir(&sup.state_dir, session_id)
            .file_name()
            .expect("the staging directory always has a final component"),
    );
    let seam = Arc::clone(&sup.seams.upload_fs);
    let name = request.name.clone();
    let publish_for_task = publish_dir.clone();
    tokio::task::spawn_blocking(move || {
        crate::files::StagedStream::create(&staging, &publish_for_task, &name, &*seam)
    })
    .await
    .unwrap_or_else(|join| Err(std::io::Error::other(join)))
    .map_err(|e| {
        RequestError::new(
            ErrorKind::Internal,
            format!(
                "could not stage this upload under {}: {e}",
                publish_dir.display()
            ),
        )
    })
}

/// How a bounded, cancellable disk stage ended.
///
/// `Cancelled` and `Failed` both mean the caller no longer holds the
/// stream: see [`await_disk_stage`] for why abandoning it is the only
/// safe answer to a blocking operation that has outlived its bound.
enum DiskStage<T> {
    Done(T),
    Cancelled(UploadSignal),
    Failed(String),
}

/// Await one blocking disk operation under BOTH a cancellation signal and
/// a time bound.
///
/// Neither guard is optional. Without the signal, a `write` on a wedged
/// filesystem is a window in which a session delete cannot proceed — it
/// waits for this transfer, which waits for the disk. Without the bound, a
/// transfer that is neither cancelled nor progressing sits in the same
/// window forever, invisible to the progress timeout (which only runs
/// between commands, not inside one).
///
/// What "cancel" can honestly mean here is the subtle part: a blocking
/// thread cannot be interrupted, so this ABANDONS the operation rather
/// than stopping it. Dropping the join handle leaves the closure to finish
/// in the blocking pool and drop the [`crate::files::StagedStream`] it
/// owns, whose `Drop` removes the staging file. So the operation may still
/// complete, but nothing the caller does afterwards can depend on it, and
/// what it wrote is removed rather than published — which is exactly the
/// guarantee that matters: a timed-out stage never contributes to a
/// published file.
async fn await_disk_stage<T>(
    bound: Duration,
    signals: &mut mpsc::Receiver<UploadSignal>,
    handle: tokio::task::JoinHandle<T>,
) -> DiskStage<T> {
    let mut handle = handle;
    tokio::select! {
        biased;
        signal = signals.recv() => {
            DiskStage::Cancelled(signal.unwrap_or_else(cancelled_without_a_reason))
        }
        joined = &mut handle => match joined {
            Ok(value) => DiskStage::Done(value),
            // A panic inside the blocking operation. The stream went with
            // it, so its own `Drop` already removed the staging file.
            Err(join) => DiskStage::Failed(format!("{join}")),
        },
        _ = tokio::time::sleep(bound) => DiskStage::Failed(format!(
            "the filesystem did not answer within {bound:?}"
        )),
    }
}

/// Write one chunk, off the async runtime's worker threads and under
/// [`await_disk_stage`]'s two guards.
///
/// `spawn_blocking` per chunk rather than once per transfer because the
/// stream is driven by messages, not by a blocking read loop: the file
/// write is the only blocking part, and a slow or stalled disk must not
/// occupy a runtime worker that other sessions' terminals are sharing. The
/// stream is moved in and back out (rather than borrowed) because
/// `spawn_blocking` needs a `'static` closure; every non-`Done` outcome
/// keeps it — `StagedStream::write_chunk` has already cleaned up after a
/// failure, and an abandoned closure cleans up when it finally drops — so
/// the caller can never publish a stream whose content has a hole in it.
async fn write_upload_chunk(
    sup: &Arc<Supervisor>,
    mut staged: crate::files::StagedStream,
    bytes: Vec<u8>,
    signals: &mut mpsc::Receiver<UploadSignal>,
) -> DiskStage<crate::files::StagedStream> {
    let seam = Arc::clone(&sup.seams.upload_fs);
    let handle = tokio::task::spawn_blocking(move || {
        let result = staged.write_chunk(&*seam, &bytes);
        (staged, result)
    });
    match await_disk_stage(sup.timeouts.upload_disk_stage, signals, handle).await {
        DiskStage::Done((staged, Ok(()))) => DiskStage::Done(staged),
        DiskStage::Done((_, Err(e))) => DiskStage::Failed(format!("{e}")),
        DiskStage::Cancelled(signal) => DiskStage::Cancelled(signal),
        DiskStage::Failed(reason) => DiskStage::Failed(reason),
    }
}

/// Verify and publish a finished transfer — `CommitUpload`'s whole
/// contract.
///
/// Three refusals, in the order they can be decided, and every one of them
/// publishes nothing and cleans the temp file:
///
/// 1. **The session was deleted underneath the transfer.** Taken under the
///    session's lifecycle claim, so a delete and a commit racing resolve to
///    one honest winner rather than a file published into a directory
///    that is being removed.
/// 2. **The declared size does not match what arrived.** Short or long,
///    both are a mismatch (`BeginUpload`'s `size` is a declaration the
///    commit verifies byte-for-byte), never a published file.
/// 3. **The publication itself failed** — a full disk at the fsync, a
///    vanished directory, or a candidate list exhausted by collisions.
async fn commit_upload(
    sup: &Arc<Supervisor>,
    priority: &mpsc::Sender<Frame>,
    request: &UploadRequest,
    staged: crate::files::StagedStream,
    received: u64,
    req_id: u64,
    signals: &mut mpsc::Receiver<UploadSignal>,
) -> bool {
    let session_id = &request.session_id;
    // Interruptible for `stage_upload`'s reason: a delete already holding
    // this claim is waiting for this very transfer to end.
    let lifecycle = tokio::select! {
        biased;
        signal = signals.recv() => {
            let signal = signal.unwrap_or_else(cancelled_without_a_reason);
            abandon_upload(sup, staged).await;
            warn!(
                session = %session_id, transfer = request.transfer, channel = request.channel,
                received_bytes = received, reason = %signal.reason,
                "attachment upload lost its commit to a cancellation"
            );
            // A correlated `Error`, not an `UploadAborted`: the commit is
            // a request with a `req_id` waiting on it, and answering that
            // is what tells the client its paste failed.
            send_upload(
                priority,
                &ControlMsg::Error {
                    req_id,
                    message: if signal.session_gone {
                        format!(
                            "session {} was deleted while this upload was in flight; nothing \
                             was published",
                            truncate_for_error(session_id)
                        )
                    } else {
                        format!("this upload was cancelled: {}", signal.reason)
                    },
                    kind: if signal.session_gone {
                        ErrorKind::NotFound
                    } else {
                        ErrorKind::InvalidRequest
                    },
                },
            )
            .await;
            return signal.session_gone;
        }
        claim = sup.lifecycle_locks.claim(session_id) => claim,
    };
    let refusal = if !sup.sessions.lock().await.contains_key(session_id) {
        Some(RequestError::new(
            ErrorKind::NotFound,
            format!(
                "session {} was deleted while this upload was in flight; nothing was published",
                truncate_for_error(session_id)
            ),
        ))
    } else if received != request.size {
        Some(RequestError::new(
            ErrorKind::InvalidRequest,
            format!(
                "this upload declared {} bytes but {received} arrived; nothing was published",
                request.size
            ),
        ))
    } else {
        None
    };
    if let Some(refusal) = refusal {
        let session_gone = refusal.kind == ErrorKind::NotFound;
        abandon_upload(sup, staged).await;
        warn!(
            session = %session_id, transfer = request.transfer, channel = request.channel,
            received_bytes = received, declared_bytes = request.size,
            reason = %refusal.message, "attachment upload failed at commit"
        );
        send_upload(
            priority,
            &ControlMsg::Error {
                req_id,
                message: refusal.message,
                kind: refusal.kind,
            },
        )
        .await;
        return session_gone;
    }

    // The publication itself is bounded, and the claim is what makes the
    // bound matter: fsync and link are blocking calls on a filesystem that
    // can wedge, and every one of this session's lifecycle operations —
    // stop, restart, delete — is queued behind this claim while they run.
    // An unbounded hold would let one stuck disk make a session
    // unmanageable. Past the bound the transfer fails, the abandoned
    // operation cleans up after itself when it finally returns
    // (`await_disk_stage`), and nothing is ever published half-way.
    let seam = Arc::clone(&sup.seams.upload_fs);
    let name = request.name.clone();
    let handle = tokio::task::spawn_blocking(move || {
        staged.publish_no_clobber(&*seam, crate::attachments::name_candidates(&name))
    });
    let published = match await_disk_stage(sup.timeouts.upload_disk_stage, signals, handle).await {
        DiskStage::Done(result) => result.map_err(|e| format!("{e}")),
        DiskStage::Cancelled(signal) => {
            drop(lifecycle);
            warn!(
                session = %session_id, transfer = request.transfer, channel = request.channel,
                received_bytes = received, reason = %signal.reason,
                "attachment upload was cancelled mid-publication"
            );
            send_upload(
                priority,
                &ControlMsg::Error {
                    req_id,
                    message: if signal.session_gone {
                        format!(
                            "session {} was deleted while this upload was publishing; nothing \
                             was published",
                            truncate_for_error(session_id)
                        )
                    } else {
                        format!("this upload was cancelled: {}", signal.reason)
                    },
                    kind: if signal.session_gone {
                        ErrorKind::NotFound
                    } else {
                        ErrorKind::InvalidRequest
                    },
                },
            )
            .await;
            return signal.session_gone;
        }
        DiskStage::Failed(reason) => Err(reason),
    };
    // Released before the reply: nothing below touches the session, and
    // the claim is contended by every lifecycle operation on it.
    drop(lifecycle);
    let path = match published {
        Ok(path) => path,
        Err(e) => {
            warn!(
                session = %session_id, transfer = request.transfer, channel = request.channel,
                received_bytes = received, error = %e, "attachment upload failed to publish"
            );
            send_upload(
                priority,
                &ControlMsg::Error {
                    req_id,
                    message: format!("could not publish this upload: {e}; nothing was stored"),
                    kind: ErrorKind::Internal,
                },
            )
            .await;
            return false;
        }
    };
    // UTF-8 by construction — `stage_upload` refused a non-UTF-8
    // directory before anything was created, and every published name is
    // ASCII by sanitizing — so this cannot fail. Handled rather than
    // unwrapped anyway: an `expect` here would turn a future change to
    // either of those two properties into a supervisor panic instead of a
    // failed paste.
    let Some(path) = path.to_str().map(str::to_string) else {
        warn!(
            session = %session_id, transfer = request.transfer,
            "attachment published under a non-UTF-8 path"
        );
        send_upload(
            priority,
            &ControlMsg::Error {
                req_id,
                message: format!(
                    "this upload published under a path that is not valid UTF-8 ({}), which \
                     cannot be reported over this protocol",
                    path.display()
                ),
                kind: ErrorKind::Internal,
            },
        )
        .await;
        return false;
    };
    info!(
        session = %session_id, transfer = request.transfer, channel = request.channel,
        bytes = received, name = %request.name, path = %path,
        "attachment upload published"
    );
    send_upload(priority, &ControlMsg::UploadCommitted { req_id, path }).await;
    false
}

/// How many times [`abandon_upload`] retries a removal that failed.
///
/// Cleanup failures on a local state directory are transient or
/// permanent, and a couple of immediate retries separate the two cheaply:
/// a momentary error clears, while a read-only mount or a vanished
/// directory does not, and nothing here can fix the latter. Small and
/// unspaced on purpose — this runs on the path that is about to tell a
/// client its upload failed, and a client waiting out a backoff schedule
/// for debris cleanup would be a worse bargain than the debris.
const ABANDON_ATTEMPTS: usize = 3;

/// Remove a transfer's staging file, off the runtime's worker threads.
///
/// Retried a few times and then LOGGED rather than propagated: every
/// caller is already ending the transfer for some other reason, and there
/// is no answer a client could act on ("your upload failed, and also we
/// left a file somewhere you cannot see" is not one). The honest contract
/// is therefore cleanup-attempt-plus-backstop, and it is stated as such
/// wherever it is promised — `StagedStream`'s docs, `ControlMsg::
/// UploadAborted`'s, and here — rather than claimed as a guarantee: what
/// survives a removal this cannot perform is a file in the reserved
/// staging directory, which `attachments::reconcile_at_startup` removes.
async fn abandon_upload(sup: &Arc<Supervisor>, staged: crate::files::StagedStream) {
    // Captured before the stream is consumed: every retry below works on
    // the path, since `abandon` takes the stream by value and there is no
    // second one to hand back.
    let path = staged.temp_path().to_path_buf();
    let seam = Arc::clone(&sup.seams.upload_fs);
    match tokio::task::spawn_blocking(move || staged.abandon(&*seam)).await {
        Ok(Ok(())) => return,
        Ok(Err(_)) => {}
        Err(join) => {
            // The blocking task panicked, taking the stream with it — its
            // own `Drop` already attempted the removal.
            warn!(error = %join, "the staging-file cleanup task failed");
            return;
        }
    }
    for attempt in 2..=ABANDON_ATTEMPTS {
        let seam = Arc::clone(&sup.seams.upload_fs);
        let retry_path = path.clone();
        let removed = tokio::task::spawn_blocking(move || {
            crate::files::FaultSeam::remove_temp(&*seam, &retry_path)
        })
        .await;
        match removed {
            Ok(Ok(())) => return,
            // Somebody else got there first (a concurrent reconciliation,
            // or the stream's own `Drop`), which is the outcome this
            // wanted.
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => return,
            Ok(Err(e)) if attempt == ABANDON_ATTEMPTS => {
                warn!(
                    path = %path.display(), error = %e, attempts = attempt,
                    "could not remove an abandoned upload's staging file; the next startup will \
                     reconcile it"
                );
                return;
            }
            Ok(Err(_)) => continue,
            Err(join) => {
                warn!(error = %join, "the staging-file cleanup task failed");
                return;
            }
        }
    }
}

/// Cancel every upload in flight for `session_id` and wait for each to
/// finish cleaning up.
///
/// `DeleteSession`'s first step, and the reason its directory handling is
/// safe: once this returns, no task can still write into (or publish
/// into) that session's attachments directory. The wait is what makes
/// that a guarantee rather than a hope — firing the signals and moving on
/// would race the delete against a transfer's last write.
///
/// Callers must hold the session's lifecycle claim, which is what keeps a
/// NEW transfer from staging into the directory between this call and the
/// delete that follows it (see `stage_upload`).
pub(crate) async fn abort_session_uploads(sup: &Supervisor, session_id: &str, reason: &str) {
    let doomed: Vec<UploadHandle> = {
        let mut uploads = sup.uploads.lock().await;
        uploads
            .extract_if(|_, handle| handle.session == session_id)
            .map(|(_, handle)| handle)
            .collect()
    };
    // Signalled for ALL of them before ANY is waited on, like the
    // attachment teardown: the waits are sequential, so signalling inside
    // the same loop would leave the later transfers still writing while
    // the earlier ones are already gone.
    for handle in &doomed {
        // `try_send` never blocks and never needs to: a full signal queue
        // already holds a cancellation, which is all this call wanted.
        let _ = handle.signals.try_send(UploadSignal {
            reason: reason.to_string(),
            tell_client: true,
            session_gone: true,
        });
    }
    for handle in doomed {
        // The task signals completion by DROPPING its sender, so the
        // error is the expected outcome; a value can never arrive
        // (`Infallible`).
        let _ = handle.finished.await;
    }
}

/// Retire this connection's finished upload routes.
///
/// Called once per read-loop iteration, like [`reap_finished_tasks`]. What
/// it does NOT do is remove a finished route outright: a tombstone is what
/// lets a commit that arrives after its transfer died be told WHY (a
/// session delete, rather than a generic "no upload here" — PLAN_M4.md
/// item 4), and the channel is legitimately reusable the moment the
/// transfer ends, which `BeginUpload` handles by replacing the tombstone.
///
/// So this only bounds them: the oldest tombstones past
/// [`MAX_UPLOAD_TOMBSTONES`] are evicted, since a client that never reuses
/// a channel number would otherwise accumulate one entry per upload for
/// the life of the connection. An evicted tombstone costs a less specific
/// error message, nothing more.
pub(crate) fn prune_finished_uploads(routes: &mut HashMap<u32, UploadRoute>) {
    let mut tombstones: Vec<(u64, u32)> = routes
        .iter()
        .filter(|(_, route)| !route.is_live())
        .map(|(channel, route)| (route.admitted, *channel))
        .collect();
    if tombstones.len() <= MAX_UPLOAD_TOMBSTONES {
        return;
    }
    tombstones.sort_unstable();
    for (_, channel) in tombstones
        .into_iter()
        .take(routes.len().saturating_sub(MAX_UPLOAD_TOMBSTONES))
    {
        routes.remove(&channel);
    }
}

/// The `Error` a `CommitUpload` gets when its channel carries no live
/// transfer.
///
/// Two genuinely different situations, and the distinction is the whole
/// reason this is not one flat message: a commit that raced a session
/// DELETE must say so (PLAN_M4.md item 4 — a commit racing deletion fails
/// with the session-gone error), because the client's paste failed for a
/// reason it can explain to the user, while a commit for a channel that
/// never carried an upload is an ordinary client bug.
///
/// The answer comes from the channel's own TOMBSTONE rather than from the
/// session map, which is what makes it stable: by the time a client's
/// commit arrives, the session it names may have been deleted and its id
/// reused by nothing at all, and asking "does this session exist now?"
/// would answer a different question than "what happened to this
/// transfer?".
pub(crate) fn commit_without_upload(
    route: Option<&UploadRoute>,
    channel: u32,
) -> (String, ErrorKind) {
    match route {
        Some(route) if route.outcome.session_gone() => (
            format!(
                "session {} was deleted while this upload was in flight; nothing was published",
                truncate_for_error(&route.session)
            ),
            ErrorKind::NotFound,
        ),
        _ => (
            format!("no upload is in flight on channel {channel}"),
            ErrorKind::InvalidRequest,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A route for the tests below, live unless `ended` says otherwise.
    ///
    /// Built by hand rather than through a real transfer because what
    /// these tests are about is the CONNECTION's bookkeeping — which
    /// entries linger, which answer a commit, which count against
    /// admission — and a real transfer would drag a session, a
    /// filesystem, and a task in to prove none of it.
    fn test_route(channel: u32, ended: Option<bool>) -> UploadRoute {
        let (commands, _commands_rx) = mpsc::channel::<UploadCommand>(1);
        let (signals, _signals_rx) = mpsc::channel::<UploadSignal>(1);
        // The receivers are dropped with this call, which is exactly what
        // a finished transfer leaves behind.
        let outcome = UploadOutcome::live();
        if let Some(session_gone) = ended {
            outcome.end(session_gone);
        }
        UploadRoute {
            transfer: u64::from(channel),
            session: "session-1".to_string(),
            commands,
            signals,
            outcome,
            answered: false,
            admitted: u64::from(channel),
        }
    }

    /// Tombstones are bounded, and only tombstones are evicted.
    ///
    /// Both halves matter. A finished transfer's entry has to LINGER —
    /// it is what tells a late commit that its session was deleted rather
    /// than that it never had an upload — but a client that never reuses
    /// a channel number would otherwise accumulate one entry per upload
    /// for the life of the connection, which is a leak nothing else in
    /// the suite would notice. A LIVE transfer's entry is never evicted at
    /// any count: it holds that transfer's channel and its admission slot.
    #[tokio::test]
    async fn finished_upload_routes_are_bounded_and_live_ones_are_never_evicted() {
        let mut routes = HashMap::new();
        for channel in 1..=(MAX_UPLOAD_TOMBSTONES as u32 + 10) {
            routes.insert(channel, test_route(channel, Some(false)));
        }
        let live = MAX_UPLOAD_TOMBSTONES as u32 + 11;
        routes.insert(live, test_route(live, None));

        prune_finished_uploads(&mut routes);

        assert!(
            routes.contains_key(&live),
            "a live transfer's route must never be evicted"
        );
        assert!(
            routes.len() <= MAX_UPLOAD_TOMBSTONES + 1,
            "tombstones must stay bounded, got {} entries",
            routes.len()
        );
        // Oldest first: the survivors are the most recent tombstones,
        // which are the ones a client is plausibly about to ask about.
        assert!(
            !routes.contains_key(&1),
            "the oldest tombstone must be the first evicted"
        );
        assert!(
            routes.contains_key(&(MAX_UPLOAD_TOMBSTONES as u32 + 10)),
            "the newest tombstone must survive"
        );
    }

    /// A commit for a channel whose transfer died with its SESSION is
    /// told so; every other late commit gets the generic answer.
    ///
    /// This is the distinction PLAN_M4.md item 4 requires — a commit
    /// racing deletion fails with the session-gone error — and the reason
    /// the answer comes from the channel's tombstone rather than from the
    /// session map: by commit time the session is gone from the map
    /// either way, so the map could only ever give the generic answer.
    #[tokio::test]
    async fn a_late_commit_is_answered_from_its_channels_tombstone() {
        let deleted = test_route(1, Some(true));
        let (message, kind) = commit_without_upload(Some(&deleted), 1);
        assert_eq!(kind, ErrorKind::NotFound);
        assert!(
            message.contains("deleted") && message.contains("session-1"),
            "a commit that lost to a delete must say so, got: {message}"
        );

        let ended = test_route(2, Some(false));
        let (message, kind) = commit_without_upload(Some(&ended), 2);
        assert_eq!(kind, ErrorKind::InvalidRequest);
        assert!(message.contains("no upload"), "got: {message}");

        let (message, kind) = commit_without_upload(None, 7);
        assert_eq!(kind, ErrorKind::InvalidRequest);
        assert!(message.contains("channel 7"), "got: {message}");
    }
}
