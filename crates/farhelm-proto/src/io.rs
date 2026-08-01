//! Async frame IO over any byte stream.
//!
//! The same reader/writer pair runs over a unix socket (local supervisor)
//! and over an ssh exec channel's stdio (remote supervisor) — this
//! transport-blindness is what makes "local and remote differ only in
//! transport" true in practice. Kept separate from the codec so the codec
//! stays IO-free and golden-testable.

use crate::{ControlMsg, ErrorKind, Frame, PROTOCOL_VERSION};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

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

/// Exchange hellos on a fresh connection and enforce the version-skew
/// rule: send ours, read theirs, refuse a protocol mismatch. Returns the
/// peer's hello on success so callers can log build versions.
///
/// Both sides call this concurrently — hellos cross on the wire rather
/// than being request/response — so neither end can deadlock waiting for
/// the other to speak first.
pub async fn handshake<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: &mut FrameReader<R>,
    writer: &mut FrameWriter<W>,
    role: &str,
) -> std::io::Result<ControlMsg> {
    writer.write_control(&ControlMsg::hello(role)).await?;
    let frame = reader
        .read_frame()
        .await?
        .ok_or_else(|| std::io::Error::other("connection closed before hello"))?;
    let msg = parse_control(&frame)?;
    let ControlMsg::Hello {
        protocol_version,
        build_version,
        ..
    } = &msg
    else {
        return Err(std::io::Error::other(format!(
            "expected hello, got {msg:?}"
        )));
    };
    if *protocol_version == PROTOCOL_VERSION {
        return Ok(msg);
    }
    let err = format!(
        "protocol version mismatch: peer speaks v{} (build {}), this side speaks v{} (build {}); \
         update one side so the versions are compatible",
        protocol_version,
        build_version,
        PROTOCOL_VERSION,
        crate::BUILD_VERSION
    );
    // Best effort: tell the peer why before hanging up. Both the helm and
    // the supervisor call `handshake` and refuse a mismatch the same way —
    // there is no distinguished "server" side here, just whichever end
    // notices the peer's version differs from its own.
    let _ = writer
        .write_control(&ControlMsg::Error {
            req_id: 0,
            message: err.clone(),
            // This side's own refusal to proceed, not a complaint about
            // anything the peer's request body contained.
            kind: ErrorKind::Internal,
        })
        .await;
    Err(std::io::Error::other(err))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The handshake must refuse a version mismatch with an actionable
    /// error — this is M1 acceptance criterion 6 (SPEC.md version-skew
    /// rule) pinned at the unit level. Both directions of skew are
    /// exercised: a NEWER peer, and — since the M2.5 bump to 4 made
    /// "older peer exists" a reality — an OLDER version-3 peer. The old
    /// side matters independently because a comparison bug that rejects
    /// only newer versions (`>` instead of `!=`) would accept a v3 peer
    /// that cannot even decode `PauseOutput`, the exact skew the bump
    /// exists to refuse.
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
            assert!(matches!(
                refusal,
                ControlMsg::Error {
                    req_id: 0,
                    kind: ErrorKind::Internal,
                    ..
                }
            ));

            let err = good.await.unwrap().unwrap_err();
            assert!(
                err.to_string().contains("protocol version mismatch"),
                "version {wrong_version} must be refused"
            );
        }
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
