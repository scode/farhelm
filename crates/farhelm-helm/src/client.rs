//! The helm's client half of the supervisor protocol.
//!
//! One connection per supervisor, multiplexing concurrent requests
//! (correlated by `req_id`) and any number of terminal attachments
//! (routed by data-channel id). The transport is opaque: a unix socket
//! for the local host, an ssh exec channel for a remote one — handed in
//! as a reader/writer pair so this code cannot tell the difference,
//! which is the SPEC_impl.md transport-blindness made structural.

use anyhow::{Context, bail};
use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
use farhelm_proto::{ControlMsg, ErrorKind, Frame, FrameKind, SessionInfo};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Mutex, mpsc, oneshot, watch};
use tracing::warn;

/// Input chunk size. Well under `MAX_FRAME_LEN`, and matched to the
/// supervisor's replay chunking so both directions behave alike.
const INPUT_CHUNK: usize = 32 * 1024;

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
/// /api/sessions` itself still serializes only `sessions` for now (see
/// that handler); passing the other two through the wire is the next
/// PR's job, not this one's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionListing {
    pub sessions: Vec<SessionInfo>,
    /// The supervisor's full session count before any truncation — see
    /// `ControlMsg::SessionList`'s own docs for why this can differ from
    /// `sessions.len()`.
    pub total: u64,
    /// Whether `sessions` is missing entries the supervisor held back
    /// (PLAN_M2.md's list cap/byte-budget). `false` for a supervisor built
    /// before this field existed, exactly like a fresh-default `total` —
    /// see `ControlMsg::SessionList`'s docs on that tolerance.
    pub truncated: bool,
}

/// What an attached terminal receives from the supervisor side.
#[derive(Debug)]
pub enum TermEvent {
    /// Raw terminal bytes (replay first, then live output).
    Data(Vec<u8>),
    /// The attachment ended: taken over by another client, or the
    /// session's terminal went away.
    Detached(String),
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
    writer_tx: mpsc::UnboundedSender<Frame>,
    pending: Mutex<Pending>,
    terminals: Mutex<HashMap<u32, mpsc::UnboundedSender<TermEvent>>>,
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

impl SupervisorClient {
    /// Perform the hello handshake and start the demux loop. Fails fast
    /// on protocol-version mismatch — the SPEC.md skew rule fires here,
    /// at connection time, not on first use.
    pub async fn start<R, W>(r: R, w: W) -> anyhow::Result<Arc<SupervisorClient>>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let mut reader = FrameReader::new(r);
        let mut writer = FrameWriter::new(w);
        handshake(&mut reader, &mut writer, "helm").await?;

        let (writer_tx, mut writer_rx) = mpsc::unbounded_channel::<Frame>();
        let (connection_done, _) = watch::channel(false);

        let client = Arc::new(SupervisorClient {
            writer_tx,
            pending: Mutex::new(Pending::default()),
            terminals: Mutex::new(HashMap::new()),
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
                let frame = tokio::select! {
                    _ = writer_cancel.changed() => break,
                    frame = writer_rx.recv() => frame,
                };
                let Some(frame) = frame else {
                    break;
                };
                // Logged, not swallowed: a write failure here (broken
                // ssh pipe, dead socket) is the one diagnostic that
                // explains why every later request starts failing.
                if let Err(e) = writer.write_frame(&frame).await {
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

    /// Declare the connection dead: detach every terminal, fail every
    /// pending request, and make later requests fail fast. Idempotent, and
    /// called from BOTH halves — the demux loop on read EOF/error, the
    /// writer task on write failure — because either half dying alone
    /// (a half-broken ssh pipe) leaves the other alive and waiters hung.
    ///
    /// Terminals get an explicit event; pending requests are failed by
    /// dropping their oneshot senders, which makes `request()` return the
    /// "connection closed" error instead of hanging an HTTP handler.
    async fn fail_all(&self, reason: &str) {
        let mut terms = self.terminals.lock().await;
        for (_, tx) in terms.drain() {
            let _ = tx.send(TermEvent::Detached(reason.to_string()));
        }
        drop(terms);
        // Flag and drain in one lock hold; see `Pending` for why.
        let mut pending = self.pending.lock().await;
        pending.closed = true;
        pending.map.clear();
    }

    /// Route one inbound frame to whoever is waiting for it: data frames
    /// to their terminal, replies to the request that carries their
    /// `req_id`, `Detached` to the terminal it names.
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
    async fn dispatch(&self, frame: Frame) -> anyhow::Result<()> {
        match frame.kind {
            FrameKind::Data => {
                let terms = self.terminals.lock().await;
                if let Some(tx) = terms.get(&frame.channel) {
                    let _ = tx.send(TermEvent::Data(frame.body));
                }
            }
            FrameKind::Control => {
                let msg = parse_control(&frame)?;
                match &msg {
                    ControlMsg::SessionCreated { req_id, .. }
                    | ControlMsg::SessionList { req_id, .. }
                    | ControlMsg::SessionStopped { req_id, .. }
                    | ControlMsg::SessionDeleted { req_id, .. }
                    | ControlMsg::Attached { req_id, .. }
                    | ControlMsg::Error { req_id, .. }
                        if *req_id != 0 =>
                    {
                        if let Some(tx) = self.pending.lock().await.map.remove(req_id) {
                            let _ = tx.send(msg);
                        }
                    }
                    ControlMsg::Detached { channel, reason } => {
                        if let Some(tx) = self.terminals.lock().await.remove(channel) {
                            let _ = tx.send(TermEvent::Detached(reason.clone()));
                        }
                    }
                    other => warn!(?other, "unexpected control message at helm"),
                }
            }
        }
        Ok(())
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
        if let Err(e) = self.writer_tx.send(Frame::control(&msg)) {
            // Do not leave a pending entry behind for a request that was
            // never sent; it would accumulate for the process's life.
            self.pending.lock().await.map.remove(&req_id);
            return Err(anyhow::Error::new(e).context("supervisor connection closed"));
        }
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
    pub async fn create_session(
        &self,
        cwd: &str,
        invocation: &str,
        title: Option<String>,
        cols: u16,
        rows: u16,
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
    pub async fn list_sessions(&self) -> anyhow::Result<SessionListing> {
        let req_id = self.req_id();
        match self
            .request(req_id, ControlMsg::ListSessions { req_id })
            .await?
        {
            ControlMsg::SessionList {
                sessions,
                total,
                truncated,
                ..
            } => Ok(SessionListing {
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
                total: total.max(sessions.len() as u64),
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

    /// Attach to a session's terminal. The returned receiver yields the
    /// replay (history + mode re-synthesis) followed by live output, and
    /// finally a `Detached` if the attachment ends server-side.
    pub async fn attach(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<(u32, mpsc::UnboundedReceiver<TermEvent>)> {
        let channel = allocate_channel(&self.next_channel)?;
        let (tx, rx) = mpsc::unbounded_channel();
        // Register before sending Attach: replay data frames may arrive
        // before the Attached reply is processed. The insert cannot clobber
        // anything — `allocate_channel` never hands out an id twice on this
        // connection.
        self.terminals.lock().await.insert(channel, tx);

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
            ControlMsg::Attached { .. } => Ok((channel, rx)),
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
            .send(Frame::control(&ControlMsg::Detach { channel }));
    }

    /// Forward terminal input, chunked below the protocol's frame cap.
    ///
    /// Chunking is not an optimization: a browser can deliver a whole
    /// clipboard paste as one WebSocket message, and a single frame over
    /// `MAX_FRAME_LEN` is a fatal decode error on the supervisor side —
    /// one oversized paste would kill the shared connection and every
    /// session on it.
    pub fn send_input(&self, channel: u32, bytes: Vec<u8>) {
        for chunk in bytes.chunks(INPUT_CHUNK) {
            if self
                .writer_tx
                .send(Frame::data(channel, chunk.to_vec()))
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
    pub fn resize(&self, session_id: &str, channel: u32, cols: u16, rows: u16) {
        let _ = self.writer_tx.send(Frame::control(&ControlMsg::Resize {
            session_id: session_id.to_string(),
            channel,
            cols,
            rows,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::io::{FrameReader, FrameWriter, handshake, parse_control};
    use std::io;
    use std::pin::Pin;
    use std::sync::atomic::AtomicBool;
    use std::task::{Context, Poll};
    use tokio::io::AsyncWrite;
    use tokio::time::{Duration, timeout};

    /// A write half whose next frame can fail while its read peer stays
    /// open, matching the asymmetric failure possible over ssh.
    struct ToggleWriteFailure<W> {
        inner: W,
        fail_writes: Arc<AtomicBool>,
    }

    impl<W: AsyncWrite + Unpin> AsyncWrite for ToggleWriteFailure<W> {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
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

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_flush(cx)
        }

        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.inner).poll_shutdown(cx)
        }
    }

    fn session(id: &str) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            title: id.into(),
            cwd: format!("/{id}"),
            invocation: "agent".into(),
            status: farhelm_proto::SessionStatus::Alive,
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
                ControlMsg::ListSessions { req_id } => req_id,
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

    /// `list_sessions` must preserve `truncated` exactly, and `total`
    /// whenever it is not SMALLER than `sessions.len()` — sentinel values
    /// here (`total: 42`, `truncated: true`, deliberately far from
    /// `sessions.len()`) prove an honest, larger `total` a truncating
    /// supervisor sent on purpose survives untouched, rather than being
    /// recomputed or dropped. This is deliberately NOT a claim that every
    /// `total` value is preserved verbatim: `total.max(sessions.len())`
    /// (see that call site's own docs, for the case where an old
    /// supervisor's `total` UNDER-reports by omitting the field) rewrites
    /// a `total` smaller than the list actually in hand — a different
    /// test (`list_sessions_reports_a_populated_legacy_reply_with_a_real_total`)
    /// pins that rewrite specifically.
    #[tokio::test]
    async fn list_sessions_preserves_the_supervisors_total_and_truncated() {
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
                    sessions: vec![session("only-one")],
                    total: 42,
                    truncated: true,
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
        assert!(listing.truncated);
        peer.await.unwrap();
    }

    /// The other half of the normalization at `list_sessions`'s call
    /// site: an older `PROTOCOL_VERSION` 3 supervisor, built before
    /// `total`/`truncated` existed, sends a `SessionList` with those
    /// fields simply ABSENT — `#[serde(default)]` decodes that as `total:
    /// 0, truncated: false` (see `ControlMsg::SessionList`'s own docs) —
    /// even though its `sessions` vec is complete and non-empty. Forwarding
    /// that raw `0` would be actively misleading ("showing 0 of 0" next to
    /// a visibly populated list), so `total.max(sessions.len())` must
    /// rewrite it up to the real count. This is the scripted-peer
    /// complement to the pure `total.max(...)` arithmetic itself: it pins
    /// that the client's `list_sessions` call site actually applies the
    /// rewrite to a wire reply shaped exactly like a real legacy sender's,
    /// not just to a value constructed directly in a unit test.
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
            let ControlMsg::ListSessions { req_id } = request else {
                panic!("unexpected request: {request:?}");
            };
            // Write the JSON by hand, WITHOUT `total`/`truncated` at all —
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
        let (term_tx, mut term_rx) = mpsc::unbounded_channel();
        client.terminals.lock().await.insert(7, term_tx);

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
}
