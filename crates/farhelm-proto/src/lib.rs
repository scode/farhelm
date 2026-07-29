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
/// bump, per version 3's own docs above; `protocol_version_is_pinned_at_5`
/// (renamed from `_at_4` when version 5 landed) and
/// `unknown_control_message_tag_fails_decode` below (plus the
/// loop-level teardown test in the farhelm crate's e2e suite) pin both the
/// number and the reasoning so the next milestone cannot re-assume
/// tolerance that was never there. Within version 4, the same additive
/// discipline applies: later M2.5 wire changes must be new optional fields
/// with decode defaults, not new variants, or they earn their own bump too.
///
/// Bumped to 5 for the complete M3 wire vocabulary (PLAN_M3.md item 1):
/// `SessionStatus::Interrupted`/`Error`, `SessionInfo`'s stop annotation
/// and restart-offer fields, `CreateSession`'s intent key and snapshot
/// overrides (`agent_kind`, `resume_template`), `ErrorKind::Conflict` for
/// an intent key reused with a different fingerprint, and
/// `RestartSession`/`SessionRestarted`. All of it lands in ONE bump rather
/// than several: each of the FIVE new tagged-enum variants above (two on
/// `SessionStatus` — `Error` and `Interrupted`; one on `ErrorKind` —
/// `Conflict`; two on `ControlMsg` — `RestartSession` and
/// `SessionRestarted`) independently earns its own bump by version 4's own
/// argument above — an unrecognized tag is connection-fatal, never a
/// tolerated no-op — so spreading M3 across a version per variant would
/// only multiply the number of protocol versions a mixed fleet has to
/// reason about, for a milestone that ships all of them together anyway.
/// (`AgentKind`'s three variants, including `Generic`, are not part of
/// this count: `AgentKind` itself is new in version 5, so there is no
/// prior decoder for it to break — the count above is specifically about
/// variants added to enums that already existed at version 4.) Within
/// version 5, new FIELDS on an existing message keep the same additive
/// discipline version 4 established: default on absence, ignored when
/// unrecognized. Only a genuinely new tagged variant, OR a field change
/// that cannot be made additive — a new REQUIRED field being the version 2
/// precedent documented above, where an old decoder has no default to
/// fall back on — needs another bump; neither is anticipated before M4.
pub const PROTOCOL_VERSION: u32 = 5;

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
/// original three kinds (M1) were exactly the statuses the HTTP surface
/// could honestly distinguish at the time (404 / 400 / 500); PLAN_M3.md
/// item 6 added the fourth, `Conflict` (409), for intent-key reuse with a
/// mismatched fingerprint — a genuinely different situation from all
/// three, not an HTTP-status-table nicety. Adding a variant here is
/// always a wire-format change — an older peer's decoder has no fallback
/// for a tag it does not recognize — so it takes the same
/// `PROTOCOL_VERSION` bump as any other incompatible change; there is
/// still no need to over-design this set beyond what a caller can
/// currently act on.
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
    /// The request conflicts with an outcome already recorded under the
    /// same identifier. As of `PROTOCOL_VERSION` 5 the one producer is
    /// `CreateSession`'s intent key (PLAN_M3.md item 6) reused with a
    /// different fingerprint: the caller has replayed a retry identifier
    /// for what is, per the fingerprint, a genuinely different request —
    /// a client bug (a stale or reused key), not something the server can
    /// honestly merge or route around. Neither `NotFound` (nothing to
    /// operate on) nor `InvalidRequest` (a self-contained flaw in this
    /// request alone) fits: only `Conflict` names "this identifier already
    /// has a meaning, and it is not the one you just sent."
    Conflict,
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
///
/// `Error` and `Interrupted` are NOT additive the same way — they are new
/// tagged-enum variants, exactly the case `PROTOCOL_VERSION`'s own docs
/// say cannot be additive, which is why they ride the bump to 5 rather
/// than landing as a same-version addition the way `total`/`truncated`
/// did.
///
/// No longer `Copy` as of `Error { detail: String }`: an owned `String`
/// cannot be duplicated bit-for-bit, so `Copy` and `String` are mutually
/// exclusive on any type containing one, forcing every prior call site
/// that copied a `SessionStatus` implicitly to `clone()` it instead
/// (`cargo check --all-targets` finds every such site).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
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
    /// The agent's process could not be started at all — the launch
    /// shim's exec-failure sentinel (PLAN_M3.md item 3), never inferred
    /// from an exit code: an invocation that execs successfully and then
    /// exits 126 or 127 is `Exited`, not `Error`, because it DID run.
    /// `detail` is the sentinel's own errno-derived explanation, carried
    /// as a plain required `String` rather than `Option` (unlike
    /// `Exited::exit_code`) because a sentinel is only ever read when it
    /// exists — there is no "error, but no detail" case the way there is
    /// a "exited, but tmux could not reduce it to a code" case.
    Error { detail: String },
    /// The host rebooted while this session's durable last-known outcome
    /// was launching or running, and tmux — the only liveness truth this
    /// system keeps — did not survive that reboot (PLAN_M3.md item 2,
    /// SPEC.md's Durability section). Distinct from `Unknown`: `Unknown`
    /// means "not yet asked"; `Interrupted` means "asked, via the one
    /// reboot detector this system has (a changed stored boot id), and
    /// the answer is structurally unknowable now, not merely unasked
    /// yet." Persists until the user acts (restart, archive, delete);
    /// nothing — including another supervisor restart on the same boot —
    /// clears it on its own.
    Interrupted,
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
    /// `Unknown`/`Alive`/`Exited` are computed fresh by the supervisor on
    /// every `ListSessions` reply through LIVE tmux probing — that half of
    /// this field is never persisted, because tmux's own pane state is the
    /// only truth for a currently-reachable session, and it does not
    /// survive a supervisor restart on its own terms either.
    /// `Interrupted` and `Error` (PLAN_M3.md items 2 and 3) are the
    /// opposite case: they exist BECAUSE live tmux probing has nothing to
    /// say (tmux itself did not survive the reboot, or never started at
    /// all) and are instead read back from the supervisor's own durable
    /// last-known-outcome record — so "never persisted" describes the
    /// live-probed variants only, not this field as a whole. Either way,
    /// this field is never trusted from an older sender.
    /// `#[serde(default)]` is what makes this field additive within
    /// `PROTOCOL_VERSION` 3: an old peer's JSON has no `status` at all and
    /// decodes to `SessionStatus::Unknown` rather than failing, and this
    /// crate carries no `deny_unknown_fields` anywhere on this path, so a
    /// NEW `status` reaching an OLD decoder is silently ignored rather
    /// than rejected. Both directions must keep holding for any later
    /// M2 wire addition, per `PROTOCOL_VERSION`'s own docs.
    #[serde(default)]
    pub status: SessionStatus,
    /// User-legible qualifier shown alongside an `Exited` status — SPEC.md's
    /// "a user-initiated stop yields exited with an annotation" (Status
    /// section). A bare optional string, not a coded enum, for the same
    /// reason `ControlMsg::Detached::reason` is one (see that field's
    /// docs): this is prose the UI renders verbatim, not a value it
    /// branches on. Absent (`None`) is the only case anything produces as
    /// of this PR — no writer exists yet, so every session decodes with no
    /// annotation until PLAN_M3.md item 4 lands the durable stop-annotation
    /// write path; an `Option<String>` field defaults to `None` on a
    /// missing key without needing `#[serde(default)]` (serde's own
    /// built-in `Option` handling — the same mechanism
    /// `ControlMsg::CreateSession::title` already relies on elsewhere in
    /// this file; `SessionInfo`'s OWN `title` above is a required
    /// `String`, not a comparable case).
    pub annotation: Option<String>,
    /// What restarting THIS session would currently do to the agent's
    /// conversation — see [`RestartOffer`]'s own docs for what it means
    /// and why it lives here rather than behind a dedicated query message
    /// (PLAN_M3.md item 9's open design question, resolved in this PR).
    /// `#[serde(default)]` keeps it additive: an old sender's JSON has no
    /// `restart_offer` at all and decodes to the safe `FreshOnly` default
    /// rather than an invented "captured" claim.
    #[serde(default)]
    pub restart_offer: RestartOffer,
}

/// What restarting a session would do to the agent's conversation, as the
/// supervisor currently understands it from the session's snapshot
/// (PLAN_M3.md item 7) and its captured conversation identity (item 8) —
/// never re-derived by a client, which cannot see either.
///
/// ## Design decision: on `SessionInfo`, not a dedicated query message
///
/// PLAN_M3.md item 9 requires the UI to know, BEFORE it even asks the user
/// to confirm a restart, what restarting would offer: resume the captured
/// conversation, fall back to an explicit placeholder-free template, or
/// offer only a fresh launch. Item 9 itself is silent on HOW that
/// knowledge reaches the client; "without a second round trip" is this
/// design's own chosen tradeoff (spelled out in the PR brief that shaped
/// this vocabulary), not a requirement PLAN_M3.md states. The alternative
/// shape considered was a dedicated `QueryRestartOffer`/`RestartOfferReply`
/// request pair, and it was rejected: opening a session (or just having
/// listed sessions at all) already means the client holds that session's
/// `SessionInfo`, so a second round trip would exist ONLY to answer a
/// question the supervisor could have answered for free while building
/// the reply it was sending anyway. The cost of embedding is symmetric
/// and small: every `SessionInfo` in every `ListSessions` reply now
/// carries a few bytes of enum whether or not the viewer ever opens a
/// restart dialog for that particular session, which is negligible
/// next to the round trip it buys back universally.
///
/// The other half of item 9's "know before asking" requirement — whether
/// to SHOW a confirm-stop dialog because the agent looks still running —
/// needed no new field here either: it is exactly `status ==
/// SessionStatus::Alive`, already on this struct. That is deliberately
/// only ever a UI-flow HINT, never an authorization, precisely because
/// this same `SessionInfo` can go stale between being cached and a
/// `RestartSession` actually being sent: the AUTHORIZATION to stop a
/// session that turns out to be live at handling time is a separate,
/// explicit field on the request itself
/// (`ControlMsg::RestartSession::stop_if_running`) that the supervisor
/// checks against liveness it rechecks at that moment — see that field's
/// docs for why deriving consent from a client-cached status would be a
/// TOCTOU bug, not just a redundant one.
///
/// Every variant is a unit variant carrying no data, so — like
/// `ErrorKind` and `AgentKind` above, and unlike `SessionStatus` (which
/// needs an internal tag because some of ITS variants carry fields) —
/// this serializes as a bare snake_case string, not a tagged object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RestartOffer {
    /// No captured conversation identity, and no explicit placeholder-free
    /// resume-template override either: restart can only offer a fresh
    /// launch. `#[default]` is deliberate, not just convenient — it is the
    /// safe reading for any session this field does not (yet) describe an
    /// old sender's session, or one this exact build has not finished
    /// classifying — because defaulting toward "captured" would risk
    /// exactly the silently-wrong-conversation resume SPEC.md forbids.
    #[default]
    FreshOnly,
    /// The session's own conversation was captured (item 8): restart fills
    /// the snapshot's resume template with that identity and resumes it.
    Resume,
    /// No captured identity, but the session's snapshot carries an
    /// explicit, placeholder-free resume-template override (item 7):
    /// restart runs that template verbatim. Kept distinct from
    /// `FreshOnly` because the user deliberately configured this
    /// fallback; the UI must not describe it as a plain fresh launch.
    FallbackTemplate,
}

/// An agent's integration kind (PLAN_M3.md item 7): the two SPEC.md
/// requires conversation-identity capture for, plus `Generic` for
/// everything else — SPEC.md's own phrase for a profile that names no
/// kind ("profiles without a kind get generic treatment").
///
/// This is a genuine three-state override on `CreateSession::agent_kind`,
/// not two states plus an absent field: `None` means "derive it from
/// `invocation`'s basename (or fail to)"; `Some(Claude)`/`Some(Codex)`
/// forces integration on for an invocation basename recognition would
/// otherwise miss (`env claude`, a wrapper script); `Some(Generic)`
/// forces integration OFF even when the basename WOULD have matched —
/// the case absence cannot express, because a caller has no way to tell
/// "let it derive" apart from "I checked, and it must not integrate"
/// without a real third value. A user running a personal script also
/// named `claude` that is not Anthropic's CLI is the motivating case:
/// without `Generic`, there is no way to stop basename recognition from
/// misclassifying it and running Claude-Code-specific status heuristics
/// and identity capture against a process that was never going to
/// produce Claude Code's on-disk records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Claude,
    Codex,
    /// Explicitly non-integrated: no status heuristics beyond the
    /// generic ones, no conversation-identity capture, regardless of
    /// what basename recognition would have concluded on its own.
    Generic,
}

/// The one stop annotation this PR reserves. SPEC.md itself only promises
/// THAT a user-initiated stop gets an annotation on an `Exited` session
/// ("'stopped' is not a distinct status"), without pinning its exact
/// text; `"stopped by user"` is the literal string PLAN_M3.md item 4
/// chose for that annotation. Named here, like [`DETACH_REASON_STALLED`],
/// so the (future) writer and the tests that check for it cannot drift
/// independently.
pub const STOP_ANNOTATION: &str = "stopped by user";

/// The user's chosen resolution on a `ControlMsg::RestartSession` request.
/// Shares its three shapes with [`RestartOffer`] but is the opposite
/// direction: `RestartOffer` is the SERVER telling the client what is
/// possible; `RestartMode` is the CLIENT telling the server what to do.
/// Like [`RestartOffer`], every variant is a unit variant, so this is a
/// bare snake_case string on the wire, not a tagged object.
///
/// ## Must match the CURRENT offer, not a cached one
///
/// A well-behaved client only ever sends `Resume` or `FallbackTemplate`
/// when the session's own `RestartOffer` said that capability exists —
/// but see `ControlMsg::RestartSession`'s "offer/mode staleness contract"
/// doc for why the supervisor, not client good behavior, is what actually
/// enforces this: the offer the client saw can be stale by request time,
/// so the handler (PLAN_M3.md item 9, not this PR) validates `mode`
/// against the CURRENT offer and rejects a mismatch with `Conflict`
/// rather than trusting the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartMode {
    /// Resume the captured conversation through the snapshot's resume
    /// template. Only valid when the CURRENT offer is `Resume`.
    Resume,
    /// Launch a fresh, unrelated agent process in the same working
    /// directory. Valid ONLY when the current offer is `FreshOnly` — NOT
    /// a way to decline an available resume: SPEC.md's restart never
    /// downgrades a resumable session to a clean conversation ("no
    /// fresh-restart variant in v1... for a clean conversation, create a
    /// new session in the same directory"), so a client cannot legally
    /// choose `Fresh` over `Resume` when `Resume` was offered — there is
    /// no "user chose not to resume" case, only "there was nothing to
    /// resume." A `Fresh` sent against a `Resume`/`FallbackTemplate`
    /// offer is exactly the staleness case above and gets `Conflict`.
    Fresh,
    /// Run the snapshot's explicit, placeholder-free resume-template
    /// override verbatim. Only a valid choice when
    /// `RestartOffer::FallbackTemplate` was offered; PLAN_M3.md item 9
    /// reserves this for the case item 7 describes (an explicitly
    /// overridden template on a non-integrated kind).
    FallbackTemplate,
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
        /// Client-supplied idempotency key (PLAN_M3.md item 6): a create
        /// retried with the same key and an identical fingerprint of
        /// every session-shaping field (this struct's fields below
        /// included, but never `cols`/`rows` — those shape the
        /// attachment, not the session) replays the original outcome
        /// instead of launching a second process. `None` preserves
        /// pre-M3 behavior exactly: every request is its own create, with
        /// no deduplication — the safe default for raw API callers (curl,
        /// an older UI build) that never learned this field exists, so
        /// its mere addition does not newly expose them to anything.
        /// Vocabulary only as of this PR: nothing stores or dedups on it
        /// yet.
        intent_key: Option<String>,
        /// Explicit override of the integrated-agent kind PLAN_M3.md item
        /// 7 would otherwise derive from `invocation`'s first token by
        /// basename recognition. A genuine tri-state via [`AgentKind`]'s
        /// three variants: `None` means "let the supervisor derive it
        /// (or fail to)"; `Some(Claude)`/`Some(Codex)` forces integration
        /// on for a basename recognition would miss (`env claude`, a
        /// wrapper script); `Some(Generic)` forces it OFF even when the
        /// basename would have matched — see `AgentKind::Generic`'s own
        /// docs for why that direction needs an explicit value rather
        /// than reusing absence. No UI surface sends this yet — the UI
        /// sends `None` and lets derivation run; this field exists for
        /// the API and for M5's future profile system to feed richer
        /// values through the same slot.
        agent_kind: Option<AgentKind>,
        /// Explicit override of the resume invocation template PLAN_M3.md
        /// item 7 would otherwise default from `invocation`'s first
        /// token. Structured as an argv vector, not a shell string, so a
        /// path containing spaces survives without quoting heroics, and
        /// `{conversation}` substitutes into its own argv slot rather
        /// than into a string that would need escaping.
        ///
        /// The placement rule is exact, not "somewhere in the template":
        /// an argv ELEMENT must equal the literal string `{conversation}`
        /// in full — `--resume={conversation}` or any other embedded
        /// form does not count as a placeholder occurrence under this
        /// rule, because substitution replaces a whole element, never
        /// splices into part of one. A session with an integrated
        /// `agent_kind` (derived or overridden) must have a template
        /// containing an element meeting that exact-equality rule — a
        /// template with no such element is only valid on a non-integrated
        /// kind, where it is a verbatim fallback resume invocation
        /// (SPEC.md's "falls back to the profile's resume invocation
        /// verbatim"). This crate does not enforce that invariant itself
        /// (it is vocabulary, not validation); the supervisor's create
        /// handler is where it will be checked once item 7 lands, and this
        /// exact-equality wording is what keeps that future validator from
        /// having to guess which reading was intended.
        resume_template: Option<Vec<String>>,
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
    /// Relaunch a session's agent (PLAN_M3.md item 9) — the only relaunch
    /// mechanism SPEC.md's lifecycle "restart" names; the resume offered
    /// when opening an interrupted session sends this same message, not a
    /// separate one. `mode` is the user's resolution among what
    /// [`SessionInfo::restart_offer`] told the client was possible for
    /// this session AT THE TIME IT WAS CACHED — see the staleness note
    /// below, because that offer can go stale before this message is
    /// sent.
    ///
    /// Deciding whether to SHOW a confirm-stop dialog is client-side UI
    /// flow, derived from `status == SessionStatus::Alive` on whatever
    /// `SessionInfo` the client last saw. But that derivation is only a
    /// hint, not an authorization: it can be stale by the time this
    /// message actually arrives (another client's action, or the agent
    /// exiting or relaunching in the interim), so `stop_if_running`
    /// carries the user's actual consent onto the wire, and the handler
    /// rechecks REAL liveness before honoring it — see that field's own
    /// docs for the full rationale. A client only sets it after the user
    /// has confirmed; the field is what lets the supervisor tell "the
    /// user agreed to stop a live agent" apart from "the client
    /// forgot to ask."
    ///
    /// ## Offer/mode staleness contract
    ///
    /// `SessionInfo::restart_offer` is a snapshot the client cached from
    /// its last `ListSessions` or `SessionCreated`/`SessionRestarted`
    /// reply; conversation capture (PLAN_M3.md item 8) can upgrade a
    /// session's real offer — `FreshOnly` to `Resume` — asynchronously,
    /// after that snapshot was taken and before this request arrives.
    /// The handler (item 9, not this PR) must therefore validate `mode`
    /// against the supervisor's CURRENT offer at handling time, not trust
    /// the client's stale copy: a mismatch is rejected with `Conflict`,
    /// and the client is expected to refresh its `SessionInfo` (polling
    /// already exists for this) and re-present the (possibly changed)
    /// offer to the user rather than retry blindly. This contract is
    /// written here, at the vocabulary level, specifically so the item 9
    /// handler cannot be implemented against a softer reading — see
    /// [`RestartMode::Fresh`]'s own docs for the one-directional
    /// consequence this has for that variant.
    ///
    /// Vocabulary only as of this PR: no handler exists yet. Until
    /// PLAN_M3.md item 9 lands terminal reuse, vanished-cwd handling, and
    /// the confirm-stop-relaunch sequence, this build cannot honor a
    /// restart at all; `handle_control` replies with a temporary
    /// `Error { kind: Internal, .. }` naming that plainly rather than
    /// silently dropping the request (see that function's own docs) —
    /// falling through to the generic "unexpected control message"
    /// fallback would leave a v5 caller's request waiting on a reply
    /// that never comes, since unlike `PauseOutput`/`ResumeOutput` this
    /// message carries a `req_id` a caller is actually blocked on.
    RestartSession {
        req_id: u64,
        session_id: String,
        mode: RestartMode,
        /// Explicit consent to stop a still-running agent before
        /// relaunching (`#[serde(default)]` false — the safe direction:
        /// an old-shaped or naive request never kills a live process by
        /// accident). SPEC.md requires restart on a running agent to
        /// confirm before it stops it, and that confirmation has to be
        /// something the SUPERVISOR checks, not something the client
        /// merely promises: a client's `SessionInfo.status` is a
        /// snapshot from its last list or its own cached copy, and the
        /// agent can transition between "the user was shown a confirm
        /// dialog" and "the request actually arrives" (another client's
        /// action, the agent exiting or being launched in the interim).
        /// The handler (PLAN_M3.md item 9, not this PR) atomically
        /// rechecks REAL liveness at handling time and rejects with
        /// `Conflict` if the session is live and this flag is false —
        /// client-derived status is a UI hint that decides whether to
        /// SHOW a confirm dialog, never the authorization to skip it.
        #[serde(default)]
        stop_if_running: bool,
    },
    /// Success reply to `RestartSession`, shaped like `SessionCreated`:
    /// `session` carries the session's resulting state (including its
    /// freshly recomputed `restart_offer`) so a caller does not have to
    /// re-list to see it.
    SessionRestarted {
        req_id: u64,
        session: SessionInfo,
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
    /// producing bytes an unmodified peer cannot parse. `Conflict`
    /// (PLAN_M3.md item 6, the fourth variant this enum has ever grown) is
    /// pinned here too, bare — as `ErrorKind` alone, not wrapped in a full
    /// `ControlMsg::Error` — since its wire shape (a snake_case string) is
    /// independent of which message happens to carry it.
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
        assert_eq!(
            serde_json::to_value(ErrorKind::Conflict).unwrap(),
            serde_json::json!("conflict")
        );
    }

    /// `PROTOCOL_VERSION` is a load-bearing constant (see the const's own
    /// docs for the M2 bump to 3, the M2.5 bump to 4, and the M3 bump to
    /// 5): pinning its value here makes an accidental re-bump (or a
    /// forgotten one, if a later change needed it) a loud test failure
    /// rather than a silent drift discovered only by two builds refusing
    /// to talk to each other.
    #[test]
    fn protocol_version_is_pinned_at_5() {
        assert_eq!(PROTOCOL_VERSION, 5);
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

    /// `SessionStatus`'s now FIVE variants and SIX distinct JSON shapes
    /// (`Exited` alone has two: `exit_code` present vs. `null`) are
    /// PLAN_M2.md's and PLAN_M3.md's "Proto growth" wire additions, pinned
    /// exactly like `ErrorKind`'s variants above: an `#[serde(tag = ...)]`
    /// or variant-naming change here would compile and round-trip cleanly
    /// while quietly producing bytes an unmodified peer cannot parse. All
    /// shapes matter individually because `Exited` and `Error` both have
    /// internally-tagged fields that flatten into the same object as the
    /// `state` tag — a detail `serde_json::to_value` equality alone makes
    /// visible, unlike a bare round-trip. `Error` and `Interrupted` are the
    /// PLAN_M3.md item 3/2 additions that forced `PROTOCOL_VERSION` to 5.
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
        assert_eq!(
            serde_json::to_value(SessionStatus::Error {
                detail: "exec: no such file or directory".to_string()
            })
            .unwrap(),
            serde_json::json!({
                "state": "error",
                "detail": "exec: no such file or directory",
            })
        );
        assert_eq!(
            serde_json::to_value(SessionStatus::Interrupted).unwrap(),
            serde_json::json!({ "state": "interrupted" })
        );
    }

    /// PLAN_M3 review batch item 28: proves the FAILURE the whole
    /// `PROTOCOL_VERSION` bump to 5 exists to cause. `Interrupted` and
    /// `Error` are new tagged-enum variants (see `SessionStatus`'s own
    /// docs on why they could not be additive), so a decoder shaped like a
    /// genuine v4 peer — one that only ever knew `unknown`/`alive`/
    /// `exited` — must FAIL to decode either of them, exactly as
    /// `unknown_control_message_tag_fails_decode` pins the same failure
    /// one level up for `ControlMsg` tags. Nothing before this test
    /// actually checked that a v4 decoder rejects these; every other
    /// `SessionStatus` test in this file decodes through the CURRENT (v5)
    /// types, which trivially accept their own variants.
    #[derive(Debug, Deserialize)]
    #[serde(tag = "state", rename_all = "snake_case")]
    enum LegacyV4SessionStatus {
        Unknown,
        Alive,
        // Included for shape-completeness (a real v4 decoder has this
        // variant too) even though this test only ever feeds it
        // `interrupted`/`error` JSON, which is exactly the point: it
        // exercises none of `Exited`'s own fields.
        #[allow(dead_code)]
        Exited {
            exit_code: Option<i32>,
        },
    }

    #[test]
    fn interrupted_and_error_status_fail_under_a_legacy_v4_decoder() {
        let interrupted = serde_json::to_value(SessionStatus::Interrupted).unwrap();
        serde_json::from_value::<LegacyV4SessionStatus>(interrupted).expect_err(
            "a v4 decoder must fail on `interrupted`, not silently ignore or default it",
        );

        let error = serde_json::to_value(SessionStatus::Error {
            detail: "exec: no such file or directory".to_string(),
        })
        .unwrap();
        serde_json::from_value::<LegacyV4SessionStatus>(error)
            .expect_err("a v4 decoder must fail on `error`, not silently ignore or default it");
    }

    /// The `ErrorKind` sibling of the test above: a decoder shaped like a
    /// genuine v4 peer — `not_found`/`invalid_request`/`internal` only —
    /// must FAIL on `conflict`, the variant `PROTOCOL_VERSION` 5 added.
    /// `ErrorKind` has no internal tag (every variant is a bare string;
    /// see `AgentKind`'s doc comment for why), so the shadow enum below
    /// needs no `#[serde(tag = ...)]` either.
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum LegacyV4ErrorKind {
        NotFound,
        InvalidRequest,
        Internal,
    }

    #[test]
    fn conflict_error_kind_fails_under_a_legacy_v4_decoder() {
        let conflict = serde_json::to_value(ErrorKind::Conflict).unwrap();
        serde_json::from_value::<LegacyV4ErrorKind>(conflict)
            .expect_err("a v4 decoder must fail on `conflict`, not silently ignore or default it");
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
                annotation: None,
                restart_offer: RestartOffer::default(),
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

    /// `SessionInfo::annotation` and `restart_offer` are PLAN_M3.md's stop-
    /// annotation (item 4) and restart-offer (item 9) additions, both
    /// defaulting on absence for the same additive-within-5 reason
    /// `status` was — `restart_offer` via an explicit `#[serde(default)]`
    /// (it is a plain enum with a `#[default]` variant), `annotation` via
    /// serde's own built-in `Option` handling (see that field's doc
    /// comment). Pinning both the present and absent shapes catches a
    /// rename or a dropped default independently of the round-trip tests
    /// elsewhere in this file.
    #[test]
    fn session_info_annotation_and_restart_offer_json_shapes_are_pinned() {
        let bare = SessionInfo {
            id: "s1".to_string(),
            title: "demo".to_string(),
            cwd: "/tmp".to_string(),
            invocation: "agent".to_string(),
            status: SessionStatus::default(),
            annotation: None,
            restart_offer: RestartOffer::default(),
        };
        assert_eq!(
            serde_json::to_value(&bare).unwrap(),
            serde_json::json!({
                "id": "s1",
                "title": "demo",
                "cwd": "/tmp",
                "invocation": "agent",
                "status": { "state": "unknown" },
                "annotation": null,
                "restart_offer": "fresh_only",
            })
        );

        let stopped = SessionInfo {
            status: SessionStatus::Exited { exit_code: Some(0) },
            annotation: Some(STOP_ANNOTATION.to_string()),
            restart_offer: RestartOffer::Resume,
            ..bare
        };
        assert_eq!(
            serde_json::to_value(&stopped).unwrap()["annotation"],
            serde_json::json!("stopped by user")
        );
        assert_eq!(
            serde_json::to_value(&stopped).unwrap()["restart_offer"],
            serde_json::json!("resume")
        );

        // JSON shaped as if these two fields had not been added YET —
        // no `annotation`, no `restart_offer` at all — must still decode,
        // defaulting both. This is intra-version-5 additive discipline,
        // not real cross-build interop: an actual pre-M3 (v4) peer is
        // refused outright at the handshake (see `PROTOCOL_VERSION`'s own
        // docs) and never reaches this decode path at all. What this
        // pins is the same guarantee `status` needed when THAT field was
        // added within v3 (see
        // `old_shape_session_list_json_decodes_with_defaulted_new_fields`
        // above) — that a later field addition inside one version stays
        // safe for any JSON that predates it, whatever the reason a given
        // sender's JSON might lack it.
        let old_shape = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
            "status": { "state": "alive" },
        });
        let decoded: SessionInfo = serde_json::from_value(old_shape).unwrap();
        assert_eq!(decoded.annotation, None);
        assert_eq!(decoded.restart_offer, RestartOffer::FreshOnly);
    }

    /// PLAN_M3 review batch item 23: a future field appearing INSIDE a
    /// nested `SessionInfo` — not just at the top level of a `ControlMsg`,
    /// which `pause_and_resume_with_future_extra_fields_decode_through_parse_control`
    /// already covers — must still decode through the REAL `parse_control`
    /// path (not a hand-rolled `serde_json::from_value::<SessionInfo>`,
    /// which would say nothing about whether `deny_unknown_fields` had
    /// crept in anywhere along the real decode chain from frame bytes to
    /// `ControlMsg`). Additivity within version 5 is only real if it holds
    /// at every nesting level a sender might grow, not just the outermost
    /// one; a `#[serde(deny_unknown_fields)]` added to `SessionInfo` alone
    /// (never touching `ControlMsg` itself) would break this specific case
    /// while every other test in this file stayed green.
    #[test]
    fn session_list_with_unknown_field_inside_session_decodes_through_parse_control() {
        let frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: serde_json::json!({
                "type": "session_list",
                "req_id": 11,
                "sessions": [
                    {
                        "id": "s1",
                        "title": "demo",
                        "cwd": "/tmp",
                        "invocation": "agent",
                        "status": { "state": "alive" },
                        "future_field_inside_session": "value from tomorrow",
                    }
                ],
                "total": 1,
                "truncated": false,
            })
            .to_string()
            .into_bytes(),
        };
        let msg = crate::io::parse_control(&frame)
            .expect("an unknown field nested inside a SessionInfo object must decode, not error");
        let ControlMsg::SessionList { sessions, .. } = msg else {
            panic!("expected ControlMsg::SessionList, got {msg:?}");
        };
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "s1");
        assert_eq!(sessions[0].status, SessionStatus::Alive);
    }

    /// `AgentKind`, `RestartOffer`, and `RestartMode` are bare snake_case
    /// strings (see their doc comments for why, unlike `SessionStatus`),
    /// golden-pinned per variant below so a rename or `rename_all` change
    /// fails loudly instead of passing a round-trip test while quietly
    /// breaking an unmodified peer.
    ///
    /// The exhaustive matches at the top are the actual mechanism that
    /// catches a future variant this test forgets: a golden assertion
    /// alone would stay green if someone added a variant and simply never
    /// came back here, since nothing about `assert_eq!` on the variants
    /// THIS test already knows about would notice. Each `match` has no
    /// wildcard arm, so a new variant on any of these three enums fails
    /// THIS FILE to compile until it is added to both the match and a
    /// golden assertion below — the earlier version of this doc comment
    /// claimed the golden assertions alone gave this protection, which
    /// was not true (a genuinely exhaustive check requires exactly this
    /// kind of construct, not more `assert_eq!` calls).
    #[test]
    fn agent_kind_and_restart_vocabulary_json_shapes_are_pinned() {
        match AgentKind::Claude {
            AgentKind::Claude | AgentKind::Codex | AgentKind::Generic => {}
        }
        match RestartOffer::FreshOnly {
            RestartOffer::FreshOnly | RestartOffer::Resume | RestartOffer::FallbackTemplate => {}
        }
        match RestartMode::Resume {
            RestartMode::Resume | RestartMode::Fresh | RestartMode::FallbackTemplate => {}
        }

        assert_eq!(
            serde_json::to_value(AgentKind::Claude).unwrap(),
            serde_json::json!("claude")
        );
        assert_eq!(
            serde_json::to_value(AgentKind::Codex).unwrap(),
            serde_json::json!("codex")
        );
        assert_eq!(
            serde_json::to_value(AgentKind::Generic).unwrap(),
            serde_json::json!("generic")
        );
        assert_eq!(
            serde_json::to_value(RestartOffer::FreshOnly).unwrap(),
            serde_json::json!("fresh_only")
        );
        assert_eq!(
            serde_json::to_value(RestartOffer::Resume).unwrap(),
            serde_json::json!("resume")
        );
        assert_eq!(
            serde_json::to_value(RestartOffer::FallbackTemplate).unwrap(),
            serde_json::json!("fallback_template")
        );
        assert_eq!(
            serde_json::to_value(RestartMode::Resume).unwrap(),
            serde_json::json!("resume")
        );
        assert_eq!(
            serde_json::to_value(RestartMode::Fresh).unwrap(),
            serde_json::json!("fresh")
        );
        assert_eq!(
            serde_json::to_value(RestartMode::FallbackTemplate).unwrap(),
            serde_json::json!("fallback_template")
        );
    }

    /// `CreateSession`'s three PLAN_M3.md additions (`intent_key`,
    /// `agent_kind`, `resume_template`) golden-pinned with every one of
    /// them present, matching the treatment every other message shape in
    /// this file gets.
    #[test]
    fn create_session_snapshot_override_fields_json_shape_is_pinned() {
        let msg = ControlMsg::CreateSession {
            req_id: 1,
            cwd: "/some/dir".to_string(),
            invocation: "/opt/bin/claude".to_string(),
            title: None,
            cols: 80,
            rows: 24,
            intent_key: Some("intent-abc".to_string()),
            agent_kind: Some(AgentKind::Claude),
            resume_template: Some(vec![
                "/opt/bin/claude".to_string(),
                "--resume".to_string(),
                "{conversation}".to_string(),
            ]),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            serde_json::json!({
                "type": "create_session",
                "req_id": 1,
                "cwd": "/some/dir",
                "invocation": "/opt/bin/claude",
                "title": null,
                "cols": 80,
                "rows": 24,
                "intent_key": "intent-abc",
                "agent_kind": "claude",
                "resume_template": ["/opt/bin/claude", "--resume", "{conversation}"],
            })
        );
    }

    /// One direction of `CreateSession`'s three new fields' additive-decode
    /// contract, mirroring
    /// `old_shape_session_list_json_decodes_with_defaulted_new_fields`:
    /// JSON shaped as if these fields had not been added yet — no
    /// `intent_key`, `agent_kind`, or `resume_template` at all — must
    /// still decode, with every new field defaulting to `None`. As with
    /// the `SessionInfo` sibling test above, this is intra-version-5
    /// additive discipline, not a claim about interoperating with an
    /// actual pre-M3 (v4) build — a real v4 peer is refused at the
    /// handshake and never reaches this decode path. This is the
    /// "preserving old behavior for raw API users" promise items 6
    /// (`intent_key` idempotency) and 7 (`agent_kind`/`resume_template`
    /// overrides) make: a caller that never learned these fields exist
    /// must get exactly the old behavior (no idempotency, no overrides),
    /// never a decode failure. The REVERSE direction — today's `CreateSession`
    /// bytes decoding under a decoder that predates these fields — is
    /// `new_create_session_json_decodes_under_a_legacy_pre_snapshot_decoder`
    /// below.
    #[test]
    fn old_shape_create_session_json_decodes_with_defaulted_new_fields() {
        let old_shape = serde_json::json!({
            "type": "create_session",
            "req_id": 2,
            "cwd": "/some/dir",
            "invocation": "some-agent",
            "title": null,
            "cols": 80,
            "rows": 24,
        });
        let decoded: ControlMsg = serde_json::from_value(old_shape).unwrap();
        let ControlMsg::CreateSession {
            intent_key,
            agent_kind,
            resume_template,
            ..
        } = decoded
        else {
            panic!("expected ControlMsg::CreateSession, got {decoded:?}");
        };
        assert_eq!(intent_key, None, "an old sender never had this field");
        assert_eq!(agent_kind, None, "an old sender never had this field");
        assert_eq!(resume_template, None, "an old sender never had this field");
    }

    /// The REVERSE tolerance direction (PLAN_M3 review batch item 7),
    /// mirroring `new_session_list_json_decodes_under_a_legacy_pre_status_decoder`:
    /// a decoder shaped like a peer built BEFORE `intent_key`/`agent_kind`/
    /// `resume_template` existed must still decode a NEW sender's
    /// `CreateSession` JSON, silently dropping the three fields it does
    /// not know about. Without this test, the "CreateSession's tolerance
    /// is covered both ways" claim made elsewhere in this file would be
    /// true in only one direction.
    #[derive(Debug, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum LegacyPreSnapshotControlMsg {
        CreateSession {
            req_id: u64,
            cwd: String,
            invocation: String,
            title: Option<String>,
            cols: u16,
            rows: u16,
        },
    }

    #[test]
    fn new_create_session_json_decodes_under_a_legacy_pre_snapshot_decoder() {
        let new_msg = ControlMsg::CreateSession {
            req_id: 6,
            cwd: "/some/dir".to_string(),
            invocation: "/opt/bin/claude".to_string(),
            title: Some("demo".to_string()),
            cols: 80,
            rows: 24,
            intent_key: Some("intent-xyz".to_string()),
            agent_kind: Some(AgentKind::Claude),
            resume_template: Some(vec![
                "/opt/bin/claude".to_string(),
                "--resume".to_string(),
                "{conversation}".to_string(),
            ]),
        };
        let json = serde_json::to_value(&new_msg).unwrap();

        let LegacyPreSnapshotControlMsg::CreateSession {
            req_id,
            cwd,
            invocation,
            title,
            cols,
            rows,
        } = serde_json::from_value(json.clone()).expect(
            "a legacy decoder without intent_key/agent_kind/resume_template must still decode \
             new-shape JSON",
        );
        assert_eq!(req_id, 6);
        assert_eq!(cwd, "/some/dir");
        assert_eq!(invocation, "/opt/bin/claude");
        assert_eq!(title, Some("demo".to_string()));
        assert_eq!((cols, rows), (80, 24));

        // The REAL type round-trips the same JSON too, same as the
        // `SessionList` sibling test does.
        let real_decoded: ControlMsg = serde_json::from_value(json).unwrap();
        assert_eq!(real_decoded, new_msg);
    }

    /// `RestartSession`/`SessionRestarted` round-tripped through the real
    /// encode/decode path, matching `stop_and_delete_roundtrip_through_frames`'s
    /// treatment of the M2 additions — this is what would catch a drift
    /// between the codec's framing and serde's JSON shape for the restart
    /// vocabulary PLAN_M3.md item 9 needs (the create/error vocabulary
    /// above is exercised at the JSON layer only, like `Attach`'s golden
    /// test, since those are simpler unnested shapes).
    ///
    /// One mode suffices here (PLAN_M3 review batch item 9): each mode's
    /// own serialized string is already golden-pinned individually by
    /// `agent_kind_and_restart_vocabulary_json_shapes_are_pinned`, so this
    /// test's only remaining job is proving the codec/serde frame path
    /// itself does not drift, which does not depend on which mode is used.
    #[test]
    fn restart_session_roundtrips_through_frames() {
        let msg = ControlMsg::RestartSession {
            req_id: 42,
            session_id: "s1".to_string(),
            mode: RestartMode::Resume,
            stop_if_running: true,
        };
        let mut wire = Vec::new();
        Frame::control(&msg).encode(&mut wire).unwrap();
        let (frame, used) = Frame::decode(&wire).unwrap().unwrap();
        assert_eq!(used, wire.len());
        assert_eq!(
            serde_json::from_slice::<ControlMsg>(&frame.body).unwrap(),
            msg
        );

        let reply = ControlMsg::SessionRestarted {
            req_id: 42,
            session: SessionInfo {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp".to_string(),
                invocation: "claude".to_string(),
                status: SessionStatus::Alive,
                annotation: None,
                restart_offer: RestartOffer::Resume,
            },
        };
        let mut wire = Vec::new();
        Frame::control(&reply).encode(&mut wire).unwrap();
        let (frame, used) = Frame::decode(&wire).unwrap().unwrap();
        assert_eq!(used, wire.len());
        assert_eq!(
            serde_json::from_slice::<ControlMsg>(&frame.body).unwrap(),
            reply
        );
    }

    /// Golden JSON for `RestartSession`, pinned the same way as
    /// `stop_and_delete_json_shapes_are_pinned`: a serde attribute change
    /// here would compile and round-trip cleanly while quietly producing
    /// bytes an unmodified peer cannot parse. `stop_if_running: true` is
    /// used here specifically (its OTHER value, absence-defaults-false, is
    /// its own separate pin below, since that direction is the safety
    /// property that actually matters).
    #[test]
    fn restart_session_json_shape_is_pinned() {
        let msg = ControlMsg::RestartSession {
            req_id: 7,
            session_id: "s1".to_string(),
            mode: RestartMode::Resume,
            stop_if_running: true,
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            serde_json::json!({
                "type": "restart_session",
                "req_id": 7,
                "session_id": "s1",
                "mode": "resume",
                "stop_if_running": true,
            })
        );
    }

    /// The safety-critical direction of `stop_if_running`'s default
    /// (PLAN_M3 review batch item 1): JSON with no `stop_if_running` key
    /// at all — an old-shaped or hand-crafted request — must decode to
    /// `false`, never `true`. Getting this backwards would mean an old or
    /// naive client's restart request kills a live agent's process tree
    /// with no consent ever expressed, which is exactly the authorization
    /// bug this field exists to prevent.
    #[test]
    fn restart_session_stop_if_running_defaults_false_when_absent() {
        let old_shape = serde_json::json!({
            "type": "restart_session",
            "req_id": 1,
            "session_id": "s1",
            "mode": "resume",
        });
        let decoded: ControlMsg = serde_json::from_value(old_shape).unwrap();
        let ControlMsg::RestartSession {
            stop_if_running, ..
        } = decoded
        else {
            panic!("expected ControlMsg::RestartSession, got {decoded:?}");
        };
        assert!(
            !stop_if_running,
            "an absent consent flag must never be read as consent"
        );
    }

    /// Golden JSON for `SessionRestarted`'s FULL outer shape, including its
    /// nested `session` object (PLAN_M3 review batch item 6): a bare
    /// round-trip test, like `restart_session_roundtrips_through_frames`
    /// above, would still pass under a coordinated drift where BOTH the
    /// encode and decode sides of a field rename happen to agree with each
    /// other but not with an unmodified peer — the same reasoning
    /// `control_json_shape_is_pinned` and `error_kind_json_shape_is_pinned`
    /// document for why round-trips alone are not enough.
    #[test]
    fn session_restarted_json_shape_is_pinned() {
        let msg = ControlMsg::SessionRestarted {
            req_id: 8,
            session: SessionInfo {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp".to_string(),
                invocation: "claude".to_string(),
                status: SessionStatus::Alive,
                annotation: None,
                restart_offer: RestartOffer::Resume,
            },
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            serde_json::json!({
                "type": "session_restarted",
                "req_id": 8,
                "session": {
                    "id": "s1",
                    "title": "demo",
                    "cwd": "/tmp",
                    "invocation": "claude",
                    "status": { "state": "alive" },
                    "annotation": null,
                    "restart_offer": "resume",
                },
            })
        );
    }

    /// Version 5's additive rule for FIELDS (see `PROTOCOL_VERSION`'s
    /// docs): the future-extra-field tolerance direction, mirroring
    /// `pause_and_resume_with_future_extra_fields_decode_through_parse_control`.
    /// `RestartSession` is the representative message here because it is
    /// the one new REQUEST-shaped variant this PR adds (`CreateSession`'s
    /// tolerance is already covered above by its own pair of tests). The
    /// reverse direction (today's bytes under a future decoder with
    /// defaults) is NOT a separate test here (PLAN_M3 review batch item
    /// 9): its only independent claim would be that a hand-rolled shadow
    /// struct's own `#[serde(default)]` works, which is serde's own
    /// well-tested behavior, not this crate's.
    #[test]
    fn restart_session_with_future_extra_field_decodes_through_parse_control() {
        let frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: br#"{"type":"restart_session","req_id":9,"session_id":"s1","mode":"fresh","stop_if_running":false,"priority":"high"}"#
                .to_vec(),
        };
        let msg = crate::io::parse_control(&frame)
            .expect("a known tag with an unknown extra field must decode, not error");
        assert_eq!(
            msg,
            ControlMsg::RestartSession {
                req_id: 9,
                session_id: "s1".to_string(),
                mode: RestartMode::Fresh,
                stop_if_running: false,
            }
        );
    }
}
