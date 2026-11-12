//! The per-connection read loop, and what every attachment's output task
//! shares: its end verdicts, pause handling and stall detection.
//!
//! `handle_connection` is the whole lifetime of one client connection: it
//! owns terminal, upload, download and raw TCP channel routes, spawns
//! (and, on the way out, drains) the slow request handlers admitted
//! through `spawn_admitted`, and reads control/data frames until the
//! peer goes away. The output side of an attachment is its grid sender
//! (`super::grid_stream`), which reports how it ended through
//! [`report_forwarder_end`].

use super::capture::note_first_input;
use super::channels::ConnectionChannels;
use super::core::{Supervisor, error_kind};
use super::handlers::{handle_control, handle_restricted_control};
use super::terminals::{ActiveAttach, AttachmentKey, TerminalId, join_forwarder};
use super::uploads::{UPLOAD_PRIORITY_QUEUE, UploadCommand, UploadRoute, UploadSignal};
use anyhow::Context;
use farhelm_proto::io::{
    FrameReader, FrameWriter, ProgressWrite, handshake_with_host_identity, parse_control,
    write_frame_before_stall,
};
use farhelm_proto::{ControlMsg, DETACH_REASON_STALLED, DetachCode, ErrorKind, Frame};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{debug, warn};

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
/// What the number buys: a bound at all, expressed in the only unit `mpsc`
/// offers. It counts FRAMES, and frame sizes vary by two orders of magnitude,
/// so it is a ceiling rather than a size. Terminal data frames are grid
/// messages: history goes out in messages of about 32 KiB, which puts a
/// history-only backlog near 2 MiB per connection, and no grid message exceeds
/// `farhelm_proto::grid::GRID_MAX_MESSAGE` (1 MiB), which puts the paper worst
/// case for data at 64 MiB; live updates are usually far smaller, so the
/// typical backlog is a fraction of that. Control frames are capped only by
/// `MAX_FRAME_LEN`, and large replies CAN queue back to back: `ListSessions`
/// handlers are spawned tasks (up to `LIST_ADMISSION_PERMITS` of them building
/// concurrently), and a helm's refresh drain, a live detail read, and agent
/// verbs can each enqueue a whole-list reply on one connection. The paper worst
/// case is therefore this bound times `MAX_FRAME_LEN` — half a gigabyte — and
/// nothing structural here bounds it lower; what keeps the realistic case small
/// is that the peer is the single helm, which sends one refresh drain at a time
/// per host and a handful of agent requests, and that the session-list cap
/// (`farhelm_proto::LIST_SESSIONS_CAP`) keeps each such reply small for any
/// ordinary fleet. Bounding by count is kept anyway as the one legible rule,
/// with that worst case stated rather than denied; a byte-accounting scheme
/// layered over the frame layer is the upgrade if it ever stops being
/// acceptable.
///
/// The accepted consequence, stated plainly: when the single multiplexed
/// consumer (the helm) is slow, control REPLIES queue behind terminal DATA at
/// this bound, so a request can wait on a busy terminal's backlog. That is
/// acceptable because the alternative — unbounded — is exactly the debt being
/// closed, and because a client that reaches this bound repeatedly is one the
/// helm's own pause (`PauseOutput`, sent from its queue for that attachment)
/// should already be holding back. Nothing that holds a supervisor mutex may
/// block on this queue; see [`notify_detached`] for the one shape that would
/// otherwise want to, and the `Attach` handler's reserved permit for the other.
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
/// its behalf, and its own multi-second stop or sweep work is exactly why it gets
/// a separate, longer budget rather than being folded into this one.
const WRITER_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// How long `handle_connection`'s shutdown tail waits for spawned slow-
/// handler tasks (see `HANDLER_ADMISSION_PERMITS`) to finish on their own,
/// tracked in a `JoinSet`, before aborting whatever remains and logging
/// it. Generous — `kill_process_tree`'s own sequence (grace period,
/// quiesce passes, kill confirmation) can legitimately take several
/// seconds — but not unbounded: a wedged host or sweep must not leak a task (and
/// this connection's own shutdown) forever. See `WRITER_DRAIN_TIMEOUT`'s
/// docs for how the two windows interact.
const HANDLER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// The connection-local state a control message may reach, bundled into
/// one value.
///
/// Passed to `handle_control` BY VALUE, not by `&mut`: it is a bundle of
/// borrows, so moving it costs nothing, and every caller builds it as a
/// single-use temporary at the call site. By value the dispatch can also
/// move each field straight into the arm that needs it, rather than
/// reborrowing out of a reference for no reason.
///
/// Everything here belongs to ONE connection and to no other: the two
/// reply queues that identify it, the channel table whose numbering is
/// per-connection, and the `JoinSet` its slow handlers are
/// tracked in. None of it is shared with the `Supervisor`, which is
/// exactly the line this struct draws — `handle_control` takes the
/// supervisor and this, and nothing else it could confuse for either.
///
/// Named queues matter even with one channel table: the two
/// `mpsc::Sender<Frame>`s are not interchangeable, but a positional
/// transposition would compile. `priority: &priority_tx` at the construction
/// site makes the distinction checkable on sight.
///
/// Deliberately NOT passed down to the per-message handlers. Each of them
/// still takes exactly the pieces its message can touch, which is what
/// makes "`ListSessions` never touches channel routes" a fact the
/// signature enforces rather than a comment to be trusted.
pub(crate) struct ConnectionCtx<'a> {
    /// This connection's ordinary reply queue, which also serves as its
    /// identity: `same_channel` against it is how a handler tells "the
    /// connection that owns this attachment" from any other.
    pub(crate) tx: &'a mpsc::Sender<Frame>,
    /// This connection's prioritized queue, for the upload family alone —
    /// see `uploads::send_upload_unless_cancelled`.
    pub(crate) priority: &'a mpsc::Sender<Frame>,
    /// Routes and retained reservations share one allocation policy.
    pub(crate) channels: &'a mut ConnectionChannels,
    pub(crate) tasks: &'a mut tokio::task::JoinSet<()>,
    /// Only newly detached work whose old inline panic unwound this reader
    /// uses this handoff. Its waiter preserves that panic policy rather than
    /// inventing a protocol Error or letting the common task reaper hide it.
    pub(crate) panics: &'a mpsc::Sender<Box<dyn std::any::Any + Send + 'static>>,
}

/// Serve one admitted protocol connection.
///
/// Generic over the byte stream so tests can drive it over an in-process
/// duplex pipe with the same code path production uses over the unix
/// socket. An auth-bearing hello enters the restricted session dispatcher;
/// an ordinary local peer keeps the full-authority path.
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
    // The one production caller of `handshake_with_host_identity` (see its
    // own docs): every hello this supervisor sends carries the identity
    // `Supervisor::new_with_seams` resolved once at construction — `None`
    // for a claimless construction with nothing yet to report, `Some` once
    // this process minted or read back a durable one — never a fresh read
    // per connection; the value is immutable for the process lifetime, so
    // cloning it here is cheap and there is nothing to race.
    let peer_hello = handshake_with_host_identity(
        &mut reader,
        &mut writer,
        sup.host_identity.clone(),
        crate::store::SCHEMA_VERSION,
    )
    .await?;
    // Read before `peer_hello` is consumed below. Meaningful only on a
    // full-authority connection; see where the link is registered.
    let declares_owning_helm = matches!(
        peer_hello,
        ControlMsg::Hello {
            owning_helm: true,
            ..
        }
    );
    let restricted_auth = match peer_hello {
        ControlMsg::Hello {
            auth: Some(auth), ..
        } => {
            let authenticated = sup
                .store
                .authenticates_session(&auth.session_id, &auth.token)
                .await
                .context("validating session authentication");
            match authenticated {
                Ok(true) => Some(auth),
                Ok(false) => {
                    const MESSAGE: &str =
                        "the session credential is invalid or its session no longer exists";
                    writer
                        .write_control(&ControlMsg::Error {
                            create_unaccepted: false,
                            req_id: 0,
                            message: MESSAGE.to_string(),
                            kind: ErrorKind::Unauthorized,
                        })
                        .await?;
                    anyhow::bail!(MESSAGE);
                }
                Err(error) => {
                    writer
                        .write_control(&ControlMsg::Error {
                            create_unaccepted: false,
                            req_id: 0,
                            message: "the supervisor could not validate the session credential"
                                .to_string(),
                            kind: ErrorKind::Internal,
                        })
                        .await?;
                    return Err(error);
                }
            }
        }
        ControlMsg::Hello { auth: None, .. } => None,
        _ => unreachable!("the handshake accepts only a peer hello"),
    };

    // Single writer task; everything that wants to send (request
    // handlers, the output forwarder, takeover notifications) goes
    // through this queue so frames never interleave mid-write. Bounded
    // since M2.5 — see CONNECTION_WRITER_QUEUE for what the bound buys
    // and what it costs.
    let (tx, rx) = mpsc::channel::<Frame>(CONNECTION_WRITER_QUEUE);
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
    // Download frames drain only after terminal/control traffic. A window
    // bounds bytes, and this shared queue also bounds frames across transfers.
    let (bulk_tx, bulk_rx) = mpsc::channel::<Frame>(super::uploads::UPLOAD_CHUNK_QUEUE);
    let (priority_tx, priority_rx) = mpsc::channel::<Frame>(UPLOAD_PRIORITY_QUEUE);
    // Only the owning helm's connections get an upward link, and with it
    // agent upcalls and change hints: a full-authority connection whose
    // hello declared itself the owning helm (`ControlMsg::Hello::owning_helm`).
    // A session-authenticated connection never does, whatever its hello
    // claims, because it is a session's agent and not a helm. A
    // full-authority connection that did not declare (the provisioning and
    // update probe, another supervisor's startup check, a test client) gets
    // none either: it sends the same diagnostic `role` as the helm and
    // answers nothing, so handing it an agent request would leave the
    // asking agent waiting out the whole budget. The hello's `role` string
    // decides nothing here; it is diagnostic free text (see
    // `ControlMsg::Hello::role`).
    //
    // Registering here, before this connection reads any request, is what
    // guarantees that a session the helm created over this connection can
    // ask the helm straight away. Which registered link gets a request is
    // `Supervisor::owning_helm_link`'s rule (the newest); what the asking
    // session's terminal is attached to plays no part.
    //
    // The link is also allowed to END this connection (see
    // `HelmLink::retire`): a relay that has provably stopped working — a
    // reply under a `req_id` nobody issued, a mutation's delete fence that
    // ran out its last bound — leaves pending upcalls that nothing on a
    // LIVE connection will resolve, and connection loss is the ending the
    // rest of the relay already knows how to report. The sender is created
    // here, and kept in scope for the whole function even on a connection
    // that registers no link, so the `changed()` arm below never fires
    // merely because the last sender was dropped.
    let (link_shutdown, mut link_shutdown_rx) = watch::channel(false);
    let link = if restricted_auth.is_none() && declares_owning_helm {
        Some(
            sup.register_helm_link(tx.clone(), link_shutdown.clone())
                .await,
        )
    } else {
        None
    };
    let (writer_failed_tx, mut writer_failed_rx) = oneshot::channel();
    // The shutdown tail owns the sender. It closes both receiver halves
    // after the last reply-producing task has been reaped, so the writer can
    // finish queued frames without waiting for unrelated sender clones to
    // disappear.
    let (writer_close_tx, writer_close_rx) = oneshot::channel();
    // Progress counter for the shutdown-tail drain: `drain_writer` reads
    // this to tell "peer merely slow" apart from "peer gone" instead of
    // enforcing one flat deadline. Relaxed is enough on both ends — this
    // is a liveness heartbeat, not a value anything is synchronized on.
    let frames_written = Arc::new(AtomicU64::new(0));
    let frames_written_for_writer = Arc::clone(&frames_written);
    let writer_stall = sup.timeouts.writer_stall;
    let mut writer_task = spawn_writer_task(
        writer,
        WriterTaskChannels {
            ordinary_rx: rx,
            priority_rx,
            bulk_rx,
            close_rx: writer_close_rx,
            failed_tx: writer_failed_tx,
        },
        WriterTaskProgress {
            bytes_written,
            frames_written: frames_written_for_writer,
            stall_window: writer_stall,
        },
    );

    // Channel numbers are meaningful only within this connection. The table
    // owns both their routes and any history that must outlive those routes.
    let mut channels = ConnectionChannels::default();

    // Tracking (not admission — that is now `sup.admission`, shared
    // across every connection this supervisor serves; see its own docs
    // for why it must NOT be per-connection; the session list has its own
    // `sup.list_admission`) for the slow handlers
    // (`ListSessions`/`StopSession`/`DeleteSession`) that `handle_control`
    // spawns instead of awaiting inline. `HANDLER_SHUTDOWN_TIMEOUT`'s own
    // docs cover why leaving these untracked is not safe: without a
    // `JoinSet`, this function's shutdown tail would have nothing to wait
    // on or clean up after.
    let mut tasks: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();

    // One payload is enough: the reader can resume only its first pending
    // unwind. Keep the sender here so an empty channel never becomes a new
    // shutdown event when no owned request currently has a waiter.
    let (panics, mut pending_panic) = mpsc::channel(1);

    let result: anyhow::Result<()> = async {
        loop {
            // Reap whatever spawned handler tasks have already finished
            // before touching the next frame — see `reap_finished_tasks`'s
            // own docs for why a long-lived connection needs this every
            // iteration, not just at shutdown.
            reap_finished_tasks(&mut tasks);
            channels.prune_finished();
            // Either half failing ends the connection. Waiting only for
            // read EOF leaks attachments when a half-broken peer keeps
            // writing after it has stopped reading our replies.
            let frame = tokio::select! {
                frame = reader.read_frame() => frame?,
                payload = pending_panic.recv() => {
                    std::panic::resume_unwind(payload.expect(
                        "the connection retains its own panic sender",
                    ));
                }
                error = &mut writer_failed_rx => {
                    let detail = error.unwrap_or_else(|_| "writer task exited".to_string());
                    return Err(anyhow::anyhow!("frame write to client failed: {detail}"));
                }
                // The agent relay asking for this connection to end. It has
                // already failed its own pending upcalls and logged why
                // (`HelmLink::retire`); what is left is the teardown of
                // everything else this connection carries, which is exactly
                // what returning an error from this block runs.
                changed = link_shutdown_rx.changed() => {
                    changed.expect("this function holds a sender for its own link shutdown");
                    return Err(anyhow::anyhow!(
                        "the agent relay retired this connection's upward link"
                    ));
                }
            };
            let Some(frame) = frame else {
                break;
            };
            match frame.kind {
                farhelm_proto::FrameKind::Data if channels.relay_reserved(frame.channel) => {
                    if let Some(route) = channels.relay(frame.channel) {
                        route.state.push(&frame.body);
                    }
                }
                // An upload's bytes, if this channel is one an accepted
                // `BeginUpload` established and whose transfer is still
                // live. Checked before the terminal-input path below
                // because the two share one channel-id space and a
                // channel is only ever one stream (the begin and
                // attach handlers each refuse a channel the other holds).
                //
                // Nothing here awaits. Chunk delivery is a `try_send` and
                // every rejection is a decision rather than a wait,
                // because this loop is also the only thing that can
                // deliver the `AbortUpload` or the `Detach` that would
                // resolve the situation: a read loop parked on a full
                // upload queue cannot read the frame that would drain it.
                farhelm_proto::FrameKind::Data
                    if channels
                        .upload(frame.channel)
                        .is_some_and(UploadRoute::is_live) =>
                {
                    let channel = frame.channel;
                    let route = channels
                        .upload(channel)
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
                    let stale_route = if let Some(route) = channels.input(frame.channel) {
                        let entry = &route.entry;
                        // Save the reading before delivering input: an Enter
                        // can dismiss a dialog before the send returns. The
                        // activity leaf lock is released before any await or
                        // attachment lock, so input never waits on a capture.
                        let before_input = entry
                            .run
                            .activity
                            .lock()
                            .expect("activity mutex poisoned")
                            .reading
                            .state;
                        // The check and the send-keys delivery run under
                        // ONE lock hold, like the Resize arm: releasing
                        // between them is a TOCTOU where a takeover
                        // completes in the gap and the kicked client's
                        // already-validated keystrokes land in the
                        // winner's terminal — and keystrokes into an agent
                        // terminal are command execution. Safe against
                        // deadlock because forwarders never take this
                        // lock, and the Attach handler already holds it
                        // across its own terminal calls.
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
                        // handle out of the map: its grid input side
                        // holds the paste in progress, which a clone would
                        // split. The borrow (via `a`) ends when this match
                        // expression finishes evaluating, which is what
                        // lets the failure arm below still mutate
                        // `attachments` (`.remove`) under the SAME lock
                        // hold — no gap where a takeover could interleave.
                        let send_result = match attachments.get_mut(&route.key) {
                            Some(a) if a.channel == frame.channel && a.notify.same_channel(&tx) => {
                                Some(a.input.send_input(&frame.body))
                            }
                            _ => None,
                        };
                        // Only a submitted line delivered to the agent starts
                        // its hook warning timer: input containing a carriage
                        // return. A tab's keystrokes say nothing about whether
                        // the agent had a prompt it could report, and neither
                        // do the terminal's automatic replies to the agent
                        // TUI's own queries, which never carry one (see
                        // `capture::note_first_input`). The whole frame must
                        // have been confirmed: the any-bytes latch is already
                        // true once those replies land, so it cannot say
                        // whether THIS frame's Enter arrived. Which bytes
                        // the frame delivered is the input side's to say: a
                        // grid attachment's frame is a message, not bytes
                        // (`grid_stream::HostAttach::send_input`).
                        if route.key.terminal == TerminalId::Agent
                            && matches!(send_result, Some(Ok(true)))
                        {
                            note_first_input(entry, before_input);
                        }
                        match send_result {
                            Some(Ok(_)) => false,
                            // A failed send is this session's problem,
                            // not the shared connection's. It is still
                            // fatal to this attachment: accepting later
                            // chunks after silently losing one can turn
                            // a command into a different command.
                            Some(Err(e)) => {
                                warn!(session = %entry.info.id, error = %e, "input dropped");
                                if let Some(old) = attachments.remove(&route.key) {
                                    old.request_forwarder_shutdown();
                                    let ActiveAttach {
                                        channel,
                                        notify,
                                        forwarder,
                                        input,
                                        ..
                                    } = old;
                                    join_forwarder(&route.key, forwarder).await;
                                    drop(input);
                                    notify_detached(
                                        &notify,
                                        channel,
                                        format!("terminal input failed: {e:#}"),
                                        DetachCode::Other,
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
                        channels.detach(frame.channel);
                    }
                }
                farhelm_proto::FrameKind::Control => {
                    let msg = parse_control(&frame)?;
                    // Record attempts before even the reservation refusals. A
                    // malformed non-relay request may already have data behind
                    // it, and that data must not feed a later TCP relay.
                    if restricted_auth.is_none()
                        && let ControlMsg::Attach { channel, .. }
                        | ControlMsg::BeginUpload { channel, .. }
                        | ControlMsg::BeginDownload { channel, .. } = &msg {
                        channels.note_non_relay_attempt(*channel);
                    }
                    match restricted_auth.as_ref() {
                        Some(auth) => handle_restricted_control(&sup, msg, &tx, auth).await,
                        // An `AgentResponse` is the answer to something
                        // the SUPERVISOR asked this connection (see
                        // `super::agent_relay`), so it is completed
                        // against this connection's pending table rather
                        // than dispatched as a request. Intercepted here,
                        // beside the data-frame routing, because the
                        // pending table is connection-local exactly like
                        // the channel maps are — the `req_id` it
                        // correlates by was minted by this connection's
                        // own counter and means nothing on any other — and
                        // because `ConnectionCtx` deliberately stops at
                        // the per-message handlers.
                        None => match msg {
                            ControlMsg::ClaimTcpTunnelPorts { req_id, ports } => {
                                let reply = match channels.tunnels.claim(ports, Arc::clone(&sup.tcp_tunnel_budget), tx.clone()) {
                                    Ok(()) => ControlMsg::TcpTunnelPortsClaimed { req_id },
                                    Err(error) => ControlMsg::Error {
                                        create_unaccepted: false, req_id,
                                        kind: error_kind(&error), message: format!("{error:#}"),
                                    },
                                };
                                send_reply(&tx, &reply).await;
                            }
                            ControlMsg::BeginAcceptedTcpRelay { req_id, accepted, channel } => {
                                if !channels.can_begin_relay(channel) {
                                    send_reply(&tx, &ControlMsg::Error {
                                        create_unaccepted: false, req_id,
                                        kind: farhelm_proto::ErrorKind::InvalidRequest,
                                        message: "TCP relay channel is invalid or was already used".into(),
                                    }).await;
                                } else if let Some(socket) = channels.tunnels.take(accepted) {
                                    channels.begin_relay(channel, || super::tcp_relays::start_accepted(
                                        &mut tasks, socket, bulk_tx.clone(), priority_tx.clone(), req_id, channel,
                                    ));
                                } else {
                                    channels.reserve_relay(channel);
                                    send_reply(&tx, &ControlMsg::Error {
                                        create_unaccepted: false, req_id,
                                        kind: farhelm_proto::ErrorKind::NotFound,
                                        message: "pending TCP tunnel connection not found".into(),
                                    }).await;
                                }
                            }
                            ControlMsg::RejectTcpTunnel { accepted } => { drop(channels.tunnels.take(accepted)); }
                            ControlMsg::TcpRelayEof { channel } => {
                                if let Some(route) = channels.relay(channel) { route.state.eof(); }
                            }
                            ControlMsg::BeginTcpRelay { req_id, session_id, channel, address, port } => {
                                if !channels.can_begin_relay(channel) {
                                    send_reply(&tx, &ControlMsg::Error {
                                        create_unaccepted: false, req_id,
                                        kind: farhelm_proto::ErrorKind::InvalidRequest,
                                        message: "TCP relay channel is invalid or was already used".into(),
                                    }).await;
                                } else {
                                    channels.begin_relay(channel, || super::tcp_relays::start(
                                        &mut tasks, Arc::clone(&sup), bulk_tx.clone(), priority_tx.clone(),
                                        super::tcp_relays::RelayRequest { req_id, session_id, channel, address, port },
                                    ));
                                }
                            }
                            ControlMsg::TcpRelayAck { channel, received } => {
                                if let Some(route) = channels.relay(channel) { route.state.ack(received); }
                            }
                            ControlMsg::CloseTcpRelay { channel } => { channels.close_relay(channel); }
                            ControlMsg::Attach { req_id, channel, .. }
                            | ControlMsg::BeginUpload { req_id, channel, .. }
                            | ControlMsg::BeginDownload { req_id, channel, .. }
                                if channels.relay_reserved(channel) => {
                                send_reply(&tx, &ControlMsg::Error {
                                    create_unaccepted: false, req_id,
                                    kind: farhelm_proto::ErrorKind::InvalidRequest,
                                    message: "channel was reserved for a TCP relay".into(),
                                }).await;
                            }
                            ControlMsg::StatFile { req_id, session_id, path } => {
                                tasks.spawn(super::downloads::stat(Arc::clone(&sup), tx.clone(), req_id, session_id, path));
                            }
                            ControlMsg::BeginDownload { req_id, session_id, path, channel } => {
                                if !channels.can_begin_stream(channel) {
                                    send_reply(&tx, &ControlMsg::Error {
                                        create_unaccepted: false, req_id, kind: farhelm_proto::ErrorKind::InvalidRequest,
                                        message: "download channel is invalid or in use".into() }).await;
                                } else {
                                    channels.begin_download(channel, || super::downloads::start(&mut tasks,
                                        Arc::clone(&sup), bulk_tx.clone(), req_id, session_id, path, channel));
                                }
                            }
                            ControlMsg::DownloadAck { channel, received } => {
                                if let Some(route) = channels.download(channel) { route.ack(received); }
                            }
                            ControlMsg::AbortDownload { channel } => { channels.abort_download(channel); }
                            ControlMsg::Attach { req_id, channel, .. } | ControlMsg::BeginUpload { req_id, channel, .. }
                                if channels.download_reserved(channel) => {
                                send_reply(&tx, &ControlMsg::Error {
                                    create_unaccepted: false, req_id, kind: farhelm_proto::ErrorKind::InvalidRequest,
                                    message: "channel was reserved for a download".into() }).await;
                            }

                            ControlMsg::AgentResponse { req_id, outcome } => {
                                // A connection with no link was never sent an
                                // upcall, so an answer from it can only be a
                                // broken or confused peer; ending it matches
                                // what the link itself does with an id it
                                // never issued (`HelmLink::complete`).
                                let Some(link) = link.as_ref() else {
                                    anyhow::bail!(
                                        "an AgentResponse arrived on a connection that did not \
                                         declare itself the owning helm, so it was never sent an \
                                         agent request"
                                    );
                                };
                                link.complete(req_id, outcome).await;
                            }
                            msg => {
                                let ctx = ConnectionCtx {
                                    tx: &tx,
                                    priority: &priority_tx,
                                    channels: &mut channels,
                                    tasks: &mut tasks,
                                    panics: &panics,
                                };
                                handle_control(&sup, msg, ctx).await
                            }
                        },
                    }
                }
            }
        }
        Ok(())
    }
    .await;

    // Tunnel sockets cannot wait for an attachment or request's cleanup. Close
    // pending sockets and cancel active pumps first, then reap accept loops so
    // their listening descriptors are gone before the other teardown waits.
    channels.tunnels.close();
    let mut closing_relays = channels.close_relays();
    channels.tunnels.shutdown().await;
    // Aborting a Tokio task schedules its destruction. Reap those particular
    // tasks here so unrelated attachment waits cannot retain active sockets.
    // Other completed handlers are collected just as in steady-state reaping.
    while !closing_relays.is_empty() {
        let Some(joined) = tasks.join_next_with_id().await else {
            break;
        };
        match joined {
            Ok((id, ())) => {
                closing_relays.remove(&id);
            }
            Err(error) => {
                closing_relays.remove(&error.id());
                if !error.is_cancelled() {
                    warn!(%error, "spawned request handler task panicked");
                }
            }
        }
    }

    // Connection gone: retire its upward channel before the slower
    // attachment and handler teardown below. A session waiting on an
    // agent request has no deadline of its own, so every moment this is
    // deferred is a moment it waits for an answer that provably cannot
    // arrive — `HelmLink::fail_all` turns that into an immediate refusal.
    if let Some(link) = &link {
        sup.unregister_helm_link(link).await;
    }
    // Every upload this connection was carrying is over too. Each is
    // SIGNALLED rather than merely dropped, because a transfer whose queue
    // still holds chunks would otherwise keep writing them to a file
    // nobody will ever commit — the signal is selected on first, so the
    // transfer stops where it stands and removes its staging file
    // (PLAN_M4.md item 4's channel-loss cleanup). Dropping the routes
    // afterwards closes the command channels behind it.
    for route in channels.uploads() {
        let _ = route.signals.try_send(UploadSignal {
            reason: "the client's connection closed".to_string(),
            tell_client: false,
            session_gone: false,
        });
    }
    channels.drop_transfers();
    drop(bulk_tx);
    // Tear down any attachments this connection owned, so that no output
    // task keeps streaming to a connection that is gone and the next attach
    // finds nothing of this one. Signal every output task first, then join
    // them, like the takeover path. Awaiting under the lock is safe (output
    // tasks never take it).
    let mut attachments = sup.attachments.lock().await;
    let mine: Vec<(AttachmentKey, ActiveAttach)> = attachments
        .extract_if(|_, attachment| attachment.notify.same_channel(&tx))
        .collect();
    for (_, attachment) in &mine {
        attachment.request_forwarder_shutdown();
    }
    for (key, attachment) in mine {
        join_forwarder(&key, attachment.forwarder).await;
    }
    drop(attachments);
    drop(tx);
    // Give the connection's spawned slow-handler tasks (list/stop/delete —
    // see `HANDLER_ADMISSION_PERMITS`'s docs) a bounded chance to finish
    // and enqueue their replies BEFORE the writer drain below starts
    // waiting on the queue those replies land in — a task that finishes
    // after the writer has already given up would have enqueued a reply
    // nobody drains. `HANDLER_SHUTDOWN_TIMEOUT` is generous (kill sweeps
    // legitimately take seconds), but a handler wedged forever must not leak
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
        // `abort_all` only SCHEDULES cancellation — as with the writer-task
        // abort in `drain_writer` below, a task is not actually gone until
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
    // All handlers that were allowed to enqueue a final reply have now
    // finished. Close the receiver halves from inside the writer task so
    // those replies drain, while sender clones owned by links and detached
    // upload work can no longer keep an otherwise idle writer alive.
    let _ = writer_close_tx.send(());
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
/// link slower than the frame size divided by `window` (for a 32 KiB grid
/// history message and the default 5s window, under roughly 6.4 KB/s) —
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

/// The two queues and one-shot signal consumed by a connection's writer.
///
/// Closing the receiver halves is deliberately a writer-owned transition:
/// the shutdown tail can signal it after all reply-producing tasks have been
/// reaped, while the writer can then drain frames already accepted by either
/// queue. Sender lifetime is otherwise intentionally independent of this
/// connection's shutdown ordering.
struct WriterTaskChannels {
    ordinary_rx: mpsc::Receiver<Frame>,
    priority_rx: mpsc::Receiver<Frame>,
    bulk_rx: mpsc::Receiver<Frame>,
    close_rx: oneshot::Receiver<()>,
    failed_tx: oneshot::Sender<String>,
}

/// Progress and timing state shared by the writer loop and its drain waiter.
struct WriterTaskProgress {
    bytes_written: Arc<AtomicU64>,
    frames_written: Arc<AtomicU64>,
    stall_window: Duration,
}

/// Run the single writer for a connection and honor its shutdown signal.
fn spawn_writer_task<W>(
    mut writer: FrameWriter<W>,
    channels: WriterTaskChannels,
    progress: WriterTaskProgress,
) -> tokio::task::JoinHandle<()>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    let WriterTaskChannels {
        mut ordinary_rx,
        mut priority_rx,
        mut bulk_rx,
        mut close_rx,
        failed_tx,
    } = channels;
    let WriterTaskProgress {
        bytes_written,
        frames_written,
        stall_window,
    } = progress;
    tokio::spawn(async move {
        let mut queues_closed = false;
        loop {
            // Biased, so an upload's control frame goes out ahead of
            // whatever terminal output is queued — see `priority_tx`'s own
            // comment for why that is a contract rather than a
            // preference. `else` fires only when all three queues are closed,
            // which is this task's ordinary end.
            let frame = tokio::select! {
                biased;
                _ = &mut close_rx, if !queues_closed => {
                    // `Receiver::close` preserves frames already queued while
                    // refusing sends from the detached task clones. The
                    // select guard makes this a one-time transition, after
                    // which the normal receive arms drain those frames.
                    ordinary_rx.close();
                    priority_rx.close();
                    bulk_rx.close();
                    queues_closed = true;
                    continue;
                }
                Some(frame) = priority_rx.recv() => frame,
                Some(frame) = ordinary_rx.recv() => frame,
                Some(frame) = bulk_rx.recv() => frame,
                else => break,
            };
            // A write that makes NO PROGRESS for a whole window is
            // treated exactly like a write that failed. See
            // WRITER_STALL_TIMEOUT: without this, bounding the queue would
            // let a peer that stops reading park every producer —
            // including this connection's own read loop, whose inline
            // replies (a busy refusal among them) await the same full
            // queue — so the connection could never notice the peer was
            // gone. Breaking here drops `rx`, which is what unblocks those
            // producers with a closed-channel error.
            if let Err(detail) =
                write_frame_before_stall(&mut writer, &bytes_written, &frame, stall_window).await
            {
                warn!(error = %detail, "frame write to client failed");
                let _ = failed_tx.send(detail);
                break;
            }
            frames_written.fetch_add(1, Ordering::Relaxed);
            // Output-path checkpoint; see the first one in
            // `OutputStream::next_output`. Data frames only: control
            // replies are not what that path loses.
            if matches!(frame.kind, farhelm_proto::FrameKind::Data) {
                debug!(
                    channel = frame.channel,
                    len = frame.body.len(),
                    "data frame written to client"
                );
            }
        }
    })
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
    // Asked through `ControlMsg::reply_req_id` rather than by re-listing
    // every reply variant here, which is what a second copy of that list
    // had been until the version-10 profile replies were added to one of
    // them and not the other. That copy could only ever be right by
    // coincidence: the accessor's own match is exhaustive with no wildcard,
    // so a new reply variant is a compile error THERE, while an omission
    // here compiled fine and turned the first successful reply of a new
    // kind into a panic that takes the connection down. The two questions
    // are the same question — "is this a reply, and to what" — so they get
    // one answer.
    let req_id = match msg.reply_req_id() {
        Some(req_id) => req_id,
        None => {
            unreachable!("reply_frame called with a message that carries no req_id: {msg:?}")
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
            create_unaccepted: false,
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
pub(crate) fn notify_detached(
    tx: &mpsc::Sender<Frame>,
    channel: u32,
    reason: String,
    code: DetachCode,
) {
    let frame = Frame::control(&ControlMsg::Detached {
        channel,
        reason,
        code,
    });
    // A `Closed` error needs no handling: the connection is gone, so there
    // is nobody left to tell.
    if let Err(mpsc::error::TrySendError::Full(frame)) = tx.try_send(frame) {
        let tx = tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(frame).await;
        });
    }
}

/// Take a management slot for one request without waiting, or refuse the
/// request with [`farhelm_proto::HOST_BUSY_REFUSAL`] and return `None`.
///
/// The admission step every management request takes in a connection's read
/// loop (Stop, Restart, Rename, and the requests admitted through
/// [`spawn_admitted`]), so that loop never waits for a slot. Waiting there is
/// what this replaced, and it was wrong: the same loop delivers every
/// keystroke, resize and detach for every session on the connection, so a
/// request parked on a full semaphore froze typing on the whole host until one
/// of the eight operations holding the slots finished (later, if Deletes were
/// already queued for a slot), which for a Stop or Delete can mean waiting out
/// a kill grace period. SPEC.md "Waiting between operations on one host"
/// forbids that. Refusing promptly is the chosen response, rather than queueing
/// the request somewhere off the loop: a queue would keep the wait away from
/// the input path too, but it is a second mechanism to bound and drain, and a
/// "try again" costs the user one click.
///
/// The refusal is `ErrorKind::Unavailable`, the kind that already means
/// "nothing happened, the same request works later"; the helm turns it into a
/// 503 and the UI shows the message on the action. It is sent before anything
/// the request asks for has been changed (a request may check its own fields
/// first, as Rename checks its title), which is what makes a retry safe.
/// Sending it awaits the bounded writer queue, like every other reply the read
/// loop sends inline; that wait is on the peer reading its own replies, not on
/// management work.
pub(crate) async fn admit_or_refuse(
    admission: &Arc<tokio::sync::Semaphore>,
    tx: &mpsc::Sender<Frame>,
    req_id: u64,
) -> Option<tokio::sync::OwnedSemaphorePermit> {
    match Arc::clone(admission).try_acquire_owned() {
        Ok(permit) => Some(permit),
        Err(tokio::sync::TryAcquireError::NoPermits) => {
            send_reply(
                tx,
                &ControlMsg::Error {
                    create_unaccepted: false,
                    req_id,
                    kind: ErrorKind::Unavailable,
                    message: farhelm_proto::HOST_BUSY_REFUSAL.to_string(),
                },
            )
            .await;
            None
        }
        Err(tokio::sync::TryAcquireError::Closed) => {
            unreachable!("admission semaphore is never closed")
        }
    }
}

/// Admit one management request through [`admit_or_refuse`] and, if a slot was
/// free, spawn the future `work` builds onto `tasks` holding it for the
/// future's whole lifetime. `work` is handed its own sender for the reply, and
/// is only called once the request is admitted: the shared admission-then-spawn
/// shape of the management requests whose work all happens on the connection's
/// tracked task (directory browse, repository search, tab open and close).
/// Stop, Restart and Rename take their slot through `admit_or_refuse`
/// directly, because part or all of their work is owned by the supervisor
/// rather than the connection, and Delete waits for its slot inside its own
/// task after its agent-request fence; see their handlers.
///
/// Admission still happens BEFORE the spawn, so a refused request leaves
/// no task behind and an admitted one never exists without its slot.
pub(crate) async fn spawn_admitted<F, Fut>(
    admission: &Arc<tokio::sync::Semaphore>,
    tasks: &mut tokio::task::JoinSet<()>,
    tx: &mpsc::Sender<Frame>,
    req_id: u64,
    work: F,
) where
    F: FnOnce(mpsc::Sender<Frame>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let Some(permit) = admit_or_refuse(admission, tx, req_id).await else {
        return;
    };
    let future = work(tx.clone());
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

/// Why an attachment's output task (`super::grid_stream::GridSender`)
/// stopped, and therefore what the client must be told
/// ([`report_forwarder_end`]).
pub(crate) enum ForwarderEnd {
    /// The attachment owner requested an orderly detach.
    Requested,
    /// The client's connection is gone (its writer queue closed). Nothing
    /// left to notify.
    ClientGone,
    /// The terminal ended: its host is gone.
    TerminalEnded,
    /// The stream itself failed: the terminal's runtime ended or became
    /// unreadable, or a message could not be encoded.
    StreamFailed(String),
    /// The attachment stayed paused past
    /// [`super::core::STALL_DETACH_TIMEOUT`]; the verdict tears the
    /// attachment down ([`detach_stalled`]).
    Stalled,
    /// The attachment's input broke where the connection's input path
    /// could not see it: a grid attachment's input written to a host
    /// terminal after the connection's data-frame arm had queued it
    /// (`super::grid_stream`). Ends the attachment as an input failure in
    /// that arm does, so no later input lands after the lost part.
    InputFailed(String),
}

/// Tell the client how its attachment ended, as `end` says, from the output
/// task that reported it. Must be called from that task: the detach arbiters
/// below identify "this attachment" by the reporting task's id.
///
/// The arbiters identify "this attachment" by the output task that reports
/// the verdict. Channel ids are reused, so a client that detaches and
/// reattaches on the same channel of the same connection produces a
/// replacement that matches on channel and connection alone; only the
/// task's id tells it apart. The grid sender's `run` executes as the task
/// `ActiveAttach::forwarder` joins.
pub(crate) fn report_forwarder_end(
    sup: &Arc<Supervisor>,
    key: AttachmentKey,
    channel: u32,
    tx: mpsc::Sender<Frame>,
    end: ForwarderEnd,
    stall_timeout: Duration,
) {
    let origin = tokio::task::id();
    match end {
        ForwarderEnd::Requested | ForwarderEnd::ClientGone => {}
        ForwarderEnd::TerminalEnded => {
            debug!(channel, session = %key.session, "terminal ended; detaching");
            detach_naturally(
                sup,
                key,
                channel,
                tx,
                origin,
                "session terminal ended".to_string(),
            );
        }
        ForwarderEnd::StreamFailed(reason) => {
            // Must notify: swallowing this leaves the client with a
            // terminal that silently stops updating while still
            // accepting input, and no log line anywhere explaining why.
            warn!(channel, error = %reason, "output stream failed");
            detach_naturally(
                sup,
                key,
                channel,
                tx,
                origin,
                format!("output stream failed: {reason}"),
            );
        }
        ForwarderEnd::InputFailed(reason) => {
            warn!(channel, error = %reason, "input dropped");
            detach_naturally(
                sup,
                key,
                channel,
                tx,
                origin,
                format!("terminal input failed: {reason}"),
            );
        }
        ForwarderEnd::Stalled => {
            warn!(
                channel,
                session = %key.session,
                "attachment paused longer than {stall_timeout:?}; detaching as stalled"
            );
            detach_stalled(sup, key, channel, tx, origin);
        }
    }
}

/// Park while the client has output paused, returning `Err(Stalled)` once
/// this ONE pause has outlived `stall_timeout`, measured from the instant it
/// started: re-entering the park after a wakeup cannot restart the clock
/// (see [`stalled_past_deadline`]).
pub(crate) async fn park_while_paused(
    pause_rx: &mut watch::Receiver<Option<tokio::time::Instant>>,
    stall_timeout: Duration,
) -> Result<(), ForwarderEnd> {
    loop {
        let Some(paused_at) = *pause_rx.borrow_and_update() else {
            return Ok(());
        };
        tokio::select! {
            () = tokio::time::sleep_until(paused_at + stall_timeout) => {
                return Err(ForwarderEnd::Stalled);
            }
            changed = pause_rx.changed() => {
                // The sender is dropped only when this attachment has
                // been removed from the map, at which point the separate
                // shutdown watch is also ready; falling through lets the
                // outer, biased select take that orderly path.
                if changed.is_err() {
                    return Ok(());
                }
            }
        }
    }
}

/// Resolve only once the attachment has been paused CONTINUOUSLY past
/// `timeout` — the stall detector every blocking await in an output task is
/// raced against.
///
/// Safe to create fresh at each `select!` site precisely because the
/// deadline is derived from the pause's stored START instant rather than
/// from now: re-creating it cannot restart the clock, so the hard maximum
/// survives however many chunks, phases, or wakeups a single pause spans.
/// (An earlier version timed from `Instant::now()` and had exactly that
/// bug — a client draining slowly enough to keep the task awaiting, but
/// fast enough to keep it moving, was never detached.)
///
/// Resolving only on a CONTINUOUS pause is what keeps this a hard maximum
/// pause duration rather than a cumulative budget: a client that pauses
/// and resumes repeatedly is a slow client being served correctly, not a
/// stalled one, and each resume clears the stored start.
pub(crate) async fn stalled_past_deadline(
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
/// Spawned rather than run inline because output tasks must never take
/// `Supervisor::attachments`: the takeover, detach, and delete paths all
/// request shutdown and await an output task while holding that mutex, so a
/// task blocking on it would deadlock the supervisor outright. A separate
/// task can wait for the lock safely: it is not the task being awaited, so a
/// teardown holding the mutex can always make progress and release it.
/// (This does NOT assume the reporting task has already returned — it may
/// still be unwinding when this runs; the signal-then-await below joins it
/// whenever it finishes.)
///
/// The identity check adds the reporting output task's id (`origin`) to
/// the channel-plus-connection check other ownership checks in this module
/// use: by the time this runs, a takeover may already have installed a
/// different attachment for this terminal, or the same client may have
/// detached and reattached on the very same channel, and tearing either one
/// down would kill a live terminal. Channel and connection cannot tell that
/// reattach from the stalled original; the task id can.
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
    origin: tokio::task::Id,
) {
    let sup = Arc::clone(sup);
    tokio::spawn(async move {
        let mut attachments = sup.attachments.lock().await;
        let mine = attachments.get(&key).is_some_and(|a| {
            a.channel == channel && a.notify.same_channel(&tx) && a.forwarder.id() == origin
        });
        if !mine {
            // A takeover, a detach-and-reattach, a delete, or the connection
            // dying got here first, so this stall belongs to an attachment
            // that no longer exists. Returning WITHOUT notifying is the
            // point: a replacement may be using the same channel id on the
            // same connection, so a stalled notice sent now would reach the
            // new client and race — or overtake — the truthful notice its
            // own teardown path already sent to the loser.
            return;
        }
        let removed = attachments.remove(&key);
        if let Some(old) = removed {
            // Signal-and-await like every other teardown, even though the
            // output task is the very task that asked for this: it is about
            // to return, so this only joins that completion.
            old.request_forwarder_shutdown();
            join_forwarder(&key, old.forwarder).await;
        }
        drop(attachments);
        let _ = tx
            .send(Frame::control(&ControlMsg::Detached {
                channel,
                reason: DETACH_REASON_STALLED.to_string(),
                code: DetachCode::Stalled,
            }))
            .await;
    });
}

/// Claim and report an output task's natural terminal verdict on a separate task.
///
/// An output task cannot take `attachments`: teardown may hold that lock while it
/// awaits this very task. The separate arbiter identity-checks and removes the
/// still-current attachment instead. An explicit restart, takeover, close, or
/// connection teardown that removed it first therefore owns the only notice;
/// the stale natural verdict is discarded. "Still current" includes the
/// reporting output task's id (`origin`), because a detach and reattach on
/// the same channel of the same connection installs a replacement that
/// matches on channel and connection alone.
///
/// Every verdict (a terminal that ended, a failed stream or input) for a
/// session a Delete is tearing down waits for the Delete to have detached
/// the session's viewers, or returned, before it claims anything: the
/// Delete ends the session's terminals before it detaches their viewers, so
/// a terminal host's stream can end first, and its viewer must hear the
/// Delete's own notice (deleted, or why it failed)
/// (`Supervisor::deletes_in_progress`).
fn detach_naturally(
    sup: &Arc<Supervisor>,
    key: AttachmentKey,
    channel: u32,
    tx: mpsc::Sender<Frame>,
    origin: tokio::task::Id,
    reason: String,
) {
    let sup = Arc::clone(sup);
    tokio::spawn(async move {
        // A Delete that ended this terminal sends the notice itself once it
        // knows its outcome; by the time it is over the attachment is either
        // gone from the map (detached as deleted, or with why the Delete
        // failed) or still here because the Delete failed before detaching
        // it, and only then is this the notice to send.
        sup.delete_over(&key.session).await;
        let mut attachments = sup.attachments.lock().await;
        let mine = attachments.get(&key).is_some_and(|a| {
            a.channel == channel && a.notify.same_channel(&tx) && a.forwarder.id() == origin
        });
        if !mine {
            return;
        }
        let old = attachments
            .remove(&key)
            .expect("the identity-checked attachment remains under the lock");
        old.request_forwarder_shutdown();
        join_forwarder(&key, old.forwarder).await;
        drop(attachments);
        notify_detached(&tx, channel, reason, DetachCode::Other);
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
/// `PauseOutput` carries no session id — unlike `Resize`, whose session id
/// is only a cross-check against the channel's route. Mirrors the `Detach`
/// arm's own search for the same reason.
///
/// A pause records WHEN it started and a resume clears it, and a pause
/// arriving while already paused changes nothing at all. That last part is
/// load-bearing rather than tidy: the output task's hard maximum is measured
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
    use super::super::core::CreateInputs;
    use super::super::core::tests::{StateDir, dummy_exe, host_supervisor};
    use super::*;
    use farhelm_proto::SessionLaunch;
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
    /// `ListSessions` is the one reply built entirely from unbounded
    /// caller-controlled data (session titles) with no byte budget of its
    /// own anywhere in the pipeline (`LIST_SESSIONS_CAP` bounds the ROW
    /// count, not the encoded size) — so it is the realistic way a control
    /// reply exceeds `MAX_FRAME_LEN`, and this check is its only backstop.
    /// One oversized title is enough to clear the limit (JSON escaping
    /// plus the frame header add overhead on top of the title itself), so
    /// a single `SessionInfo` suffices here.
    #[farhelm_testtrace::test]
    fn reply_frame_substitutes_error_for_oversized_reply() {
        let req_id = 42;
        let oversized = ControlMsg::SessionList {
            req_id,
            sealed: false,
            sessions: vec![SessionInfo {
                screen_yolo: false,
                claude_trust_prompt: false,
                workspace_trust_prompt: false,
                agent_kind: farhelm_proto::AgentKind::Generic,
                parent: None,
                id: "s1".to_string(),
                title: "x".repeat(farhelm_proto::MAX_FRAME_LEN as usize),
                created_at: 1_700_000_000,
                last_activity_at: 1_700_000_000,
                last_work_started_at: 0,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                canonical_cwd: None,
                invocation: "agent".to_string(),
                launch: farhelm_proto::SessionLaunch::plain_command("agent"),
                status: SessionStatus::Running,
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
                github_repo: None,
                working_copy: None,
                notifications: Vec::new(),
            }],
            truncated: false,
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
            ..
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
    #[farhelm_testtrace::test]
    fn reply_frame_passes_through_normal_reply_unchanged() {
        let msg = ControlMsg::SessionCreated {
            req_id: 7,
            session: SessionInfo {
                screen_yolo: false,
                claude_trust_prompt: false,
                workspace_trust_prompt: false,
                agent_kind: farhelm_proto::AgentKind::Generic,
                parent: None,
                id: "s1".to_string(),
                title: "demo".to_string(),
                created_at: 1_700_000_000,
                last_activity_at: 1_700_000_000,
                last_work_started_at: 0,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                canonical_cwd: None,
                invocation: "agent".to_string(),
                launch: farhelm_proto::SessionLaunch::plain_command("agent"),
                // Matches real `create_session` output: `Unknown`, not
                // a live status (see that function's own doc comment).
                status: SessionStatus::Unknown,
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
                github_repo: None,
                working_copy: None,
                notifications: Vec::new(),
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
    #[farhelm_testtrace::test]
    fn reply_frame_accepts_session_restarted() {
        let msg = ControlMsg::SessionRestarted {
            req_id: 9,
            session: SessionInfo {
                screen_yolo: false,
                claude_trust_prompt: false,
                workspace_trust_prompt: false,
                agent_kind: farhelm_proto::AgentKind::Generic,
                parent: None,
                id: "s1".to_string(),
                title: "demo".to_string(),
                created_at: 1_700_000_000,
                last_activity_at: 1_700_000_000,
                last_work_started_at: 0,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                canonical_cwd: None,
                invocation: "agent".to_string(),
                launch: farhelm_proto::SessionLaunch::plain_command("agent"),
                status: SessionStatus::Running,
                annotation: None,
                restart_offer: RestartOffer::Resume,
                tabs: Vec::new(),
                github_repo: None,
                working_copy: None,
                notifications: Vec::new(),
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
    #[should_panic(expected = "carries no req_id")]
    fn reply_frame_panics_on_message_without_req_id() {
        reply_frame(&ControlMsg::Detached {
            channel: 1,
            reason: "x".into(),
            code: DetachCode::Other,
        });
    }

    /// A real session id paired with the wrong token is refused before the
    /// peer can submit even the restricted operation slice.
    ///
    /// Keeping the row present distinguishes token validation from the
    /// already-covered stale-session case: accepting any token for a known id
    /// would otherwise pass a test that authenticates only missing rows.
    #[farhelm_testtrace::test]
    async fn an_authenticated_spawn_hello_with_the_wrong_token_is_unauthorized() {
        let state = StateDir::new();
        let sup = host_supervisor(state.path(), dummy_exe())
            .await
            .expect("supervisor construction touches no terminal and not the launch shim");
        let cwd = state.path().to_string_lossy().into_owned();
        let parent = sup
            .create_session(
                CreateInputs {
                    prompt: None,
                    named_start: None,
                    github_checkout: None,
                    cwd: &cwd,
                    parent: None,
                    launch: SessionLaunch::plain_command("agent"),
                    title: Some("parent".to_string()),
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await
            .expect("create authenticating session");
        let (client_side, server_side) = tokio::io::duplex(64 * 1024);
        let server = tokio::spawn(async move { handle_connection(sup, server_side).await });
        let (r, w) = tokio::io::split(client_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        writer
            .write_control(&ControlMsg::Hello {
                database_schema: None,
                protocol_version: farhelm_proto::PROTOCOL_VERSION,
                build_version: farhelm_proto::BUILD_VERSION.to_string(),
                role: "spawn".to_string(),
                host_identity: None,
                auth: Some(farhelm_proto::SessionAuth {
                    session_id: parent.id,
                    token: "definitely-not-the-minted-token".to_string(),
                }),
                owning_helm: false,
            })
            .await
            .expect("send authenticated hello");

        let hello = reader
            .read_frame()
            .await
            .expect("read supervisor hello")
            .expect("supervisor must answer first");
        assert!(matches!(
            parse_control(&hello),
            Ok(ControlMsg::Hello { .. })
        ));
        let refusal = reader
            .read_frame()
            .await
            .expect("read admission refusal")
            .expect("the refusal must reach the peer before close");
        let ControlMsg::Error {
            req_id,
            kind,
            message,
            ..
        } = parse_control(&refusal).expect("decode refusal")
        else {
            panic!("an authenticated hello must receive an Error");
        };
        assert_eq!(req_id, 0);
        assert_eq!(kind, ErrorKind::Unauthorized);
        assert!(
            message.contains("credential") && message.contains("no longer exists"),
            "the refusal must identify a bad or stale session credential without disclosing \
             which half failed: {message}"
        );

        let error = server
            .await
            .expect("connection task must not panic")
            .expect_err("the restricted peer must not enter the request loop");
        assert!(format!("{error:#}").contains("session credential is invalid"));
    }

    /// Authority follows the presence of session authentication, not the
    /// peer's self-declared role string.
    ///
    /// The `spawn` role remains descriptive wire metadata. Without an auth
    /// bearer this is the same local Unix-socket peer as every helm client and
    /// therefore retains the full dispatcher, including `ListSessions`.
    #[farhelm_testtrace::test]
    async fn a_spawn_role_without_authentication_retains_full_authority() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (client_side, server_side) = tokio::io::duplex(64 * 1024);
        let server = tokio::spawn(async move { handle_connection(sup, server_side).await });
        let (r, w) = tokio::io::split(client_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        writer
            .write_control(&ControlMsg::Hello {
                database_schema: None,
                protocol_version: farhelm_proto::PROTOCOL_VERSION,
                build_version: farhelm_proto::BUILD_VERSION.to_string(),
                role: "spawn".to_string(),
                host_identity: None,
                auth: None,
                owning_helm: false,
            })
            .await
            .expect("send unauthenticated spawn-role hello");
        let hello = reader.read_frame().await.unwrap().expect("server hello");
        assert!(matches!(
            parse_control(&hello),
            Ok(ControlMsg::Hello { .. })
        ));

        writer
            .write_control(&ControlMsg::ListSessions { req_id: 60 })
            .await
            .expect("send a full-authority request");
        let reply = reader.read_frame().await.unwrap().expect("list reply");
        assert!(matches!(
            parse_control(&reply),
            Ok(ControlMsg::SessionList { req_id: 60, .. })
        ));

        drop(writer);
        drop(reader);
        server.await.unwrap().expect("clean client close");
    }

    /// A writer signaled after its final frame is queued must close both
    /// receiver halves and finish naturally instead of waiting for sender
    /// clones owned by the supervisor's link and background tasks. The paused
    /// clock makes a completion bound shorter than
    /// `WRITER_DRAIN_TIMEOUT` deterministic, while reading the queued frame
    /// proves closure did not discard the shutdown backlog.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn a_signaled_writer_drains_queued_frames_without_the_drain_window() {
        let (server_side, client_side) = tokio::io::duplex(64 * 1024);
        let (server_read, server_write) = tokio::io::split(server_side);
        let (client_read, _client_write) = tokio::io::split(client_side);
        drop(server_read);
        let (server_write, bytes_written) = ProgressWrite::new(server_write);
        let frames_written = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel(1);
        let (_priority_tx, priority_rx) = mpsc::channel(1);
        let (close_tx, close_rx) = oneshot::channel();
        let (writer_failed_tx, _writer_failed_rx) = oneshot::channel();
        tx.send(Frame::data(1, b"queued before close".to_vec()))
            .await
            .expect("ordinary writer queue is open");
        let mut writer_task = spawn_writer_task(
            FrameWriter::new(server_write),
            WriterTaskChannels {
                ordinary_rx: rx,
                priority_rx,
                bulk_rx: mpsc::channel(1).1,
                close_rx,
                failed_tx: writer_failed_tx,
            },
            WriterTaskProgress {
                bytes_written,
                frames_written: Arc::clone(&frames_written),
                stall_window: Duration::from_secs(5),
            },
        );
        close_tx.send(()).expect("writer close signal is open");

        let mut client_reader = FrameReader::new(client_read);
        let frame = tokio::time::timeout(Duration::from_millis(1), client_reader.read_frame())
            .await
            .expect("queued frame must not wait for the writer drain window")
            .expect("read queued frame")
            .expect("queued frame must reach the peer");
        assert_eq!(frame, Frame::data(1, b"queued before close".to_vec()));
        // The production drain helper may abort a stuck writer. That is
        // cleanup, not evidence that this close signal finished the task.
        let completed = tokio::time::timeout(Duration::from_millis(1), &mut writer_task).await;
        if completed.is_err() {
            writer_task.abort();
            let _ = writer_task.await;
        }
        completed
            .expect("signaled writer must finish before the drain window")
            .expect("signaled writer must finish without panic or cancellation");
        assert_eq!(frames_written.load(Ordering::Relaxed), 1);
    }

    /// A valid credential enters the restricted request loop rather than
    /// receiving the handshake-level refusal used for a forgery.
    #[farhelm_testtrace::test]
    async fn a_valid_session_credential_is_admitted_with_restricted_authority() {
        let state = StateDir::new();
        let sup = host_supervisor(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let cwd = state.path().to_string_lossy().into_owned();
        let parent = sup
            .create_session(
                CreateInputs {
                    prompt: None,
                    named_start: None,
                    github_checkout: None,
                    cwd: &cwd,
                    parent: None,
                    launch: SessionLaunch::plain_command("agent"),
                    title: Some("parent".to_string()),
                    cols: 80,
                    rows: 24,
                },
                None,
            )
            .await
            .expect("create authenticating session");
        let token = sup
            .store
            .session_token(&parent.id)
            .await
            .unwrap()
            .expect("created session has a credential");
        let (client_side, server_side) = tokio::io::duplex(64 * 1024);
        let server_sup = Arc::clone(&sup);
        let server = tokio::spawn(async move { handle_connection(server_sup, server_side).await });
        let (r, w) = tokio::io::split(client_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        writer
            .write_control(&ControlMsg::Hello {
                database_schema: None,
                protocol_version: farhelm_proto::PROTOCOL_VERSION,
                build_version: farhelm_proto::BUILD_VERSION.to_string(),
                role: "spawn".to_string(),
                host_identity: None,
                auth: Some(farhelm_proto::SessionAuth {
                    session_id: parent.id,
                    token,
                }),
                owning_helm: false,
            })
            .await
            .unwrap();
        let hello = reader.read_frame().await.unwrap().expect("server hello");
        assert!(matches!(
            parse_control(&hello),
            Ok(ControlMsg::Hello { .. })
        ));
        writer
            .write_control(&ControlMsg::ListSessions { req_id: 61 })
            .await
            .unwrap();
        let refusal = reader
            .read_frame()
            .await
            .unwrap()
            .expect("restricted refusal");
        assert!(matches!(
            parse_control(&refusal).unwrap(),
            ControlMsg::Error {
                req_id: 61,
                kind: ErrorKind::Unauthorized,
                ..
            }
        ));
        drop(writer);
        drop(reader);
        server.await.unwrap().expect("clean client close");
    }

    /// Drive one real `handle_connection` server task over an in-process
    /// duplex pipe, perform the (helm-side) handshake against it, and
    /// return the host identity its hello carries — the duplex/handshake/
    /// hello-extraction sequence shared by every case in
    /// `supervisor_hello_carries_a_real_stable_host_identity` and
    /// `supervisor_hello_reports_the_owners_identity_to_a_claimless_reader`
    /// below, so it exists once rather than being hand-rolled repeatedly
    /// with the risk of the copies drifting apart.
    ///
    /// Takes `sup` BY REFERENCE and clones only what the spawned task
    /// needs: the restart case below has to keep observing the caller's
    /// own `Arc` after this call returns (to prove the predecessor's
    /// claim is actually gone before the second `Supervisor` is
    /// constructed), which an owning parameter would make impossible —
    /// the clone moved into `handle_connection` would be this function's
    /// last use of it either way.
    ///
    /// Joins the server task itself (after dropping the client's own
    /// reader/writer, which is what lets `handle_connection` see EOF and
    /// return) and asserts BOTH result layers: the `JoinHandle`'s own
    /// `Result` (a panicked server task) and the `anyhow::Result`
    /// `handle_connection` returns (a clean but erroring exit). A bare
    /// `let _ = server.await;` would silently accept either failure mode —
    /// exactly the shape this helper exists to close off.
    async fn hello_host_identity_over_a_fresh_connection(sup: &Arc<Supervisor>) -> String {
        let sup = Arc::clone(sup);
        let (client_side, server_side) = tokio::io::duplex(64 * 1024);
        let server = tokio::spawn(async move { handle_connection(sup, server_side).await });
        let (r, w) = tokio::io::split(client_side);
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        let hello = farhelm_proto::io::handshake(&mut reader, &mut writer, "helm")
            .await
            .expect("handshake");
        let ControlMsg::Hello {
            host_identity,
            database_schema,
            ..
        } = hello
        else {
            panic!("expected a Hello, got {hello:?}");
        };
        let identity = host_identity.expect("a supervisor's hello must carry an identity");
        // Update's newer-schema refusal reads this; nothing else pins the producer.
        assert_eq!(database_schema, Some(crate::store::SCHEMA_VERSION));
        drop(reader);
        drop(writer);
        server
            .await
            .expect("the connection server task must not panic")
            .expect("handle_connection must return Ok for a clean client-initiated close");
        identity
    }

    /// PLAN_M6.md item 2's end-to-end proof that host identity actually
    /// reaches the wire: drives the REAL `handle_connection` (not
    /// `handshake_with_host_identity` in isolation, which `farhelm-proto`'s
    /// own tests already cover) over an in-process duplex pipe, and reads
    /// back the hello it sends. Pinned here rather than only at the store
    /// level (`store::tests`'s own identity tests) because minting a value
    /// and actually PUTTING it on the wire are two different things — a
    /// bug that left `Supervisor::host_identity` unused would pass every
    /// store-level test and still ship a supervisor whose hello lied by
    /// omission.
    ///
    /// Stability across a supervisor RESTART (mirroring the boot_id
    /// tests' own shape) is pinned by constructing a SECOND `Supervisor`
    /// against the same state directory, ONLY after the first has fully
    /// released its claim, and checking its hello carries the identical
    /// identity — the store-level
    /// `host_identity_is_minted_once_and_stable_across_reopen` test
    /// already covers the store's own durability; this is the same
    /// guarantee observed from the wire a real client actually sees.
    ///
    /// The strong-count drain before dropping `sup` (rather than trusting
    /// the join above alone) is what makes "restart" an honest label
    /// rather than an assumption: `StateDirOwnership::claim`'s own docs
    /// name a real, if narrow, race between one claim's concurrent drop
    /// and a fresh claim attempt, and `owns_state_dir`'s docs are explicit
    /// that a "restarted" supervisor built while its predecessor is still
    /// alive silently takes the READ-ONLY path instead — a claimless
    /// second instance would still answer this test's hello, so nothing
    /// about the OLD assertions here would have caught that regression.
    /// Asserting `owns_state_dir()` on the second instance is what turns
    /// that silent substitution into a loud one.
    #[farhelm_testtrace::test]
    async fn supervisor_hello_carries_a_real_stable_host_identity() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction does not run the launch shim");
        let first_identity = hello_host_identity_over_a_fresh_connection(&sup).await;
        uuid::Uuid::parse_str(&first_identity).expect("the identity must be a real UUID");

        // Prove the predecessor is REALLY gone (see this test's own docs)
        // before the second `Supervisor` ever calls `StateDirOwnership::
        // claim`: `hello_host_identity_over_a_fresh_connection` already
        // joins its connection task, so this loop ordinarily spins zero
        // times, but only the strong count — not the join alone — is
        // actually load-bearing for the claim below.
        while Arc::strong_count(&sup) > 1 {
            tokio::task::yield_now().await;
        }
        drop(sup);

        // A second `Supervisor` against the SAME state directory — the
        // restart case — must report the identical identity, not a fresh
        // one: `StateDir` does not wipe anything between these two
        // constructions, so this is the durable-reuse path, not the
        // wiped-state-dir path `a_wiped_state_dir_mints_a_different_
        // host_identity` (store-level) covers separately.
        let sup2 = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("second construction against the same state dir");
        assert!(
            sup2.owns_state_dir(),
            "a genuine restart (predecessor's claim released) must own the state dir, not \
             merely read alongside a still-live one"
        );
        let second_identity = hello_host_identity_over_a_fresh_connection(&sup2).await;
        assert_eq!(
            first_identity, second_identity,
            "identity must survive a supervisor restart against the same state directory"
        );
    }

    /// The other half of the shape the RESTART test above deliberately
    /// excludes: a `Supervisor` constructed while something ELSE holds the
    /// state directory's claim takes the claimless, read-only path instead
    /// — `Supervisor::owns_state_dir`'s own docs call this "a handoff's
    /// brief overlap," never a restart. Truthfully labeled reader-reopen
    /// coverage, kept alongside the restart case rather than folded into
    /// it: the two exercise different branches of `Supervisor::
    /// new_with_seams`'s ownership gate, and a single test conflating them
    /// would only ever exercise whichever branch construction ordering
    /// happened to win.
    ///
    /// The incumbent is a bare `flock` on the lock file, NOT a second live
    /// `Supervisor` — mirroring `a_supervisor_without_the_state_dir_claim_
    /// reconciles_nothing_durably`'s own fixture (`core.rs`) and for the
    /// same reason its docs give: `StateDirOwnership::claim` reuses a
    /// still-live claim for a second in-process `Supervisor` against the
    /// same directory (deliberately — see that type's docs), so two real
    /// `Supervisor`s here would both report ownership and this test would
    /// prove nothing about the claimless path at all. A raw `flock`
    /// conflicts at the syscall level regardless of which process (or
    /// which in-process registry) holds it, which is what actually forces
    /// the second construction below to lose the race.
    #[farhelm_testtrace::test]
    async fn supervisor_hello_reports_the_owners_identity_to_a_claimless_reader() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction does not run the launch shim");
        assert!(
            sup.owns_state_dir(),
            "test fixture: the first construction against a fresh state dir must own it"
        );
        let owner_identity = hello_host_identity_over_a_fresh_connection(&sup).await;

        // Release the owner's claim (see the restart test's own docs on
        // why the strong-count drain, not just the join, is what actually
        // proves this) before an unrelated `flock` takes its place —
        // otherwise `StateDirOwnership::claim`'s in-process registry
        // reuse would hand the flock-holder's later `try_lock` a moot
        // point: this process's own live claim would already have
        // answered the reader's construction before it ever touched the
        // lock file.
        while Arc::strong_count(&sup) > 1 {
            tokio::task::yield_now().await;
        }
        drop(sup);

        let incumbent = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(state.path().join("supervisor.lock"))
            .expect("lock file");
        // `StateDirOwnership::claim`'s own docs name a real (if narrow)
        // race here: the OS-level `flock` this process's own claim held
        // is released synchronously by `sup`'s drop above, but that is a
        // kernel-level fact this test thread only learns about through an
        // ordinary syscall — under a heavily loaded test run (many other
        // tests' real terminal hosts competing for the scheduler) the retry
        // below is what absorbs the gap, rather than this test flaking on
        // a claim that is releasing but has not quite finished the
        // instant this line runs.
        let mut attempts_remaining = 50;
        loop {
            match incumbent.try_lock() {
                Ok(()) => break,
                Err(_) if attempts_remaining > 0 => {
                    attempts_remaining -= 1;
                    // sleep-ok: retry actual lock acquisition with a finite attempt allowance.
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(e) => panic!("the incumbent never took the claim: {e}"),
            }
        }

        let reader = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("a claimless construction must still succeed, per StateDirOwnership::claim");
        assert!(
            !reader.owns_state_dir(),
            "a construction that lost the flock race must NOT claim ownership"
        );

        let reader_identity = hello_host_identity_over_a_fresh_connection(&reader).await;
        assert_eq!(
            owner_identity, reader_identity,
            "a claimless reader must report the SAME identity the owner already minted, not \
             one of its own — it never writes, only reads back what is already durable"
        );
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
    #[farhelm_testtrace::test]
    async fn steady_state_reaping_keeps_the_tracked_task_count_bounded() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor construction does not run the launch shim");
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let mut channels = ConnectionChannels::default();
        let mut tasks = tokio::task::JoinSet::new();

        const REQUESTS: u64 = 50;
        for req_id in 0..REQUESTS {
            handle_control(
                &sup,
                ControlMsg::ListSessions { req_id },
                ConnectionCtx {
                    panics: &mpsc::channel(1).0,
                    tx: &tx,
                    priority: &tx,
                    channels: &mut channels,
                    tasks: &mut tasks,
                },
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

    /// Spec: `spawn_admitted` never waits for a slot. With every permit
    /// held, a further request is answered at once with
    /// `ErrorKind::Unavailable` and [`farhelm_proto::HOST_BUSY_REFUSAL`] under its own
    /// `req_id`, and nothing is spawned for it; once a permit frees, the
    /// next request is admitted and runs.
    ///
    /// Why: the caller is the connection's read loop, which also delivers
    /// every keystroke on the connection. The helper used to wait for a
    /// permit there, which froze typing on the whole host while eight
    /// management operations held the slots. This pins the replacement at
    /// the helper itself, with a 2-permit semaphore and tasks held by a
    /// `Notify`, so no real terminal or timing decides the outcome. A
    /// regression back to waiting makes the third call below not complete
    /// at all, which the bounded await turns into a failure.
    #[farhelm_testtrace::test]
    async fn spawn_admitted_refuses_at_once_when_every_slot_is_taken() {
        let admission = Arc::new(tokio::sync::Semaphore::new(2));
        let mut tasks = tokio::task::JoinSet::new();
        let (tx, mut rx) = mpsc::channel(CONNECTION_WRITER_QUEUE);
        let release = Arc::new(tokio::sync::Notify::new());
        let started = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let held_task = |started: &Arc<std::sync::atomic::AtomicUsize>| {
            let started = Arc::clone(started);
            let release = Arc::clone(&release);
            async move {
                started.fetch_add(1, Ordering::SeqCst);
                release.notified().await;
            }
        };

        for req_id in 1..=2 {
            spawn_admitted(&admission, &mut tasks, &tx, req_id, |_| held_task(&started)).await;
        }
        assert_eq!(
            admission.available_permits(),
            0,
            "fixture premise: the first two requests hold both permits"
        );

        tokio::time::timeout(
            Duration::from_secs(5),
            spawn_admitted(&admission, &mut tasks, &tx, 3, |_| held_task(&started)),
        )
        .await
        .expect("spawn_admitted must answer a request at once when no permit is free");
        let refusal = parse_control(&rx.try_recv().expect("the refusal must already be queued"))
            .expect("decode the refusal");
        assert_eq!(
            refusal,
            ControlMsg::Error {
                req_id: 3,
                kind: ErrorKind::Unavailable,
                message: farhelm_proto::HOST_BUSY_REFUSAL.to_string(),
                create_unaccepted: false,
            }
        );
        assert_eq!(
            tasks.len(),
            2,
            "a refused request must not leave a task behind"
        );

        // Free exactly one permit: one held task finishes and is reaped,
        // and the next request is admitted rather than refused.
        release.notify_one();
        tasks
            .join_next()
            .await
            .expect("a held task")
            .expect("held task must not panic");
        spawn_admitted(&admission, &mut tasks, &tx, 4, |_| held_task(&started)).await;
        assert!(
            rx.try_recv().is_err(),
            "an admitted request must not be refused"
        );
        // `tasks.spawn` only schedules; yielding lets the new task run up
        // to its first await, where it records that it started.
        tokio::task::yield_now().await;
        assert_eq!(
            started.load(Ordering::SeqCst),
            3,
            "the two held tasks and the admitted fourth ran; the refused third never did"
        );

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
    #[farhelm_testtrace::test(start_paused = true)]
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
    #[farhelm_testtrace::test(start_paused = true)]

    async fn drain_writer_waits_through_progress_and_returns_on_completion() {
        let frames_written = Arc::new(AtomicU64::new(0));
        let completed = Arc::new(AtomicBool::new(false));
        let writer_counter = Arc::clone(&frames_written);
        let completed_flag = Arc::clone(&completed);
        let mut task = tokio::spawn(async move {
            for _ in 0..10 {
                // sleep-ok: renew progress across several virtual drain windows before allowing natural completion.
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
    /// Under a paused clock, "measured from the first pause" and "measured
    /// from anything else" resolve at two DIFFERENT virtual instants, so any
    /// wrong anchor misses the assertion below deterministically, which a
    /// wall-clock end-to-end test (the tmux era's byte-stream one) could
    /// only bound loosely.
    ///
    /// No real process or I/O is awaited anywhere in this test, which
    /// matters under `start_paused`: an awaited real-world event
    /// parks the runtime with nothing else ready, and tokio's time driver
    /// is then free to auto-advance the frozen clock straight through the
    /// very window being measured. Everything exercised here — a `watch`
    /// channel and [`stalled_past_deadline`]'s own virtual sleep — is a
    /// timer the paused-clock driver tracks explicitly, so `advance` only
    /// ever moves exactly as far as this test tells it to.
    #[farhelm_testtrace::test(start_paused = true)]
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
    #[farhelm_testtrace::test]
    async fn notify_detached_never_blocks_on_a_full_queue_and_never_drops_the_notice() {
        let (tx, mut rx) = mpsc::channel::<Frame>(1);
        tx.send(Frame::data(1, b"occupying the only slot".to_vec()))
            .await
            .expect("channel is open");

        let () = notify_detached(&tx, 7, "stalled".to_string(), DetachCode::Stalled);

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
                ControlMsg::Detached { channel: 7, reason, code }
                    if reason == "stalled" && code == DetachCode::Stalled
            ),
            "the deferred notice must be this channel's detach, unchanged"
        );
    }
}
