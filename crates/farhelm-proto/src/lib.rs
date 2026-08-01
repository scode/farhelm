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
///
/// Bumped to 3 for `StopSession`/`SessionStopped` and
/// `DeleteSession`/`SessionDeleted` (PLAN_M2.md step 4). This is the ONE
/// version bump M2 gets: PLAN_M2.md commits every later M2 wire change to
/// being strictly additive and tolerant on decode within version 3 (new
/// optional fields with defaults, new message variants nothing yet sends)
/// so that mixed M2-era builds keep interoperating without a bump each
/// time. Anything that cannot be made additive earns its own bump instead
/// of retroactively stretching this one's meaning.
///
/// Bumped to 4 for `ControlMsg::PauseOutput`/`ResumeOutput` (PLAN_M2_5.md
/// step 2). M2's "new message variants are additive" premise above turned
/// out to be false: `io::parse_control` decodes `ControlMsg` through a
/// single `#[serde(tag = "type")]` enum, so an unrecognized tag is a decode
/// error, not a defaulted no-op, and both the helm and the supervisor
/// connection loops treat that error as fatal — an unknown variant tears
/// down an already-established connection instead of being ignored. A new
/// variant is exactly the "cannot be additive" case that earns its own
/// bump, per version 3's own docs above; `protocol_version_is_pinned_at_4`
/// and `unknown_control_message_tag_fails_decode` below (plus the
/// loop-level teardown test in the farhelm crate's e2e suite) pin both the
/// number and the reasoning so the next milestone cannot re-assume
/// tolerance that was never there. Within version 4, the same additive
/// discipline applies: later M2.5 wire changes must be new optional fields
/// with decode defaults, not new variants, or they earn their own bump too.
pub const PROTOCOL_VERSION: u32 = 4;

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

/// The `reason` string a `ControlMsg::Detached` carries when a stalled
/// viewer is given up on (PLAN_M2_5.md's stall-detach contract). Two
/// emitters will send it: the supervisor, when a single pause lasts
/// longer than the stall timeout (a hard maximum pause duration — there
/// is deliberately no progress measurement while paused), and the helm,
/// when a terminal's bounded event channel fills because its consumer
/// stopped draining without ever pausing. Named here, in the proto
/// crate, so the emitters and the tests that match on it cannot drift
/// independently.
///
/// Clients display `reason` inside their own detach banner (terminal.js
/// prefixes "Detached: "), so this string must read as a bare cause with
/// no leading "detached:" of its own, one line, user-legible.
///
/// This PR only reserves the string as wire vocabulary; nothing sends it
/// yet. The supervisor stall timer and the helm channel bound land in
/// later M2.5 PRs.
pub const DETACH_REASON_STALLED: &str = "terminal stopped consuming output (stalled)";

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

/// Whether a session's agent is running, and — once it is not — how it
/// ended.
///
/// Additive within `PROTOCOL_VERSION` 3 (PLAN_M2.md's "Proto growth"):
/// this enum, `SessionInfo::status`, and `SessionList::total`/`truncated`
/// are the one wire change M2's list step gets, and it must stay
/// tolerant on decode both ways — see `SessionInfo::status`'s docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SessionStatus {
    /// The default exists for wire tolerance first: a peer built before
    /// this field existed sends `SessionInfo` JSON with no `state` at
    /// all, and `#[serde(default)]` on the field decodes that as
    /// `Unknown` — never as a fabricated liveness claim one way or the
    /// other. This build's own supervisor ALSO produces `Unknown`
    /// deliberately, for the exact same reason: `SessionCreated`'s reply
    /// carries it as a create-time placeholder, because creation
    /// establishes only that the session and terminal exist, not that
    /// the agent's later `exec` inside it succeeded (see
    /// `ControlMsg::SessionCreated`'s own docs) — a fast-exiting command
    /// can already be dead by the time that reply reaches the caller, so
    /// claiming `Alive` there would itself be a fabricated liveness claim.
    /// `ListSessions` is the only reply that computes a REAL answer (from
    /// tmux, via `service.rs`'s `session_status`); every other place this
    /// value is produced is honestly saying "not yet known", not "known
    /// to be running".
    #[default]
    Unknown,
    /// The agent's process is running (tmux's pane is not marked dead).
    Alive,
    /// The agent's process has ended. `exit_code` is tmux's own
    /// `#{pane_dead_status}` when parseable — `None` covers a signal
    /// death tmux cannot reduce to a plain code, and the restart-gap and
    /// stale-lookup cases where there is no live pane to ask at all (see
    /// the supervisor's `ListSessions` handler).
    Exited { exit_code: Option<i32> },
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
    /// Computed fresh by the supervisor on every `ListSessions` reply —
    /// never persisted (SQLite has no liveness truth to persist; tmux is
    /// the only truth, and it does not survive a restart on its own
    /// terms either) and never trusted from an older sender.
    /// `#[serde(default)]` is what makes this field additive within
    /// `PROTOCOL_VERSION` 3: an old peer's JSON has no `status` at all and
    /// decodes to `SessionStatus::Unknown` rather than failing, and this
    /// crate carries no `deny_unknown_fields` anywhere on this path, so a
    /// NEW `status` reaching an OLD decoder is silently ignored rather
    /// than rejected. Both directions must keep holding for any later
    /// M2 wire addition, per `PROTOCOL_VERSION`'s own docs.
    #[serde(default)]
    pub status: SessionStatus,
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
    ///
    /// Consequently `session.status` here is `SessionStatus::Unknown`, not
    /// `Alive` — a create-time placeholder consistent with the paragraph
    /// above, since a fast-exiting command can already be dead by the time
    /// this reply reaches the caller. `ListSessions` computes the real
    /// answer from tmux (`service.rs`'s `session_status`); nothing about
    /// creation itself can honestly claim more.
    SessionCreated {
        req_id: u64,
        session: SessionInfo,
    },
    ListSessions {
        req_id: u64,
    },
    /// Reply to `ListSessions`: the supervisor's session set, in no
    /// defined order, subject to two independent cuts in
    /// `service.rs` — a count cap enforced by the `ListSessions` handler
    /// itself before a single entry is even cloned, and an encoded-size
    /// budget `build_list_reply` enforces on top of that (see its own
    /// docs for why the cap deliberately does NOT live inside that
    /// function).
    ///
    /// `total` and `truncated` are additive within `PROTOCOL_VERSION` 3,
    /// like `SessionInfo::status` (see that field's docs for the same
    /// tolerance argument). `total` is the FULL session count before any
    /// truncation, not `sessions.len()`; an old sender's reply decodes
    /// `total` as 0 via `#[serde(default)]`, which is documented here as
    /// "sender predates the field" rather than "zero sessions" — tolerable
    /// because `sessions` itself is still present and correct either way,
    /// and no M2 caller treats a 0 `total` as authoritative proof of
    /// emptiness on its own.
    SessionList {
        req_id: u64,
        sessions: Vec<SessionInfo>,
        #[serde(default)]
        total: u64,
        #[serde(default)]
        truncated: bool,
    },
    /// Kill the agent's entire process tree (MCP servers, dev servers,
    /// every descendant), leaving the session and its terminal in place —
    /// SPEC.md's "stop" (as distinct from "delete", below). The pane
    /// survives (`remain-on-exit`), so the terminal stays viewable,
    /// including replaying to a client that was attached when the agent
    /// died. Stop touches nothing else: not the session's DB row, not the
    /// in-memory session map, not any live attachment.
    ///
    /// Idempotent by design, not by accident: stopping a session whose
    /// agent already exited (or one whose terminal never existed, the
    /// restart-gap case) still replies `SessionStopped` rather than
    /// erring, because from the caller's point of view "make sure nothing
    /// is running" already holds.
    ///
    /// Two distinct failure modes, not one: an unknown `session_id` is the
    /// only PRECONDITION failure, reported the same way `Attach` reports
    /// it. But the kill sweep itself — enumerating and signaling the
    /// process tree, see `kill_process_tree` in the supervisor — can also
    /// fail (a `/proc` read erroring out, a signal coming back `EPERM`),
    /// and that is reported as an `Error` too rather than a false
    /// `SessionStopped`: a caller must be able to tell "nothing was
    /// running" from "the sweep could not confirm nothing is running"
    /// apart.
    StopSession {
        req_id: u64,
        session_id: String,
    },
    /// Acknowledges `StopSession`: sent only once the kill sweep has
    /// actually run to completion (or been confirmed unnecessary — a dead
    /// or absent pane, the restart-gap case), never merely because the
    /// request was accepted.
    SessionStopped {
        req_id: u64,
    },
    /// Remove a session and all its stored state — the DB row, the
    /// in-memory entry, and (if the terminal is still live) the agent's
    /// process tree and its tmux session — regardless of whether the
    /// agent is running, stopped, or was never live this process
    /// (SPEC.md's delete works "in any state"). Unlike `StopSession`, this
    /// ends the session's existence outright; a subsequent `ListSessions`
    /// or `Attach` must not find it.
    ///
    /// A live attachment is torn down as part of deletion: the attached
    /// client is told `Detached` before its connection loses the ability
    /// to reach this session at all, so it learns why rather than just
    /// going quiet.
    DeleteSession {
        req_id: u64,
        session_id: String,
    },
    /// Acknowledges `DeleteSession`: sent only once the row, the tmux
    /// session, and (if one existed) the process tree are all positively
    /// confirmed gone. A teardown failure never yields this reply — it
    /// yields `Error` instead, with the row and in-memory entry left in
    /// place for a retry (see the supervisor's delete handler and
    /// lore/2026-07-27-m2-process-tree-stop.md for why removing the last
    /// handle on a possibly-running agent is the one outcome that must
    /// never happen silently).
    SessionDeleted {
        req_id: u64,
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
    ///
    /// `reason` is one of a small open-ended set of user-legible strings,
    /// not a coded enum — clients render every reason generically inside
    /// their detach banner without matching on its value.
    /// [`DETACH_REASON_STALLED`] is the one reason this crate names,
    /// because two independent emitters (supervisor and helm) and their
    /// tests must produce the identical string; the "another client took
    /// over" case has no constant because only one place emits it.
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
    /// The client's terminal cannot keep up: stop sending this
    /// attachment's output until a matching `ResumeOutput`. This is
    /// PLAN_M2_5.md's watermark-driven flow control — the intended sender
    /// is xterm.js once its unflushed `term.write()` backlog crosses the
    /// high-water mark, a sender that lands with the UI work in
    /// PLAN_M2_5.md step 4. Fire-and-forget like `Resize`, and `channel`
    /// exists for exactly `Resize`'s reason: see that variant's doc
    /// comment for why the supervisor must be able to ignore a pause
    /// still in flight from a client that just lost this attachment's
    /// takeover.
    ///
    /// Wire vocabulary only as of this PR — the supervisor gains no
    /// handler for it until PLAN_M2_5.md step 3.
    PauseOutput {
        channel: u32,
    },
    /// The client has drained its backlog below the low-water mark; output
    /// may flow again. Pairs with `PauseOutput` and shares its rationale
    /// for carrying `channel` (see `Resize`'s doc comment) and its
    /// fire-and-forget shape.
    ///
    /// Wire vocabulary only as of this PR — the supervisor gains no
    /// handler for it until PLAN_M2_5.md step 3.
    ResumeOutput {
        channel: u32,
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

    /// `PROTOCOL_VERSION` is a load-bearing constant (see the const's own
    /// docs for the M2 bump to 3 and the M2.5 bump to 4): pinning its value
    /// here makes an accidental re-bump (or a forgotten one, if a later
    /// change needed it) a loud test failure rather than a silent drift
    /// discovered only by two builds refusing to talk to each other.
    #[test]
    fn protocol_version_is_pinned_at_4() {
        assert_eq!(PROTOCOL_VERSION, 4);
    }

    /// Pins the decode half of the failure PLAN_M2_5.md's version bump
    /// exists because of: `parse_control` fed a syntactically valid
    /// control frame whose `type` tag is not one this build knows must
    /// error, not silently ignore the message or default it away. M2
    /// believed new `ControlMsg` variants could be additive within one
    /// protocol version; this pins the opposite so a future milestone
    /// cannot re-assume a tolerance that was never actually there.
    ///
    /// This is deliberately only the parse-layer half: that the
    /// supervisor's connection loop propagates this error and tears the
    /// connection down is pinned by an integration test in the farhelm
    /// crate's e2e suite (the helm's loop shares the same `?` shape), so
    /// a later refactor that catches and swallows the parse error cannot
    /// hide behind this unit test staying green.
    #[test]
    fn unknown_control_message_tag_fails_decode() {
        let frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: br#"{"type":"not_a_real_message"}"#.to_vec(),
        };
        crate::io::parse_control(&frame).expect_err(
            "an unrecognized ControlMsg tag must be a decode error, not a tolerated no-op",
        );
    }

    /// Both future emitters of `DETACH_REASON_STALLED` (supervisor stall
    /// timer, helm channel-full backstop) and the UI tests that look for
    /// the banner depend on this exact wire string; golden-pinning the
    /// full `Detached` JSON here means a wording edit or a reintroduced
    /// "detached:" prefix (the UI adds its own banner prefix — see the
    /// constant's docs) fails loudly instead of surfacing as a garbled
    /// user-facing banner.
    #[test]
    fn stall_detach_reason_json_shape_is_pinned() {
        let detached = ControlMsg::Detached {
            channel: 7,
            reason: DETACH_REASON_STALLED.to_string(),
        };
        assert_eq!(
            serde_json::to_value(&detached).unwrap(),
            serde_json::json!({
                "type": "detached",
                "channel": 7,
                "reason": "terminal stopped consuming output (stalled)",
            })
        );
    }

    /// `StopSession`/`SessionStopped` and `DeleteSession`/`SessionDeleted`
    /// are the wire-format additions `PROTOCOL_VERSION` 3 exists for, so
    /// they get the same golden-JSON treatment as `Attach` and `Error`
    /// above: a serde attribute change here would compile and pass a
    /// round-trip test while quietly producing bytes an unmodified peer
    /// cannot parse.
    #[test]
    fn stop_and_delete_json_shapes_are_pinned() {
        let stop = ControlMsg::StopSession {
            req_id: 11,
            session_id: "s1".to_string(),
        };
        assert_eq!(
            serde_json::to_value(&stop).unwrap(),
            serde_json::json!({
                "type": "stop_session",
                "req_id": 11,
                "session_id": "s1",
            })
        );

        let stopped = ControlMsg::SessionStopped { req_id: 11 };
        assert_eq!(
            serde_json::to_value(&stopped).unwrap(),
            serde_json::json!({
                "type": "session_stopped",
                "req_id": 11,
            })
        );

        let delete = ControlMsg::DeleteSession {
            req_id: 12,
            session_id: "s1".to_string(),
        };
        assert_eq!(
            serde_json::to_value(&delete).unwrap(),
            serde_json::json!({
                "type": "delete_session",
                "req_id": 12,
                "session_id": "s1",
            })
        );

        let deleted = ControlMsg::SessionDeleted { req_id: 12 };
        assert_eq!(
            serde_json::to_value(&deleted).unwrap(),
            serde_json::json!({
                "type": "session_deleted",
                "req_id": 12,
            })
        );
    }

    /// Round-trip every new variant through the real encode/decode path
    /// (not just `serde_json::to_value`), matching how
    /// `frame_roundtrip_control_and_data` exercises `Hello`/data frames
    /// above — this is what would catch a drift between the codec's
    /// framing and serde's JSON shape, which the pure-JSON test just
    /// above cannot see.
    #[test]
    fn stop_and_delete_roundtrip_through_frames() {
        for msg in [
            ControlMsg::StopSession {
                req_id: 1,
                session_id: "s1".to_string(),
            },
            ControlMsg::SessionStopped { req_id: 1 },
            ControlMsg::DeleteSession {
                req_id: 2,
                session_id: "s1".to_string(),
            },
            ControlMsg::SessionDeleted { req_id: 2 },
        ] {
            let mut wire = Vec::new();
            Frame::control(&msg).encode(&mut wire).unwrap();
            let (frame, used) = Frame::decode(&wire).unwrap().unwrap();
            assert_eq!(used, wire.len());
            assert_eq!(
                serde_json::from_slice::<ControlMsg>(&frame.body).unwrap(),
                msg
            );
        }
    }

    /// `PauseOutput`/`ResumeOutput` round-tripped through the real
    /// encode/decode path, matching how `stop_and_delete_roundtrip_through_frames`
    /// exercises the M2 additions above — this is what would catch a drift
    /// between the codec's framing and serde's JSON shape for the wire
    /// vocabulary PLAN_M2_5.md's version 4 bump exists for.
    #[test]
    fn pause_and_resume_output_roundtrip_through_frames() {
        for msg in [
            ControlMsg::PauseOutput { channel: 9 },
            ControlMsg::ResumeOutput { channel: 9 },
        ] {
            let mut wire = Vec::new();
            Frame::control(&msg).encode(&mut wire).unwrap();
            let (frame, used) = Frame::decode(&wire).unwrap().unwrap();
            assert_eq!(used, wire.len());
            assert_eq!(
                serde_json::from_slice::<ControlMsg>(&frame.body).unwrap(),
                msg
            );
        }
    }

    /// Golden JSON for `PauseOutput`/`ResumeOutput`, pinned the same way as
    /// `stop_and_delete_json_shapes_are_pinned`: a serde attribute change
    /// here (dropping `rename_all`, renaming a field) would compile and
    /// round-trip cleanly while quietly producing bytes an unmodified peer
    /// cannot parse — and for these two variants specifically, "unmodified
    /// peer" now means a hard decode error rather than a silently ignored
    /// field, per `unknown_control_message_tag_is_connection_fatal` above.
    #[test]
    fn pause_and_resume_output_json_shapes_are_pinned() {
        let pause = ControlMsg::PauseOutput { channel: 4 };
        assert_eq!(
            serde_json::to_value(&pause).unwrap(),
            serde_json::json!({
                "type": "pause_output",
                "channel": 4,
            })
        );

        let resume = ControlMsg::ResumeOutput { channel: 4 };
        assert_eq!(
            serde_json::to_value(&resume).unwrap(),
            serde_json::json!({
                "type": "resume_output",
                "channel": 4,
            })
        );
    }

    /// Version 4's additive rule (see `PROTOCOL_VERSION`'s docs): later
    /// M2.5 wire growth must be new optional fields, which only works if
    /// today's decoder ignores fields it does not know. This is the
    /// baseline-v4-decoder-meets-future-sender direction, mirroring the
    /// `SessionList` old/new tolerance pair from M2; without it, someone
    /// could add `deny_unknown_fields` (or serde could change defaults)
    /// and mixed v4 builds would break with every test green.
    #[test]
    fn pause_and_resume_with_future_extra_fields_decode_through_parse_control() {
        for (tag, extra) in [("pause_output", "budget"), ("resume_output", "credit")] {
            let frame = Frame {
                kind: FrameKind::Control,
                channel: 0,
                body: format!(r#"{{"type":"{tag}","channel":6,"{extra}":123}}"#).into_bytes(),
            };
            let msg = crate::io::parse_control(&frame)
                .expect("a known tag with an unknown extra field must decode, not error");
            let channel = match msg {
                ControlMsg::PauseOutput { channel } | ControlMsg::ResumeOutput { channel } => {
                    channel
                }
                other => panic!("expected pause/resume, got {other:?}"),
            };
            assert_eq!(
                channel, 6,
                "known fields must survive alongside ignored ones"
            );
        }
    }

    /// The other tolerance direction within version 4: a decoder built
    /// with a later optional field (modeled by a shadow struct with a
    /// serde default, the same technique as M2's legacy-decoder test)
    /// must accept today's pause/resume bytes and default the absent
    /// field. Together with the future-extra-fields test above this pins
    /// both halves of the additive discipline the `PROTOCOL_VERSION`
    /// docs promise for 4.
    #[test]
    fn current_pause_output_decodes_under_a_future_v4_decoder_with_defaults() {
        #[derive(serde::Deserialize)]
        struct FuturePauseOutput {
            channel: u32,
            #[serde(default)]
            budget_bytes: Option<u64>,
        }
        let mut wire = Vec::new();
        Frame::control(&ControlMsg::PauseOutput { channel: 5 })
            .encode(&mut wire)
            .unwrap();
        let (frame, _) = Frame::decode(&wire).unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&frame.body).unwrap();
        assert_eq!(value["type"], "pause_output");
        let decoded: FuturePauseOutput = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.channel, 5);
        assert_eq!(
            decoded.budget_bytes, None,
            "an absent future field must default, never fail the decode"
        );
    }

    /// `SessionStatus`'s three variants and FOUR distinct JSON shapes
    /// (`Exited` alone has two: `exit_code` present vs. `null`) are
    /// PLAN_M2.md's "Proto growth" wire addition, pinned exactly like
    /// `ErrorKind`'s variants above: an `#[serde(tag = ...)]` or
    /// variant-naming change here would compile and round-trip cleanly
    /// while quietly producing bytes an unmodified peer cannot parse. All
    /// four shapes matter individually because `Exited` has an
    /// internally-tagged field (`exit_code`) that flattens into the same
    /// object as the `state` tag — a detail `serde_json::to_value`
    /// equality alone makes visible, unlike a bare round-trip.
    #[test]
    fn session_status_json_shapes_are_pinned() {
        assert_eq!(
            serde_json::to_value(SessionStatus::Alive).unwrap(),
            serde_json::json!({ "state": "alive" })
        );
        assert_eq!(
            serde_json::to_value(SessionStatus::Exited { exit_code: Some(3) }).unwrap(),
            serde_json::json!({ "state": "exited", "exit_code": 3 })
        );
        assert_eq!(
            serde_json::to_value(SessionStatus::Exited { exit_code: None }).unwrap(),
            serde_json::json!({ "state": "exited", "exit_code": null })
        );
        assert_eq!(
            serde_json::to_value(SessionStatus::Unknown).unwrap(),
            serde_json::json!({ "state": "unknown" })
        );
    }

    /// `SessionList`'s `total`/`truncated` addition, pinned the same way:
    /// both fields must sit alongside `sessions` in the encoded object,
    /// under their exact snake_case names, or a mismatched rename would
    /// pass every Rust-side test here while a real cross-build peer reads
    /// something else entirely.
    #[test]
    fn session_list_total_and_truncated_json_shape_is_pinned() {
        let msg = ControlMsg::SessionList {
            req_id: 5,
            sessions: vec![],
            total: 7,
            truncated: true,
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            serde_json::json!({
                "type": "session_list",
                "req_id": 5,
                "sessions": [],
                "total": 7,
                "truncated": true,
            })
        );
    }

    /// The additive-decode half of PLAN_M2.md's "Proto growth" contract:
    /// JSON shaped exactly like a PRE-this-change sender — a `SessionInfo`
    /// with no `state` field, a `SessionList` with no `total`/`truncated`
    /// — must still decode successfully, defaulting the new fields rather
    /// than failing. This is what makes the addition safe within the
    /// existing `PROTOCOL_VERSION` 3 instead of needing its own bump: an
    /// old-build supervisor and a new-build helm (or vice versa) must keep
    /// interoperating.
    #[test]
    fn old_shape_session_list_json_decodes_with_defaulted_new_fields() {
        let old_shape = serde_json::json!({
            "type": "session_list",
            "req_id": 9,
            "sessions": [
                {
                    "id": "s1",
                    "title": "demo",
                    "cwd": "/tmp",
                    "invocation": "agent",
                }
            ],
        });
        let decoded: ControlMsg = serde_json::from_value(old_shape).unwrap();
        let ControlMsg::SessionList {
            req_id,
            sessions,
            total,
            truncated,
        } = decoded
        else {
            panic!("expected ControlMsg::SessionList, got {decoded:?}");
        };
        assert_eq!(req_id, 9);
        assert_eq!(total, 0, "an old sender's reply predates the field");
        assert!(!truncated, "an old sender's reply predates the field");
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].status,
            SessionStatus::Unknown,
            "a SessionInfo with no state field must decode as Unknown, never a guess"
        );
    }

    /// The REVERSE direction from the test above: a hand-rolled decoder
    /// shaped like a peer built BEFORE this PR — no `status`, no
    /// `total`/`truncated` — must still decode a NEW sender's JSON
    /// successfully, silently dropping the fields it does not know
    /// about. Serde's default of ignoring unrecognized object keys is
    /// what makes this work. The shadow types below deliberately carry no
    /// `#[serde(deny_unknown_fields)]` either, standing in for a real old
    /// peer that never had a reason to add one — but this test's decode
    /// path never touches the REAL `ControlMsg`/`SessionInfo` types at
    /// all, so it says nothing about whether THOSE gaining
    /// `deny_unknown_fields` would break anything (an earlier version of
    /// this comment claimed otherwise; it does not, precisely because
    /// decoding here goes through these shadow types, not the real ones).
    /// The guarantee that DOES depend on the real types — an old-SHAPED
    /// JSON still decoding under the CURRENT real decoder — is what the
    /// sibling `old_shape_session_list_json_decodes_with_defaulted_new_fields`
    /// test above pins instead. Together, the two tests cover both
    /// directions of PLAN_M2.md's additivity claim.
    #[derive(Debug, Deserialize)]
    struct LegacySessionInfo {
        id: String,
        title: String,
        cwd: String,
        invocation: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum LegacyControlMsg {
        SessionList {
            req_id: u64,
            sessions: Vec<LegacySessionInfo>,
        },
    }

    #[test]
    fn new_session_list_json_decodes_under_a_legacy_pre_status_decoder() {
        let new_msg = ControlMsg::SessionList {
            req_id: 4,
            sessions: vec![SessionInfo {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Exited { exit_code: Some(1) },
            }],
            total: 3,
            truncated: true,
        };
        let json = serde_json::to_value(&new_msg).unwrap();

        let LegacyControlMsg::SessionList { req_id, sessions } =
            serde_json::from_value(json.clone()).expect(
                "a legacy decoder without status/total/truncated must still decode new-shape \
                 JSON",
            );
        assert_eq!(req_id, 4);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "s1");
        assert_eq!(sessions[0].title, "demo");
        assert_eq!(sessions[0].cwd, "/tmp");
        assert_eq!(sessions[0].invocation, "agent");

        // The REAL types round-trip the same JSON too — cheap to check
        // here, and it makes explicit that this test's own `json` value
        // is exactly what a real `ControlMsg::SessionList` produces, not
        // some hand-crafted stand-in.
        let real_decoded: ControlMsg = serde_json::from_value(json).unwrap();
        assert_eq!(real_decoded, new_msg);
    }
}
