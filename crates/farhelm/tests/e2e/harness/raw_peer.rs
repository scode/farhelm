//! A raw protocol peer for e2e tests that talk to a real supervisor frame
//! by frame, as a helm would, without going through `SupervisorClient`.
//!
//! Shared so each such test does not redo the duplex, the connection task,
//! and the hello. Tests add their own verbs with an `impl RawPeer` block in
//! their own file (see `attachment_uploads.rs`); this module keeps only the
//! transport.

use super::*;

/// One connection to a supervisor, spoken as frames rather than through
/// `SupervisorClient`: the helm's side of the wire, with nothing in between.
///
/// Owns both halves of an in-process duplex pipe, like `connect_client`,
/// so a test can send anything the protocol can express and observe every
/// frame that comes back — including the unsolicited `UploadAck` and
/// `UploadAborted` events, which correlate by channel and which a
/// request/reply client API would have no place to surface.
pub(crate) struct RawPeer {
    /// Open for a test that must watch for SILENCE (a bounded read that is
    /// expected to time out) or write something the methods below do not.
    pub(crate) reader: FrameReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
    /// See [`RawPeer::reader`].
    pub(crate) writer: FrameWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>,
}

impl RawPeer {
    /// Connect and complete the hello, leaving the peer ready to send.
    pub(crate) async fn connect(sup: &Arc<Supervisor>) -> RawPeer {
        RawPeer::connect_with_buffer(sup, 1 << 20).await
    }

    /// [`RawPeer::connect`] with an explicit transport buffer.
    ///
    /// A small buffer is how a test observes the supervisor's own
    /// queueing: with a megabyte of slack, everything the writer produces
    /// disappears into the pipe and nothing about its ORDERING is
    /// visible.
    pub(crate) async fn connect_with_buffer(sup: &Arc<Supervisor>, bytes: usize) -> RawPeer {
        let (client_side, server_side) = tokio::io::duplex(bytes);
        let sup = Arc::clone(sup);
        tokio::spawn(async move {
            let _ = handle_connection(sup, server_side, None).await;
        });
        let (read_half, write_half) = tokio::io::split(client_side);
        let mut peer = RawPeer {
            reader: FrameReader::new(read_half),
            writer: FrameWriter::new(write_half),
        };
        handshake(&mut peer.reader, &mut peer.writer, "helm")
            .await
            .expect("handshake");
        peer
    }

    pub(crate) async fn control(&mut self, msg: &ControlMsg) {
        self.writer.write_control(msg).await.expect("write control");
    }

    /// Send one data frame on `channel`.
    pub(crate) async fn data(&mut self, channel: u32, bytes: Vec<u8>) {
        self.writer
            .write_frame(&Frame::data(channel, bytes))
            .await
            .expect("write data frame");
    }

    /// The next control message, failing the test rather than hanging if
    /// none arrives.
    pub(crate) async fn next_control(&mut self, secs: u64) -> ControlMsg {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let frame = tokio::time::timeout(remaining, self.reader.read_frame())
                .await
                .expect("timed out waiting for a control message")
                .expect("read frame")
                .expect("connection closed while waiting for a control message");
            if frame.kind == FrameKind::Control {
                return parse_control(&frame).expect("parse control");
            }
        }
    }
}
