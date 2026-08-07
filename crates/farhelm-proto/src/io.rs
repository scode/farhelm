//! Async frame IO over any byte stream.
//!
//! The same reader/writer pair runs over a unix socket (local supervisor)
//! and over an ssh exec channel's stdio (remote supervisor) — this
//! transport-blindness is what makes "local and remote differ only in
//! transport" true in practice. Kept separate from the codec so the codec
//! stays IO-free and golden-testable.

use crate::{ControlMsg, ErrorKind, Frame, PROTOCOL_VERSION, SessionAuth};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// An `AsyncWrite` that counts bytes the transport actually accepted, so
/// a writer that has STALLED can be told apart from one that is merely
/// slow.
///
/// That distinction is a product requirement, not a tuning detail.
/// SPEC.md promises a slow viewer is served "for as long as it takes",
/// and only one that stops consuming entirely may be cut. Both ends of
/// this protocol funnel every sender through a single writer task behind
/// a BOUNDED queue, so a peer that stops reading parks that task and
/// everything queued behind it — with no error to report and no EOF to
/// notice — which is why some bound is needed at all. Timing a whole
/// [`FrameWriter::write_frame`] would be the wrong bound: one large frame
/// (a full `SessionList` reply) on a slow link can outlast any window
/// while bytes land steadily the entire time, and cutting there would
/// break exactly the promise above.
///
/// Counting successful `poll_write` calls is the finest progress signal
/// available at this layer, and it is sufficient: any peer still reading
/// advances the counter. `Relaxed` throughout — this is a liveness
/// heartbeat compared against its own earlier value by one observer, not
/// a value anything is synchronized on.
///
/// Lives here, beside `FrameWriter`, because both the supervisor's and
/// the helm's writer tasks need identical behavior and neither crate is
/// the natural owner of the other's copy.
pub struct ProgressWrite<W> {
    inner: W,
    bytes_written: Arc<AtomicU64>,
}

impl<W: AsyncWrite + Unpin> ProgressWrite<W> {
    /// Wrap `inner`, returning the wrapper and a shared counter of bytes
    /// accepted so far. The counter is handed out rather than read back
    /// through the wrapper because the writer is owned by its writer task
    /// while the progress check happens around it.
    pub fn new(inner: W) -> (ProgressWrite<W>, Arc<AtomicU64>) {
        let bytes_written = Arc::new(AtomicU64::new(0));
        (
            ProgressWrite {
                inner,
                bytes_written: Arc::clone(&bytes_written),
            },
            bytes_written,
        )
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for ProgressWrite<W> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let polled = std::pin::Pin::new(&mut self.inner).poll_write(cx, buf);
        if let std::task::Poll::Ready(Ok(written)) = &polled
            && *written > 0
        {
            self.bytes_written
                .fetch_add(*written as u64, Ordering::Relaxed);
        }
        polled
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Write one frame, failing only if the transport accepts NOTHING for a
/// whole `window`.
///
/// The window re-arms on every byte of progress rather than bounding the
/// frame as a whole, so a frame arbitrarily larger than the window still
/// reaches a peer that is merely slow — see [`ProgressWrite`] for why that
/// is the contract rather than a nicety. The `Err` carries a diagnostic
/// for the caller to report; a genuine write error and a total stall are
/// reported the same way on purpose, because both mean this connection can
/// no longer deliver anything.
///
/// The in-flight write is polled again after each expired window rather
/// than dropped and retried: `write_frame` is not cancel-safe, and
/// abandoning it mid-frame would put half a frame on the wire and
/// desynchronize the stream permanently.
pub async fn write_frame_before_stall<W>(
    writer: &mut FrameWriter<W>,
    progress: &AtomicU64,
    frame: &Frame,
    window: std::time::Duration,
) -> Result<(), String>
where
    W: AsyncWrite + Unpin,
{
    let write = writer.write_frame(frame);
    tokio::pin!(write);
    loop {
        let before = progress.load(Ordering::Relaxed);
        match tokio::time::timeout(window, &mut write).await {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(e)) => return Err(e.to_string()),
            Err(_) => {
                if progress.load(Ordering::Relaxed) == before {
                    return Err(format!("peer accepted no bytes for {window:?}"));
                }
                // Bytes landed during the window that just expired: the
                // peer is slow, not gone. Re-arm and keep writing.
            }
        }
    }
}

/// Incremental frame reader: buffers bytes from the stream and yields
/// complete frames. Returns `Ok(None)` on clean EOF at a frame boundary.
pub struct FrameReader<R> {
    inner: R,
    buf: Vec<u8>,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
    pub fn new(inner: R) -> Self {
        FrameReader {
            inner,
            buf: Vec::with_capacity(16 * 1024),
        }
    }

    /// Read the next complete frame. `Ok(None)` means the peer closed
    /// cleanly at a frame boundary and is the only non-error way this
    /// ends; a close mid-frame is an error, because both connection loops
    /// treat `None` as "the peer is done" and would otherwise swallow a
    /// killed ssh or a crashed supervisor.
    ///
    /// Errors are terminal: a decode error is an unrecoverable protocol
    /// violation and there is no resynchronization scheme, so the caller
    /// closes the connection rather than trying to skip ahead.
    pub async fn read_frame(&mut self) -> std::io::Result<Option<Frame>> {
        loop {
            if let Some((frame, used)) = Frame::decode(&self.buf).map_err(std::io::Error::other)? {
                self.buf.drain(..used);
                return Ok(Some(frame));
            }
            // Append directly into the owned buffer. A stack scratch
            // array would live across the await, inflating every future
            // that contains a FrameReader and then copying the same bytes
            // here anyway.
            self.buf.reserve(16 * 1024);
            let n = self.inner.read_buf(&mut self.buf).await?;
            if n == 0 {
                if self.buf.is_empty() {
                    return Ok(None);
                }
                // EOF mid-frame is a real error, not a clean close: the
                // peer died or the transport was cut. Deliberately not
                // FrameError::Truncated — that variant describes a
                // malformed length field, and reusing it here would
                // print an invented byte count in the one diagnostic
                // that has to be legible (a killed ssh, a crashed
                // supervisor).
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!(
                        "connection closed mid-frame with {} buffered bytes",
                        self.buf.len()
                    ),
                ));
            }
        }
    }
}

/// Frame writer. Writes are flushed per frame: terminal interactivity
/// depends on keystroke-sized frames not sitting in a buffer.
pub struct FrameWriter<W> {
    inner: W,
    scratch: Vec<u8>,
}

impl<W: AsyncWrite + Unpin> FrameWriter<W> {
    pub fn new(inner: W) -> Self {
        FrameWriter {
            inner,
            scratch: Vec::with_capacity(16 * 1024),
        }
    }

    /// Encode and write one frame, flushing before returning.
    ///
    /// There is no internal synchronization and none is possible with
    /// `&mut self`: a writer must be owned by exactly one task. Both ends
    /// of the protocol therefore funnel every sender — request handlers,
    /// the output forwarder, takeover notices — through a single writer
    /// task fed by a channel. Two tasks writing would interleave halves of
    /// two frames and desynchronize the stream permanently.
    pub async fn write_frame(&mut self, frame: &Frame) -> std::io::Result<()> {
        self.scratch.clear();
        frame
            .encode(&mut self.scratch)
            .map_err(std::io::Error::other)?;
        self.inner.write_all(&self.scratch).await?;
        self.inner.flush().await
    }

    pub async fn write_control(&mut self, msg: &ControlMsg) -> std::io::Result<()> {
        self.write_frame(&Frame::control(msg)).await
    }

    /// Close the stream's write direction after all queued frames.
    ///
    /// Dropping a generic split write half need not notify its peer while
    /// the matching read half remains alive. An explicit shutdown is what
    /// makes ownership of a writer task line up with transport EOF.
    pub async fn shutdown(&mut self) -> std::io::Result<()> {
        self.inner.shutdown().await
    }
}

/// Parse a control frame's body, surfacing malformed JSON as an IO error
/// (a peer speaking broken JSON on channel 0 is a protocol violation).
pub fn parse_control(frame: &Frame) -> std::io::Result<ControlMsg> {
    serde_json::from_slice(&frame.body).map_err(std::io::Error::other)
}

/// The peer hung up cleanly, at a frame boundary, before it ever sent a
/// hello — i.e. it never spoke this protocol at all.
///
/// Exists as a distinct type because `io::ErrorKind` cannot express it:
/// [`FrameReader::read_frame`] reports a mid-frame hangup with
/// `UnexpectedEof` too, and those two failures call for opposite advice.
/// "Nothing arrived" is the signature of a transport that came up and
/// found nothing behind it (an ssh exec channel whose remote
/// `farhelm internal stdio` could not reach a supervisor and exited);
/// "half a frame arrived, then silence" means something WAS speaking and
/// died mid-sentence, where suggesting "start a supervisor" would send
/// the operator chasing the wrong problem. Callers that add a remedy must
/// therefore match on this type, never on the kind.
///
/// Rides inside the `io::Error` this crate already returns rather than
/// replacing `handshake`'s signature: `farhelm-proto` is deliberately
/// free of an error-reporting framework, and every other caller of
/// `handshake` wants a plain `io::Error`. Use [`ClosedBeforeHello::is_cause_of`]
/// to recognize it — the marker is the error's *payload*, not a link in
/// its `source()` chain (`io::Error` forwards `source()` past its own
/// inner error), so a plain chain walk will not find it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedBeforeHello;

impl std::fmt::Display for ClosedBeforeHello {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // This is also the Display of the wrapping `io::Error`, so it has
        // to read as a complete diagnostic on its own.
        f.write_str("connection closed before hello")
    }
}

impl std::error::Error for ClosedBeforeHello {}

impl ClosedBeforeHello {
    /// Whether `err` is the clean-close-before-any-hello failure, as
    /// opposed to any other `UnexpectedEof`.
    pub fn is_cause_of(err: &std::io::Error) -> bool {
        err.get_ref()
            .is_some_and(|inner| inner.is::<ClosedBeforeHello>())
    }
}

/// The peer spoke this protocol, at a version this side cannot talk to.
///
/// A distinct payload type for the same reason [`ClosedBeforeHello`] is
/// one, but answering a different question. That marker exists so a caller
/// can tell two failures apart; this one exists so a caller can READ the
/// refusal's contents. SPEC.md's skew rule ("incompatible versions refuse
/// to connect with a clear, actionable error") is surfaced per host by the
/// helm's connection manager as a state carrying BOTH versions plus a
/// remediation, and both numbers are known only here, at the one place that
/// compares them. Recovering them from [`Display`]'s prose would make a
/// diagnostic message load-bearing for a state machine — the message would
/// then be unable to change without breaking the manager, and a
/// re-wording would break it silently.
///
/// Rides inside the `io::Error` [`handshake`] already returns rather than
/// widening its signature, exactly as [`ClosedBeforeHello`] does, and for
/// the same reason: `farhelm-proto` carries no error framework, and every
/// other caller wants a plain `io::Error`. Use [`VersionSkew::cause_of`]
/// to recover it — the marker is the error's *payload*, not a link in its
/// `source()` chain.
///
/// [`Display`] is the exact refusal text this handshake has always
/// produced, and stays so deliberately: it is what the peer is told over
/// the wire (`ControlMsg::Error`), what an operator reads on the helm's
/// stderr, and what `annotate_ssh_handshake_eof`'s sibling tests assert on.
/// Introducing this type changed the error's payload, never its rendering.
///
/// [`Display`]: std::fmt::Display
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSkew {
    /// What the peer said it speaks, from its own hello.
    pub peer_protocol: u32,
    /// The peer's build version, for diagnostics — it names which side
    /// needs updating in a way a protocol number alone does not.
    pub peer_build: String,
    /// [`PROTOCOL_VERSION`] as compiled into THIS binary.
    pub our_protocol: u32,
    /// [`crate::BUILD_VERSION`] as compiled into this binary.
    pub our_build: String,
}

impl std::fmt::Display for VersionSkew {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Also the Display of the wrapping `io::Error` and the text sent to
        // the peer, so it has to read as a complete diagnostic on its own.
        write!(
            f,
            "protocol version mismatch: peer speaks v{} (build {}), this side speaks v{} (build \
             {}); update one side so the versions are compatible",
            self.peer_protocol, self.peer_build, self.our_protocol, self.our_build
        )
    }
}

impl std::error::Error for VersionSkew {}

impl VersionSkew {
    /// Recover the skew details from an error [`handshake`] returned, or
    /// `None` if the handshake failed for any other reason.
    ///
    /// Matches by payload type rather than by `io::ErrorKind` (which is
    /// `Other` for several unrelated handshake failures) or by message
    /// text — see this type's own docs for why the prose must not be a
    /// state machine's input.
    pub fn cause_of(err: &std::io::Error) -> Option<&VersionSkew> {
        err.get_ref()?.downcast_ref::<VersionSkew>()
    }
}

/// Exchange hellos on a fresh connection and enforce the version-skew
/// rule: send ours, read theirs, refuse a protocol mismatch. Returns the
/// peer's hello on success so callers can log build versions.
///
/// Both sides call this concurrently — hellos cross on the wire rather
/// than being request/response — so neither end can deadlock waiting for
/// the other to speak first.
///
/// Sends `ControlMsg::hello(role)` — always `host_identity: None` — which
/// is correct for the helm (no identity of its own to report) and for
/// every test double standing in for a supervisor. The one caller that
/// needs a REAL identity on the wire is the production supervisor
/// connection loop, which calls [`handshake_with_host_identity`] instead;
/// see that function's docs for why a separate entry point, rather than a
/// parameter here, is what keeps the other ~80-odd call sites (helm
/// production code and every mock-supervisor test fixture in this
/// workspace) untouched by a field only one of them ever fills.
pub async fn handshake<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: &mut FrameReader<R>,
    writer: &mut FrameWriter<W>,
    role: &str,
) -> std::io::Result<ControlMsg> {
    handshake_with_hello(reader, writer, ControlMsg::hello(role)).await
}

/// [`handshake`], but for the one caller that has a real
/// [`ControlMsg::Hello::host_identity`] to send: the supervisor's
/// production connection loop
/// (`farhelm_supervisor::service::connection::handle_connection`), whose
/// `Supervisor::host_identity` is resolved once at construction and must
/// reach every peer's hello, per SPEC.md's "generated by its supervisor at
/// install time" promise.
///
/// `host_identity` is `Option`, not a plain `String`: a claimless
/// construction (`Supervisor::host_identity`'s own docs) has no standing
/// to mint one and may legitimately have none yet to report, and this
/// function's whole job is to put the SUPERVISOR's actual value — whatever
/// it is — on the wire honestly, not to paper over that case with a
/// fabricated one.
///
/// `role` is not a parameter: this function has exactly one production
/// caller, and that caller is always the supervisor's own connection loop
/// (see above) — hardcoding `"supervisor"` here removes a value every call
/// site would otherwise have to pass identically. If a genuine second
/// caller with a different role ever needs this, that is the point to
/// reintroduce the parameter, not to guess at the need now.
///
/// A separate function rather than a new parameter on [`handshake`]
/// itself: `handshake(reader, writer, role)` is called from roughly 80
/// sites across this workspace — the helm's real connection code (which
/// has no identity to send) and every mock-supervisor test fixture that
/// plays the supervisor's side of a handshake without needing to model
/// identity minting at all. Adding a parameter there would force every
/// one of those call sites to thread through a value that means nothing
/// to them; a second, narrowly-scoped function instead leaves `handshake`
/// exactly as every existing caller already uses it, and the identity
/// requirement lands on the one call site that has an identity to supply.
pub async fn handshake_with_host_identity<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: &mut FrameReader<R>,
    writer: &mut FrameWriter<W>,
    host_identity: Option<String>,
) -> std::io::Result<ControlMsg> {
    handshake_with_hello(
        reader,
        writer,
        ControlMsg::Hello {
            protocol_version: PROTOCOL_VERSION,
            build_version: crate::BUILD_VERSION.to_string(),
            role: "supervisor".to_string(),
            host_identity,
            auth: None,
        },
    )
    .await
}

/// Exchange hellos while asking the supervisor for the restricted spawn
/// authority attributed to `auth.session_id`.
///
/// A successful crossing-hello exchange only proves wire compatibility.
/// The supervisor validates the bearer immediately afterward and may send
/// an uncorrelated `Unauthorized` error before accepting a request; callers
/// must therefore keep reading after this function returns.
pub async fn handshake_with_session_auth<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: &mut FrameReader<R>,
    writer: &mut FrameWriter<W>,
    auth: SessionAuth,
) -> std::io::Result<ControlMsg> {
    handshake_with_hello(
        reader,
        writer,
        ControlMsg::Hello {
            protocol_version: PROTOCOL_VERSION,
            build_version: crate::BUILD_VERSION.to_string(),
            role: "spawn".to_string(),
            host_identity: None,
            auth: Some(auth),
        },
    )
    .await
}

/// Longest `build_version` a peer's hello may carry.
///
/// A build version is a semver string with, at most, a git describe suffix
/// — tens of bytes. The cap exists because this value is RETAINED for the
/// connection's whole life on the other side (the helm's `PeerHello`, and
/// from there a per-host state a UI renders), so an unbounded one is a
/// per-connection memory cost a peer chooses for its counterpart. 256 bytes
/// is generous enough that no honest build can hit it.
pub const MAX_BUILD_VERSION_BYTES: usize = 256;

/// Longest `host_identity` a peer's hello may carry.
///
/// Sized the same way and for the same reason: the identities this project
/// mints are UUIDs (36 bytes), and 256 leaves room for a differently-shaped
/// one without leaving room for a payload.
///
/// LENGTH only — deliberately not shape. An identity is opaque to
/// everything that handles it (see `ControlMsg::Hello::host_identity`),
/// which is a settled disposition rather than an oversight: the helm
/// compares identities and never parses them, so a format check here would
/// invent a compatibility constraint no other code has, and would break the
/// day a supervisor legitimately mints something else. What a length cap
/// costs a well-behaved peer is nothing; what shape validation would cost
/// is a class of hosts that can never connect.
pub const MAX_HOST_IDENTITY_BYTES: usize = 256;

/// Longest encoded session bearer token a peer's hello may carry.
///
/// PLAN_M7.md item 4 will mint a random token whose normal hex or base64
/// encoding is only tens of bytes. 128 bytes accommodates stronger random
/// values and future encoding changes without letting a peer retain an
/// arbitrary secret-sized payload for the connection's whole life.
///
/// LENGTH only — deliberately not shape. The token is an opaque bearer
/// value, so validating its alphabet here would couple the handshake to a
/// minting format that item 4 has not chosen and authentication itself does
/// not need.
pub const MAX_SESSION_AUTH_TOKEN_BYTES: usize = 128;

/// Refuse an over-long hello field, naming which one and by how much.
///
/// A refusal at handshake time, not a truncation: a peer whose identity
/// does not fit is not a peer whose identity can be safely shortened —
/// truncating one would silently map two distinct hosts onto one claim,
/// which is the exact confusion the identity vocabulary exists to prevent.
fn check_hello_length(field: &str, value: &str, cap: usize) -> std::io::Result<()> {
    if value.len() > cap {
        return Err(std::io::Error::other(format!(
            "the peer's hello carries a {field} of {} bytes, over the {cap}-byte limit; refusing \
             the connection",
            value.len()
        )));
    }
    Ok(())
}

/// Shared implementation behind [`handshake`] and
/// [`handshake_with_host_identity`]: send `hello`, read the peer's own
/// hello, and enforce the version-skew rule. Splitting the hello VALUE
/// from this logic is what lets both public entry points share every byte
/// of the actual handshake behavior — there is exactly one place that
/// reads the peer's frame, checks its version, and sends the refusal.
async fn handshake_with_hello<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: &mut FrameReader<R>,
    writer: &mut FrameWriter<W>,
    hello: ControlMsg,
) -> std::io::Result<ControlMsg> {
    writer.write_control(&hello).await?;
    // The kind is `UnexpectedEof` for anything that only looks at kinds,
    // but the actionable signal is the [`ClosedBeforeHello`] payload:
    // `read_frame`'s mid-frame hangup carries the same kind and means
    // something quite different. Callers over an ssh exec channel — where
    // a dead remote proxy is indistinguishable from "no supervisor there"
    // until you look at whether ANY bytes arrived — match the type and add
    // host-aware guidance, instead of matching a kind that is ambiguous or
    // parsing this message's text.
    let frame = reader
        .read_frame()
        .await?
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::UnexpectedEof, ClosedBeforeHello))?;
    let msg = parse_control(&frame)?;
    let ControlMsg::Hello {
        protocol_version,
        build_version,
        host_identity,
        auth,
        ..
    } = &msg
    else {
        return Err(std::io::Error::other(format!(
            "expected hello, got {msg:?}"
        )));
    };
    check_hello_length("build_version", build_version, MAX_BUILD_VERSION_BYTES)?;
    if let Some(identity) = host_identity {
        check_hello_length("host_identity", identity, MAX_HOST_IDENTITY_BYTES)?;
    }
    if let Some(auth) = auth {
        check_hello_length(
            "auth.session_id",
            &auth.session_id,
            crate::MAX_SESSION_ID_BYTES,
        )?;
        check_hello_length("auth.token", &auth.token, MAX_SESSION_AUTH_TOKEN_BYTES)?;
    }
    if *protocol_version == PROTOCOL_VERSION {
        return Ok(msg);
    }
    // Both versions are captured structurally, not just formatted into a
    // message: the helm's connection manager renders them as a per-host
    // version-skew state (PLAN_M6.md item 4), and the wire text below is
    // this value's `Display` rather than the other way round — see
    // [`VersionSkew`] for why the prose must not be the only carrier.
    let skew = VersionSkew {
        peer_protocol: *protocol_version,
        peer_build: build_version.clone(),
        our_protocol: PROTOCOL_VERSION,
        our_build: crate::BUILD_VERSION.to_string(),
    };
    // Best effort: tell the peer why before hanging up. Both the helm and
    // the supervisor call `handshake` and refuse a mismatch the same way —
    // there is no distinguished "server" side here, just whichever end
    // notices the peer's version differs from its own.
    let _ = writer
        .write_control(&ControlMsg::Error {
            req_id: 0,
            message: skew.to_string(),
            // This side's own refusal to proceed, not a complaint about
            // anything the peer's request body contained.
            kind: ErrorKind::Internal,
        })
        .await;
    Err(std::io::Error::other(skew))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The handshake must refuse a version mismatch with an actionable
    /// error — this is M1 acceptance criterion 6 (SPEC.md version-skew
    /// rule) pinned at the unit level. Both directions of skew are
    /// exercised: a NEWER peer, and — since the M2.5 bump to 4 first made
    /// "older peer exists" a reality, and every milestone bump since (5
    /// for M3, 6 for M4, 7 for M5, 8 for M6) keeps it one — an OLDER,
    /// version-7 peer. The old side matters independently because a
    /// comparison bug that rejects only newer versions (`>` instead of
    /// `!=`) would accept a v7 peer that cannot even decode the M6 host-
    /// identity/pagination/creation-timestamp vocabulary (`Hello::
    /// host_identity`, `ListSessions::cursor`/`limit`,
    /// `SessionList::next_cursor` in place of the removed `truncated`),
    /// the exact skew the version 8 bump exists to refuse.
    ///
    /// Also pins the two properties [`VersionSkew`] added on top of that
    /// refusal, both of which have consumers that would otherwise only
    /// break at a distance: the refusal's payload is recoverable as a
    /// TYPED value carrying both versions (PLAN_M6.md item 4's per-host
    /// version-skew state is built from those fields, not from parsing
    /// this message), and the message the peer is sent is byte-identical
    /// to that value's `Display` — the property that let the payload be
    /// introduced without changing a single rendered diagnostic.
    #[tokio::test]
    async fn handshake_refuses_protocol_mismatch() {
        for wrong_version in [PROTOCOL_VERSION + 1, PROTOCOL_VERSION - 1] {
            let (a, b) = tokio::io::duplex(64 * 1024);
            let (ar, aw) = tokio::io::split(a);
            let (br, bw) = tokio::io::split(b);

            let good = tokio::spawn(async move {
                let mut r = FrameReader::new(ar);
                let mut w = FrameWriter::new(aw);
                handshake(&mut r, &mut w, "helm").await
            });

            // The mismatched-build peer: sends a hello with a wrong version.
            let mut r = FrameReader::new(br);
            let mut w = FrameWriter::new(bw);
            w.write_control(&ControlMsg::Hello {
                protocol_version: wrong_version,
                build_version: "9.9.9".into(),
                role: "supervisor".into(),
                host_identity: None,
                auth: None,
            })
            .await
            .unwrap();
            // It receives our hello, then the refusal — pinning both fields a
            // caller relies on to tell this apart from a request-scoped
            // failure: `req_id: 0` (nothing was waiting on this reply) and
            // `kind: Internal` (this side's own refusal, not a complaint about
            // the peer's request).
            let _their_hello = r.read_frame().await.unwrap().unwrap();
            let refusal = parse_control(&r.read_frame().await.unwrap().unwrap()).unwrap();
            let ControlMsg::Error {
                req_id: 0,
                kind: ErrorKind::Internal,
                message: refusal_text,
            } = refusal
            else {
                panic!("expected an unsolicited Internal refusal, got {refusal:?}");
            };

            let err = good.await.unwrap().unwrap_err();
            assert!(
                err.to_string().contains("protocol version mismatch"),
                "version {wrong_version} must be refused"
            );
            let skew = VersionSkew::cause_of(&err)
                .expect("a version refusal must carry its versions as a typed payload");
            assert_eq!(
                skew,
                &VersionSkew {
                    peer_protocol: wrong_version,
                    peer_build: "9.9.9".to_string(),
                    our_protocol: PROTOCOL_VERSION,
                    our_build: crate::BUILD_VERSION.to_string(),
                }
            );
            assert_eq!(
                refusal_text,
                skew.to_string(),
                "the text the peer is told must remain exactly the payload's own rendering"
            );
        }
    }

    /// The hello's retained free-text fields are LENGTH-capped at decode,
    /// and a peer that exceeds one is refused rather than truncated.
    ///
    /// These fields are retained by the peer's counterpart for the whole
    /// life of the connection — the helm keeps them in `PeerHello` and
    /// renders them in a per-host state — so an unbounded one is a memory
    /// cost a peer chooses for the other side, per connection, and a
    /// reconnecting host would choose it again on every attempt.
    ///
    /// Refusal rather than truncation for `host_identity` specifically:
    /// shortening an identity would map two distinct hosts onto one claim,
    /// which is precisely the confusion the identity vocabulary exists to
    /// prevent. And length ONLY — nothing here inspects the shape, because
    /// an identity is opaque to every consumer by design (see
    /// [`ControlMsg::Hello`]); a format check would invent a compatibility
    /// rule no other code has.
    ///
    /// The authentication pair gets the same boundary treatment. Its
    /// session id shares the protocol-wide id cap, while its opaque bearer
    /// token has its own encoding-sized cap; both must be checked before a
    /// future item-4 connection keeps either value as connection state.
    #[tokio::test]
    async fn handshake_refuses_over_long_hello_fields() {
        let cases = [
            (
                "build_version",
                "x".repeat(MAX_BUILD_VERSION_BYTES + 1),
                None,
                None,
            ),
            (
                "host_identity",
                crate::BUILD_VERSION.to_string(),
                Some("x".repeat(MAX_HOST_IDENTITY_BYTES + 1)),
                None,
            ),
            (
                "auth.session_id",
                crate::BUILD_VERSION.to_string(),
                None,
                Some(crate::SessionAuth {
                    session_id: "x".repeat(crate::MAX_SESSION_ID_BYTES + 1),
                    token: "token".to_string(),
                }),
            ),
            (
                "auth.token",
                crate::BUILD_VERSION.to_string(),
                None,
                Some(crate::SessionAuth {
                    session_id: "session".to_string(),
                    token: "x".repeat(MAX_SESSION_AUTH_TOKEN_BYTES + 1),
                }),
            ),
        ];
        for (field, build_version, host_identity, auth) in cases {
            let (a, b) = tokio::io::duplex(64 * 1024);
            let (ar, aw) = tokio::io::split(a);
            let (br, bw) = tokio::io::split(b);

            let ours = tokio::spawn(async move {
                let mut r = FrameReader::new(ar);
                let mut w = FrameWriter::new(aw);
                handshake(&mut r, &mut w, "helm").await
            });

            let mut r = FrameReader::new(br);
            let mut w = FrameWriter::new(bw);
            w.write_control(&ControlMsg::Hello {
                // A CURRENT protocol version on purpose: the length check
                // must bind on a peer this side would otherwise accept, not
                // ride along with a skew refusal.
                protocol_version: PROTOCOL_VERSION,
                build_version,
                role: "supervisor".into(),
                host_identity,
                auth,
            })
            .await
            .unwrap();
            let _their_hello = r.read_frame().await.unwrap().unwrap();

            let err = ours.await.unwrap().unwrap_err();
            let rendered = err.to_string();
            assert!(
                rendered.contains(field) && rendered.contains("limit"),
                "the refusal must name the offending field and the bound: {rendered}"
            );
        }
    }

    /// Every hello cap is inclusive: ordinary diagnostic fields and
    /// authentication fields exactly at their limits pass. This is the
    /// half of the refusal test that keeps each bound honest rather than
    /// merely strict.
    #[tokio::test]
    async fn handshake_accepts_ordinary_hello_field_lengths() {
        let (a, b) = tokio::io::duplex(64 * 1024);
        let (ar, aw) = tokio::io::split(a);
        let (br, bw) = tokio::io::split(b);

        let ours = tokio::spawn(async move {
            let mut r = FrameReader::new(ar);
            let mut w = FrameWriter::new(aw);
            handshake(&mut r, &mut w, "helm").await
        });

        let mut r = FrameReader::new(br);
        let mut w = FrameWriter::new(bw);
        w.write_control(&ControlMsg::Hello {
            protocol_version: PROTOCOL_VERSION,
            build_version: crate::BUILD_VERSION.to_string(),
            role: "supervisor".into(),
            host_identity: Some("2f8a1c3e-9d4b-4f7a-8e21-6b5c0d9a7e13".to_string()),
            auth: Some(crate::SessionAuth {
                session_id: "s".repeat(crate::MAX_SESSION_ID_BYTES),
                token: "t".repeat(MAX_SESSION_AUTH_TOKEN_BYTES),
            }),
        })
        .await
        .unwrap();
        let _their_hello = r.read_frame().await.unwrap().unwrap();

        let hello = ours.await.unwrap().expect("an ordinary hello must pass");
        assert!(matches!(hello, ControlMsg::Hello { .. }));
    }

    /// A peer that dies mid-frame must look like an error, not a clean
    /// shutdown: both the helm's demux loop and the supervisor's
    /// connection loop treat clean EOF as "the peer is done", which
    /// would silently swallow a killed ssh or a crashed supervisor.
    #[tokio::test]
    async fn eof_mid_frame_is_an_error_but_clean_eof_is_not() {
        // NOTE: the peer half must be dropped WHOLE. Holding either half
        // of a split duplex keeps the stream open, and the reader below
        // would block forever instead of seeing EOF.
        let (a, mut b) = tokio::io::duplex(64);
        // Only the length prefix, then hang up.
        tokio::io::AsyncWriteExt::write_all(&mut b, &[0, 0, 0, 9])
            .await
            .unwrap();
        drop(b);
        let mut reader = FrameReader::new(a);
        assert!(
            reader.read_frame().await.is_err(),
            "a peer that dies mid-frame must not look like a clean close"
        );

        let (a, b) = tokio::io::duplex(64);
        drop(b);
        let mut reader = FrameReader::new(a);
        assert!(reader.read_frame().await.unwrap().is_none());
    }

    /// The two ways a handshake can end in EOF must stay TELLABLE APART,
    /// because a caller turns one of them into "no supervisor is running
    /// over there, start one" and would be lying if it said that about the
    /// other. A peer that says nothing and hangs up never spoke this
    /// protocol; a peer that emits half a hello and dies plainly did.
    ///
    /// The kind is deliberately identical in both cases (`UnexpectedEof`
    /// describes both truthfully), which is exactly why this test asserts
    /// on [`ClosedBeforeHello::is_cause_of`] and pins that the kind alone
    /// does NOT discriminate — a caller tempted back to kind-matching
    /// fails here.
    #[tokio::test]
    async fn handshake_marks_a_clean_close_but_not_a_mid_frame_death() {
        // The peer half is kept alive (it lives to the end of this scope)
        // and only its WRITE direction is shut down. Dropping it whole
        // would fail this side's hello write instead, and the read-side
        // failure under test would never be reached — the half-open shape
        // here is that of a live ssh channel whose remote end exited.
        async fn handshake_against(peer_prefix: &[u8]) -> std::io::Error {
            let (a, mut b) = tokio::io::duplex(64 * 1024);
            b.write_all(peer_prefix).await.unwrap();
            b.shutdown().await.unwrap();
            let (ar, aw) = tokio::io::split(a);
            let mut r = FrameReader::new(ar);
            let mut w = FrameWriter::new(aw);
            handshake(&mut r, &mut w, "helm").await.unwrap_err()
        }

        let clean = handshake_against(&[]).await;
        assert_eq!(clean.kind(), std::io::ErrorKind::UnexpectedEof);
        assert!(ClosedBeforeHello::is_cause_of(&clean));

        // Half of a real hello frame: something spoke, then died.
        let mut hello = Vec::new();
        Frame::control(&ControlMsg::hello("supervisor"))
            .encode(&mut hello)
            .unwrap();
        let partial = handshake_against(&hello[..hello.len() / 2]).await;
        assert_eq!(partial.kind(), std::io::ErrorKind::UnexpectedEof);
        assert!(
            !ClosedBeforeHello::is_cause_of(&partial),
            "a peer that spoke a partial hello must not be reported as one that never spoke: {partial}"
        );
    }

    /// Matching versions succeed and surface the peer's hello.
    #[tokio::test]
    async fn handshake_accepts_matching_versions() {
        let (a, b) = tokio::io::duplex(64 * 1024);
        let (ar, aw) = tokio::io::split(a);
        let (br, bw) = tokio::io::split(b);

        let helm = tokio::spawn(async move {
            let mut r = FrameReader::new(ar);
            let mut w = FrameWriter::new(aw);
            handshake(&mut r, &mut w, "helm").await
        });
        let sup = tokio::spawn(async move {
            let mut r = FrameReader::new(br);
            let mut w = FrameWriter::new(bw);
            handshake(&mut r, &mut w, "supervisor").await
        });

        let helm_saw = helm.await.unwrap().unwrap();
        let sup_saw = sup.await.unwrap().unwrap();
        assert!(matches!(helm_saw, ControlMsg::Hello { ref role, .. } if role == "supervisor"));
        assert!(matches!(sup_saw, ControlMsg::Hello { ref role, .. } if role == "helm"));
    }
}
