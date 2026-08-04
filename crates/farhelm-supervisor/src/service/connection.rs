//! The per-connection read loop, output forwarder, and stall detection.
//!
//! `handle_connection` is the whole lifetime of one client connection: it
//! owns the connection-local `input_routes`/`upload_routes` maps, spawns
//! (and, on the way out, drains) the slow request handlers admitted
//! through `spawn_admitted`, and reads control/data frames until the
//! peer goes away. `Forwarder` is the other half — one per attachment,
//! pumping tmux output back to the client and detecting a stalled reader.

use super::core::{Supervisor, note_first_input};
use super::handlers::handle_control;
use super::snapshots::load_alt_screen_snapshot;
use super::terminals::{ActiveAttach, AttachmentKey, InputRoute, TerminalId};
use super::uploads::{
    UPLOAD_PRIORITY_QUEUE, UploadCommand, UploadRoute, UploadSignal, prune_finished_uploads,
};
use crate::tmux::{OutputEvent, OutputStream, PaneModes};
use farhelm_proto::io::{
    FrameReader, FrameWriter, ProgressWrite, handshake, parse_control, write_frame_before_stall,
};
use farhelm_proto::{ControlMsg, DETACH_REASON_STALLED, ErrorKind, Frame};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{info, warn};

/// Data-frame chunk size for replay. Well under MAX_FRAME_LEN; small
/// enough that the first screenful renders while the rest streams.
const REPLAY_CHUNK: usize = 32 * 1024;

/// Depth of the per-connection writer queue — the single queue every
/// reply, notification, and terminal data frame on one connection passes
/// through on its way to the socket.
///
/// This bound is the supervisor half of PLAN_M2_5.md's "no unbounded
/// queue remains on the terminal output path". It was an
/// `unbounded_channel` through M2, which meant a helm slower than the
/// panes it was watching grew this process's memory with no ceiling at
/// all — the debt this milestone exists to close.
///
/// What the number buys: a bound at all, expressed in the only unit
/// `mpsc` offers. It counts FRAMES, and frame sizes vary by two orders of
/// magnitude, so it is a ceiling rather than a size. Terminal data frames
/// are chunked at [`REPLAY_CHUNK`] (32 KiB), which puts the worst case for
/// a data-only backlog at 2 MiB per connection; live pane output usually
/// arrives in far smaller notifications, so the typical backlog is a
/// fraction of that. Control frames are capped only by `MAX_FRAME_LEN`,
/// making the absolute worst case much larger on paper — in practice
/// nothing generates large replies back to back, and `LIST_BYTE_BUDGET`
/// already halves the one reply that can be big. Bounding by count keeps
/// this one legible rule instead of a byte-accounting scheme layered over
/// the frame layer.
///
/// The accepted consequence, stated plainly: when the single multiplexed
/// consumer (the helm) is slow, control REPLIES queue behind terminal
/// DATA at this bound, so a request can wait on a busy terminal's
/// backlog. That is acceptable because the alternative — unbounded — is
/// exactly the debt being closed, and because a client that reaches this
/// bound repeatedly is one the browser's watermark should already be
/// pausing. Nothing that holds a supervisor mutex may block on this
/// queue; see [`notify_detached`] for the one shape that would otherwise
/// want to, and the `Attach` handler's reserved permit for the other.
pub(crate) const CONNECTION_WRITER_QUEUE: usize = 64;

/// The no-progress window `handle_connection`'s shutdown tail allows the
/// writer task before giving up on it — NOT a total deadline on the drain.
///
/// "Drain everything queued" and "never hang" cannot both hold against a
/// peer that stops reading without erroring (a full TCP/pipe window — a
/// wedged ssh session, say): the writer's `write_frame` call just parks,
/// there is no error for the `writer_failed` oneshot to carry, and without
/// a bound the connection task (plus its entire backlog) leaks for the
/// process lifetime. But a flat deadline over-punishes a peer that is
/// merely slow rather than gone — backpressured but still reading, so
/// frames keep landing, just not inside any one fixed window — and killing
/// that connection breaks the half-close contract that replies already
/// queued before shutdown began still reach a peer that is still reading.
/// `drain_writer` resets this window every time a frame completes, so a
/// slow-but-live peer gets unbounded total time to drain; only a peer that
/// lands zero frames across one whole window is treated as gone. See
/// `drain_writer` for the honest residual this still leaves (a single
/// frame slower than the window). This only bounds shutdown —
/// steady-state backpressure while a connection is still active is out of
/// scope here (M2.5, PLAN.md).
///
/// Interplay with `HANDLER_SHUTDOWN_TIMEOUT`: that bound runs FIRST, and
/// is what this one implicitly assumes has already happened. Every slow
/// handler task (`ListSessions`/`StopSession`/`DeleteSession`) that
/// finishes within `HANDLER_SHUTDOWN_TIMEOUT`'s window enqueues its reply
/// exactly like a synchronous handler would, and THIS window is what then
/// gets that reply to a still-reading peer. A straggling task aborted by
/// `HANDLER_SHUTDOWN_TIMEOUT` instead, by contrast, never enqueues
/// anything at all — there is nothing left for this drain to wait out on
/// its behalf, and its own multi-second tmux work is exactly why it gets
/// a separate, longer budget rather than being folded into this one.
const WRITER_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// How long `handle_connection`'s shutdown tail waits for spawned slow-
/// handler tasks (see `HANDLER_ADMISSION_PERMITS`) to finish on their own,
/// tracked in a `JoinSet`, before aborting whatever remains and logging
/// it. Generous — `kill_process_tree`'s own sequence (grace period,
/// quiesce passes, kill confirmation) can legitimately take several
/// seconds — but not unbounded: a wedged tmux must not leak a task (and
/// this connection's own shutdown) forever. See `WRITER_DRAIN_TIMEOUT`'s
/// docs for how the two windows interact.
const HANDLER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// Serve one protocol connection. Generic over the byte stream so tests
/// can drive it over an in-process duplex pipe with the same code path
/// production uses over the unix socket.
pub async fn handle_connection<S>(sup: Arc<Supervisor>, stream: S) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (r, w) = tokio::io::split(stream);
    // Byte-level progress on the write half, so the writer task can tell a
    // slow peer from one that has stopped consuming — see `ProgressWrite`.
    let (w, bytes_written) = ProgressWrite::new(w);
    let mut reader = FrameReader::new(r);
    let mut writer = FrameWriter::new(w);
    handshake(&mut reader, &mut writer, "supervisor").await?;

    // Single writer task; everything that wants to send (request
    // handlers, the output forwarder, takeover notifications) goes
    // through this queue so frames never interleave mid-write. Bounded
    // since M2.5 — see CONNECTION_WRITER_QUEUE for what the bound buys
    // and what it costs.
    let (tx, mut rx) = mpsc::channel::<Frame>(CONNECTION_WRITER_QUEUE);
    // The upload family's own queue, drained ahead of the one above.
    //
    // `UploadAck`'s contract requires acks that "must not queue behind
    // bulk frames": on one FIFO an ack enqueued behind a screenful of
    // terminal output waits for all of it, which stalls the sender on
    // credit and — since the sender is timing the same silence — invites
    // it to declare a healthy receiver stalled. Bounded like the main
    // queue, and much smaller, because upload control frames are tiny and
    // few per transfer; a transfer that fills even this is one whose
    // client has stopped reading entirely, which the writer stall timeout
    // already covers.
    let (priority_tx, mut priority_rx) = mpsc::channel::<Frame>(UPLOAD_PRIORITY_QUEUE);
    let (writer_failed_tx, mut writer_failed_rx) = oneshot::channel();
    // Progress counter for the shutdown-tail drain: `drain_writer` reads
    // this to tell "peer merely slow" apart from "peer gone" instead of
    // enforcing one flat deadline. Relaxed is enough on both ends — this
    // is a liveness heartbeat, not a value anything is synchronized on.
    let frames_written = Arc::new(AtomicU64::new(0));
    let frames_written_for_writer = Arc::clone(&frames_written);
    let writer_stall = sup.timeouts.writer_stall;
    let mut writer_task = tokio::spawn(async move {
        loop {
            // Biased, so an upload's control frame goes out ahead of
            // whatever terminal output is queued — see `priority_tx`'s own
            // comment for why that is a contract rather than a
            // preference. `else` fires only when BOTH queues are closed,
            // which is this task's ordinary end.
            let frame = tokio::select! {
                biased;
                Some(frame) = priority_rx.recv() => frame,
                Some(frame) = rx.recv() => frame,
                else => break,
            };
            // A write that makes NO PROGRESS for a whole window is
            // treated exactly like a write that failed. See
            // WRITER_STALL_TIMEOUT: without this, bounding the queue would
            // let a peer that stops reading park every producer —
            // including this connection's own read loop, via the admission
            // permits — so the connection could never notice the peer was
            // gone. Breaking here drops `rx`, which is what unblocks those
            // producers with a closed-channel error.
            if let Err(detail) =
                write_frame_before_stall(&mut writer, &bytes_written, &frame, writer_stall).await
            {
                warn!(error = %detail, "frame write to client failed");
                let _ = writer_failed_tx.send(detail);
                break;
            }
            frames_written_for_writer.fetch_add(1, Ordering::Relaxed);
        }
    });

    // Which terminal each of this connection's data channels types into.
    // Connection-local by necessity: channel ids are unique only within a
    // connection, since every client numbers its channels from 1.
    let mut input_routes: HashMap<u32, InputRoute> = HashMap::new();
    // The other meaning a data channel can have as of version 6: bytes
    // flowing the OTHER way, into an attachment upload (see
    // [`UploadRoute`]). Connection-local for the same reason, and
    // separate from `input_routes` because the two carry different
    // directions and different lifetimes — an upload ends at its commit,
    // an attachment when its terminal is taken over.
    let mut upload_routes: HashMap<u32, UploadRoute> = HashMap::new();

    // Tracking (not admission — that is now `sup.admission`, shared
    // across every connection this supervisor serves; see its own docs
    // for why it must NOT be per-connection) for the slow handlers
    // (`ListSessions`/`StopSession`/`DeleteSession`) that `handle_control`
    // spawns instead of awaiting inline. `HANDLER_SHUTDOWN_TIMEOUT`'s own
    // docs cover why leaving these untracked is not safe: without a
    // `JoinSet`, this function's shutdown tail would have nothing to wait
    // on or clean up after.
    let mut tasks: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();

    let result: anyhow::Result<()> = async {
        loop {
            // Reap whatever spawned handler tasks have already finished
            // before touching the next frame — see `reap_finished_tasks`'s
            // own docs for why a long-lived connection needs this every
            // iteration, not just at shutdown.
            reap_finished_tasks(&mut tasks);
            // Same discipline for finished transfers: a route left behind
            // by a completed upload would make its channel number
            // permanently unusable on this connection (see
            // `prune_finished_uploads`).
            prune_finished_uploads(&mut upload_routes);
            // Either half failing ends the connection. Waiting only for
            // read EOF leaks attachments when a half-broken peer keeps
            // writing after it has stopped reading our replies.
            let frame = tokio::select! {
                frame = reader.read_frame() => frame?,
                error = &mut writer_failed_rx => {
                    let detail = error.unwrap_or_else(|_| "writer task exited".to_string());
                    return Err(anyhow::anyhow!("frame write to client failed: {detail}"));
                }
            };
            let Some(frame) = frame else {
                break;
            };
            match frame.kind {
                // An upload's bytes, if this channel is one an accepted
                // `BeginUpload` established and whose transfer is still
                // live. Checked before the terminal-input path below
                // because the two share one channel-id space and a
                // channel is only ever one of the two (the begin and
                // attach handlers each refuse a channel the other holds).
                //
                // Nothing here awaits. Chunk delivery is a `try_send` and
                // every rejection is a decision rather than a wait,
                // because this loop is also the only thing that can
                // deliver the `AbortUpload` or the `Detach` that would
                // resolve the situation: a read loop parked on a full
                // upload queue cannot read the frame that would drain it.
                farhelm_proto::FrameKind::Data
                    if upload_routes
                        .get(&frame.channel)
                        .is_some_and(UploadRoute::is_live) =>
                {
                    let channel = frame.channel;
                    let route = upload_routes
                        .get(&channel)
                        .expect("the guard above matched this channel");
                    // Size is checked HERE rather than in the transfer's
                    // task, and the difference is memory: a frame handed
                    // to the queue has already been allocated, and
                    // `UPLOAD_CHUNK_QUEUE` slots of `MAX_FRAME_LEN`
                    // frames is orders of magnitude more than the credit
                    // window this connection is supposed to cost. Rejected
                    // the channel-correlated way (`UPLOAD_CHUNK_BYTES`'s
                    // own contract): a data frame has no `req_id` to hang
                    // an `Error` on.
                    if frame.body.len() > farhelm_proto::UPLOAD_CHUNK_BYTES {
                        let _ = route.signals.try_send(UploadSignal {
                            reason: format!(
                                "an upload chunk of {} bytes exceeds the {}-byte chunk limit",
                                frame.body.len(),
                                farhelm_proto::UPLOAD_CHUNK_BYTES
                            ),
                            tell_client: true,
                            session_gone: false,
                        });
                        continue;
                    }
                    match route.commands.try_send(UploadCommand::Chunk(frame.body)) {
                        Ok(()) => {}
                        // More queued than the credit window can justify
                        // (see `UPLOAD_CHUNK_QUEUE`): this transfer is
                        // ended rather than allowed to buffer without
                        // bound, and the client is told on its channel.
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            let _ = route.signals.try_send(UploadSignal {
                                reason: "this connection queued more upload data than it could \
                                         be credited for"
                                    .to_string(),
                                tell_client: true,
                                session_gone: false,
                            });
                        }
                        // The transfer ended between the guard above and
                        // this send; the bytes belong to nothing. Its
                        // route stays as a tombstone until the task
                        // publishes its ending.
                        Err(mpsc::error::TrySendError::Closed(_)) => {}
                    }
                }
                farhelm_proto::FrameKind::Data => {
                    // Route input only if this channel is still the live
                    // attachment of the TERMINAL it attached to: a client
                    // kicked by a takeover must not keep typing into a
                    // pane it no longer owns, and the supervisor enforces
                    // that rather than trusting clients to stop. The
                    // route's own terminal is what makes the lookup exact
                    // now that one session can have several attached at
                    // once — a session-keyed lookup would let a channel
                    // detached from one terminal match whichever other
                    // terminal of the same session happened to be found.
                    //
                    // The route is BORROWED, not cloned: it is read-only
                    // here, and a channel that turns out to have lost its
                    // attachment is dropped from the map after the borrow
                    // ends (`stale_route` below) rather than mid-arm.
                    let stale_route = if let Some(route) = input_routes.get(&frame.channel) {
                        let entry = &route.entry;
                        // The check and the send-keys delivery run under
                        // ONE lock hold, like the Resize arm: releasing
                        // between them is a TOCTOU where a takeover
                        // completes in the gap and the kicked client's
                        // already-validated keystrokes land in the
                        // winner's pane — and keystrokes into an agent
                        // terminal are command execution. Safe against
                        // deadlock because forwarders never take this
                        // lock, and the Attach handler already holds it
                        // across its own tmux calls.
                        //
                        // Both halves of the check matter: channel ids are
                        // unique only within a connection (every client
                        // numbers from 1), so comparing the channel alone
                        // would let a kicked client on another connection
                        // pass whenever the numbers collide.
                        // `same_channel` identifies the owning connection.
                        let mut attachments = sup.attachments.lock().await;
                        // Borrow the matched `ActiveAttach` directly and
                        // send on it in place, rather than cloning a
                        // handle out of the map: `InputClient` owns a
                        // child process and its pipes, so unlike the old
                        // `InputWriter` there is nothing cheap to clone.
                        // The borrow (via `a`) ends when this match
                        // expression finishes evaluating, which is what
                        // lets the failure arm below still mutate
                        // `attachments` (`.remove`) under the SAME lock
                        // hold — no gap where a takeover could interleave.
                        // The delivery flag is read under the SAME lock hold
                        // as the send, and is what PLAN_M3.md item 8's
                        // correlator anchors on: an empty frame delivers
                        // nothing, and a send that failed part-way still
                        // delivered what it confirmed — see
                        // `InputClient::delivered_any_bytes`.
                        let (send_result, delivered) = match attachments.get_mut(&route.key) {
                            Some(a) if a.channel == frame.channel && a.notify.same_channel(&tx) => {
                                let result = a.input.send(&frame.body).await;
                                (Some(result), a.input.delivered_any_bytes())
                            }
                            _ => (None, false),
                        };
                        if delivered {
                            note_first_input(&sup, entry);
                        }
                        match send_result {
                            Some(Ok(())) => false,
                            // A failed send is this session's problem,
                            // not the shared connection's. It is still
                            // fatal to this attachment: accepting later
                            // chunks after silently losing one can turn
                            // a command into a different command.
                            Some(Err(e)) => {
                                warn!(session = %entry.info.id, error = %e, "input dropped");
                                if let Some(old) = attachments.remove(&route.key) {
                                    old.forwarder.abort();
                                    let _ = old.forwarder.await;
                                    notify_detached(
                                        &old.notify,
                                        old.channel,
                                        format!("terminal input failed: {e:#}"),
                                    );
                                }
                                true
                            }
                            // This channel is no longer the terminal's
                            // attachment — a takeover, a detach, or a
                            // delete got there first.
                            None => {
                                drop(attachments);
                                true
                            }
                        }
                    } else {
                        false
                    };
                    if stale_route {
                        // The route outlived its attachment, so stop
                        // holding the session entry alive for a channel
                        // that can never type again.
                        input_routes.remove(&frame.channel);
                    }
                }
                farhelm_proto::FrameKind::Control => {
                    let msg = parse_control(&frame)?;
                    handle_control(
                        &sup,
                        msg,
                        &tx,
                        &priority_tx,
                        &mut input_routes,
                        &mut upload_routes,
                        &mut tasks,
                    )
                    .await;
                }
            }
        }
        Ok(())
    }
    .await;

    // Connection gone: every upload it was carrying is over. Each is
    // SIGNALLED rather than merely dropped, because a transfer whose queue
    // still holds chunks would otherwise keep writing them to a file
    // nobody will ever commit — the signal is selected on first, so the
    // transfer stops where it stands and removes its staging file
    // (PLAN_M4.md item 4's channel-loss cleanup). Dropping the routes
    // afterwards closes the command channels behind it.
    for route in upload_routes.values() {
        let _ = route.signals.try_send(UploadSignal {
            reason: "the client's connection closed".to_string(),
            tell_client: false,
            session_gone: false,
        });
    }
    drop(upload_routes);
    // Tear down any attachments this connection owned so the next
    // attach doesn't fight a dead forwarder. Abort AND await, exactly
    // like the takeover path: abort only schedules cancellation, and the
    // old control-mode client's process is not gone until the cancelled
    // task has been polled to completion. Removing the entry without
    // waiting would let a new connection's attach — which finds no
    // incumbent to kick — open its control client while the old one is
    // still dying, the documented frozen-replay hazard. Awaiting under
    // the lock is safe (forwarders never take it) and is what serializes
    // that new attach behind this teardown.
    let mut attachments = sup.attachments.lock().await;
    let mine: Vec<ActiveAttach> = attachments
        .extract_if(|_, attachment| attachment.notify.same_channel(&tx))
        .map(|(_, attachment)| attachment)
        .collect();
    for attachment in mine {
        attachment.forwarder.abort();
        let _ = attachment.forwarder.await;
    }
    drop(attachments);
    drop(tx);
    // Give the connection's spawned slow-handler tasks (list/stop/delete —
    // see `HANDLER_ADMISSION_PERMITS`'s docs) a bounded chance to finish
    // and enqueue their replies BEFORE the writer drain below starts
    // waiting on the queue those replies land in — a task that finishes
    // after the writer has already given up would have enqueued a reply
    // nobody drains. `HANDLER_SHUTDOWN_TIMEOUT` is generous (kill sweeps
    // legitimately take seconds), but a tmux wedged forever must not leak
    // this connection's shutdown forever either: past that bound, every
    // remaining task is aborted and logged rather than awaited
    // unconditionally.
    if tokio::time::timeout(HANDLER_SHUTDOWN_TIMEOUT, async {
        while tasks.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        warn!(
            remaining = tasks.len(),
            "spawned request handler(s) did not finish within {HANDLER_SHUTDOWN_TIMEOUT:?}; \
             aborting"
        );
        tasks.abort_all();
        // `abort_all` only SCHEDULES cancellation — exactly like every
        // other abort in this module (the attachment forwarders just
        // above, `drain_writer` below), a task is not actually gone until
        // its cancellation has been delivered and polled to completion.
        // Draining `join_next` to empty here is what proves that: only
        // once every aborted task has been reaped are its resources
        // (the `tx` clone it held, any locks it could have been mid-
        // acquiring) provably released. Proceeding to `drain_writer`
        // before that could undercount `frames_written` against a
        // straggling task that was about to enqueue a reply, or race a
        // lock a still-cancelling task had not yet released.
        while tasks.join_next().await.is_some() {}
    }
    // Progress-bounded drain, not an unconditional await: see
    // WRITER_DRAIN_TIMEOUT and drain_writer. A peer that stopped reading
    // without erroring leaves the writer parked mid-write forever, with no
    // error for `writer_failed` to report — that path is already handled
    // above; this is the silent-stall case. Unlike a flat deadline,
    // `drain_writer` only gives up once a whole window passes with zero
    // frames landing, so a live-but-slow peer still gets its queued
    // replies.
    drain_writer(&mut writer_task, &frames_written, WRITER_DRAIN_TIMEOUT).await;
    result
}

/// Wait for `writer_task` to finish, giving up only once a full `window`
/// passes without a single frame completing — not after `window` elapses
/// in total.
///
/// This is what lets `handle_connection`'s shutdown tail honor the
/// half-close contract (queued replies reach a peer that is still
/// reading) without also being willing to wait forever: a peer that is
/// backpressured but alive keeps landing frames, each of which resets the
/// window, so it gets unbounded total time to drain. Only a peer that
/// lands zero frames across one whole window — the "gone" case this
/// function exists to bound — gets its writer task aborted. The abort is
/// always followed by an await of the same handle, matching every other
/// abort-then-await pairing in this module: a bare `abort()` only
/// schedules cancellation, and returning before the task is actually
/// polled to completion would let a new attach race the old writer's last
/// touch of the socket.
///
/// Honest residual: progress is observed at frame granularity via
/// `frames_written`, not at the byte level. A single frame whose own
/// write spans the entire window with nothing else completing — i.e. a
/// link slower than the frame size divided by `window` (for the 32 KiB
/// `REPLAY_CHUNK` and the default 5s window, under roughly 6.4 KB/s) —
/// is indistinguishable here from a peer that is truly gone, and gets
/// aborted. That is accepted, not accidental: catching it would need
/// sub-frame progress reporting, which `FrameWriter` does not have.
async fn drain_writer(
    writer_task: &mut tokio::task::JoinHandle<()>,
    frames_written: &AtomicU64,
    window: Duration,
) {
    loop {
        let before = frames_written.load(Ordering::Relaxed);
        if tokio::time::timeout(window, &mut *writer_task)
            .await
            .is_ok()
        {
            // Writer task finished (queue drained or it hit a write
            // error) within this window; nothing left to bound.
            return;
        }
        if frames_written.load(Ordering::Relaxed) != before {
            // At least one frame landed during the window that just
            // timed out: the peer is slow, not gone. Give it another
            // window instead of counting this as no progress.
            continue;
        }
        warn!("no frame completed for a full {window:?} window; aborting writer task");
        writer_task.abort();
        let _ = writer_task.await;
        return;
    }
}

/// Build the frame for a per-request reply, degrading to `ControlMsg::Error`
/// if the honest reply would not fit on the wire.
///
/// Callers send by pushing onto `tx`, the channel the writer task
/// (`handle_connection`) drains — without ever observing whether the
/// frame they built actually encodes. The writer discovers an oversized
/// frame only later, as a write error indistinguishable from the
/// transport genuinely breaking, and (correctly, for a real transport
/// failure) treats ANY write failure as connection-fatal. An oversized
/// frame is not a transport failure, though; it is a message this
/// connection can never send, and finding that out at the writer would
/// tear down every attachment sharing the connection over one bad reply.
/// So the check has to happen here, before the frame is ever enqueued.
/// `ListSessions` is the only M1 reply that can realistically hit this:
/// one frame carries every session, so a host with enough sessions (or
/// unusually large titles) can legitimately exceed `MAX_FRAME_LEN`. M2
/// adds a count cap plus an encoded-size budget to the list reply, and
/// real pagination is M6 (PLAN.md); this defusal stays as the last-resort
/// backstop even then. The substituted `Error` reply is small by
/// construction — just a `req_id` and a fixed-shape message — so it
/// always fits.
///
/// Only call this for messages that carry a `req_id`: it panics otherwise,
/// which is deliberate. A reply silently sent unchecked here would be the
/// same oversized-frame-reaches-the-writer bug this function exists to
/// close; better to fail loudly at the call site than send something that
/// might carry no `req_id` for the substitute `Error` to correlate against.
pub(crate) fn reply_frame(msg: &ControlMsg) -> Frame {
    let req_id = match *msg {
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
        | ControlMsg::Error { req_id, .. } => req_id,
        ref other => {
            unreachable!("reply_frame called with a message that carries no req_id: {other:?}")
        }
    };
    let frame = Frame::control(msg);
    if frame.exceeds_max_len() {
        warn!(
            req_id,
            size = frame.encoded_len(),
            "control reply exceeds max frame size; substituting an error reply"
        );
        Frame::control(&ControlMsg::Error {
            req_id,
            message: format!(
                "reply encodes to {} bytes, exceeding the {}-byte frame limit",
                frame.encoded_len(),
                farhelm_proto::MAX_FRAME_LEN,
            ),
            // Encoding succeeded; the encoded frame is simply too big for
            // the wire. Still the server's own limit, not something the
            // caller's request got wrong.
            kind: ErrorKind::Internal,
        })
    } else {
        frame
    }
}

/// Push `m` (through [`reply_frame`]'s oversize check) onto `tx`.
///
/// A tiny free function rather than a closure over `tx` specifically so
/// spawned handler tasks (`ListSessions`/`StopSession`/`DeleteSession` in
/// `handle_control` — see those arms' own comments on why they spawn) can
/// share it after moving their own OWNED clone of `tx` into the task: a
/// closure captured by reference cannot outlive the stack frame that
/// spawned it, but this function only ever borrows `tx` for the instant
/// of the call, so the same helper works whether the caller holds `tx` by
/// reference (the synchronous handlers in `service::handlers`) or by owned
/// clone (the spawned ones).
///
/// Awaits on a FULL queue, which is the intended backpressure (see
/// [`CONNECTION_WRITER_QUEUE`]): every caller is either
/// `handle_connection`'s own read loop — where blocking is exactly the
/// "stop accepting requests from a peer that is not reading its replies"
/// behavior wanted — or a spawned handler task holding nothing but its
/// admission permit. It must NOT be called while a supervisor mutex is
/// held; the arms that reply after a lock-held section all drop the guard
/// first, and [`notify_detached`] exists for the one shape that cannot.
pub(crate) async fn send_reply(tx: &mpsc::Sender<Frame>, m: &ControlMsg) {
    let _ = tx.send(reply_frame(m)).await;
}

/// Enqueue a `Detached` notice for `channel` without ever blocking the
/// caller — the one send shape that runs with `Supervisor::attachments`
/// held.
///
/// Every teardown path (takeover, delete, failed input, stall) has to tell
/// a client it lost its attachment, and all but one of them do so while
/// holding the global attachments mutex, because the ownership check and
/// the teardown must be atomic against a racing attach. Awaiting a
/// bounded send there would hand a wedged peer the ability to freeze
/// EVERY session's attach, input, and delete behind its own full queue —
/// turning this milestone's bound into a supervisor-wide deadlock.
///
/// So: `try_send` first, and only if the queue is genuinely full does the
/// blocking send move to its own task. The notice is never dropped (the
/// spawned task owns a sender clone and waits), and the only cost is that
/// a full-queue notice may land after frames enqueued behind it. That is
/// harmless here: a `Detached` is the last thing that channel will ever
/// carry, so nothing it could be reordered against still matters.
pub(crate) fn notify_detached(tx: &mpsc::Sender<Frame>, channel: u32, reason: String) {
    let frame = Frame::control(&ControlMsg::Detached { channel, reason });
    // A `Closed` error needs no handling: the connection is gone, so there
    // is nobody left to tell.
    if let Err(mpsc::error::TrySendError::Full(frame)) = tx.try_send(frame) {
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(frame).await;
        });
    }
}

/// Acquire an admission permit and spawn `future` onto `tasks`, holding
/// the permit for the future's entire lifetime — the one, shared
/// implementation of the admission-then-spawn pattern every slow
/// `handle_control` arm (`ListSessions`/`StopSession`/`DeleteSession`)
/// uses, so the ordering below cannot drift between call sites.
///
/// The permit is acquired HERE, in THIS function's own await — which
/// means in the CALLER's await point, since this is not itself spawned —
/// not inside `future` once it is already running as its own task. That
/// ordering is the entire point: every real caller is `handle_control`,
/// invoked directly from `handle_connection`'s read loop, so an
/// admission-exhausted flood of slow requests blocks THAT loop right
/// here, before a task (or a `JoinSet` entry for it) exists at all —
/// rather than spawning and tracking an unbounded number of not-yet-
/// admitted tasks that all sit parked on the semaphore. See
/// `HANDLER_ADMISSION_PERMITS`'s docs for why that distinction matters.
pub(crate) async fn spawn_admitted<F>(
    admission: &Arc<tokio::sync::Semaphore>,
    tasks: &mut tokio::task::JoinSet<()>,
    future: F,
) where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let permit = Arc::clone(admission)
        .acquire_owned()
        .await
        .expect("admission semaphore is never closed");
    tasks.spawn(async move {
        let _permit = permit;
        future.await;
    });
}

/// Non-blockingly collect every spawned handler task that has ALREADY
/// finished, logging (not propagating) any `JoinError` — a panic inside a
/// handler, or a cancellation from `handle_connection`'s own shutdown
/// tail. `JoinSet` does not free a task's slot on its own just because
/// the task completed; something has to call `join_next`/
/// `try_join_next` to actually collect it, so this must run periodically
/// on any connection expected to live a while — `handle_connection`'s read
/// loop calls it once per iteration specifically so a long-lived polling
/// connection (a UI's `ListSessions` loop, potentially running for hours)
/// does not accumulate one finished-but-unreaped entry per request for
/// its entire lifetime. `try_join_next` (not `join_next`) is what keeps
/// this non-blocking: it returns `None` immediately once nothing is
/// ready, rather than waiting for the next task to finish.
fn reap_finished_tasks(tasks: &mut tokio::task::JoinSet<()>) {
    while let Some(result) = tasks.try_join_next() {
        if let Err(e) = result {
            warn!(error = %e, "spawned request handler task panicked or was cancelled");
        }
    }
}

/// Why a forwarder stopped, and therefore what the client must be told.
enum ForwarderEnd {
    /// The client's connection is gone (its writer queue closed). Nothing
    /// left to notify.
    ClientGone,
    /// The pane's control client ended: session killed, tmux server gone.
    TerminalEnded,
    /// The control stream itself failed mid-read.
    StreamFailed(String),
    /// The attachment stayed paused past [`STALL_DETACH_TIMEOUT`]. Unlike
    /// every other outcome this one also TEARS THE ATTACHMENT DOWN — see
    /// `Forwarder::run`.
    Stalled,
}

/// One attachment's output pump: everything between a pane's control-mode
/// client and one client channel's data frames.
///
/// A struct rather than a closure because this task now owns four
/// intertwined responsibilities that each need most of the same state —
/// writing the initial replay, honoring client pause/resume, recovering
/// from a tmux-side pause, and detaching a stalled client — and threading
/// eight captured variables through free functions read far worse than
/// methods on the thing they all belong to.
///
/// It never takes `Supervisor::attachments`. That is the invariant — and
/// only that one — which lets the takeover, detach, and delete paths abort
/// *and await* a forwarder while holding that mutex; the one place this
/// task needs the map (the stall teardown) hands the work to a separate
/// task for exactly that reason. It is NOT lock-free in general:
/// `send_dead_pane_snapshot` briefly takes `pending_snapshots`, which is
/// safe precisely because nothing ever holds that lock across a wait on a
/// forwarder.
pub(crate) struct Forwarder {
    pub(crate) sup: Arc<Supervisor>,
    pub(crate) session_id: String,
    /// Which of the session's terminals this pump belongs to — carried
    /// only so a stall detach can name its own attachment exactly (see
    /// [`detach_stalled`]). A stall is a property of ONE terminal's
    /// client (PLAN_M4.md item 3: `pause-after`/`%pause` are per control
    /// client), so the teardown it triggers must not reach the session's
    /// other terminals, and reconstructing the key from the session id
    /// alone could no longer say which one stalled.
    pub(crate) terminal: TerminalId,
    pub(crate) channel: u32,
    pub(crate) tx: mpsc::Sender<Frame>,
    pub(crate) stream: OutputStream,
    pub(crate) pause_rx: watch::Receiver<Option<tokio::time::Instant>>,
    pub(crate) stall_timeout: Duration,
}

impl Forwarder {
    /// Write the attach replay, mark its end, then pump live output until
    /// something ends it.
    ///
    /// The whole task body, so the teardown obligations live in exactly
    /// one place: the control client is always shut down, and a stall
    /// additionally removes the attachment (via `detach_stalled`, which
    /// must run on its own task — see there).
    ///
    /// The three phases are in this order because the marker's whole
    /// meaning is its POSITION between them (PLAN_M5.md item 2): it is
    /// emitted once the initial replay has been enqueued in full and
    /// before `pump` can enqueue a single live byte. Nothing else in this
    /// task may be inserted between the second and third steps.
    pub(crate) async fn run(mut self, modes: PaneModes, prefill: Vec<u8>) {
        // The attach replay never resets: the client's terminal is brand
        // new. Only the catch-up path passes `true` — see `send_replay`.
        let end = match self.send_replay(modes, prefill, false).await {
            Ok(()) => match self.send_replay_complete().await {
                Ok(()) => self.pump().await,
                Err(end) => end,
            },
            Err(end) => end,
        };
        // Ordered: kill the control client BEFORE announcing the detach,
        // matching every other teardown in this module. A client that
        // reattaches the instant it sees `Detached` must not race a
        // control client that is still dying — the documented
        // frozen-replay hazard.
        self.stream.shutdown().await;
        match end {
            ForwarderEnd::ClientGone => {}
            ForwarderEnd::TerminalEnded => {
                notify_detached(&self.tx, self.channel, "session terminal ended".to_string());
            }
            ForwarderEnd::StreamFailed(reason) => {
                // Must notify: swallowing this leaves the client with a
                // terminal that silently stops updating while still
                // accepting input, and no log line anywhere explaining why.
                warn!(channel = self.channel, error = %reason, "output stream failed");
                notify_detached(
                    &self.tx,
                    self.channel,
                    format!("output stream failed: {reason}"),
                );
            }
            ForwarderEnd::Stalled => {
                warn!(
                    channel = self.channel,
                    session = %self.session_id,
                    "attachment paused longer than {:?}; detaching as stalled",
                    self.stall_timeout
                );
                detach_stalled(
                    &self.sup,
                    AttachmentKey::new(&self.session_id, self.terminal),
                    self.channel,
                    self.tx,
                );
            }
        }
    }

    /// Write one replay to the client: optional reset, the pre-content
    /// mode sequences, the captured content, then the post-content
    /// sequences.
    ///
    /// Order is load-bearing (see `PaneModes`): the alternate-screen
    /// switch must precede the content because it CLEARS the buffer it
    /// switches to, and cursor placement must follow it because writing
    /// content moves the cursor.
    ///
    /// `reset_first` is the only difference between the two callers, and
    /// the whole correctness argument for the second. An ATTACH replays
    /// into a terminal the client just created (empty by construction),
    /// while a post-stall CATCH-UP replays into one already showing
    /// everything received before tmux cut the stream. PLAN_M2_5.md is
    /// explicit: never replay into a populated terminal. `\x1bc` (RIS) is
    /// what makes the second case equivalent to the first — it clears
    /// screen AND scrollback, so the catch-up's end state is a fresh
    /// reattach's end state rather than the old content with a second copy
    /// of history appended under it.
    async fn send_replay(
        &mut self,
        modes: PaneModes,
        content: Vec<u8>,
        reset_first: bool,
    ) -> Result<(), ForwarderEnd> {
        if reset_first {
            self.send_bytes(b"\x1bc".to_vec()).await?;
        }
        self.send_bytes(modes.pre_content_sequences().into_bytes())
            .await?;
        self.send_bytes(content).await?;
        self.send_bytes(modes.post_content_sequences().into_bytes())
            .await?;
        if modes.pane_dead {
            self.send_dead_pane_snapshot(modes.alternate_on).await?;
        }
        Ok(())
    }

    /// Append the stop-time alt-screen snapshot, if this session has one,
    /// after a dead pane's ordinary prefill.
    ///
    /// See [`load_alt_screen_snapshot`] for what is appended and why the
    /// gate is the snapshot's existence rather than the pane's current
    /// screen. `pane_alternate` decides only PLACEMENT: when the dead pane
    /// is still on the alternate screen, `\x1b[?1049l` leaves it first, so
    /// the divider and snapshot land on the primary screen whose real
    /// scrollback can absorb whatever does not fit — otherwise they would
    /// land in the scrollback-less alternate buffer the mode replay just
    /// re-entered and bury their own top rows with nowhere to overflow to.
    ///
    /// Sent as separate pieces rather than one concatenated buffer, and
    /// through [`Self::send_bytes`] like every other byte a forwarder
    /// writes: avoids a second full copy of a snapshot that may be
    /// megabytes, and inherits the same chunking, pause gating, and stall
    /// deadline as the rest of the replay.
    async fn send_dead_pane_snapshot(&mut self, pane_alternate: bool) -> Result<(), ForwarderEnd> {
        let Some(bytes) = load_alt_screen_snapshot(&self.sup, &self.session_id).await else {
            return Ok(());
        };
        if pane_alternate {
            self.send_bytes(b"\x1b[?1049l".to_vec()).await?;
        }
        self.send_bytes(b"\r\n\x1b[2m-- last screen before stop --\x1b[0m\r\n".to_vec())
            .await?;
        self.send_bytes(bytes).await?;
        self.send_bytes(b"\r\n".to_vec()).await
    }

    /// Tell the client its attach's catch-up is over: every byte this
    /// forwarder will replay from history has been enqueued, and the live
    /// stream follows (`ControlMsg::ReplayComplete`, PLAN_M5.md item 2).
    ///
    /// Called from exactly one place — between the initial replay and the
    /// live pump in [`Self::run`] — and deliberately NOT from
    /// [`Self::catch_up_after_tmux_pause`]: M2.5's flow-control recovery
    /// replays history into an attachment that is already live, and the
    /// marker bounds the ATTACH's catch-up, not every catch-up the
    /// attachment will ever perform. A marker there would tell a consumer
    /// a second catch-up phase had just ended when none had begun, and
    /// the pause recovery's presentation is explicitly out of M5's scope.
    ///
    /// Ordering needs no machinery of its own, which is the design: the
    /// marker is enqueued onto the SAME per-connection writer queue as
    /// this channel's data frames (see [`CONNECTION_WRITER_QUEUE`]), and
    /// this task is the only producer of that channel's replay, marker and
    /// live data — so "after every replay byte, before any live byte"
    /// follows from the two calls around this one rather than from a
    /// promise anybody has to keep. (It is not the only producer on the
    /// channel at all: a teardown elsewhere can enqueue a channel-
    /// correlated `Detached`, which by construction is the last frame the
    /// channel carries and so orders against nothing that still matters.)
    ///
    /// Unlike [`Self::send_bytes`] this does not park on a client pause.
    /// The pause contract is about not pushing terminal OUTPUT at a
    /// client that asked for silence; the marker carries none, is already
    /// queued behind every replay frame it describes, and holding it back
    /// would leave a paused client's catch-up unbounded from its own
    /// point of view. It is still raced against the same absolute stall
    /// deadline, because a client that has stopped draining can otherwise
    /// pin this task on a full queue forever.
    async fn send_replay_complete(&mut self) -> Result<(), ForwarderEnd> {
        let frame = Frame::control(&ControlMsg::ReplayComplete {
            channel: self.channel,
        });
        tokio::select! {
            result = self.tx.send(frame) => result.map_err(|_| ForwarderEnd::ClientGone),
            () = stalled_past_deadline(self.pause_rx.clone(), self.stall_timeout) => {
                Err(ForwarderEnd::Stalled)
            }
        }
    }

    /// Park while the client has output paused, returning `Err(Stalled)`
    /// once this ONE pause has outlived [`STALL_DETACH_TIMEOUT`].
    ///
    /// The deadline is absolute: it is computed from the instant the pause
    /// STARTED (stored in the watch by `set_attachment_paused`), never
    /// from whenever this function happened to be called. That is what
    /// makes the timeout a hard maximum across every phase a forwarder can
    /// be in — initial replay, live pump, catch-up replay — and across
    /// every individual chunk within them. A per-call timer would be
    /// restarted by any progress at all, so a client draining just fast
    /// enough to keep the forwarder moving between chunks could stay
    /// paused forever.
    async fn park_while_paused(&mut self) -> Result<(), ForwarderEnd> {
        loop {
            let Some(paused_at) = *self.pause_rx.borrow_and_update() else {
                return Ok(());
            };
            tokio::select! {
                () = tokio::time::sleep_until(paused_at + self.stall_timeout) => {
                    return Err(ForwarderEnd::Stalled);
                }
                changed = self.pause_rx.changed() => {
                    // The sender is dropped only when this attachment has
                    // been removed from the map, at which point this task
                    // is being aborted anyway; falling through is the
                    // harmless answer.
                    if changed.is_err() {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Enqueue `bytes` as one or more data frames, honoring the client's
    /// pause before every one and blocking on the bounded writer queue
    /// when it is full — which is the backpressure this milestone exists
    /// to introduce.
    ///
    /// Gating EVERY chunk, not merely the top of the pump loop, is what
    /// makes the watermark actually work: a replay, or a single large
    /// output notification, can be megabytes, and a version that consulted
    /// the pause only between EVENTS would push that entire burst at a
    /// client which had already said stop.
    ///
    /// Chunked at [`REPLAY_CHUNK`] for a harsher reason than the replay's
    /// own progressiveness: one bounded output notification may still be
    /// larger than a protocol frame, and the encoder rejects that rather
    /// than sending something the far side cannot decode. This is the
    /// last chunking boundary; input and replay already do the same.
    async fn send_bytes(&mut self, bytes: Vec<u8>) -> Result<(), ForwarderEnd> {
        for chunk in bytes.chunks(REPLAY_CHUNK) {
            self.park_while_paused().await?;
            let frame = Frame::data(self.channel, chunk.to_vec());
            // Raced against the SAME absolute deadline so a client that
            // pauses mid-send cannot pin this task (and the frames behind
            // it) on a queue nobody is draining. Cancelling a
            // `Sender::send` is safe — the frame is either enqueued or it
            // is not — and on the stall path the whole attachment is torn
            // down anyway.
            tokio::select! {
                result = self.tx.send(frame) => {
                    if result.is_err() {
                        return Err(ForwarderEnd::ClientGone);
                    }
                }
                () = stalled_past_deadline(self.pause_rx.clone(), self.stall_timeout) => {
                    return Err(ForwarderEnd::Stalled);
                }
            }
        }
        Ok(())
    }

    /// Pump pane output to the client until the stream, the client, or
    /// the client's patience ends.
    async fn pump(&mut self) -> ForwarderEnd {
        loop {
            // Park while the client has asked for silence. Not reading
            // the control client at all IS the flow control: past
            // `pause-after`, tmux answers by either throttling the pane
            // (the agent's write blocks) or cutting this client's stream
            // with `%pause` — see TMUX_PAUSE_AFTER_SECS for why both
            // happen and why nothing here may assume which. Either way
            // the tmux server's memory stays flat, which is the property
            // this parking exists to get.
            if let Err(end) = self.park_while_paused().await {
                return end;
            }

            let event = tokio::select! {
                event = self.stream.next_output() => event,
                () = stalled_past_deadline(self.pause_rx.clone(), self.stall_timeout) => {
                    // Abandoning `next_output` mid-read can drop a
                    // partial line, which is why this is only ever done
                    // on a path that tears the stream down immediately
                    // afterwards. See that method's cancel-safety note.
                    return ForwarderEnd::Stalled;
                }
            };
            match event {
                Ok(Some(OutputEvent::Bytes(bytes))) => {
                    if let Err(end) = self.send_bytes(bytes).await {
                        return end;
                    }
                }
                Ok(Some(OutputEvent::Paused)) => {
                    if let Err(end) = self.catch_up_after_tmux_pause().await {
                        return end;
                    }
                }
                Ok(None) => return ForwarderEnd::TerminalEnded,
                Err(e) => return ForwarderEnd::StreamFailed(format!("{e:#}")),
            }
        }
    }

    /// Recover from tmux having cut this client's pane stream: continue
    /// the pane and replay it as a full reattach.
    ///
    /// A `%pause` means bytes were dropped from the LIVE path — they are
    /// not buffered anywhere and will never arrive. What tmux still has is
    /// its history, which is exactly what the attach path already knows
    /// how to replay, so the catch-up reuses that machinery verbatim
    /// (`OutputStream::resume_paused_with_replay`) instead of growing a
    /// second replay implementation. Alternate-screen and normal-screen
    /// panes are both covered for free: the shared snapshot code already
    /// picks the right capture for the pane's current mode.
    ///
    /// Reached only on the tmux behavior that cuts the stream — the other
    /// one (tmux throttling the pane instead) never gets here, because
    /// nothing was dropped and the pump simply keeps reading. See
    /// `TMUX_PAUSE_AFTER_SECS` for why both exist; this method is
    /// correctness-critical but not on every run's path.
    ///
    /// The client's terminal is reset first because the replay assumes an
    /// empty one — see [`Self::send_replay`]. Within `HISTORY_LIMIT`
    /// this is lossless; past it, it degrades to the history floor, which
    /// is the same floor the browser's own scrollback is capped at, so
    /// the end state stays observably equivalent to lossless slow
    /// delivery (PLAN_M2_5.md).
    ///
    /// A failure here is fatal to the attachment rather than ignorable: a
    /// pane left paused delivers nothing ever again, so pretending
    /// otherwise would leave a live-looking terminal that has silently
    /// stopped.
    ///
    /// This replay is deliberately MARKERLESS: no `ReplayComplete` is
    /// emitted here, and none may be added (PLAN_M5.md item 2 draws the
    /// boundary, and a test pins it). The marker means "this attach's
    /// initial catch-up ended"; this catch-up happens mid-stream on an
    /// attachment that already finished one, so history reappearing on
    /// the channel after the marker is a documented possibility rather
    /// than a contradiction — see [`Self::send_replay_complete`].
    async fn catch_up_after_tmux_pause(&mut self) -> Result<(), ForwarderEnd> {
        info!(
            channel = self.channel,
            session = %self.session_id,
            "tmux paused the pane for this client; catching up by reset and replay"
        );
        let (modes, content) = match self.stream.resume_paused_with_replay().await {
            Ok(replay) => replay,
            Err(e) => return Err(ForwarderEnd::StreamFailed(format!("{e:#}"))),
        };
        self.send_replay(modes, content, true).await
    }
}

/// Resolve only once the attachment has been paused CONTINUOUSLY past
/// `timeout` — the stall detector every blocking await in a forwarder is
/// raced against.
///
/// Safe to create fresh at each `select!` site precisely because the
/// deadline is derived from the pause's stored START instant rather than
/// from now: re-creating it cannot restart the clock, so the hard maximum
/// survives however many chunks, phases, or wakeups a single pause spans.
/// (An earlier version timed from `Instant::now()` and had exactly that
/// bug — a client draining slowly enough to keep the forwarder awaiting,
/// but fast enough to keep it moving, was never detached.)
///
/// Resolving only on a CONTINUOUS pause is what keeps this a hard maximum
/// pause duration rather than a cumulative budget: a client that pauses
/// and resumes repeatedly is a slow client being served correctly, not a
/// stalled one, and each resume clears the stored start.
async fn stalled_past_deadline(
    mut pause_rx: watch::Receiver<Option<tokio::time::Instant>>,
    timeout: Duration,
) {
    loop {
        let paused_at = *pause_rx.borrow_and_update();
        let Some(paused_at) = paused_at else {
            if pause_rx.changed().await.is_err() {
                // The attachment is gone; this future must simply never
                // resolve, so its `select!` arm cannot fabricate a stall
                // out of a teardown that is already under way.
                return std::future::pending().await;
            }
            continue;
        };
        tokio::select! {
            () = tokio::time::sleep_until(paused_at + timeout) => return,
            changed = pause_rx.changed() => {
                if changed.is_err() {
                    return std::future::pending().await;
                }
            }
        }
    }
}

/// Tear down an attachment whose client stalled, on a task of its own.
///
/// Spawned rather than run inline because forwarders must never take
/// `Supervisor::attachments`: the takeover, detach, and delete paths all
/// abort AND AWAIT a forwarder while holding that mutex, so a forwarder
/// blocking on it would deadlock the supervisor outright. A separate task
/// can wait for the lock safely: it is not the task being awaited, so a
/// teardown holding the mutex can always make progress and release it.
/// (This does NOT assume the spawning forwarder has already returned — it
/// may still be unwinding when this runs. Nothing here depends on that:
/// the forwarder has already shut its control client down before spawning
/// this, and the `abort`-then-`await` below reaps the handle whenever it
/// finishes.)
///
/// The identity check is the same two-part one every other ownership
/// check in this module uses (channel plus owning connection): by the
/// time this runs, a takeover may already have installed a different
/// attachment for this terminal, and tearing THAT one down would detach
/// an innocent client.
///
/// Scoped to ONE terminal, deliberately (PLAN_M4.md item 3 settles this):
/// a client whose agent view is healthy but whose background tab wedged
/// loses only the tab, while a genuinely wedged client hits every
/// terminal's stall bound in turn and converges on a whole-client detach
/// on its own. Detaching the session's other terminals here would punish
/// exactly the terminal the user is looking at.
fn detach_stalled(
    sup: &Arc<Supervisor>,
    key: AttachmentKey,
    channel: u32,
    tx: mpsc::Sender<Frame>,
) {
    let sup = Arc::clone(sup);
    tokio::spawn(async move {
        let mut attachments = sup.attachments.lock().await;
        let mine = attachments
            .get(&key)
            .is_some_and(|a| a.channel == channel && a.notify.same_channel(&tx));
        if !mine {
            // A takeover (or a delete, or the connection dying) got here
            // first, so this stall belongs to an attachment that no longer
            // exists. Returning WITHOUT notifying is the point: the winner
            // is using the same channel id on the same connection, so a
            // stalled notice sent now would reach the new client and race
            // — or overtake — the truthful notice its own teardown path
            // already sent to the loser.
            return;
        }
        let removed = attachments.remove(&key);
        if let Some(old) = removed {
            // Abort-and-await like every other teardown, even though the
            // forwarder is the very task that asked for this: it has
            // already shut its control client down and returned, so this
            // only reaps the handle. Dropping the removed `ActiveAttach`
            // is also what kills the input client.
            old.forwarder.abort();
            let _ = old.forwarder.await;
        }
        drop(attachments);
        let _ = tx
            .send(Frame::control(&ControlMsg::Detached {
                channel,
                reason: DETACH_REASON_STALLED.to_string(),
            }))
            .await;
    });
}

/// Apply a client's `PauseOutput`/`ResumeOutput` to whichever attachment
/// it owns, or ignore it.
///
/// The ownership check is deliberately the SAME shape as the `Resize`
/// arm's — see that arm's comment for the TOCTOU argument in full: both
/// halves matter, because channel ids are unique only within a
/// connection, so `same_channel` is what identifies the owning
/// connection and the channel id is what tells apart clients multiplexed
/// over one connection (every browser tab rides the helm's single
/// supervisor connection). The check and the state change run under one
/// lock hold for the same reason too, though the stakes are lower here
/// than for input or resize: the worst a stale pause could do is silence
/// a terminal the sender no longer owns until its real owner resumes it.
///
/// The lookup is by channel rather than by session id because
/// `PauseOutput` carries no session id — unlike `Resize`, which needs one
/// to reach tmux. Mirrors the `Detach` arm's own search for the same
/// reason.
///
/// A pause records WHEN it started and a resume clears it, and a pause
/// arriving while already paused changes nothing at all. That last part is
/// load-bearing rather than tidy: the forwarder's hard maximum is measured
/// from this stored instant (see [`stalled_past_deadline`]), so letting a
/// repeated `PauseOutput` overwrite it would let a client hold an
/// attachment open forever simply by re-sending pause. `send_if_modified`
/// then also suppresses the pointless wakeup.
pub(crate) async fn set_attachment_paused(
    sup: &Arc<Supervisor>,
    tx: &mpsc::Sender<Frame>,
    channel: u32,
    paused: bool,
) {
    let attachments = sup.attachments.lock().await;
    if let Some(attachment) = attachments
        .values()
        .find(|a| a.channel == channel && a.notify.same_channel(tx))
    {
        apply_pause_transition(&attachment.pause, paused);
    }
}

/// The transition rule itself, pulled out of [`set_attachment_paused`] so it
/// has a unit-test surface that needs no `Supervisor`, no attachment-map
/// lookup, and no connection — see this module's
/// `repeated_pause_spam_never_moves_the_stall_anchor` test, which drives
/// this function and [`stalled_past_deadline`] directly against a bare
/// `watch` channel under a paused clock.
///
/// Keeping the ORIGINAL start instant on a repeat of the current state is
/// the one property that matters: see [`set_attachment_paused`]'s docs for
/// why moving it would let a client hold an attachment open forever.
fn apply_pause_transition(pause: &watch::Sender<Option<tokio::time::Instant>>, paused: bool) {
    pause.send_if_modified(|current| match (paused, *current) {
        // Already paused: keep the ORIGINAL start instant.
        (true, Some(_)) => false,
        (true, None) => {
            *current = Some(tokio::time::Instant::now());
            true
        }
        (false, None) => false,
        (false, Some(_)) => {
            *current = None;
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::super::core::tests::{StateDir, dummy_exe, no_uploads};
    use super::*;
    use farhelm_proto::{RestartOffer, SessionInfo, SessionStatus};

    /// The defusal this whole change exists for: an oversized reply must
    /// never reach the writer task, because the writer's fatal-on-any-
    /// write-error handling would tear down the shared connection (and
    /// every attachment on it) over what should have been a single
    /// request's failure. Assert the substitute frame decodes as an
    /// `Error` correlated to the same `req_id` the caller was waiting on,
    /// and that its message actually names the problem (both the size
    /// figure and the fact that it exceeds the limit) rather than being an
    /// opaque placeholder string.
    ///
    /// `ListSessions` is the one M1 reply built entirely from unbounded
    /// caller-controlled data (session titles), uncapped until M2's list
    /// budget — so it is the realistic way a control reply exceeds
    /// `MAX_FRAME_LEN`. One oversized title is enough to clear the cap
    /// (JSON escaping plus the frame header add overhead on top of the
    /// title itself), so a single `SessionInfo` suffices here.
    #[test]
    fn reply_frame_substitutes_error_for_oversized_reply() {
        let req_id = 42;
        let oversized = ControlMsg::SessionList {
            req_id,
            sessions: vec![SessionInfo {
                id: "s1".to_string(),
                title: "x".repeat(farhelm_proto::MAX_FRAME_LEN as usize),
                created_at: 1_700_000_000,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Alive,
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
            }],
            total: 1,
            next_cursor: None,
        };
        assert!(
            Frame::control(&oversized).exceeds_max_len(),
            "test fixture must actually exceed MAX_FRAME_LEN"
        );

        let frame = reply_frame(&oversized);
        assert!(!frame.exceeds_max_len(), "substituted reply must fit");
        let decoded: ControlMsg = serde_json::from_slice(&frame.body).unwrap();
        let ControlMsg::Error {
            req_id: got_req_id,
            message,
            kind,
        } = decoded
        else {
            panic!("expected ControlMsg::Error, got {decoded:?}");
        };
        assert_eq!(got_req_id, req_id);
        assert!(
            message.contains(&farhelm_proto::MAX_FRAME_LEN.to_string()),
            "error message must name the limit that was exceeded: {message}"
        );
        assert!(
            message.contains("exceeding"),
            "error message must describe the problem concretely: {message}"
        );
        assert_eq!(
            kind,
            ErrorKind::Internal,
            "the reply was too big for the wire, not something the caller's request got wrong"
        );
    }

    /// The common case: a reply that fits comes back byte-identical to
    /// what `Frame::control` alone would produce. `reply_frame` always
    /// serializes and size-checks the message — there is no cheaper path
    /// that skips that for the common case — but the *result* for a
    /// fitting reply must be indistinguishable from the unwrapped frame.
    #[test]
    fn reply_frame_passes_through_normal_reply_unchanged() {
        let msg = ControlMsg::SessionCreated {
            req_id: 7,
            session: SessionInfo {
                id: "s1".to_string(),
                title: "demo".to_string(),
                created_at: 1_700_000_000,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                // Matches real `create_session` output: `Unknown`, not
                // `Alive` (see that function's own doc comment).
                status: SessionStatus::Unknown,
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
            },
        };
        assert_eq!(reply_frame(&msg), Frame::control(&msg));
    }

    /// `SessionRestarted` joined `reply_frame`'s req_id correlator alongside
    /// the helm demux (PLAN_M3 review batch item 5): this is the
    /// `unreachable!`-on-unknown-variant match, so proving it accepts the
    /// new variant here — rather than only via the round-trip tests in
    /// farhelm-proto — is what would catch a future refactor that forgets
    /// this arm and reintroduces the panic for a message the
    /// `RestartSession` handler sends on every successful restart.
    #[test]
    fn reply_frame_accepts_session_restarted() {
        let msg = ControlMsg::SessionRestarted {
            req_id: 9,
            session: SessionInfo {
                id: "s1".to_string(),
                title: "demo".to_string(),
                created_at: 1_700_000_000,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Alive,
                annotation: None,
                restart_offer: RestartOffer::Resume,
                tabs: Vec::new(),
            },
        };
        assert_eq!(reply_frame(&msg), Frame::control(&msg));
    }

    /// PLAN_M4.md item 1's four tab/upload replies (`TabOpened`,
    /// `TabClosed`, `UploadStarted`, `UploadCommitted`) joined
    /// `reply_frame`'s req_id correlator alongside `SessionRestarted`
    /// above, and for the identical reason: each is a req_id-bearing
    /// reply that a not-yet-implemented handler (PLAN_M4.md step 4) will
    /// eventually build and pass through `reply_frame`, and without an
    /// arm here that call hits the `unreachable!` branch instead of ever
    /// reaching the wire. One table-driven test covers all four rather
    /// than four near-identical copies of
    /// `reply_frame_accepts_session_restarted`, since none of them nests
    /// a `SessionInfo` the way that one does.
    #[test]
    fn reply_frame_accepts_the_tab_and_upload_replies() {
        for msg in [
            ControlMsg::TabOpened {
                req_id: 10,
                tab: farhelm_proto::TabInfo {
                    id: "t1".to_string(),
                },
            },
            ControlMsg::TabClosed { req_id: 11 },
            ControlMsg::UploadStarted {
                req_id: 12,
                channel: 3,
            },
            ControlMsg::UploadCommitted {
                req_id: 13,
                path: "/tmp/a.txt".to_string(),
            },
        ] {
            assert_eq!(reply_frame(&msg), Frame::control(&msg));
        }
    }

    /// `reply_frame` panics on a message with no `req_id` to correlate an
    /// oversize substitution against. This is pinning an explicit
    /// invariant, not documenting an accident: a silent fallback here
    /// (inventing a `req_id`, or sending the oversized reply unchecked)
    /// would recreate the exact oversize-reaches-the-writer bug this
    /// function exists to close, just for a different message shape. If a
    /// future caller ever routes an event message (like `Detached`)
    /// through `reply_frame`, this test is what catches it.
    #[test]
    #[should_panic(expected = "carries no req_id")]
    fn reply_frame_panics_on_message_without_req_id() {
        reply_frame(&ControlMsg::Detached {
            channel: 1,
            reason: "x".into(),
        });
    }

    /// Steady-state contract for `reap_finished_tasks` (its own docs): a
    /// connection that stays open and keeps issuing requests must not
    /// accumulate one unreaped `JoinSet` entry per request. Drives many
    /// `ListSessions` requests through the REAL `handle_control` dispatch
    /// — the same spawn/admission path production uses, not a synthetic
    /// `JoinSet` fixture — waiting for each reply before reaping
    /// (mirroring one iteration of `handle_connection`'s read loop: read,
    /// dispatch, reap) and asserting the tracked task count returns to
    /// ZERO every time, rather than growing with the number of requests
    /// issued so far.
    #[tokio::test]
    async fn steady_state_reaping_keeps_the_tracked_task_count_bounded() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches only tmux, not the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut input_routes = HashMap::new();
        let mut tasks = tokio::task::JoinSet::new();

        const REQUESTS: u64 = 50;
        for req_id in 0..REQUESTS {
            handle_control(
                &sup,
                ControlMsg::ListSessions {
                    req_id,
                    cursor: None,
                    limit: None,
                },
                &tx,
                &tx,
                &mut input_routes,
                &mut no_uploads(),
                &mut tasks,
            )
            .await;
            let reply = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                .await
                .expect("spawned ListSessions handler never replied")
                .expect("reply channel closed before a reply arrived");
            let decoded: ControlMsg = serde_json::from_slice(&reply.body).unwrap();
            assert!(
                matches!(decoded, ControlMsg::SessionList { .. }),
                "expected ControlMsg::SessionList, got {decoded:?}"
            );

            reap_finished_tasks(&mut tasks);
            assert_eq!(
                tasks.len(),
                0,
                "the tracked task count must return to zero after reaping (request {req_id} \
                 of {REQUESTS}), not grow with the number of requests issued so far"
            );
        }
    }

    /// `spawn_admitted`'s entire contract (see its own docs): the permit
    /// is acquired in the CALLER's own await, before the task is ever
    /// spawned — not inside the spawned future. Proven with a 2-permit
    /// semaphore and manually-controlled tasks (a `Notify`, not real
    /// timing) rather than routing through real `StopSession`/tmux, which
    /// would make "has the Nth task started running yet" unobservable
    /// without racing real kill-sweep durations — exactly the flakiness
    /// this test is designed to avoid.
    ///
    /// A regression that moved `acquire_owned().await` to INSIDE the
    /// spawned task (permit acquired AFTER `tasks.spawn`, not before)
    /// would make the third `spawn_admitted` call below return
    /// immediately regardless of how many permits are free, since nothing
    /// would then block spawning it — the bounded-timeout assertion in
    /// the middle of this test is exactly what catches that.
    #[tokio::test]
    async fn spawn_admitted_acquires_the_permit_before_spawning_not_inside_the_task() {
        let admission = Arc::new(tokio::sync::Semaphore::new(2));
        let mut tasks = tokio::task::JoinSet::new();
        let release = Arc::new(tokio::sync::Notify::new());
        let started = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Claim both permits with two tasks that run and then block on
        // `release`, so they stay "in flight" (holding their permits)
        // until this test lets them go.
        for _ in 0..2 {
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            spawn_admitted(&admission, &mut tasks, async move {
                started.fetch_add(1, Ordering::SeqCst);
                release.notified().await;
            })
            .await;
        }
        // `tasks.spawn` only SCHEDULES the task; it does not run until
        // this task yields to the executor. Neither does
        // `spawn_admitted`'s own `acquire_owned().await` necessarily
        // yield — an uncontended semaphore can resolve without ever
        // suspending. `yield_now` is what actually lets the two spawned
        // tasks run up to their own first await point (`notified()`).
        tokio::task::yield_now().await;
        assert_eq!(
            started.load(Ordering::SeqCst),
            2,
            "both permits must have been claimed by two running tasks"
        );

        // A third admission call, with both permits still held, must not
        // resolve at all yet. Scoped in its own block: `tokio::pin!`'s
        // hidden storage borrows `tasks` for the rest of ITS enclosing
        // scope regardless of when the `Pin<&mut _>` handle itself is
        // dropped, so the block boundary — not a manual `drop` — is what
        // releases that borrow before `tasks` is touched again below.
        {
            let started3 = Arc::clone(&started);
            let release3 = Arc::clone(&release);
            let third = spawn_admitted(&admission, &mut tasks, async move {
                started3.fetch_add(1, Ordering::SeqCst);
                release3.notified().await;
            });
            tokio::pin!(third);
            assert!(
                tokio::time::timeout(Duration::from_millis(50), &mut third)
                    .await
                    .is_err(),
                "spawn_admitted must block acquiring a permit while both are held, not spawn \
                 (and run) immediately"
            );
            assert_eq!(
                started.load(Ordering::SeqCst),
                2,
                "the third task must not have started running while its spawn_admitted call \
                 is still blocked on admission"
            );

            // Free exactly one permit: wakes one of the first two tasks,
            // which finishes and drops its permit, which is what lets the
            // third admission proceed.
            release.notify_one();
            tokio::time::timeout(Duration::from_secs(5), &mut third)
                .await
                .expect("spawn_admitted must proceed once a permit frees");
            // `third` resolving only proves the permit was acquired and
            // the task was handed to `tasks.spawn` — not that the newly
            // spawned task has been polled yet (the same
            // spawn-schedules-but-does-not-run distinction as the first
            // `yield_now` above).
            tokio::task::yield_now().await;
            assert_eq!(started.load(Ordering::SeqCst), 3);
        }

        // Let every remaining task finish and reap them all.
        release.notify_waiters();
        while tasks.join_next().await.is_some() {}
    }

    use std::sync::atomic::AtomicBool;

    /// A guard whose `Drop` flips a sentinel, used below to distinguish a
    /// task that was genuinely aborted-and-polled-to-completion from one
    /// merely abandoned. A tokio task's future is dropped only once
    /// cancellation has actually been delivered and polled, so observing
    /// the sentinel is proof of that, not just proof that `abort()` was
    /// called (which alone only schedules cancellation).
    struct DropSignal(Arc<AtomicBool>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    /// Pins the abort-then-await ordering in `drain_writer`'s no-progress
    /// path: a writer landing zero frames across one whole window must be
    /// aborted — and that abort actually awaited to completion — before
    /// the helper returns, not merely timed out and abandoned.
    ///
    /// The task under test never completes on its own
    /// (`std::future::pending`) and holds a `DropSignal` guard; the
    /// sentinel firing is the only way to know the task's future was
    /// truly dropped, which only happens once its cancellation has been
    /// delivered and polled to completion. The outer 5s timeout catches
    /// the regression this test exists to prevent: a helper whose abort
    /// path fires `abort()` without awaiting the handle, or that never
    /// reaches the abort branch at all, would leave `drain_writer` — and
    /// this test — hanging.
    ///
    /// `start_paused` puts the test on tokio's virtual clock: real time
    /// never elapses, so the window boundary is crossed deterministically
    /// at the next await point instead of racing a wall-clock sleep
    /// against scheduler jitter.
    #[tokio::test(start_paused = true)]
    async fn drain_writer_aborts_and_awaits_on_sustained_no_progress() {
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = DropSignal(Arc::clone(&dropped));
        let mut task = tokio::spawn(async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        });
        let frames_written = AtomicU64::new(0);

        tokio::time::timeout(
            Duration::from_secs(5),
            drain_writer(&mut task, &frames_written, Duration::from_millis(50)),
        )
        .await
        .expect("drain_writer must not hang waiting on a task making zero progress");

        assert!(
            dropped.load(Ordering::SeqCst),
            "writer task must have been aborted, and that abort awaited to \
             completion, before drain_writer returns"
        );
    }

    /// The whole reason `drain_writer` exists: a writer that is merely
    /// slow — landing a frame every window instead of finishing inside a
    /// single one — must be allowed to keep going rather than being
    /// aborted the moment one window elapses. This is what tells
    /// "backpressured but alive" apart from "gone" (the case the sibling
    /// test above covers).
    ///
    /// The task increments `frames_written` on a cadence shorter than the
    /// drain window, several times over, before completing — so
    /// `drain_writer` has to ride out multiple windows on renewed
    /// progress and only return once the task finishes naturally. The
    /// `completed` flag is set exclusively on that natural-completion
    /// path (the abort path in `drain_writer` never gets to run it), so
    /// seeing it true is proof the return came from completion, not from
    /// `drain_writer` giving up and aborting the task after the fact.
    ///
    /// Also on `start_paused`'s virtual clock, for the same reason as the
    /// sibling test: the multi-window wait this test exercises would
    /// otherwise depend on real sleeps landing inside real window
    /// boundaries, which is exactly the kind of timing assumption that is
    /// fine on a fast idle machine and flaky under load.
    #[tokio::test(start_paused = true)]

    async fn drain_writer_waits_through_progress_and_returns_on_completion() {
        let frames_written = Arc::new(AtomicU64::new(0));
        let completed = Arc::new(AtomicBool::new(false));
        let writer_counter = Arc::clone(&frames_written);
        let completed_flag = Arc::clone(&completed);
        let mut task = tokio::spawn(async move {
            for _ in 0..10 {
                tokio::time::sleep(Duration::from_millis(20)).await;
                writer_counter.fetch_add(1, Ordering::Relaxed);
            }
            completed_flag.store(true, Ordering::SeqCst);
        });

        tokio::time::timeout(
            Duration::from_secs(5),
            drain_writer(&mut task, &frames_written, Duration::from_millis(60)),
        )
        .await
        .expect("drain_writer must return once the task completes naturally");

        assert!(
            completed.load(Ordering::SeqCst),
            "task must have run to natural completion, not been aborted \
             mid-flight while progress was still arriving"
        );
    }

    /// A repeated `PauseOutput` must never move the stored pause anchor,
    /// and [`stalled_past_deadline`] must fire measured from the FIRST
    /// pause — not the most recent spam — within one 50ms virtual-time step
    /// of the exact deadline (the loop's advance granularity, not wall-clock
    /// slack: virtual time cannot drift, so the bound is one tick wide).
    ///
    /// This is the precise version of the property the e2e suite's
    /// `terminal_backpressure::a_paused_replay_detaches_relative_to_the_
    /// first_pause_despite_pause_spam` test can only bound loosely: that
    /// test measures real wall-clock elapsed time against a fire-and-
    /// forget `PauseOutput` send, so delivery and teardown latency eat
    /// into whatever slack its ceiling allows, and an implementation whose
    /// anchor drifts by less than that slack would still pass it. Here,
    /// under a paused clock, "measured from the first pause" and "measured
    /// from anything else" resolve at two DIFFERENT virtual instants, so
    /// any wrong anchor misses the assertion below deterministically.
    ///
    /// No real process or I/O is awaited anywhere in this test — see
    /// `terminals.rs`'s `a_healthy_sink_run_resets_the_respawn_backoff` for
    /// why that matters under `start_paused`: an awaited real-world event
    /// parks the runtime with nothing else ready, and tokio's time driver
    /// is then free to auto-advance the frozen clock straight through the
    /// very window being measured. Everything exercised here — a `watch`
    /// channel and [`stalled_past_deadline`]'s own virtual sleep — is a
    /// timer the paused-clock driver tracks explicitly, so `advance` only
    /// ever moves exactly as far as this test tells it to.
    #[tokio::test(start_paused = true)]
    async fn repeated_pause_spam_never_moves_the_stall_anchor() {
        let stall_timeout = Duration::from_secs(3);
        // Shorter than the timeout, mirroring a client that re-sends
        // `PauseOutput` on some cadence well inside the maximum it is
        // trying (and failing) to outlast.
        let spam_period = Duration::from_millis(300);
        let spam_count = 5u32;

        let (pause_tx, pause_rx) = watch::channel(None::<tokio::time::Instant>);
        apply_pause_transition(&pause_tx, true);
        let anchor = pause_rx
            .borrow()
            .expect("the first pause must record an anchor");

        // Spam the SAME (paused, already paused) transition repeatedly,
        // with virtual time elapsing between each — the anchor must not
        // move on any of them.
        for _ in 0..spam_count {
            tokio::time::advance(spam_period).await;
            apply_pause_transition(&pause_tx, true);
            assert_eq!(
                *pause_rx.borrow(),
                Some(anchor),
                "a repeated PauseOutput must never move the stored pause anchor"
            );
        }

        let task = tokio::spawn(stalled_past_deadline(pause_rx.clone(), stall_timeout));
        tokio::task::yield_now().await;
        assert!(
            !task.is_finished(),
            "stalled_past_deadline fired before its deadline"
        );

        // Advance to just past the TRUE deadline, measured from the FIRST
        // pause (`anchor + stall_timeout`) rather than from the most
        // recent spam (`spam_count * spam_period` after it). A wrong-anchor
        // implementation anchored to the latest spam would still be
        // asleep here, since its own deadline lands `spam_count *
        // spam_period` later than this one.
        let elapsed_during_spam = spam_period * spam_count;
        let remaining = stall_timeout.saturating_sub(elapsed_during_spam);
        tokio::time::advance(remaining + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        assert!(
            task.is_finished(),
            "stalled_past_deadline did not fire within one tick of anchor + {stall_timeout:?} — \
             the anchor is being computed from something other than the first pause"
        );
        task.await.expect("stalled_past_deadline must not panic");
    }

    /// The lock-held detach notice must never block, and must never be
    /// dropped, even against a queue that is completely full.
    ///
    /// Both halves are load-bearing and they pull in opposite directions.
    /// Every teardown path calls this while holding the supervisor-wide
    /// `attachments` mutex, so an awaiting send would let one wedged peer
    /// freeze every session's attach, input, and delete — a
    /// supervisor-wide deadlock introduced by the very bound meant to
    /// prevent unbounded growth. But a notice that is simply discarded
    /// leaves a client with a terminal that silently stopped.
    ///
    /// The `let () =` binding is the mutation guard for the first half:
    /// it fails to COMPILE if `notify_detached` ever becomes async or
    /// otherwise starts returning a future.
    #[tokio::test]
    async fn notify_detached_never_blocks_on_a_full_queue_and_never_drops_the_notice() {
        let (tx, mut rx) = mpsc::channel::<Frame>(1);
        tx.send(Frame::data(1, b"occupying the only slot".to_vec()))
            .await
            .expect("channel is open");

        let () = notify_detached(&tx, 7, "stalled".to_string());

        // The queue was full, so the notice had to be deferred — but it
        // must arrive once capacity appears.
        let first = rx.recv().await.expect("the pre-existing frame");
        assert_eq!(first.kind, farhelm_proto::FrameKind::Data);
        let notice = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("the deferred detach notice never arrived")
            .expect("channel is still open");
        assert!(
            matches!(
                parse_control(&notice).expect("valid control frame"),
                ControlMsg::Detached { channel: 7, reason } if reason == "stalled"
            ),
            "the deferred notice must be this channel's detach, unchanged"
        );
    }
}
