//! Wire protocol shared by the helm and supervisors.
//!
//! This crate is the seam that keeps helm and supervisor honestly
//! decoupled: they meet only over this protocol, whether the transport is
//! a local unix socket or an ssh exec channel — "local host" and "remote
//! host" differ only in transport (SPEC_impl.md). The codec itself is
//! IO-free: frames encode to and decode from byte buffers, so golden
//! tests pin the wire format without async machinery. Transport-agnostic
//! async read/write helpers live in `io`.
//!
//! Wire format, deliberately minimal (all integers big-endian):
//!
//! ```text
//! u32  length of everything after this field
//! u8   frame kind: 0 = control, 1 = data
//! u32  channel id (0 for control frames)
//! ...  body
//! ```
//!
//! Control frames carry JSON (`ControlMsg`) on channel 0 — JSON so a human
//! can eyeball a protocol trace. Data frames carry raw terminal bytes on a
//! per-attachment channel; keeping them binary keeps PTY throughput off
//! the JSON path.
//!
//! ## Paths that cross this protocol are UTF-8-only
//!
//! Every path field that actually travels over the wire (`SessionInfo::cwd`,
//! `ControlMsg::CreateSession::cwd`) is a Rust `String`, and a `String` is
//! valid UTF-8 by construction — there is no wire representation for a
//! path that isn't. This is a deliberate v1 contract for *this specific
//! boundary*, not an oversight: a bytes-preserving encoding was considered
//! and rejected, because JSON, the web UI, and SQLite all fight arbitrary
//! bytes, while every place a session `cwd` actually enters the system
//! (clap's `String` args, JSON request bodies) already rejects non-UTF-8
//! input on its own.
//!
//! The corollary is what matters operationally: a session `cwd` that is
//! not valid UTF-8 cannot be represented on this protocol at all, and must
//! be rejected — loudly, with an actionable error — at the boundary where
//! it first enters (CLI argument parsing, the HTTP API), never
//! lossy-converted into something that merely *resembles* it. A lossy
//! conversion (`Path::to_string_lossy`) does not fail; it silently
//! produces a different path, and every downstream consumer would then act
//! on that different path with no indication anything went wrong. Callers
//! on both sides of this crate must reject at the boundary rather than
//! launder a non-UTF-8 `cwd` through `to_string_lossy` on the way in.
//!
//! This contract governs only paths that are `ControlMsg`/`SessionInfo`
//! fields. It says nothing about host paths that never cross this wire —
//! `farhelm-helm`'s ssh ControlPath and `--remote-state-dir`, for
//! instance, are textual for reasons specific to ssh, and enforce their
//! own UTF-8 requirement independently (see that crate's docs).
//!
//! Terminal *output*, by contrast, is fine as arbitrary bytes — see
//! `Frame::data`. The UTF-8-only rule is specifically about paths, not
//! about the protocol as a whole.

use serde::{Deserialize, Serialize};

pub mod io;

/// Protocol version exchanged in the hello. Bumped only for incompatible
/// frame or message changes; the receiving side refuses a mismatch with a
/// clear error per SPEC.md's version-skew rule. Build versions travel
/// alongside for diagnostics only and never gate anything.
///
/// Bumped to 2 when `ControlMsg::Error` gained its required `kind` field.
/// That is a wire-format change, not a compatible addition: an old peer's
/// decoder has no default for a missing field and errors on the first
/// `Error` message it receives — which, unlike a refused handshake, tears
/// down an already-established, possibly multi-session connection instead
/// of failing cleanly before anything was shared. Version skew must be
/// caught at the hello, not discovered mid-connection.
pub const PROTOCOL_VERSION: u32 = 2;

/// The build version compiled into this binary, carried in the hello for
/// diagnostics.
pub const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Frame kind tags on the wire. `Control` is JSON on channel 0; `Data` is
/// raw bytes on an attachment channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Control,
    Data,
}

/// One decoded frame. `channel` is 0 for control frames; data frames use
/// the channel assigned at attach time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: FrameKind,
    pub channel: u32,
    pub body: Vec<u8>,
}

/// Coarse classification of `ControlMsg::Error`, deliberately minimal: the
/// three kinds are exactly the statuses the M1 HTTP surface can honestly
/// distinguish (404 / 400 / 500). M2's GUI error surfacing grows this
/// taxonomy (PLAN.md) once there is a UI that can act on finer distinctions.
/// Adding a variant here is still a wire-format change — an older peer's
/// decoder has no fallback for a tag it does not recognize — so it takes
/// the same `PROTOCOL_VERSION` bump as any other incompatible change; there
/// is just no need to over-design the set now, before M2's requirements are
/// known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// The referenced resource (a session id, typically) does not exist.
    NotFound,
    /// The request itself is malformed or fails a precondition the caller
    /// could have avoided (bad cwd, empty invocation, oversized request).
    InvalidRequest,
    /// Anything else: a server-side fault the caller could not have
    /// prevented by sending a different request.
    Internal,
}

/// Frames larger than this are rejected at decode time. Terminal output is
/// chunked well below this by the supervisor; the cap exists so a
/// corrupted length prefix cannot make the reader allocate gigabytes.
pub const MAX_FRAME_LEN: u32 = 8 * 1024 * 1024;

/// Errors surfaced by frame encoding/decoding.
#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame length {0} exceeds maximum {MAX_FRAME_LEN}")]
    TooLarge(usize),
    #[error("frame truncated: need {need} bytes, have {have}")]
    Truncated { need: usize, have: usize },
    #[error("unknown frame kind tag {0}")]
    UnknownKind(u8),
}

impl Frame {
    /// Wrap a control message as a channel-0 frame. Infallible: the
    /// serialization cannot fail, so callers get a `Frame` rather than a
    /// `Result` they would only ever unwrap.
    pub fn control(msg: &ControlMsg) -> Frame {
        Frame {
            kind: FrameKind::Control,
            channel: 0,
            // ControlMsg contains no map types or non-string keys, so
            // serialization cannot fail.
            body: serde_json::to_vec(msg).expect("ControlMsg is always serializable"),
        }
    }

    /// Wrap raw terminal bytes for an attachment channel. `bytes` is
    /// opaque — never inspected, never re-encoded — which is what keeps
    /// arbitrary PTY output (binary, invalid UTF-8) crossing the wire
    /// intact. Encoding rejects a body that would exceed
    /// [`MAX_FRAME_LEN`], before writing any partial frame.
    pub fn data(channel: u32, bytes: Vec<u8>) -> Frame {
        Frame {
            kind: FrameKind::Data,
            channel,
            body: bytes,
        }
    }

    /// Bytes this frame would occupy on the wire *after* the 4-byte length
    /// prefix — i.e. the value that prefix itself carries, and the same
    /// quantity [`Frame::exceeds_max_len`] and `encode` compare against
    /// [`MAX_FRAME_LEN`].
    pub fn encoded_len(&self) -> usize {
        5 + self.body.len()
    }

    /// Whether `encode` would refuse this frame for exceeding
    /// [`MAX_FRAME_LEN`].
    ///
    /// This exists so a sender can check *before* handing a frame to the
    /// writer task: senders enqueue frames onto that task's channel
    /// without observing whether the eventual encode succeeds, and the
    /// writer only discovers an oversized frame later as a write error it
    /// cannot attribute — indistinguishable, at that point, from the
    /// transport genuinely breaking. Checking here lets the sender
    /// substitute something small (a per-request error reply, say)
    /// instead of losing the whole connection over one oversized message.
    /// Deliberately shares `encoded_len`'s arithmetic with `encode` rather
    /// than recomputing the header size independently: the two must never
    /// drift, because a false "fits" here still turns into that same
    /// fatal write error downstream.
    pub fn exceeds_max_len(&self) -> bool {
        self.encoded_len() > MAX_FRAME_LEN as usize
    }

    /// Encode to the wire format, appending to `out`.
    ///
    /// The size check mirrors [`Frame::decode`]: anything written here
    /// must be acceptable to the peer. The output is untouched on error,
    /// so a caller may reuse its scratch buffer after rejecting a frame.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), FrameError> {
        let len = self.encoded_len();
        if len > MAX_FRAME_LEN as usize {
            return Err(FrameError::TooLarge(len));
        }
        let len = len as u32;
        out.extend_from_slice(&len.to_be_bytes());
        out.push(match self.kind {
            FrameKind::Control => 0,
            FrameKind::Data => 1,
        });
        out.extend_from_slice(&self.channel.to_be_bytes());
        out.extend_from_slice(&self.body);
        Ok(())
    }

    /// Decode one frame from the front of `buf`. Returns the frame and the
    /// number of bytes consumed, or `None` when `buf` does not yet hold a
    /// complete frame (the caller reads more and retries). Errors are
    /// unrecoverable protocol violations; the connection should be closed.
    pub fn decode(buf: &[u8]) -> Result<Option<(Frame, usize)>, FrameError> {
        if buf.len() < 4 {
            return Ok(None);
        }
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if len > MAX_FRAME_LEN {
            return Err(FrameError::TooLarge(len as usize));
        }
        if len < 5 {
            // A frame body is at least kind + channel.
            return Err(FrameError::Truncated {
                need: 5,
                have: len as usize,
            });
        }
        let total = 4 + len as usize;
        if buf.len() < total {
            return Ok(None);
        }
        let kind = match buf[4] {
            0 => FrameKind::Control,
            1 => FrameKind::Data,
            other => return Err(FrameError::UnknownKind(other)),
        };
        let channel = u32::from_be_bytes([buf[5], buf[6], buf[7], buf[8]]);
        let body = buf[9..total].to_vec();
        Ok(Some((
            Frame {
                kind,
                channel,
                body,
            },
            total,
        )))
    }
}

/// A session as the supervisor reports it. The supervisor is authoritative
/// (SPEC.md); the helm never invents or mutates these fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    /// Working directory the session was created in. UTF-8-only by
    /// construction (see the module-level "Paths are UTF-8-only" note) —
    /// a non-UTF-8 host path cannot reach this field; it must have been
    /// rejected at the boundary before a `SessionInfo` could exist.
    pub cwd: String,
    pub invocation: String,
}

/// Control-channel messages. `req_id` correlates a response to its request
/// so one connection can carry concurrent requests; unsolicited events
/// (`Detached`) carry no `req_id`.
///
/// Compatibility posture: within one protocol version the set of messages
/// is fixed; anything incompatible bumps `PROTOCOL_VERSION` rather than
/// negotiating per-message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlMsg {
    /// First message in each direction on every connection. The receiver
    /// refuses a `protocol_version` mismatch by replying `Error` and
    /// closing — SPEC.md's version-skew rule, enforced at the edge.
    Hello {
        protocol_version: u32,
        build_version: String,
        /// "helm" or "supervisor"; diagnostic only.
        role: String,
    },
    /// Create and launch a session. This is the one true creation path:
    /// the M1 CLI flags and any future UI dialog both land here
    /// (PLAN_M1.md: flags bypass the creation UI, never the creation API).
    CreateSession {
        req_id: u64,
        /// Working directory to launch the agent in. UTF-8-only (see the
        /// module-level "Paths are UTF-8-only" note); the sender must
        /// reject a non-UTF-8 host path before it ever reaches this
        /// field, not launder it through a lossy conversion.
        cwd: String,
        invocation: String,
        title: Option<String>,
        cols: u16,
        rows: u16,
    },
    /// Success reply to `CreateSession`. The session and terminal exist,
    /// but this does not establish that the agent's later `exec`
    /// succeeded. M1 has no structured launch-status message; failures
    /// remain visible as terminal diagnostics for future classification.
    SessionCreated {
        req_id: u64,
        session: SessionInfo,
    },
    ListSessions {
        req_id: u64,
    },
    /// Reply to `ListSessions`: the supervisor's complete session set, in
    /// no defined order.
    SessionList {
        req_id: u64,
        sessions: Vec<SessionInfo>,
    },
    /// Attach to a session's terminal. The requester picks the (connection
    /// -unique) data channel; the supervisor replays history onto it and
    /// then streams live output. Attaching implicitly detaches any
    /// previous attachment (SPEC.md: one attachment, last attach wins).
    Attach {
        req_id: u64,
        session_id: String,
        channel: u32,
        cols: u16,
        rows: u16,
    },
    /// Attach accepted. Data frames on `channel` may arrive *before* this
    /// reply is processed — the supervisor starts the replay as soon as
    /// the attachment is installed — so a client must have the channel
    /// registered before it sends `Attach`, not after it sees `Attached`.
    Attached {
        req_id: u64,
        channel: u32,
    },
    /// Give up an attachment voluntarily. No reply, and no error if the
    /// channel was never attached or was already taken over: detach is
    /// idempotent so a client tearing down a closed terminal never has to
    /// reason about who won a race.
    Detach {
        channel: u32,
    },
    /// Unsolicited: this channel's attachment was taken over or torn down.
    Detached {
        channel: u32,
        reason: String,
    },
    /// Set the session's terminal dimensions. Fire-and-forget: no
    /// `req_id`, no reply, and the supervisor ignores it unless `channel`
    /// is the session's live attachment on the sending connection —
    /// otherwise a resize still in flight from a client that just lost a
    /// takeover would reflow the winner's terminal. `channel` is what
    /// makes that enforceable when several clients multiplex over one
    /// connection (every browser tab rides the helm's single supervisor
    /// connection, so connection identity alone cannot tell them apart).
    Resize {
        session_id: String,
        channel: u32,
        cols: u16,
        rows: u16,
    },
    /// A request failed, or (with `req_id` 0) something went wrong that no
    /// request is waiting on. `message` is meant for the user verbatim —
    /// SPEC.md requires concrete, actionable errors, so it travels
    /// unabridged from the supervisor to the client.
    Error {
        /// 0 when the error is not tied to a specific request.
        req_id: u64,
        message: String,
        /// Coarse classification an HTTP-facing caller can map to a status
        /// code without parsing `message` (see [`ErrorKind`]).
        kind: ErrorKind,
    },
}

impl ControlMsg {
    /// The hello this build sends.
    pub fn hello(role: &str) -> ControlMsg {
        ControlMsg::Hello {
            protocol_version: PROTOCOL_VERSION,
            build_version: BUILD_VERSION.to_string(),
            role: role.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip through the real encoder/decoder: any drift between
    /// encode and decode shows up here before it shows up between a helm
    /// and a supervisor built from different checkouts.
    #[test]
    fn frame_roundtrip_control_and_data() {
        let mut wire = Vec::new();
        let hello = ControlMsg::hello("helm");
        Frame::control(&hello).encode(&mut wire).unwrap();
        Frame::data(7, b"hello \x1b[1mworld\x1b[0m".to_vec())
            .encode(&mut wire)
            .unwrap();

        let (f1, used1) = Frame::decode(&wire).unwrap().unwrap();
        assert_eq!(f1.kind, FrameKind::Control);
        assert_eq!(f1.channel, 0);
        assert_eq!(
            serde_json::from_slice::<ControlMsg>(&f1.body).unwrap(),
            hello
        );
        let (f2, used2) = Frame::decode(&wire[used1..]).unwrap().unwrap();
        assert_eq!(f2.kind, FrameKind::Data);
        assert_eq!(f2.channel, 7);
        assert_eq!(f2.body, b"hello \x1b[1mworld\x1b[0m");
        assert_eq!(used1 + used2, wire.len());
    }

    /// Golden bytes for the simplest data frame. This is the test that
    /// makes wire-format changes loud: if it fails, PROTOCOL_VERSION must
    /// be bumped, not the expectation silently updated.
    #[test]
    fn frame_wire_format_is_pinned() {
        let mut wire = Vec::new();
        Frame::data(2, vec![0x41, 0x42]).encode(&mut wire).unwrap();
        assert_eq!(
            wire,
            vec![
                0x00, 0x00, 0x00, 0x07, // len = kind(1) + channel(4) + body(2)
                0x01, // kind = data
                0x00, 0x00, 0x00, 0x02, // channel 2
                0x41, 0x42, // body
            ]
        );
    }

    /// Partial buffers must return None (read more), not an error: the
    /// stream reader depends on this to handle short reads.
    #[test]
    fn decode_of_partial_frame_asks_for_more() {
        let mut wire = Vec::new();
        Frame::data(1, vec![1, 2, 3]).encode(&mut wire).unwrap();
        for cut in 0..wire.len() {
            assert!(Frame::decode(&wire[..cut]).unwrap().is_none());
        }
    }

    /// A hostile or corrupt length prefix must be rejected before
    /// allocation, and an unknown kind tag must fail decode.
    #[test]
    fn decode_rejects_garbage() {
        let huge = (MAX_FRAME_LEN + 1).to_be_bytes();
        let mut buf = huge.to_vec();
        buf.extend_from_slice(&[0; 16]);
        assert!(matches!(Frame::decode(&buf), Err(FrameError::TooLarge(_))));

        let mut wire = Vec::new();
        Frame::data(1, vec![9]).encode(&mut wire).unwrap();
        wire[4] = 0xff; // corrupt the kind tag
        assert!(matches!(
            Frame::decode(&wire),
            Err(FrameError::UnknownKind(0xff))
        ));

        // A length too small to hold even kind+channel must error, not
        // report "need more bytes": returning Ok(None) here would make
        // the reader wait forever for bytes that can never complete a
        // frame.
        let undersized = [0u8, 0, 0, 2, 0, 0];
        assert!(matches!(
            Frame::decode(&undersized),
            Err(FrameError::Truncated { .. })
        ));
    }

    /// The frame cap includes kind and channel but excludes the four-byte
    /// length prefix. Pinning both sides of that exact boundary prevents
    /// encoder and decoder limits from drifting by one header width.
    ///
    /// Also pins that `exceeds_max_len` agrees with `encode` at that same
    /// boundary in both directions: a false "fits" from the predicate
    /// would sail past a sender's guard undetected and only surface once
    /// the frame reaches the writer, as a write error it cannot attribute
    /// to this specific oversized message.
    #[test]
    fn frame_size_boundary_accepts_the_maximum_and_rejects_one_more() {
        let largest_body = MAX_FRAME_LEN as usize - 5;
        let at_cap = Frame::data(1, vec![0; largest_body]);
        assert!(!at_cap.exceeds_max_len());
        let mut wire = Vec::new();
        at_cap
            .encode(&mut wire)
            .expect("largest valid frame must encode");
        let (decoded, used) = Frame::decode(&wire).unwrap().unwrap();
        assert_eq!(decoded.body.len(), largest_body);
        assert_eq!(used, wire.len());

        let one_past = Frame::data(1, vec![0; largest_body + 1]);
        assert!(one_past.exceeds_max_len());
        let mut unchanged = b"prefix".to_vec();
        assert!(matches!(
            one_past.encode(&mut unchanged),
            Err(FrameError::TooLarge(_))
        ));
        assert_eq!(unchanged, b"prefix");
    }

    /// The JSON control encoding is part of the wire contract (a different
    /// serde representation is a protocol change even if the Rust types
    /// compile), so pin one message's exact JSON.
    #[test]
    fn control_json_shape_is_pinned() {
        let msg = ControlMsg::Attach {
            req_id: 3,
            session_id: "s1".into(),
            channel: 9,
            cols: 80,
            rows: 24,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "attach",
                "req_id": 3,
                "session_id": "s1",
                "channel": 9,
                "cols": 80,
                "rows": 24,
            })
        );
    }

    /// `ControlMsg::Error`'s `kind` field is the thing `PROTOCOL_VERSION`
    /// was bumped for (see that const's docs), so its exact snake_case wire
    /// form deserves the same golden-JSON pinning as any other message: a
    /// serde attribute change here (dropping `rename_all`, renaming a
    /// variant) would compile and pass every round-trip test while quietly
    /// producing bytes an unmodified peer cannot parse.
    #[test]
    fn error_kind_json_shape_is_pinned() {
        let msg = ControlMsg::Error {
            req_id: 7,
            message: "no such session: abc".to_string(),
            kind: ErrorKind::NotFound,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "error",
                "req_id": 7,
                "message": "no such session: abc",
                "kind": "not_found",
            })
        );
    }
}
