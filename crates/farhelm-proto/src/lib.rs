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
//! can eyeball a protocol trace. Data frames carry opaque bytes on a
//! per-channel basis, and as of version 6 a data channel has one of two
//! meanings, fixed by the control message that established it: terminal
//! output flowing supervisor-toward-client (established by `Attach`), or
//! attachment-upload bytes flowing client-toward-supervisor (established
//! by `BeginUpload`). Keeping both binary keeps PTY throughput and file
//! bytes off the JSON path.
//!
//! ## Paths that cross this protocol are UTF-8-only
//!
//! Every path field that actually travels over the wire (`SessionInfo::cwd`,
//! `ControlMsg::CreateSession::cwd`, and — as of version 6 —
//! `ControlMsg::UploadCommitted::path`) is a Rust `String`, and a `String`
//! is valid UTF-8 by construction — there is no wire representation for a
//! path that isn't. For `UploadCommitted::path` the producing boundary is
//! the supervisor itself: a published attachment path that is not valid
//! UTF-8 (a non-UTF-8 state-directory override is the only way to get
//! one) must fail the commit with an actionable error, never launder
//! through a lossy conversion — the inserted path IS the product here, so
//! a path that merely resembles the real one is worse than a refusal. This is a deliberate v1 contract for *this specific
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

/// Longest session identity accepted from a protocol peer.
///
/// Farhelm currently mints UUIDs (36 bytes), but the wire treats the value
/// as opaque and the helm embeds it verbatim in list cursors. One kibibyte
/// leaves ample room for a future identity format while keeping those
/// cursors replayable through the HTTP request-head limits that carry them.
/// The handshake applies the same bound to `SessionAuth::session_id`, so an
/// authenticated connection cannot retain an identity the rest of the
/// protocol would later refuse.
pub const MAX_SESSION_ID_BYTES: usize = 1024;

/// Protocol version exchanged in the hello. Bumped only for incompatible
/// frame or message changes; the receiving side refuses a mismatch with a
/// clear error per SPEC.md's version-skew rule. Build versions travel
/// alongside for diagnostics only and never gate anything.
///
/// Within version 11 the additive discipline of every prior version
/// continues to apply, with version 9's sharper reading intact: new
/// optional fields with decode defaults are fine WHEN ignoring one is
/// harmless; a field whose omission changes behavior, a new tagged variant,
/// a new REQUIRED field, or a field REMOVAL earns the next bump.
///
/// A field whose whole purpose is to CHANGE what the receiver does is not
/// additive, whatever serde makes of it. SPEC.md's version rule
/// ("Incompatible versions refuse to connect with a clear, actionable
/// error; there is no silent degradation") is the standard being met
/// here, and the hello refusal is the machinery that meets it — a helm on
/// 9 and a supervisor on 8 refuse each other at connect, and the host
/// surfaces `version-skew` with both builds named and the helm's own
/// remediation sentence, instead of quietly running a fleet where
/// automatic reconnects steal sessions.
///
/// The browser edge cannot use the hello, having none: it is gated on the
/// helm's build stamp instead (farhelm-ui's `skew` module), which refuses
/// unattended attaches whenever the helm answering is not the build this
/// bundle was made for. Same rule, same milestone, different handshake.
///
/// What version 10 deliberately does NOT carry, decided in PLAN_M6_75.md item
/// 3 so it is not re-litigated per PR: any supervisor-edge PUSH channel. The
/// helm keeps its 3-second drain plus the existing post-write wake,
/// accepting one drain interval of status staleness; the push problem M6.75
/// solves is the CLIENT edge, where the helm coalesces revisions it would
/// have had to build regardless. Also absent: remembered profile defaults,
/// which the helm owns in helm.db and resolves into a concrete `profile_id`
/// before it ever sends a create — there is nothing for this protocol to
/// carry.
///
/// `protocol_version_is_pinned_at_11` (renamed at every bump since `_at_4`)
/// and `unknown_control_message_tag_fails_decode` below, plus the loop-level
/// teardown test in the farhelm crate's e2e suite, pin both the number and
/// the reasoning so the next milestone cannot re-assume tolerance that was
/// never there.
///
/// The entry-by-entry history for versions 2 through 11 is preserved in
/// `lore/2026-08-20-protocol-version-changelog.md`.
pub const PROTOCOL_VERSION: u32 = 11;

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

/// One decoded frame. `channel` is 0 for control frames; a data frame's
/// channel was established by whichever control request created it —
/// `Attach` for a terminal stream, `BeginUpload` for an attachment
/// upload — and that request is also what fixed the channel's direction
/// and meaning.
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
    /// The peer failed connection admission or a restricted peer requested
    /// an operation outside its permitted slice (PLAN_M7.md item 2).
    Unauthorized,
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

/// Why a non-displacing attach ([`ControlMsg::Attach::if_unowned`]) was
/// refused: another client holds this session.
///
/// Named in this crate for the same reason [`DETACH_REASON_STALLED`] is —
/// three parties have to agree on it byte for byte and none of them can
/// see the others' source: the supervisor emits it, the helm relays it
/// into the browser's detach notice unchanged (the attach-failure arm of
/// its terminal socket), and the browser matches it to decide that this
/// view lost the session and must show its take-control surface rather
/// than keep reconnecting.
///
/// The wording is deliberately IDENTICAL to what a displaced client is
/// told when it is taken over while attached, because the fact is
/// identical — another client attached — and only the timing differs
/// (this one had no socket to be told on at the time). Keeping the two
/// strings the same is what lets a client render one state for one
/// situation instead of inventing a second vocabulary for the half of it
/// that happens to be observed late.
pub const ATTACH_REFUSED_TAKEN_OVER: &str = "another client attached";

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

    /// Wrap opaque bytes for a data channel — terminal output on an
    /// `Attach`-established channel, or an upload chunk on a
    /// `BeginUpload`-established one; the channel's controlling request
    /// is what gives the bytes their meaning and direction. `bytes` is
    /// opaque either way — never inspected, never re-encoded — which is
    /// what keeps arbitrary PTY output (binary, invalid UTF-8) and raw
    /// file contents crossing the wire intact. Encoding rejects a body
    /// that would exceed [`MAX_FRAME_LEN`], before writing any partial
    /// frame.
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

/// What a session's agent is doing, and — once it is doing nothing ever
/// again — how it ended.
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
/// ## The live split (`PROTOCOL_VERSION` 10)
///
/// Version 10 REPLACES the single live status `Alive` with three:
/// [`Running`](SessionStatus::Running), [`Waiting`](SessionStatus::Waiting)
/// and [`Idle`](SessionStatus::Idle) (PLAN_M6_75.md item 3). All three mean
/// the agent's process is up; they differ only in what it appears to be
/// doing, which is SPEC.md's whole promise for this milestone — a fleet
/// list where a user can see which agent needs them.
///
/// Two consequences a reader must hold onto:
///
/// - **Wrong is cosmetic, always.** SPEC.md fixes this: the waiting/idle
///   boundary is heuristic by contract, and nothing about interaction may
///   ever wait on a status. Typing into a mis-classified session works
///   untouched. Consumers may render a status and may filter by one; none
///   may gate a lifecycle decision on the difference between these three.
///   The one question anybody is entitled to branch on is live-versus-
///   ended, which is what every consumer's own liveness predicate answers.
/// - **Replacement, not addition.** `Alive` is gone from the wire, so this
///   is a removal alongside three new tagged variants — the two cases
///   `PROTOCOL_VERSION`'s own docs say can never be additive, in one
///   change. A v9 decoder cannot represent `running`; a v10 decoder cannot
///   represent `alive`. The hello refusal is the machinery that makes
///   neither reachable.
///
/// ## Where the three come from, and why no consumer may model it
///
/// A supervisor classifies a live session by SAMPLING its terminal
/// periodically (PLAN_M6_75.md item 2): recent output means `Running`,
/// quiet means `Idle`, and a captured screen matching a known per-agent
/// prompt shape means `Waiting`. That is the mechanism today, and naming it
/// here is documentation, not a promise: the heuristic is expected to be
/// re-tuned as real agents change, and a peer that inferred timing or
/// transition ORDER from it — "idle is always preceded by running", "a
/// waiting session must have been running within N seconds" — would be
/// building on something no version of this protocol guarantees. The
/// guarantees are the ones above: all three mean alive, and which one it is
/// is cosmetic.
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
    /// claiming a live status there would itself be a fabricated liveness
    /// claim.
    /// `ListSessions` is the only reply that computes a REAL answer (from
    /// tmux, via `service.rs`'s `session_status`); every other place this
    /// value is produced is honestly saying "not yet known", not "known
    /// to be running".
    ///
    /// ## Internal/compat vocabulary: this variant MUST NEVER RENDER
    ///
    /// As of `PROTOCOL_VERSION` 10 (PLAN_M6_75.md item 3) `Unknown` is
    /// plumbing, not a status a user is ever shown. It stays on the wire
    /// because it has two jobs nothing else can do — decoding a sender that
    /// predates the field, and letting a create-time reply refuse to
    /// fabricate liveness — but "the system has not classified this yet" is
    /// not information a badge can honestly carry, and a badge reading
    /// `unknown` is worse than no badge: it looks like a verdict.
    ///
    /// So the rule is stated here, at the vocabulary, because two different
    /// mechanisms enforce it in two different places and neither can see the
    /// other:
    ///
    /// - **Restart** never surfaces it at all: the helm's merge rule
    ///   (`manager::merged_status`) refuses to let an `Unknown` overwrite a
    ///   status it already knows definitely, so the prior classification
    ///   stays on screen across the gap.
    /// - **Create** has no prior definite status to hold, so the client
    ///   shows NO STATUS BADGE until the first classified status arrives
    ///   (farhelm-ui's `status::status_badge` returns nothing for this
    ///   variant, and the row renders no badge element at all).
    ///
    /// No latency is promised for that gap, and none can be: nothing in
    /// this protocol orders a classification against the write that created
    /// the session, so "until the first classified status arrives" is the
    /// whole of what can honestly be said. The badge's ABSENCE is what
    /// makes the gap harmless at any length — which is precisely why the
    /// never-render rule, not a latency bound, is the contract.
    #[default]
    Unknown,
    /// The agent is alive and appears to be working — the baseline live
    /// status, and the one a session gets whenever nothing more specific
    /// has been established, including before it has ever been sampled.
    ///
    /// Alive-ness is tmux's pane being not-dead, exactly as `Alive` was
    /// before version 10; what is NEW is the claim about activity, which is
    /// sampled rather than observed and therefore heuristic. A consumer that
    /// needs "is this session live" must ask that question directly (its own
    /// liveness predicate over all three live variants), never by comparing
    /// against this one variant — that comparison is exactly what the
    /// version-10 split turned into a silent wrong answer.
    Running,
    /// The agent is alive and appears to be blocked on the user — a detected
    /// question or an approval prompt sitting unanswered.
    ///
    /// This is the status the whole milestone exists for: SPEC.md's promise
    /// is a fleet list where the sessions that need a human stand out.
    /// Detection is per-agent-kind and best-effort by contract (the captured
    /// tail matching a known prompt shape), so a `Waiting` that is really
    /// idle, or an idle that is really waiting, is a cosmetic wrong answer
    /// and never a functional one.
    Waiting,
    /// The agent is alive and at rest — no recent output, and nothing that
    /// looks like a pending question.
    ///
    /// Deliberately distinct from `Waiting` rather than merged into one
    /// not-running status: "finished, awaiting nothing" and "stuck on a
    /// question nobody answered" call for opposite user actions, and
    /// collapsing them would hide exactly the sessions the list is supposed
    /// to surface. Distinct from `Exited` in the other direction: the agent
    /// is still there, and typing into it still works.
    Idle,
    /// The agent's process has ended. `exit_code` is tmux's own
    /// `#{pane_dead_status}` when parseable — `None` covers a signal
    /// death tmux cannot reduce to a plain code, and the restart-gap and
    /// stale-lookup cases where there is no live pane to ask at all (see
    /// the supervisor's `ListSessions` handler). As of PLAN_M3.md item 2 a
    /// code the supervisor witnessed EARLIER is retained once the pane
    /// that held it is gone, so `None` now means nothing ever observed one
    /// — not merely that nothing can observe one right now.
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

impl SessionStatus {
    /// Whether the agent behind this session is still there — the ONE
    /// question a consumer of this enum is entitled to branch behavior on
    /// (see the type's own docs: the live statuses differ cosmetically, and
    /// nothing about interaction may wait on which one it is).
    ///
    /// Exists because `PROTOCOL_VERSION` 10 turned "is this session live"
    /// from an equality against a single variant into a three-way question,
    /// and every `== Alive` in the tree would otherwise have become a silent
    /// `false` for a session that is very much alive — a wrong answer with
    /// no compile error to announce it. Routing the question through one
    /// predicate is what makes the NEXT such split a single edit.
    ///
    /// Written as an exhaustive `match` rather than a `matches!` for the
    /// same reason farhelm-ui's mirror of this predicate is: `matches!`
    /// would send every future variant to `false` by default, moving the
    /// trap rather than removing it. Spelling out every arm makes a new
    /// status a compile error here, which is where the decision belongs.
    ///
    /// `Unknown` is deliberately NOT live, matching SPEC.md's no-guessing
    /// rule: an unclassified status is uncertainty, and rounding it up to a
    /// liveness claim is precisely the guess that rule forbids.
    pub fn is_live(&self) -> bool {
        match self {
            SessionStatus::Running | SessionStatus::Waiting | SessionStatus::Idle => true,
            SessionStatus::Unknown
            | SessionStatus::Exited { .. }
            | SessionStatus::Error { .. }
            | SessionStatus::Interrupted => false,
        }
    }
}

/// A session as the supervisor reports it. The supervisor is authoritative
/// (SPEC.md); the helm never invents or mutates these fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    /// The session that created this one through `farhelm spawn`, if any
    /// (PLAN_M7.md item 2). `None` covers interactive sessions,
    /// deliberately parentless spawned sessions, and senders predating the
    /// field; all three mean that no trustworthy parent is known.
    pub parent: Option<String>,
    pub title: String,
    /// Seconds since the Unix epoch when this session was created —
    /// `StoredSession::created_at`'s own value, carried onto the wire
    /// unchanged (see that field's docs for where it comes from: the
    /// supervisor mints it once at insert time and every later reply
    /// re-reads the stored row, so SQLite is the durable record, not a
    /// second independently-timed source). PLAN_M6.md item 1
    /// needs it on the wire for the pagination cursor's ordering key: pages
    /// are creation-time descending with `id` as tiebreak, and a cursor
    /// resuming that walk has to encode the same key `SessionInfo` reports.
    /// `#[serde(default)]` is what makes this additive within
    /// `PROTOCOL_VERSION` 8, the same tolerance argument `status` and
    /// `tabs` make above: an old sender's JSON has no `created_at` at all
    /// and decodes to `0`, which readers must document as "sender predates
    /// the field" (the same reading `SessionList::total` assigns its own
    /// defaulted 0), not "created at the epoch". In the descending order
    /// pagination uses, a 0 simply sorts last; no consumer may assign it
    /// more meaning than that.
    ///
    /// DECISION (accepted collision, not guarded against): a genuine
    /// pre-epoch host clock also mints `0` (`store::now_unix`'s own docs
    /// carry the mirrored argument), which is indistinguishable on this
    /// side of the wire from "sender predates the field" — both read the
    /// same way and sort the same way. Accepted rather than special-cased,
    /// because a pre-epoch system clock is not a supported configuration
    /// and the worst case is exactly the sort-last treatment a legacy
    /// sender already gets; total order over the page still holds either
    /// way through the `id` tiebreak.
    #[serde(default)]
    pub created_at: i64,
    /// Seconds since the Unix epoch when the supervisor last saw this
    /// session's agent pane CHANGE — the ordering key a "most recently
    /// active" session sort needs, and nothing more.
    ///
    /// The observation behind it is the same screen comparison that drives
    /// `status` (the supervisor's activity sampler), so it inherits that
    /// comparison's coarseness: output that lands and is overwritten
    /// between two samples was never seen, and a session sampled rarely
    /// because its host is busy advances this rarely. It is also
    /// QUANTIZED at the source — the supervisor only moves it when the
    /// change it saw is at least a minute newer than the value it already
    /// holds — so consumers must read it as "activity around this time",
    /// never as a precise instant. Nothing about liveness may be inferred
    /// from it: `status` is the only field that answers "is this session
    /// doing something right now", and this one keeps its last value for
    /// a session that has exited.
    ///
    /// A session that has never produced observed output carries its own
    /// `created_at`, so a sort over this field degrades to creation order
    /// rather than to a pile of sessions at the epoch. That is what a
    /// CURRENT sender always sends — including for rows that predate the
    /// field, which the supervisor's schema-13 migration normalized at
    /// upgrade time — so 0 on the wire says one thing only: the sender is
    /// older than the field, and `#[serde(default)]` filled the missing
    /// key in. Receivers must read that 0 as "unknown, fall back to
    /// `created_at`" rather than as 1970, and the fallback is exactly that
    /// compatibility rule rather than a general one — nothing local
    /// should be storing a synthesized value back. It carries the same
    /// pre-epoch-clock collision `created_at`'s own docs accept —
    /// indistinguishable, and harmless for the same reason.
    #[serde(default)]
    pub last_activity_at: i64,
    /// Monotonic creation order assigned by this session's supervisor.
    ///
    /// `None` means the sender predates the field. Consumers comparing
    /// provenance must then fall back to the older `(created_at, id)`
    /// ordering; they must not compare a present sequence with an absent
    /// one as though absence meant zero.
    #[serde(default)]
    pub creation_seq: Option<u64>,
    /// Working directory the session was created in. UTF-8-only by
    /// construction (see the module-level "Paths are UTF-8-only" note) —
    /// a non-UTF-8 host path cannot reach this field; it must have been
    /// rejected at the boundary before a `SessionInfo` could exist.
    pub cwd: String,
    pub invocation: String,
    /// `Unknown`, the live statuses (`Running`/`Waiting`/`Idle`), and
    /// `Exited` are computed fresh by the supervisor on
    /// every `ListSessions` reply through LIVE tmux probing — that half of
    /// this field is never persisted, because tmux's own pane state is the
    /// only truth for a currently-reachable session, and it does not
    /// survive a supervisor restart on its own terms either.
    /// `Interrupted` and `Error` (PLAN_M3.md items 2 and 3) are the
    /// opposite case: they exist BECAUSE live tmux probing has nothing to
    /// say (tmux itself did not survive the reboot, or never started at
    /// all) and are instead read back from the supervisor's own durable
    /// last-known-outcome record — so "never persisted" describes the
    /// live-probed variants only, not this field as a whole. `Exited` is
    /// now BOTH: computed from a live dead pane where one still exists,
    /// and otherwise retained from that same record — which is what lets
    /// an exit code and a stop annotation outlive the pane that held them.
    /// Either way, this field is never trusted from an older sender.
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
    /// branches on. The supervisor's StopSession handler records it
    /// durably (PLAN_M3.md item 4); sessions never stopped by the user
    /// carry `None`; an `Option<String>` field defaults to `None` on a
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
    /// The session's terminal tabs, in creation order (PLAN_M4.md item
    /// 2). Rediscovered from tmux for every reply that carries a
    /// `SessionInfo` — never persisted, exactly like the live-probed half
    /// of `status`, and for the same reason: tmux is the only truth for
    /// what windows exist, and SPEC.md makes tabs non-durable anyway.
    /// Rides on `SessionInfo` rather than behind a dedicated list message
    /// for `RestartOffer`'s reason (see that type's design-decision doc):
    /// every client that could ask already holds a `SessionInfo`, so a
    /// query pair would buy a round trip and nothing else. Empty for a
    /// session with no tabs — and, via `#[serde(default)]`, for a sender
    /// that predates the field, which is the same honest statement: no
    /// tabs known.
    #[serde(default)]
    pub tabs: Vec<TabInfo>,
    /// Durable archive metadata, not a session status (PLAN_M7.md item 2).
    /// An older sender has no archive vocabulary, so absence honestly
    /// decodes as `false`.
    #[serde(default)]
    pub archived: bool,
    /// The profile this session was CREATED from, if it was created from
    /// one at all (PLAN_M6_75.md item 3). `None` means raw-created — the
    /// session names an invocation and no profile ever shaped it — which is
    /// also what a sender predating this field decodes to, and the two
    /// readings agree: no profile is known for this session either way.
    ///
    /// See [`SourceProfile`] for the durability contract this field
    /// implements (an immutable snapshot plus one derived existence state)
    /// and for why the profile's CURRENT name is deliberately not here.
    ///
    /// Absent from EVERY session in the build that introduced it: the
    /// supervisor's profile catalog is PLAN_M6_75.md's step 5, so at this
    /// bump every session is raw-created and this field is `None`
    /// everywhere on the wire. That is the same vocabulary-first shape
    /// version 8 shipped `Hello::host_identity` under, and it makes the
    /// absent case the one every current consumer must handle correctly.
    pub source_profile: Option<SourceProfile>,
}

/// A spawned session's attribution credential, presented once in its
/// connection hello (PLAN_M7.md item 2).
///
/// Presence asks the supervisor to treat the peer as restricted. This is
/// deliberate self-scoping rather than a same-uid security boundary: a
/// local process can still omit the credential and connect through the
/// user's protected socket. Admission and operation filtering arrive in
/// PLAN_M7.md item 4; this type only fixes the wire shape.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionAuth {
    /// The session identity this credential authenticates. It is distinct
    /// from `CreateSession::parent`, which is optional ancestry metadata,
    /// and is capped at [`MAX_SESSION_ID_BYTES`] during the handshake.
    pub session_id: String,
    /// The unguessable bearer value minted specifically for that session.
    /// It remains opaque, but the handshake bounds its encoded length; see
    /// [`io::MAX_SESSION_AUTH_TOKEN_BYTES`].
    pub token: String,
}

impl std::fmt::Debug for SessionAuth {
    /// Keep attribution useful in diagnostics without exposing the bearer
    /// value through `ControlMsg`'s derived `Debug` implementation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionAuth")
            .field("session_id", &self.session_id)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// Which profile a session was created from, as the session itself
/// remembers it (PLAN_M6_75.md item 3).
///
/// ## The snapshot rule: immutable identity, derived existence
///
/// SPEC.md requires that editing or deleting a profile does not disturb the
/// sessions already created from it. This type is how that promise is kept
/// with exactly ONE copy of the truth rather than two that could disagree:
///
/// - `id` and `name` are SNAPSHOTTED at creation and never rewritten. They
///   describe what the user chose at the moment they chose it, so a session
///   list stays stable — and filterable — under any later edit. Nothing
///   MUTABLE lives in the snapshot.
/// - `existence` is DERIVED when a reply is built, by one catalog lookup on
///   `id`. Absent from the catalog means the profile was deleted; present
///   under a different name means it was renamed.
///
/// The alternative — rewriting every historical session's row on a profile
/// delete — was rejected: it destroys the record of what the session was
/// actually created from, it is O(sessions) work on a user action that
/// should be O(1), and it can half-fail. Deriving on read costs one catalog
/// lookup per SNAPSHOT resolved (so a page of sessions costs one per
/// profile-created row on it, not one for the reply) and cannot get out of
/// step with the catalog, because it IS the catalog.
///
/// ## Why the CURRENT name is not carried
///
/// A renamed profile's new name is knowable at reply-build time, and is
/// still deliberately absent. Carrying it would put a mutable copy of
/// catalog state on every session row — precisely the second copy of
/// existence truth this design exists to avoid — and it is not what a
/// client should render anyway: a session created from "Claude Code" was
/// created from "Claude Code", and SPEC.md's snapshot rule is a promise the
/// list keeps saying so. A surface that genuinely needs today's name (a
/// profile editor, say) reads the catalog, where it is authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProfile {
    /// The profile's immutable identity, as snapshotted at creation. This
    /// is the key `existence` was derived by, and the key a client filters
    /// or groups by — never `name`, which two profiles may share over time.
    pub id: String,
    /// The profile's name AS SNAPSHOTTED at creation — not its current
    /// name, and never refreshed. See this type's own docs for why.
    pub name: String,
    /// What a catalog lookup on `id` found when this reply was built.
    pub existence: ProfileExistence,
}

/// What became of the profile a session was created from, derived fresh on
/// every reply that carries a [`SourceProfile`] (PLAN_M6_75.md item 3).
///
/// Never persisted anywhere: this is a statement about the catalog AT REPLY
/// TIME, and a session row that stored it would be wrong the moment the
/// catalog changed. A client caches it exactly as long as it caches the
/// `SessionInfo` that carried it, and no longer.
///
/// Every variant is a unit variant, so — like [`RestartOffer`] and
/// [`AgentKind`], and unlike [`SessionStatus`] — this serializes as a bare
/// snake_case string rather than a tagged object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileExistence {
    /// The profile still exists under the snapshotted name: what the
    /// session says it came from is what the catalog still holds.
    Present,
    /// The profile still exists, under a DIFFERENT name than the one
    /// snapshotted here. The session keeps showing its snapshotted name
    /// (SPEC.md's rule that an edit does not touch existing sessions); this
    /// variant is what lets a client say so honestly rather than implying
    /// the snapshot is current.
    Renamed,
    /// No profile with this id is in the catalog any more. The session is
    /// unaffected — it holds its own durable launch and resume snapshot,
    /// which is what a restart runs — and still filters under its
    /// snapshotted name; a client renders it as naming a profile that no
    /// longer exists.
    Deleted,
}

/// How many profiles one supervisor's catalog may hold (PLAN_M6_75.md items
/// 3 and 4).
///
/// The bound is what makes [`ControlMsg::ListProfiles`]'s unpaginated reply
/// SAFE rather than merely convenient: together with
/// [`PROFILE_FIELD_CAP`] it puts a hard ceiling on `ProfileList`'s encoded
/// size, so a catalog can never grow into a reply too large to send. That
/// failure would be unrecoverable in the worst way — the listing is also
/// how a client would find the profile it needs to DELETE, so a catalog
/// that outgrew the frame limit could never be listed and therefore never
/// be trimmed back. `profile_bounds_keep_a_full_catalog_sendable` pins the
/// arithmetic.
///
/// 128 is far past any hand-curated set (SPEC.md's starter catalog is two,
/// and a profile is something a person writes by hand), which is the point:
/// a bound nobody legitimately reaches costs nothing and closes the hole
/// anyway. The alternative — cursor pagination, as `ListSessions` has — was
/// rejected as disproportionate: sessions accumulate on their own without a
/// ceiling, while profiles do not, and paginating a picker that must show
/// every option to be usable would buy complexity and lose nothing.
///
/// ENFORCEMENT is the supervisor's create/update handlers (PLAN_M6_75.md
/// step 5): a create past this bound is refused with
/// [`ErrorKind::InvalidRequest`] naming the limit, the same shape the
/// supervisor's other caller-supplied bounds use. This bump only fixes the
/// number both sides reason about — like [`DETACH_REASON_STALLED`] before
/// its emitters existed.
pub const MAX_PROFILES_PER_HOST: usize = 128;

/// Combined byte cap on one profile's caller-supplied text — [`Profile`]'s
/// `name` plus `invocation` plus every element of `resume_template`
/// (PLAN_M6_75.md items 3 and 4).
///
/// The per-record half of the bound [`MAX_PROFILES_PER_HOST`] completes;
/// neither alone is enough, since a catalog is oversized either by holding
/// too many profiles or by holding a few enormous ones.
///
/// Deliberately SMALLER than the supervisor's `CREATE_FIELD_CAP` (64 KiB)
/// for the equivalent per-session fields, and the asymmetry is the whole
/// design: a session's fields are bounded because ONE reply carries them,
/// while a profile's are multiplied by the catalog bound before they ever
/// reach a reply. 8 KiB is still three orders of magnitude beyond a real
/// profile (`claude --resume {conversation}` is 30 bytes), and the product
/// with `MAX_PROFILES_PER_HOST` leaves `ProfileList` an order of magnitude
/// of headroom under [`MAX_FRAME_LEN`] — pinned, by encoding an actual
/// worst-case catalog rather than by multiplying, in
/// `profile_bounds_keep_a_full_catalog_sendable`.
pub const PROFILE_FIELD_CAP: usize = 8 * 1024;

/// One agent profile as the supervisor holds it (PLAN_M6_75.md items 3 and
/// 4): a named, editable definition of how to launch an agent, and how to
/// resume one.
///
/// SPEC.md's "a fresh supervisor is not empty" makes profiles the ordinary
/// way sessions get created — a user picks a profile rather than typing a
/// command line — while the raw invocation path stays for the API, the e2e
/// harness, and anyone who wants to run something a profile does not
/// describe. Profiles are per-supervisor: they are not synced between hosts
/// (post-v1), so a profile id only means anything to the host that minted
/// it.
///
/// Deliberately NOT carrying an initial prompt: automatic prompt delivery is
/// post-v1 and PLAN_M6_75.md keeps the field out of the schema on purpose,
/// so that a later decision about how prompts are delivered is not
/// pre-empted by a field nothing fills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Supervisor-minted opaque identity, stable across every rename. This
    /// is what a create names ([`ControlMsg::CreateSession::profile_id`])
    /// and what a session's [`SourceProfile`] snapshots; clients echo it
    /// back and never parse it. Distinct from `name` on purpose: a name is
    /// the user's label and changes, an id is the reference and does not.
    pub id: String,
    /// The user's label for this profile, shown in pickers and in the
    /// session list. Mutable — an edit changes it, and every session
    /// already created from this profile keeps the name it snapshotted.
    pub name: String,
    /// The launch invocation, as one shell-parsed command line — the same
    /// spelling and the same parsing rules as
    /// [`ControlMsg::CreateSession::invocation`], because that is exactly
    /// what a profile-backed create resolves this into.
    pub invocation: String,
    /// Which integrated agent this profile IS, or [`AgentKind::Generic`]
    /// for a profile that names no kind — SPEC.md's "profiles without a
    /// kind get generic treatment", spelled explicitly.
    ///
    /// Required rather than `Option`, unlike
    /// [`ControlMsg::CreateSession::agent_kind`]'s tri-state, and the
    /// difference is real: `CreateSession`'s absence means "derive the kind
    /// from the invocation's basename", which is a guess a raw caller may
    /// want. A profile is never a guess — a user picked from a list, and
    /// `Generic` is the wire spelling of "I picked none". Two ways to say
    /// the same thing (an absent field AND a `Generic` value) would be one
    /// way too many for a value that decides whether conversation capture
    /// and per-kind status sharpening run at all.
    pub agent_kind: AgentKind,
    /// The resume invocation template, as an argv vector. `None` has TWO
    /// outcomes, decided by `agent_kind`: for a kind with an integration
    /// the supervisor derives that integration's default template, while
    /// for `Generic` there is no integration to derive from and `None`
    /// simply means no resume template — restart falls back to a fresh
    /// launch per SPEC.md. Identical in its `{conversation}` placement
    /// rule to [`ControlMsg::CreateSession::resume_template`] — see that
    /// field for the exact-equality rule and for which kinds require a
    /// placeholder.
    pub resume_template: Option<Vec<String>>,
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
/// needed no new field here either: it is exactly "is `status` one of the
/// live variants", already answerable from this struct (as of
/// `PROTOCOL_VERSION` 10 that is a three-variant question rather than an
/// equality against `Alive` — see [`SessionStatus`]'s live split, and note
/// that a consumer asking it by equality against ONE live variant is
/// exactly the silent wrong answer that split introduced). That is
/// deliberately
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

/// Which of a session's terminals an `Attach` targets (PLAN_M4.md item 1).
/// Before version 6 a session had exactly one terminal, so `Attach` named
/// none; the selector makes that implicit choice explicit without
/// breaking it — `#[default]` is `Agent`, and the field it lives on
/// (`ControlMsg::Attach::terminal`) is `#[serde(default)]`, so a request
/// that says nothing still means what every pre-M4 request meant.
///
/// Internally tagged (`kind`) rather than a bare string because `Tab`
/// carries the tab id — the same reason `SessionStatus` is tagged while
/// `ErrorKind` is not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminalSelector {
    /// The agent's own terminal — the AGENT-MARKED window, which is
    /// window 0 in practice (creation order) but is resolved by its
    /// marker, never by position: PLAN_M4.md item 2 makes rediscovery
    /// marker-based because pane processes inherit `TMUX` and can
    /// conjure windows the supervisor never made. The only terminal
    /// that existed before M4.
    #[default]
    Agent,
    /// A terminal tab by id. The id is the one `TabInfo::id` carries;
    /// naming a tab that no longer exists (closed, or erased by a reboot)
    /// is a `NotFound` error, not a silent fallback to the agent
    /// terminal — attaching the WRONG terminal quietly would be worse
    /// than failing.
    Tab { id: String },
}

/// One terminal tab as the supervisor reports it (PLAN_M4.md item 2). Not
/// durable metadata — SPEC.md says tabs are gone after a reboot or
/// archive and nothing recreates them — so this is a live-rediscovered
/// fact (from tmux, via the window markers), never a stored row.
///
/// Deliberately minimal: SPEC.md gives tabs no names and close is their
/// only operation, so an id is the whole identity. Clients derive their
/// positional labels ("Terminal 1", "Terminal 2") from list order, which
/// the supervisor keeps stable (creation order) across rediscovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabInfo {
    /// Supervisor-assigned opaque tab identity — MINTED per tab and
    /// stored in the window's marker, deliberately not the raw tmux
    /// window id, so it is unique across tmux-server lifetimes, not just
    /// within one: a client holding a selector from before a reboot must
    /// get `NotFound`, never a recycled window index that happens to
    /// name someone else's new tab. Clients echo it back
    /// (`TerminalSelector::Tab`, `ControlMsg::CloseTab`) and never parse
    /// it.
    pub id: String,
}

/// Ceiling for one attachment-upload data frame's body (PLAN_M4.md item
/// 1). Uploads are chunked well below [`MAX_FRAME_LEN`] for the same
/// reason terminal output is — no bulk payload may monopolize the
/// framing layer — and the specific value bounds how long any single
/// frame can occupy the shared connection ahead of latency-sensitive
/// terminal input. Rechunking is the PROTOCOL-FRAME SENDER's job,
/// independent of whatever boundaries the bytes arrived with: the helm
/// relay splits at this size no matter how its HTTP body stream chunked,
/// and any client speaking this protocol directly owes the same. The
/// window alone does not keep typing responsive — a whole window of
/// chunks sitting in the shared writer's FIFO would still delay input
/// frames behind megabytes of bulk — so a sender must also bound how
/// many upload frames it has ENQUEUED to the shared writer at once,
/// interleaving them with whatever else the connection carries; the
/// credit window bounds transit, the sender bounds queue occupancy.
/// An oversized chunk arriving anyway is a data frame with no `req_id`
/// to hang an `Error` on, so the receiver rejects it the
/// channel-correlated way: [`ControlMsg::UploadAborted`] on that
/// channel, temp cleaned — never an uncorrelated `Error` some other
/// upload might claim.
pub const UPLOAD_CHUNK_BYTES: usize = 256 * 1024;

/// Maximum bytes an upload sender may have in flight beyond the highest
/// cumulative [`ControlMsg::UploadAck`] it has seen (PLAN_M4.md item 1).
/// This is the upload direction's flow control — the analogue of the
/// terminal path's watermark pause/resume, shaped as a credit window
/// because the receiver (disk, or the helm→supervisor relay) is the slow
/// side here, not a rendering client. The initial credit is explicit so
/// no implementation pair can deadlock waiting for the other to move:
/// [`ControlMsg::UploadStarted`] itself grants the first window from a
/// cumulative baseline of zero — the sender may put `UPLOAD_WINDOW_BYTES`
/// on the wire before the first ack ever arrives. The window is what
/// keeps "no size cap in v1" honest: a large file costs time, never
/// memory, on every hop. It bounds ONE transfer; the bound on how many
/// transfers may run at once is `BeginUpload`'s admission decision (see
/// that variant's docs), not this constant.
pub const UPLOAD_WINDOW_BYTES: u64 = 4 * 1024 * 1024;

/// The `reason` an unsolicited [`ControlMsg::UploadAborted`] carries when
/// a transfer was given up on for lack of progress (PLAN_M4.md item 4's
/// per-hop progress timeout — SPEC.md's health-check requirement applied
/// to the paste path). Like [`DETACH_REASON_STALLED`], it is named here
/// because two independent emitters produce it — the supervisor (its
/// receiving hop stopped progressing) and the helm (the client→helm hop
/// stalled) — and their tests must match the identical string. Clients
/// render it inside their upload-failure surface verbatim, so it reads
/// as a bare cause: one line, user-legible, no leading "aborted:".
pub const UPLOAD_ABORT_REASON_STALLED: &str = "transfer stopped making progress (stalled)";

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
/// so one connection can carry concurrent requests; unsolicited events —
/// `Detached`, as of version 6 `UploadAck` and `UploadAborted`, and as of
/// version 7 `ReplayComplete` — carry no `req_id` and correlate by
/// `channel` instead, so a demultiplexer must route them by channel
/// rather than treating a missing `req_id` as an error.
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
        /// Diagnostic free text such as "helm" or "supervisor". It is
        /// never an authorization input; only `auth` selects restricted
        /// admission.
        role: String,
        /// The supervisor's identity (PLAN_M6.md item 2): a UUIDv4 minted
        /// once at first run (`SessionStore::ensure_host_identity`) and
        /// stored in `supervisor_meta`, never regenerated while that row
        /// exists — SPEC.md's promise that a supervisor is "generated by
        /// its supervisor at install time" and immutable thereafter, so
        /// wiping a supervisor's state dir mints a new one and is exactly
        /// SPEC.md's reinstall semantics. `Some` on every hello the
        /// supervisor sends (`io::handshake_with_host_identity`); always
        /// `None` from the helm, which has no identity of its own to
        /// report — only the supervisor's value is ever consulted, by the
        /// connection manager's first-contact and mismatch handling
        /// (PLAN_M6.md item 4, a later PR: this one only fills the field,
        /// it does not yet act on a mismatch).
        host_identity: Option<String>,
        /// Attribution and deliberate self-scoping for a spawned peer.
        /// Presence, never `role`, selects restricted admission. The
        /// admission logic itself arrives in PLAN_M7.md item 4.
        auth: Option<SessionAuth>,
    },
    /// Create and launch a session. This is the one true creation path:
    /// the M1 CLI flags and any future UI dialog both land here
    /// (PLAN_M1.md: flags bypass the creation UI, never the creation API).
    ///
    /// ## Three launch selectors, exactly one per request
    ///
    /// PLAN_M7.md item 2 adds name-based profile selection to the raw and
    /// profile-id selectors version 10 already carried:
    ///
    /// - **Raw**: `invocation` names a command line, and `agent_kind` /
    ///   `resume_template` optionally override what would be derived from
    ///   it. Both profile selectors are `None`.
    /// - **Profile id**: `profile_id` names one of the target host's profiles
    ///   and the SUPERVISOR resolves every launch-shaping value from it.
    /// - **Profile name**: `profile_name` carries spawn's human-facing
    ///   `--agent` value. PLAN_M7.md item 4 resolves it inside creation,
    ///   before reserving or launching anything, so a restricted peer
    ///   needs no catalog-read authority and cannot race a list result.
    ///
    /// **A request naming more than one selector is refused with
    /// [`ErrorKind::InvalidRequest`].** A full-authority peer naming none is
    /// refused too. A session-authenticated peer is the sole exception:
    /// omitting all three means derive the host's last-used profile, which
    /// is the `farhelm spawn --cwd ...` default. The exclusivity is stated
    /// here and enforced by the supervisor's create handler rather than
    /// made structurally impossible by the type, deliberately: a hybrid —
    /// a profile plus a hand-written override —
    /// is exactly the request whose meaning nobody can pin down (does the
    /// override win? does the session's snapshot then still belong to the
    /// profile it names?), and the honest answer to an ambiguous request is
    /// a refusal, not a precedence rule invented at the handler. Refusing
    /// it explicitly also means the refusal has a MESSAGE, which a type
    /// that simply could not express the request would not.
    ///
    /// The chosen selector and its value join the idempotency fingerprint
    /// (`intent_key` below), as does `parent`: a retry that changes any of
    /// them is refused as key reuse rather than silently launching a
    /// different child.
    ///
    /// A profile-mode create names a profile that may have been deleted
    /// between the picker read and the submit — a real race, not a
    /// theoretical one — and that is a visible failure with no session
    /// created, never a silent fall back to some other profile
    /// (PLAN_M6_75.md item 4, checked before launch).
    CreateSession {
        req_id: u64,
        /// The spawning session, when this create came from `farhelm
        /// spawn`. PLAN_M7.md item 4 validates it against the connection's
        /// authenticated identity; this vocabulary-only step carries and
        /// fingerprints the caller's value without trusting it.
        parent: Option<String>,
        /// Working directory to launch the agent in. UTF-8-only (see the
        /// module-level "Paths are UTF-8-only" note); the sender must
        /// reject a non-UTF-8 host path before it ever reaches this
        /// field, not launder it through a lossy conversion.
        ///
        /// Required under every selector: a profile says what to run,
        /// never where. SPEC.md's session identity is an agent in a
        /// directory, and the directory is always the caller's choice.
        cwd: String,
        /// The agent command line under the raw selector. `None` means one
        /// of the profile selectors supplies it — see this variant's own
        /// exclusivity contract.
        ///
        /// Was a required `String` before `PROTOCOL_VERSION` 10, which is
        /// part of what forced that bump. A profile-mode request reaches a
        /// v9 peer as `"invocation": null` — the key is PRESENT, since this
        /// crate's encoder never omits an `Option` — and a required
        /// `String` refuses a null outright. (Absence would have been the
        /// lenient case; this is not that case, which is what makes the
        /// refusal dependable.) The other direction is safe: a v9 request
        /// decoded here is a raw create exactly as it always was. The
        /// handshake is what keeps the unsafe direction from happening at
        /// all.
        invocation: Option<String>,
        /// The profile to create from, in PROFILE mode — a [`Profile::id`]
        /// belonging to the supervisor being asked (profiles are
        /// per-host and never synced, so an id from another host means
        /// nothing here). `None` means another selector was chosen.
        ///
        /// A REMEMBERED default profile is not on this wire and never will
        /// be: the helm owns per-host last-used defaults in helm.db and
        /// resolves one into a concrete id before sending, so the
        /// supervisor only ever sees a definite choice. That keeps
        /// defaulting policy — including SPEC.md's ask-don't-guess rule
        /// when the remembered profile is gone — in the one component that
        /// can actually ask the user.
        profile_id: Option<String>,
        /// A human-facing profile name selected by `farhelm spawn`.
        /// Exactly one of `invocation`, `profile_id`, or `profile_name` is
        /// present. PLAN_M7.md item 4 resolves the name atomically inside
        /// creation before reserving or launching anything.
        profile_name: Option<String>,
        title: Option<String>,
        cols: u16,
        rows: u16,
        /// Client-supplied idempotency key (PLAN_M3.md item 6): a create
        /// retried with the same key and an identical fingerprint of
        /// every session-shaping field (this struct's fields below
        /// included, but never `cols`/`rows` — those shape the
        /// attachment, not the session) replays the original outcome
        /// instead of launching a second process. The selected launch
        /// selector and value join the fingerprint, and version 11 adds
        /// `parent`; a retry cannot change any of them under cover of the
        /// same key. `None` preserves
        /// pre-M3 behavior exactly: every request is its own create, with
        /// no deduplication — the safe default for raw API callers (curl,
        /// an older UI build) that never learned this field exists, so
        /// its mere addition does not newly expose them to anything.
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
        /// than reusing absence.
        ///
        /// RAW MODE ONLY. A profile already states its kind
        /// ([`Profile::agent_kind`], where `Generic` is the explicit "no
        /// kind" spelling), so this field alongside either profile
        /// selector is one of the ambiguous requests refused as invalid — see this
        /// variant's own docs.
        agent_kind: Option<AgentKind>,
        /// Explicit override of the resume invocation template PLAN_M3.md
        /// item 7 would otherwise default from `invocation`'s first
        /// token. RAW MODE ONLY, for `agent_kind`'s reason directly above:
        /// a profile carries its own [`Profile::resume_template`], and
        /// naming both is a refused request rather than an override.
        /// Structured as an argv vector, not a shell string, so a
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
    /// a live status — a create-time placeholder consistent with the paragraph
    /// above, since a fast-exiting command can already be dead by the time
    /// this reply reaches the caller. `ListSessions` computes the real
    /// answer from tmux (`service.rs`'s `session_status`); nothing about
    /// creation itself can honestly claim more.
    SessionCreated { req_id: u64, session: SessionInfo },
    /// List sessions, one page at a time (PLAN_M6.md item 1, "Pagination
    /// shape"; served by PLAN_M6.md item 2). `cursor: None` starts from the
    /// front of the order `SessionList`'s own docs define (creation-time
    /// descending, session id ascending as tiebreak); `Some` resumes after
    /// the ordering key a previous `SessionList::next_cursor` encoded, and
    /// is opaque to callers — a client stores and replays it verbatim,
    /// never parses or constructs one itself
    /// (`service::handle_list_sessions`'s own docs cover the encoding and
    /// what an undecodable cursor does). `limit: None` takes the server's
    /// default page size (`LIST_SESSION_CAP`); `Some(n)` for `n >= 1` is
    /// HONORED AS GIVEN — `LIST_SESSION_CAP` is only the default, not a
    /// ceiling a caller cannot ask past (the byte budget remains the real
    /// bound on what any one page can carry, `Some` or not). A caller-
    /// supplied `0` is refused outright rather than honored (see that
    /// handler's docs for why an empty, cursor-bearing page is unsafe to
    /// serve at all).
    ListSessions {
        req_id: u64,
        cursor: Option<String>,
        limit: Option<u32>,
    },
    /// Reply to `ListSessions`: one page of the supervisor's session set,
    /// subject to two independent cuts in `service.rs` — the page limit
    /// (`ListSessions::limit`, or `LIST_SESSION_CAP` by default) enforced
    /// by the `ListSessions` handler itself before a single entry is even
    /// cloned, and an encoded-size budget `build_list_reply` enforces on
    /// top of that (see its own docs for why the cap deliberately does NOT
    /// live inside that function).
    ///
    /// Ordering (PLAN_M6.md's "Pagination shape") is creation-time
    /// descending with session id ascending as the tiebreak — a total order
    /// over stable columns, so a page walk survives sessions being created
    /// or deleted between requests: the order is total and stable, so an
    /// issued cursor stays valid; a walk never sees a duplicate and never
    /// tears. A session created DURING an in-progress walk may be missed by
    /// that walk — same-second creations tie-break by id and can land
    /// behind the cursor, and a clock rollback can place a new session
    /// mid-order — but is guaranteed visible in the next walk, from the
    /// start. A deletion behind the cursor is simply absent from the next
    /// page, never corrupting it; a cursor naming a since-deleted session's
    /// key still resumes cleanly, because resuming means "strictly after
    /// this key" rather than "starting from this row."
    ///
    /// `total` is additive within `PROTOCOL_VERSION` 3, like
    /// `SessionInfo::status` (see that field's docs for the same tolerance
    /// argument), and unchanged in meaning by the version 8 bump: the FULL
    /// session count before any cut, not `sessions.len()` and not scoped
    /// to one page. An old sender's reply decodes `total` as 0 via
    /// `#[serde(default)]`, documented here as "sender predates the field"
    /// rather than "zero sessions" — tolerable because `sessions` itself
    /// is still present and correct either way, and no M2 caller treats a
    /// 0 `total` as authoritative proof of emptiness on its own.
    ///
    /// `next_cursor` REPLACES the pre-8 `truncated` flag (removed, not
    /// deprecated — see `PROTOCOL_VERSION`'s own docs for why that forced
    /// a bump rather than riding an additive one): `None` means this page
    /// reached the end of the order, `Some` carries the opaque resume point
    /// for the next `ListSessions::cursor`. Truncation was an error the
    /// caller had no way to recover from; a cursor is a continuation the
    /// caller can just follow. `Some` is issued whenever EITHER cut above
    /// left sessions unreturned beyond the last entry this page actually
    /// carries — the count limit, the byte budget, or both — never merely
    /// because a cut happened to fire with nothing left behind it. The one
    /// exception is not a page at all: a single session too large to fit
    /// under the byte budget even alone is refused as `ErrorKind::Internal`
    /// rather than sent as `SessionList` (`service::build_list_reply`'s own
    /// docs cover why — an empty page with `next_cursor: None` would claim
    /// the walk was exhausted when it was not).
    SessionList {
        req_id: u64,
        sessions: Vec<SessionInfo>,
        #[serde(default)]
        total: u64,
        next_cursor: Option<String>,
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
    /// is running" already holds. Such a stop records a PLAIN exit, not
    /// [`SessionInfo::annotation`]'s "stopped by user": the annotation
    /// says who ended the run, and a run that had already ended was not
    /// ended by this request (PLAN_M3.md item 4). A stop that does find a
    /// live agent annotates it, and that annotation stays with the session
    /// until a restart replaces the run it describes or a delete removes
    /// the session — surviving supervisor restarts and reboots alike.
    ///
    /// Three distinct failure modes, not one: an unknown `session_id` is
    /// the only PRECONDITION failure, reported the same way `Attach`
    /// reports it. The kill sweep itself — enumerating and signaling the
    /// process tree, see `kill_process_tree` in the supervisor — can also
    /// fail (a `/proc` read erroring out, a signal coming back `EPERM`),
    /// and that is reported as an `Error` too rather than a false
    /// `SessionStopped`: a caller must be able to tell "nothing was
    /// running" from "the sweep could not confirm nothing is running"
    /// apart. And the durable record of the stop can fail to write, which
    /// is reported as well — with wording that distinguishes the two
    /// sides of the kill, since a failure BEFORE it means nothing was
    /// killed at all while one after it means the session did stop but may
    /// list as an ordinary exit.
    StopSession { req_id: u64, session_id: String },
    /// Acknowledges `StopSession`: sent only once the kill sweep has
    /// actually run to completion (or been confirmed unnecessary — a dead
    /// or absent pane, the restart-gap case), never merely because the
    /// request was accepted.
    SessionStopped { req_id: u64 },
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
    DeleteSession { req_id: u64, session_id: String },
    /// Acknowledges `DeleteSession`: sent only once the row, the tmux
    /// session, and (if one existed) the process tree are all positively
    /// confirmed gone. A teardown failure never yields this reply — it
    /// yields `Error` instead, with the row and in-memory entry left in
    /// place for a retry (see the supervisor's delete handler and
    /// lore/2026-07-27-m2-process-tree-stop.md for why removing the last
    /// handle on a possibly-running agent is the one outcome that must
    /// never happen silently).
    SessionDeleted { req_id: u64 },
    /// Tear a session down and hide it from the default merged view while
    /// retaining its metadata (PLAN_M7.md item 5). Confirmation is a client
    /// obligation, so no confirmation flag appears on the wire.
    ArchiveSession { req_id: u64, session_id: String },
    /// Acknowledges [`ControlMsg::ArchiveSession`] with the session as it
    /// stands after teardown. Returning the row makes an ambiguous retry
    /// and an already-archived request the same successful answer, and lets
    /// the helm update its cache before it answers its own caller.
    ///
    /// There is deliberately no unarchive message; restarting an archived
    /// session clears the flag in PLAN_M7.md item 5.
    SessionArchived { req_id: u64, session: SessionInfo },
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
    /// flow, derived from whether `status` is one of [`SessionStatus`]'s
    /// live variants on whatever `SessionInfo` the client last saw (a
    /// three-way question since version 10's live split, never an equality
    /// against a single variant). But that derivation is only a
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
    /// The handler (PLAN_M3.md item 9) is live: it stops a still-running
    /// agent when this request carries consent, reaps the prior run's
    /// descendants, relaunches into the session's own terminal when it
    /// survived, and replies `SessionRestarted`. Every refusal it can
    /// make — a stale mode, a live agent without consent, a vanished or
    /// repointed working directory — leaves the session untouched.
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
    SessionRestarted { req_id: u64, session: SessionInfo },
    /// Rename a session (PLAN_M5.md item 1) — SPEC.md's v1 client-surface
    /// rename verb. The supervisor is the authority on accepted title text;
    /// clients carry it verbatim and render any refusal.
    ///
    /// Validation is supervisor-authoritative, and deliberately identical
    /// in shape to `CreateSession`'s explicit-title rule rather than a
    /// stricter rename-specific rule: a `title` containing any
    /// `char::is_control` character is refused `InvalidRequest` (the
    /// title is echoed into terminals by `tracing` consumers, so an
    /// embedded escape sequence would be terminal injection), and
    /// `title` is capped at create's existing 64 KiB field cap — same
    /// constant, same reason, keeping the `SessionRenamed` reply that
    /// echoes it back structurally deliverable. An empty `title` is
    /// accepted, exactly as an explicit empty title is on create:
    /// SPEC.md names control characters as THE refusal for a supplied
    /// title, and rename inventing a stricter rule would be an asymmetry
    /// SPEC.md nowhere asks for. There is likewise no U+FFFD
    /// sanitization here: sanitization exists for server-derived titles
    /// the caller never chose, and a rename is always caller data, so it
    /// gets the refuse-don't-rewrite treatment instead. Renaming a
    /// session that does not exist is `NotFound`.
    ///
    /// Concurrent renames are last-write-wins, with no version token:
    /// this is one mutable metadata field, both writers hold equal
    /// authority to set it, and optimistic concurrency for a label would
    /// add a conflict surface no user flow can hit deliberately — both
    /// callers get a success reply, and the store ends up holding
    /// whichever write lands last.
    RenameSession {
        req_id: u64,
        session_id: String,
        /// Sent verbatim: no trimming, no client-side validation. The
        /// supervisor's refusal text is the contract (see this variant's
        /// own docs above); duplicating its rules client-side would let
        /// them drift, and rewriting the caller's input before sending
        /// would be the same silently-altering-caller-data move the
        /// supervisor itself refuses to make.
        title: String,
    },
    /// Success reply to `RenameSession`, shaped like `SessionCreated` and
    /// `SessionRestarted` deliberately: the caller gets the authoritative
    /// answer back, not an ack it must follow with a fetch. `session` is
    /// built the same way `ListSessions` builds one — live-probed
    /// `status`, rediscovered `tabs`, freshly computed `restart_offer` —
    /// never a stale stored row with the new title spliced in.
    /// `SessionInfo` is more than a stored title (see its own docs), and
    /// echoing the rest of it stale would hand the caller a `SessionInfo`
    /// that lies about everything but the one field this request changed.
    SessionRenamed { req_id: u64, session: SessionInfo },
    /// Every profile this supervisor holds (PLAN_M6_75.md item 3), ordered
    /// by [`Profile::id`] ASCENDING.
    ///
    /// The order is stated rather than left to whatever the storage layer
    /// happens to return: a contract-free order is one clients depend on
    /// anyway and a query plan changes underneath them. By id rather than
    /// by name because ids are immutable — the list does not reshuffle when
    /// a profile is renamed — and a client wanting the user's alphabet
    /// sorts locally, where it knows the locale.
    ///
    /// Unpaginated, unlike `ListSessions`, and that is arithmetic rather
    /// than optimism: [`MAX_PROFILES_PER_HOST`] and [`PROFILE_FIELD_CAP`]
    /// together cap what this reply can encode to, far below
    /// [`MAX_FRAME_LEN`], so one page is ALWAYS enough. See
    /// `MAX_PROFILES_PER_HOST`'s own docs for why pagination would be the
    /// wrong trade for a hand-curated catalog, and for what an unbounded
    /// one would cost: a catalog too large to list is also too large to
    /// trim, since the listing is how a client finds what to delete.
    ListProfiles { req_id: u64 },
    /// Reply to `ListProfiles`.
    ProfileList { req_id: u64, profiles: Vec<Profile> },
    /// Define a new profile. The supervisor mints its [`Profile::id`] —
    /// which is why this carries the fields rather than a whole [`Profile`]
    /// with a placeholder id — exactly as `CreateSession` lets the
    /// supervisor mint a session id.
    ///
    /// `name` is caller data and gets `RenameSession`'s treatment, for
    /// `RenameSession`'s reasons: a control character is refused rather
    /// than sanitized, and the field is capped. Names are NOT required to
    /// be unique — two profiles may share a name, since `id` is what
    /// anything actually references, and refusing a duplicate name would
    /// turn a cosmetic collision into a workflow dead end.
    ///
    /// TWO BOUNDS apply here and to `UpdateProfile`, and both are refused
    /// with [`ErrorKind::InvalidRequest`] naming the limit: this record's
    /// own text against [`PROFILE_FIELD_CAP`], and — for a create, which is
    /// the only verb that grows the catalog — the catalog's size against
    /// [`MAX_PROFILES_PER_HOST`]. They are what keep `ProfileList`
    /// sendable; see that variant's docs for why an unbounded catalog is a
    /// trap rather than merely untidy.
    CreateProfile {
        req_id: u64,
        name: String,
        invocation: String,
        agent_kind: AgentKind,
        resume_template: Option<Vec<String>>,
    },
    /// Success reply to `CreateProfile`, carrying the profile as stored —
    /// including the id the supervisor just minted, which the caller has no
    /// other way to learn.
    ProfileCreated { req_id: u64, profile: Profile },
    /// Replace a profile's definition wholesale, keyed by
    /// [`Profile::id`]. Every field except the id is mutable; the id is
    /// what makes this an edit rather than a create, and a `profile` naming
    /// an id the catalog does not hold is `NotFound`.
    ///
    /// A FULL replacement rather than a patch of changed fields: a profile
    /// is small, a client editing one already holds all of it, and
    /// per-field optionality would make "clear the resume template" and
    /// "leave the resume template alone" the same request.
    ///
    /// Concurrent edits are last-write-wins with no version token, exactly
    /// as `RenameSession` is and for the same reason — see that variant's
    /// docs. **Nothing about an edit touches sessions already created from
    /// this profile**: their launch and resume snapshots are their own
    /// (SPEC.md's snapshot rule), and their [`SourceProfile`] keeps the
    /// name it snapshotted, reporting [`ProfileExistence::Renamed`] once
    /// this edit changes the name out from under it.
    UpdateProfile { req_id: u64, profile: Profile },
    /// Success reply to `UpdateProfile`, carrying the profile as stored.
    /// Echoed back rather than acknowledged bare for `SessionRenamed`'s
    /// reason: the caller gets the authoritative answer instead of an ack
    /// it must follow with a read.
    ProfileUpdated { req_id: u64, profile: Profile },
    /// Remove a profile from the catalog. Deleting a profile NEVER touches
    /// the sessions created from it — they keep running, keep their durable
    /// launch and resume snapshots, and keep filtering under the name they
    /// snapshotted; what changes is that their [`SourceProfile::existence`]
    /// starts reporting [`ProfileExistence::Deleted`].
    ///
    /// Deleting an unknown id is `NotFound` rather than a silent success:
    /// unlike `StopSession`'s "make sure nothing is running", there is no
    /// weaker reading of "delete this profile" that an absent profile
    /// already satisfies for the caller — a client asking to delete
    /// something that is not there is working from a stale catalog and
    /// should be told.
    DeleteProfile { req_id: u64, profile_id: String },
    /// Acknowledges `DeleteProfile`: the profile is gone from the catalog.
    /// Carries no profile — there is nothing left to describe.
    ProfileDeleted { req_id: u64 },
    /// Open a terminal tab: a plain shell in the session's working
    /// directory, as a new window on the session's tmux session
    /// (PLAN_M4.md item 2). Refused — with the session untouched — when
    /// the working directory has vanished (M3's restart precondition,
    /// same error shape), when the session's tmux session no longer
    /// exists (a rebooted or archived session must be restarted first;
    /// a tab-only terminal substrate is not a state this system has),
    /// and when the shell is already dead by reply time (the pane's last
    /// words travel as the error detail — a launch that failed must not
    /// masquerade as a successful open holding a dead pane).
    OpenTab { req_id: u64, session_id: String },
    /// Success reply to `OpenTab`. The tab exists and its shell was alive
    /// when this was sent; attach it via `Attach` with
    /// `TerminalSelector::Tab`. Carries the `TabInfo` alone rather than a
    /// whole refreshed `SessionInfo` because opening a tab changes
    /// nothing else about the session — which also means this reply
    /// says nothing about ORDER: a client deriving positional labels
    /// (or racing another client's opens) reads the creation-ordered
    /// list from a refreshed `SessionInfo::tabs`, the one place ordering
    /// is authoritative; this reply is only what the opener needs to
    /// attach immediately.
    TabOpened { req_id: u64, tab: TabInfo },
    /// Close a terminal tab: kill its shell and every process it left
    /// behind (SPEC.md: close "kills that shell and its processes" —
    /// daemonized children included), then drop the window. The reap runs
    /// in M2's stop ordering, scoped to the one tab (PLAN_M4.md item 2);
    /// the agent terminal and other tabs are untouched. An unknown
    /// `tab_id` is `NotFound`; closing a tab whose shell already exited
    /// still succeeds — like `StopSession`, "make sure nothing is
    /// running" already holds, and the window is dropped either way.
    CloseTab {
        req_id: u64,
        session_id: String,
        tab_id: String,
    },
    /// Acknowledges `CloseTab`: sent only once the tab-scoped reap ran to
    /// completion and the window is gone — `SessionStopped`'s honesty
    /// rule, per tab. A reap or teardown failure yields `Error` instead,
    /// never a false success.
    TabClosed { req_id: u64 },
    /// Attach to one of a session's terminals. The requester picks the
    /// (connection-unique) data channel; the supervisor replays history
    /// onto it and then streams live output. SPEC.md's one-attachment
    /// rule is enforced per SESSION across all its terminals, grouped by
    /// `lease` (PLAN_M4.md item 3): an attach under a different lease
    /// detaches EVERY channel the previous lease held on this session,
    /// atomically — all old channels are detached before the new attach
    /// completes, so no interleaving where both leases hold terminals is
    /// ever observable. Each detached channel still gets its own
    /// `Detached` (there is deliberately no session-scoped takeover
    /// message: the losing client knows its own lease, and back-to-back
    /// `Detached`s sharing a reason coalesce into one banner client-side
    /// without any new vocabulary). An attach under the SAME lease
    /// replaces only the named terminal's channel — an ordinary
    /// reconnect.
    Attach {
        req_id: u64,
        session_id: String,
        channel: u32,
        cols: u16,
        rows: u16,
        /// Which of the session's terminals to attach. `#[serde(default)]`
        /// is the agent terminal, so a request that says nothing means
        /// what every pre-M4 request meant — see [`TerminalSelector`].
        #[serde(default)]
        terminal: TerminalSelector,
        /// The client identity this attachment belongs to, minted by the
        /// CLIENT (one per session view instance) — not by the helm,
        /// because every browser tab multiplexes over the helm's single
        /// supervisor connection, so connection identity cannot tell
        /// clients apart (the same fact that put `channel` on `Resize`).
        /// The supervisor groups a session's terminal channels by this
        /// value to enforce the session-scoped takeover described above.
        /// Version-6 clients must mint it high-entropy and non-empty
        /// (one random id per session-view instance): grouping is by
        /// bare equality, so a collision would fuse two clients into one
        /// lease and silently bypass the visible takeover. It is a
        /// CORRECTNESS mechanism, not an authentication boundary —
        /// anything that can speak this protocol already runs as the
        /// user (the unix socket is the auth boundary), so guessing a
        /// lease grants nothing an attacker did not have.
        /// `#[serde(default)]` (empty) preserves the pre-M4 reading: an
        /// un-leased attach is its own one-terminal client, so it both
        /// takes over everything and is taken over by anything — exactly
        /// the single-terminal semantics every older caller expects.
        #[serde(default)]
        lease: String,
        /// Refuse this attach instead of displacing another client
        /// (PLAN_M6.md item 7's auto-reconnect).
        ///
        /// The default — `false`, and therefore every pre-M6 caller — is
        /// the displacing attach the takeover rule above describes: last
        /// attach wins. `true` asks for the opposite trade, and exists
        /// because ONE caller cannot honestly make that claim: a client
        /// reconnecting after transport loss was not there to be told the
        /// session had been taken over (it had no socket to be told on),
        /// so its attach carries no user intent at all. A displacing
        /// automatic attach would silently take the session back from
        /// whoever legitimately holds it, which is the eviction loop the
        /// takeover latch exists to prevent — and worse than the latched
        /// case, because no one pressed anything.
        ///
        /// Refused with [`ErrorKind::Conflict`] and
        /// [`ATTACH_REFUSED_TAKEN_OVER`] as the message, which is what
        /// lets a client render the refusal as the ordinary takeover it
        /// is. A refusal installs nothing: the channel this attach named
        /// stays unattached and its caller must not send on it.
        ///
        /// "Owned" means the session-scoped rule already spelled out
        /// above: any attachment under a DIFFERENT lease, on any of this
        /// session's terminals. The client's OWN stale attachment — same
        /// lease, same terminal — is not ownership by anyone else and is
        /// replaced exactly as it always was, which is precisely what a
        /// reconnect after transport loss finds waiting for it.
        #[serde(default)]
        if_unowned: bool,
    },
    /// Attach accepted. Data frames on `channel` may arrive *before* this
    /// reply is processed — the supervisor starts the replay as soon as
    /// the attachment is installed — so a client must have the channel
    /// registered before it sends `Attach`, not after it sees `Attached`.
    Attached { req_id: u64, channel: u32 },
    /// Give up an attachment voluntarily. No reply, and no error if the
    /// channel was never attached or was already taken over: detach is
    /// idempotent so a client tearing down a closed terminal never has to
    /// reason about who won a race.
    Detach { channel: u32 },
    /// Unsolicited: this channel's attachment was taken over or torn down.
    ///
    /// `reason` is one of a small open-ended set of user-legible strings,
    /// not a coded enum — clients render every reason generically inside
    /// their detach banner without matching on its value.
    /// [`DETACH_REASON_STALLED`] is the one reason this crate names,
    /// because two independent emitters (supervisor and helm) and their
    /// tests must produce the identical string; the "another client took
    /// over" case has no constant because only one place emits it.
    Detached { channel: u32, reason: String },
    /// Unsolicited: this attach's initial catch-up is over — every byte
    /// the supervisor is going to replay from history FOR THE ATTACH has
    /// been written to `channel`, and the live stream follows
    /// (PLAN_M5.md item 1).
    ///
    /// The contract in one sentence: **exactly once per attach that
    /// completes its catch-up**, after the pane-mode re-synthesis and
    /// snapshot prefill for that attach and before any live output. That
    /// is a consequence of the attach-cutover contract (PLAN_M5.md item
    /// 2) rather than a best-effort claim — one task writes the replay,
    /// this marker, and then the live stream into one ordered pipe. A
    /// dead-pane attach emits it right after its own snapshot; a fresh
    /// terminal with nothing to replay emits it immediately.
    ///
    /// The qualifier is load-bearing: an attach whose catch-up is ENDED
    /// early never receives one. The attachment can be torn down wherever
    /// it happens to be, including between the `Attached` reply and the
    /// marker, and nothing is sent on a channel that no longer exists.
    /// There are two such endings and they look different to a client:
    ///
    /// - **Someone else ended it** — a takeover, a delete, a stall, the
    ///   terminal's own end. A [`ControlMsg::Detached`] for the channel
    ///   reports it, and that notice ends the catch-up phase whether or
    ///   not a marker preceded it.
    /// - **This client ended it** with [`ControlMsg::Detach`]. Nothing
    ///   comes back at all — the initiator already knows — so a client
    ///   must stop expecting a marker on a channel it detached itself.
    ///
    /// Either way, a presentation that waits for the marker ALONE hides a
    /// terminal forever on exactly these paths.
    ///
    /// There is deliberately no marker outside an attach: it describes the
    /// catch-up phase of ONE attachment, not a property of the session, so
    /// a second attach to the same terminal gets its own marker and a
    /// takeover's incumbent never sees the replacement's.
    ///
    /// One boundary is drawn deliberately narrow: "the live stream
    /// follows" does not promise history can never appear on this
    /// channel again. M2.5's flow-control recovery — the supervisor's
    /// catch-up after a tmux `%pause` cut this client's stream — replays
    /// retained history into the SAME attachment mid-stream, and that
    /// recovery arrives as ordinary output with no marker of its own.
    /// The marker bounds the attach's catch-up, not every catch-up the
    /// attachment will ever perform; pause-recovery presentation is
    /// explicitly outside M5's scope (PLAN_M5.md scopes REATTACH
    /// presentation), and a consumer that assumed
    /// all-bytes-after-marker-are-fresh would misrender exactly that
    /// recovery path.
    ///
    /// The emitting side is the supervisor's attach cutover (PLAN_M5.md
    /// item 2). The contract is stated HERE because the wire vocabulary is
    /// where every implementation and test points.
    ///
    /// ## Why a control message, not an in-band byte sentinel
    ///
    /// The alternative — splicing a sentinel byte sequence into the
    /// terminal data stream itself — was rejected (PLAN_M5.md item 1):
    /// replay bytes are arbitrary terminal history, so any sentinel can
    /// be forged by, or collide with, content the agent itself already
    /// printed, and this protocol's data path exists specifically to
    /// never interpret or rewrite terminal bytes (see the module-level
    /// docs above). A control message sidesteps both problems: it
    /// travels its own channel-0 JSON path, never mixed into the opaque
    /// bytes it describes. Its POSITION in the combined control-and-data
    /// stream is its whole meaning, and that needs no new ordering
    /// machinery anywhere — the supervisor writes replay data frames and
    /// then this message into one pipe, the helm demultiplexes that pipe
    /// sequentially, and one WebSocket's messages arrive in the order
    /// they were sent, so every hop already preserves the ordering this
    /// message depends on.
    ///
    /// Consumers must not branch anything but PRESENTATION on this
    /// message: it drives exactly one transition — a terminal's catch-up
    /// phase ending — and no session, lifecycle, or other client
    /// behavior may key off it (PLAN_M5.md's scope explicitly rules this
    /// out).
    ReplayComplete { channel: u32 },
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
    PauseOutput { channel: u32 },
    /// The client has drained its backlog below the low-water mark; output
    /// may flow again. Pairs with `PauseOutput` and shares its rationale
    /// for carrying `channel` (see `Resize`'s doc comment) and its
    /// fire-and-forget shape.
    ///
    ResumeOutput { channel: u32 },
    /// Start an attachment upload into a session's attachments directory
    /// (PLAN_M4.md items 1 and 4). The requester picks a
    /// connection-unique data channel — `Attach`'s convention, but the
    /// bytes flow the OTHER way (client toward supervisor), and the
    /// ordering contract is simpler than `Attach`'s
    /// register-before-request rule because nothing can arrive early
    /// here: choose an unused non-zero channel, send this, wait for
    /// `UploadStarted`, and only then send chunks. `filename` is a
    /// proposal: the supervisor reduces it to a shell-safe basename,
    /// generates a fallback name when nothing survives sanitizing — an
    /// empty proposal, or one that reduces to nothing or to the reserved
    /// path components `.` and `..`, is NEVER a refusal, because SPEC.md
    /// rejects only directories, not files for their names — and
    /// resolves collisions; only `UploadCommitted` says what path
    /// actually published. `size` is a declaration the commit verifies
    /// byte-for-byte — a mismatch publishes nothing. What DOES refuse a
    /// begin: an unknown session, channel 0 (the control channel), a
    /// channel already in use, and the receiver's admission bound — an
    /// implementation may cap concurrent uploads per connection, and a
    /// begin past that cap is an ordinary correlated `Error`, which
    /// bounds aggregate temp files and window memory the way
    /// [`UPLOAD_WINDOW_BYTES`] bounds a single transfer.
    BeginUpload {
        req_id: u64,
        session_id: String,
        channel: u32,
        filename: String,
        size: u64,
    },
    /// Upload accepted: data frames may flow on `channel`, in chunks of
    /// at most [`UPLOAD_CHUNK_BYTES`], subject to the credit window.
    /// This reply IS the initial credit — it grants
    /// [`UPLOAD_WINDOW_BYTES`] from a cumulative baseline of zero, so
    /// the sender never waits for a first ack that the receiver is
    /// waiting to send until data arrives (the deadlock an implicit
    /// baseline would invite). A refused begin (see `BeginUpload` for
    /// the refusal set — filenames are never in it) is `Error`, and
    /// nothing was created on disk.
    UploadStarted { req_id: u64, channel: u32 },
    /// Unsolicited receiver progress: `received` is the CUMULATIVE byte
    /// count safely written so far on `channel`. Two jobs in one message
    /// (PLAN_M4.md items 1 and 4): it extends the sender's credit window,
    /// and it is the per-hop progress evidence the stall detection
    /// watches — a window that stays open with no advancing ack is
    /// exactly what "stopped making progress" means. Both jobs put
    /// obligations on the receiver's cadence: acks must advance
    /// promptly as bytes are written (per chunk, or batched no coarser
    /// than a fraction of the window) and must not queue behind bulk
    /// frames, or a healthy transfer stalls on credit and a healthy
    /// receiver gets declared stalled by the sender's own timeout.
    /// Validity is part of the contract, not left to good faith:
    /// `received` is monotonic, never exceeds the bytes actually sent,
    /// and never exceeds `BeginUpload`'s declared `size` — a violating
    /// ack is a protocol error the sender answers by aborting the
    /// transfer, and window arithmetic is checked so no ack value can
    /// overflow it.
    UploadAck { channel: u32, received: u64 },
    /// All bytes sent; publish the file. The supervisor verifies the
    /// received count against `BeginUpload`'s declared `size`, fsyncs,
    /// and publishes atomically WITHOUT CLOBBERING (item 4's atomicity
    /// tier): the staged file is `link`ed to the first candidate name
    /// that is not already taken, so two concurrent uploads proposing one
    /// filename both publish under distinct paths and neither can replace
    /// an existing attachment. A plain rename would satisfy the atomicity
    /// requirement and violate that one. The reply is
    /// `UploadCommitted` only for a file that actually published. Every
    /// failure AT commit — size mismatch, a rename or fsync error, the
    /// session's deletion winning the race — is a correlated `Error`,
    /// and the temp is cleaned; a torn or partial file is never
    /// observable at the published path. Failures BEFORE commit are not
    /// this message's to report: they already tore the channel down as
    /// `UploadAborted`, and a commit naming a channel that no longer
    /// carries an upload is itself an `Error`.
    CommitUpload { req_id: u64, channel: u32 },
    /// The upload published. `path` is the RAW absolute host-side path —
    /// UTF-8 by the module-level path contract (a non-UTF-8 publish path
    /// fails the commit, never lossy-converts), but otherwise
    /// unescaped: the FILENAME component is shell-safe by item 4's
    /// sanitizing, while the parent directory is whatever the state dir
    /// is, spaces and all. How it enters the terminal (the text-paste
    /// code path, and any insertion-time escaping) is the client's
    /// contract, decided where insertion lives (PLAN_M4.md item 7) —
    /// this field just promises the true path, exactly as the
    /// filesystem knows it.
    UploadCommitted { req_id: u64, path: String },
    /// Abandon an upload. Fire-and-forget and idempotent like `Detach`,
    /// and for the same reason: a client tearing down (a cancelled drop,
    /// a closed view) must never have to reason about who won a race
    /// against a concurrent abort or completion. The receiver drops the
    /// channel and cleans the temp file.
    AbortUpload { channel: u32 },
    /// Unsolicited: the receiver gave up on this upload. This is the
    /// outcome for EVERY post-start receiver-side failure — the stall
    /// timeout ([`UPLOAD_ABORT_REASON_STALLED`]), the session deleted
    /// mid-transfer, a storage or write error while streaming, an
    /// oversized or otherwise invalid chunk — because once
    /// `UploadStarted` answered the begin, no pending `req_id` exists
    /// for an `Error` to correlate with, and `channel` is the only
    /// identity that can name one of several concurrent transfers.
    /// `Detached`'s shape and philosophy, including deliberately NO
    /// `ErrorKind` field: `reason` is a small open-ended set of
    /// user-legible strings the client renders verbatim in its
    /// upload-failure surface, never a coded enum it branches on — every
    /// abort means the same thing to a client (show the reason, insert
    /// nothing), so a classification would be dead weight on the wire.
    ///
    /// NOTHING PUBLISHED is unconditional: no client ever has to wonder
    /// whether an aborted transfer left a file at some path it was not
    /// told. The receiver's own staging file is a weaker promise, stated
    /// honestly: its removal is ATTEMPTED (and retried) before this
    /// message is sent, but a removal can fail for reasons no
    /// implementation can overrule — a read-only mount, a filesystem
    /// error — and what survives such a failure is a file in the
    /// receiver's private staging area, which its next startup
    /// reconciles. Neither case is visible to the client, and neither
    /// affects the published path.
    UploadAborted { channel: u32, reason: String },
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
    /// The identity-less hello: `host_identity: None` regardless of
    /// `role`. Correct as-is for the helm (which has no identity of its
    /// own — see the field's own docs) and for every test double playing
    /// a supervisor without needing to model minting. The REAL supervisor
    /// connection loop does not call this constructor at all; it builds
    /// its own `Hello` carrying `Supervisor::host_identity` (`io::
    /// handshake_with_host_identity`'s job) precisely because this one
    /// shared constructor cannot know that value.
    pub fn hello(role: &str) -> ControlMsg {
        ControlMsg::Hello {
            protocol_version: PROTOCOL_VERSION,
            build_version: BUILD_VERSION.to_string(),
            role: role.to_string(),
            host_identity: None,
            auth: None,
        }
    }

    /// The request this message is a REPLY to, or `None` if it is not a
    /// reply at all.
    ///
    /// What a demultiplexer needs to know before it hands a message to
    /// whoever is waiting on a `req_id`. Living here rather than in each
    /// client is what keeps that decision in step with the enum: a reply
    /// variant added beside its request, in this file, is one edit away
    /// from being routed, instead of silently hanging every caller whose
    /// or-pattern was never extended.
    ///
    /// **The match below is exhaustive on purpose — no wildcard arm.** That
    /// is the enforcement, and it is why this method is worth having at
    /// all. A `_ => None` would classify a newly added reply variant as
    /// "not a reply", which is precisely the silent hang this exists to
    /// prevent, and no test could catch it without re-deriving the variant
    /// list somewhere else. Listing every variant makes adding one a
    /// compile error HERE, at the moment someone must decide what it is.
    ///
    /// **`None` for REQUESTS, even though most of them carry a `req_id`.**
    /// That is the whole point of the distinction and not an oversight:
    /// `req_id` means "this correlates with request N" in both directions,
    /// so a peer that echoed a `CreateSession` back — through malice, a
    /// crossed connection, or a loopback bug — would otherwise be handed to
    /// the caller as if it were that request's ANSWER. Direction is not on
    /// the wire; it is only ever inferable from the message type, which is
    /// exactly what this method encodes.
    ///
    /// `Error` counts as a reply because it is the failure form of every
    /// request — but it is also the ONE message that can arrive
    /// uncorrelated, and it says so with `req_id` 0 (see that variant's
    /// docs). That convention is `Error`'s alone: the other unsolicited
    /// messages carry no `req_id` field at all and correlate by `channel`.
    /// It is deliberately not special-cased here, because "is this a reply"
    /// and "is this reply anybody's" are separate questions and only the
    /// caller holding the pending-request table can answer the second.
    pub fn reply_req_id(&self) -> Option<u64> {
        match self {
            ControlMsg::SessionCreated { req_id, .. }
            | ControlMsg::SessionList { req_id, .. }
            | ControlMsg::SessionStopped { req_id, .. }
            | ControlMsg::SessionDeleted { req_id, .. }
            | ControlMsg::SessionArchived { req_id, .. }
            | ControlMsg::SessionRestarted { req_id, .. }
            | ControlMsg::SessionRenamed { req_id, .. }
            | ControlMsg::ProfileList { req_id, .. }
            | ControlMsg::ProfileCreated { req_id, .. }
            | ControlMsg::ProfileUpdated { req_id, .. }
            | ControlMsg::ProfileDeleted { req_id, .. }
            | ControlMsg::Attached { req_id, .. }
            | ControlMsg::TabOpened { req_id, .. }
            | ControlMsg::TabClosed { req_id, .. }
            | ControlMsg::UploadStarted { req_id, .. }
            | ControlMsg::UploadCommitted { req_id, .. }
            | ControlMsg::Error { req_id, .. } => Some(*req_id),
            // Requests are listed one by one rather than swept up by a
            // wildcard: each is a message a hostile or confused peer could
            // echo back, and this `None` is what stops that echo from being
            // delivered as an answer.
            ControlMsg::CreateSession { .. }
            | ControlMsg::ListSessions { .. }
            | ControlMsg::StopSession { .. }
            | ControlMsg::DeleteSession { .. }
            | ControlMsg::ArchiveSession { .. }
            | ControlMsg::RestartSession { .. }
            | ControlMsg::RenameSession { .. }
            | ControlMsg::ListProfiles { .. }
            | ControlMsg::CreateProfile { .. }
            | ControlMsg::UpdateProfile { .. }
            | ControlMsg::DeleteProfile { .. }
            | ControlMsg::OpenTab { .. }
            | ControlMsg::CloseTab { .. }
            | ControlMsg::Attach { .. }
            | ControlMsg::BeginUpload { .. }
            | ControlMsg::CommitUpload { .. } => None,
            // The handshake, and the channel-correlated events: nothing
            // here has a `req_id` field to return in the first place.
            ControlMsg::Hello { .. }
            | ControlMsg::Detach { .. }
            | ControlMsg::Detached { .. }
            | ControlMsg::ReplayComplete { .. }
            | ControlMsg::Resize { .. }
            | ControlMsg::PauseOutput { .. }
            | ControlMsg::ResumeOutput { .. }
            | ControlMsg::UploadAck { .. }
            | ControlMsg::AbortUpload { .. }
            | ControlMsg::UploadAborted { .. } => None,
        }
    }

    /// The request id a server must correlate a refusal with.
    ///
    /// Restricted peers are rejected before ordinary dispatch. Keeping the
    /// request classification here lets that authorization gate answer the
    /// caller instead of sending an uncorrelated error that would leave its
    /// request waiting forever.
    pub fn request_req_id(&self) -> Option<u64> {
        match self {
            ControlMsg::CreateSession { req_id, .. }
            | ControlMsg::ListSessions { req_id, .. }
            | ControlMsg::StopSession { req_id, .. }
            | ControlMsg::DeleteSession { req_id, .. }
            | ControlMsg::ArchiveSession { req_id, .. }
            | ControlMsg::RestartSession { req_id, .. }
            | ControlMsg::RenameSession { req_id, .. }
            | ControlMsg::ListProfiles { req_id, .. }
            | ControlMsg::CreateProfile { req_id, .. }
            | ControlMsg::UpdateProfile { req_id, .. }
            | ControlMsg::DeleteProfile { req_id, .. }
            | ControlMsg::OpenTab { req_id, .. }
            | ControlMsg::CloseTab { req_id, .. }
            | ControlMsg::Attach { req_id, .. }
            | ControlMsg::BeginUpload { req_id, .. }
            | ControlMsg::CommitUpload { req_id, .. } => Some(*req_id),
            ControlMsg::Hello { .. }
            | ControlMsg::SessionCreated { .. }
            | ControlMsg::SessionList { .. }
            | ControlMsg::SessionStopped { .. }
            | ControlMsg::SessionDeleted { .. }
            | ControlMsg::SessionArchived { .. }
            | ControlMsg::SessionRestarted { .. }
            | ControlMsg::SessionRenamed { .. }
            | ControlMsg::ProfileList { .. }
            | ControlMsg::ProfileCreated { .. }
            | ControlMsg::ProfileUpdated { .. }
            | ControlMsg::ProfileDeleted { .. }
            | ControlMsg::Attached { .. }
            | ControlMsg::TabOpened { .. }
            | ControlMsg::TabClosed { .. }
            | ControlMsg::UploadStarted { .. }
            | ControlMsg::UploadCommitted { .. }
            | ControlMsg::Error { .. }
            | ControlMsg::Detach { .. }
            | ControlMsg::Detached { .. }
            | ControlMsg::ReplayComplete { .. }
            | ControlMsg::Resize { .. }
            | ControlMsg::PauseOutput { .. }
            | ControlMsg::ResumeOutput { .. }
            | ControlMsg::UploadAck { .. }
            | ControlMsg::AbortUpload { .. }
            | ControlMsg::UploadAborted { .. } => None,
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
    /// compile), so pin one message's exact JSON. `terminal`/`lease`
    /// (PLAN_M4.md item 1) are `#[serde(default)]` but — like every other
    /// defaulted field in this file (see
    /// `session_info_annotation_and_restart_offer_json_shapes_are_pinned`)
    /// — still serialize on every encode, so this default-valued case
    /// (agent terminal, empty lease) belongs here rather than only in
    /// `attach_terminal_selector_and_lease_json_shapes_are_pinned` below,
    /// which covers the non-default values.
    #[test]
    fn control_json_shape_is_pinned() {
        let msg = ControlMsg::Attach {
            req_id: 3,
            session_id: "s1".into(),
            channel: 9,
            cols: 80,
            rows: 24,
            terminal: TerminalSelector::default(),
            lease: String::new(),
            if_unowned: false,
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
                "terminal": { "kind": "agent" },
                "lease": "",
                "if_unowned": false,
            })
        );
    }

    /// PLAN_M4.md item 1's `terminal`/`lease` additions to `Attach`, golden-
    /// pinned with non-default values for both `TerminalSelector` shapes —
    /// the sibling test above only exercises the default (agent, empty
    /// lease) case. A (message, expected JSON) table rather than two
    /// separately-named locals with their own assertion blocks, since the
    /// two cases differ only in the `terminal` value. Checked in both
    /// directions (`to_value` then `from_value` on the real type) since
    /// these are the two new fields this PR actually adds meaning to,
    /// unlike the rest of `Attach`.
    #[test]
    fn attach_terminal_selector_and_lease_json_shapes_are_pinned() {
        for (msg, expected) in [
            (
                ControlMsg::Attach {
                    req_id: 10,
                    session_id: "s1".into(),
                    channel: 1,
                    cols: 80,
                    rows: 24,
                    terminal: TerminalSelector::Agent,
                    lease: "client-abc".into(),
                    if_unowned: false,
                },
                serde_json::json!({
                    "type": "attach",
                    "req_id": 10,
                    "session_id": "s1",
                    "channel": 1,
                    "cols": 80,
                    "rows": 24,
                    "terminal": { "kind": "agent" },
                    "lease": "client-abc",
                    "if_unowned": false,
                }),
            ),
            (
                ControlMsg::Attach {
                    req_id: 11,
                    session_id: "s1".into(),
                    channel: 2,
                    cols: 80,
                    rows: 24,
                    terminal: TerminalSelector::Tab { id: "t1".into() },
                    lease: "client-abc".into(),
                    if_unowned: true,
                },
                serde_json::json!({
                    "type": "attach",
                    "req_id": 11,
                    "session_id": "s1",
                    "channel": 2,
                    "cols": 80,
                    "rows": 24,
                    "terminal": { "kind": "tab", "id": "t1" },
                    "lease": "client-abc",
                    "if_unowned": true,
                }),
            ),
        ] {
            let json = serde_json::to_value(&msg).unwrap();
            assert_eq!(json, expected);
            assert_eq!(serde_json::from_value::<ControlMsg>(json).unwrap(), msg);
        }
    }

    /// The reverse direction of the two tests above: JSON shaped exactly
    /// as every pre-M4 (version 5) `Attach` request always was — no
    /// `terminal`, no `lease` key at all — must still decode, defaulting
    /// to `TerminalSelector::Agent` and an empty lease. This is what keeps
    /// `Attach`'s pre-M4 meaning ("attach my one implicit terminal, own
    /// everything") alive for any caller that predates the selector,
    /// mirroring `restart_session_stop_if_running_defaults_false_when_absent`'s
    /// treatment of an older field whose absence must resolve to a
    /// specific, safe default rather than an arbitrary one.
    #[test]
    fn bare_legacy_attach_json_decodes_to_agent_terminal_and_empty_lease() {
        let old_shape = serde_json::json!({
            "type": "attach",
            "req_id": 5,
            "session_id": "s1",
            "channel": 3,
            "cols": 80,
            "rows": 24,
        });
        let decoded: ControlMsg = serde_json::from_value(old_shape).unwrap();
        let ControlMsg::Attach {
            terminal, lease, ..
        } = decoded
        else {
            panic!("expected ControlMsg::Attach, got {decoded:?}");
        };
        assert_eq!(terminal, TerminalSelector::Agent);
        assert_eq!(lease, "");
    }

    /// The REVERSE tolerance direction from the two tests above
    /// (mirroring `new_session_list_json_decodes_under_a_legacy_pre_status_decoder`):
    /// a hand-rolled decoder shaped like a genuine pre-M4 (version 5)
    /// peer — no `terminal`, no `lease` field at all — must still decode
    /// a CURRENT sender's `Attach` JSON, silently dropping the fields it
    /// predates. `terminal` is deliberately set to the NON-default `Tab`
    /// selector with a non-empty `lease` here, unlike
    /// `control_json_shape_is_pinned`'s all-defaults case above, so this
    /// pins that a legacy peer tolerates losing real information, not
    /// just a default it would have reconstructed anyway.
    ///
    /// What this DOES NOT license, and what a reader coming to it for
    /// reassurance most needs to hear: decoding tolerance is why the
    /// version bumps are REQUIRED, not why they are unnecessary. Silently
    /// dropping a field is exactly the failure mode that makes a mixed
    /// fleet dangerous — most sharply for `if_unowned` (set here, and
    /// dropped by the legacy shape below), whose whole meaning is "do
    /// something DIFFERENT from what you would otherwise do". A peer that
    /// drops it displaces a client it was asked to leave alone, and
    /// nothing on either side notices. The hello refusal is what makes
    /// that unreachable; this test documents the shape of the hazard it
    /// closes, not a tolerance anyone may rely on.
    #[derive(Debug, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum LegacyV5ControlMsg {
        Attach {
            req_id: u64,
            session_id: String,
            channel: u32,
            cols: u16,
            rows: u16,
        },
    }

    #[test]
    fn new_attach_json_decodes_under_a_legacy_pre_terminal_selector_decoder() {
        let new_msg = ControlMsg::Attach {
            req_id: 12,
            session_id: "s1".to_string(),
            channel: 4,
            cols: 80,
            rows: 24,
            terminal: TerminalSelector::Tab {
                id: "t1".to_string(),
            },
            lease: "client-xyz".to_string(),
            if_unowned: true,
        };
        let json = serde_json::to_value(&new_msg).unwrap();

        let LegacyV5ControlMsg::Attach {
            req_id,
            session_id,
            channel,
            cols,
            rows,
        } = serde_json::from_value(json.clone()).expect(
            "a legacy (pre-M4, version 5) decoder without terminal/lease must still decode \
             new-shape JSON",
        );
        assert_eq!(req_id, 12);
        assert_eq!(session_id, "s1");
        assert_eq!(channel, 4);
        assert_eq!((cols, rows), (80, 24));

        // The REAL type round-trips the same JSON too, same as every
        // sibling test of this shape does.
        let real_decoded: ControlMsg = serde_json::from_value(json).unwrap();
        assert_eq!(real_decoded, new_msg);
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
        let unauthorized = ControlMsg::Error {
            req_id: 8,
            message: "session credential rejected".to_string(),
            kind: ErrorKind::Unauthorized,
        };
        let expected = serde_json::json!({
            "type": "error",
            "req_id": 8,
            "message": "session credential rejected",
            "kind": "unauthorized",
        });
        assert_eq!(serde_json::to_value(&unauthorized).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<ControlMsg>(expected).unwrap(),
            unauthorized
        );
    }

    /// `Hello::host_identity` (PLAN_M6.md item 1), golden-pinned in both
    /// the explicit-`null` and the present-value shape — both are real
    /// production shapes now, not just what this crate's `Serialize` impl
    /// happens to be capable of producing. `None` is what `ControlMsg::
    /// hello` sends (the helm, and every identity-less test double — see
    /// that function's own docs) and what a claimless supervisor
    /// construction sends too (`Supervisor::host_identity`'s own docs);
    /// `Some` is what `io::handshake_with_host_identity` sends for a
    /// supervisor that minted or read back a real one. Each pair
    /// round-trips (encode matches the golden value, decoding that same
    /// value returns the original message) — but round-tripping this
    /// crate's own `null` says nothing about a KEY that never appears on
    /// the wire at all, which is what a real absent-field sender would
    /// produce; the sibling test below covers that case with hand-written
    /// JSON instead.
    #[test]
    fn hello_json_shape_is_pinned_with_and_without_host_identity() {
        for (msg, expected) in [
            (
                ControlMsg::Hello {
                    protocol_version: 8,
                    build_version: "1.2.3".to_string(),
                    role: "supervisor".to_string(),
                    host_identity: None,
                    auth: None,
                },
                serde_json::json!({
                    "type": "hello",
                    "protocol_version": 8,
                    "build_version": "1.2.3",
                    "role": "supervisor",
                    "host_identity": null,
                    "auth": null,
                }),
            ),
            (
                ControlMsg::Hello {
                    protocol_version: 8,
                    build_version: "1.2.3".to_string(),
                    role: "supervisor".to_string(),
                    host_identity: Some("11111111-1111-1111-1111-111111111111".to_string()),
                    auth: None,
                },
                serde_json::json!({
                    "type": "hello",
                    "protocol_version": 8,
                    "build_version": "1.2.3",
                    "role": "supervisor",
                    "host_identity": "11111111-1111-1111-1111-111111111111",
                    "auth": null,
                }),
            ),
            (
                ControlMsg::Hello {
                    protocol_version: 11,
                    build_version: "0.0.3".to_string(),
                    role: "spawn".to_string(),
                    host_identity: None,
                    auth: Some(SessionAuth {
                        session_id: "parent-1".to_string(),
                        token: "secret-token".to_string(),
                    }),
                },
                serde_json::json!({
                    "type": "hello",
                    "protocol_version": 11,
                    "build_version": "0.0.3",
                    "role": "spawn",
                    "host_identity": null,
                    "auth": {
                        "session_id": "parent-1",
                        "token": "secret-token",
                    },
                }),
            ),
        ] {
            assert_eq!(serde_json::to_value(&msg).unwrap(), expected);
            let decoded: ControlMsg = serde_json::from_value(expected).unwrap();
            assert_eq!(decoded, msg);
        }
    }

    /// Debugging a hello must retain the authenticated session identity
    /// without copying its bearer secret into logs or panic output.
    #[test]
    fn session_auth_debug_redacts_the_bearer_token() {
        let token = "token-that-must-never-appear";
        let hello = ControlMsg::Hello {
            protocol_version: PROTOCOL_VERSION,
            build_version: BUILD_VERSION.to_string(),
            role: "spawn".to_string(),
            host_identity: None,
            auth: Some(SessionAuth {
                session_id: "session-1".to_string(),
                token: token.to_string(),
            }),
        };
        let rendered = format!("{hello:?}");
        assert!(
            rendered.contains("session-1") && rendered.contains("<redacted>"),
            "the diagnostic must retain attribution and mark the omitted secret: {rendered}"
        );
        assert!(
            !rendered
                .as_bytes()
                .windows(token.len())
                .any(|bytes| bytes == token.as_bytes()),
            "the bearer token bytes must not appear in Debug output: {rendered}"
        );
    }

    /// The case the golden test above cannot reach: a hello whose JSON
    /// never mentions `host_identity` at all, the shape an OLD-8 or
    /// genuinely pre-8 sender's `Hello` would have (impossible in
    /// practice for a real peer — the handshake refuses a mismatched
    /// version before any other field is inspected — but decode
    /// tolerance for it is still part of the contract `#[serde(default)]`-
    /// less `Option` fields make elsewhere in this file, see that field's
    /// own docs). Hand-written JSON, not `serde_json::json!` built from a
    /// `ControlMsg`, is what makes the key's absence real instead of an
    /// explicit `null` that merely happens to render the same in this
    /// one crate's own encoder.
    #[test]
    fn hello_json_decodes_with_host_identity_key_entirely_absent() {
        let raw = r#"{
            "type": "hello",
            "protocol_version": 8,
            "build_version": "1.2.3",
            "role": "supervisor"
        }"#;
        let decoded: ControlMsg = serde_json::from_str(raw)
            .expect("a hello JSON object with no host_identity key at all must still decode");
        let ControlMsg::Hello {
            host_identity,
            auth,
            ..
        } = decoded
        else {
            panic!("expected ControlMsg::Hello, got {decoded:?}");
        };
        assert_eq!(
            host_identity, None,
            "an absent key must decode the same as an explicit null"
        );
        assert_eq!(auth, None, "a v10 hello has no session attribution");
    }

    /// The REVERSE direction from the two tests above, following the
    /// shadow-decoder pattern near
    /// `new_session_list_json_decodes_under_a_legacy_pre_status_decoder`: a
    /// hand-rolled decoder shaped like the genuine v7 `Hello` — before
    /// `host_identity` existed at all (see the version history linked from
    /// `PROTOCOL_VERSION`'s own docs for when it was added) — must still
    /// decode a NEW v8 sender's `Hello` JSON, silently ignoring the field it
    /// does not recognize.
    /// As with the `SessionList` sibling, a real peer never exercises this
    /// path (the handshake refuses a `protocol_version` mismatch before
    /// any other field is inspected), but the decode tolerance is still
    /// part of the additive contract this file pins independently of
    /// whether production code currently walks it.
    #[derive(Debug, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum LegacyHelloControlMsg {
        Hello {
            protocol_version: u32,
            build_version: String,
            role: String,
        },
    }

    #[test]
    fn new_hello_json_decodes_under_a_legacy_pre_host_identity_decoder() {
        let new_msg = ControlMsg::Hello {
            protocol_version: 8,
            build_version: "1.2.3".to_string(),
            role: "supervisor".to_string(),
            host_identity: Some("11111111-1111-1111-1111-111111111111".to_string()),
            auth: None,
        };
        let json = serde_json::to_value(&new_msg).unwrap();

        let LegacyHelloControlMsg::Hello {
            protocol_version,
            build_version,
            role,
        } = serde_json::from_value(json.clone())
            .expect("a legacy decoder without host_identity must still decode new-shape JSON");
        assert_eq!(protocol_version, 8);
        assert_eq!(build_version, "1.2.3");
        assert_eq!(role, "supervisor");

        // The REAL type round-trips the same JSON too — same discipline as
        // the SessionList sibling test, making explicit that `json` is
        // exactly what a real `ControlMsg::Hello` produces.
        let real_decoded: ControlMsg = serde_json::from_value(json).unwrap();
        assert_eq!(real_decoded, new_msg);
    }

    /// `ListSessions::cursor`/`limit` (PLAN_M6.md item 1's pagination
    /// request shape), golden-pinned with both absent (a first page at the
    /// server's default size) and both present (a resumed, size-capped
    /// page). Both fields are plain `Option`s with no `#[serde(default)]`,
    /// matching every other optional request field this file pins the
    /// same way (`CreateSession::title`, `RestartSession`'s snapshot
    /// overrides) — see `SessionInfo::annotation`'s docs for why that
    /// attribute is not needed for `Option` to decode absence as `None`.
    #[test]
    fn list_sessions_cursor_and_limit_json_shapes_are_pinned() {
        for (msg, expected) in [
            (
                ControlMsg::ListSessions {
                    req_id: 1,
                    cursor: None,
                    limit: None,
                },
                serde_json::json!({
                    "type": "list_sessions",
                    "req_id": 1,
                    "cursor": null,
                    "limit": null,
                }),
            ),
            (
                ControlMsg::ListSessions {
                    req_id: 2,
                    cursor: Some("opaque-cursor-value".to_string()),
                    limit: Some(50),
                },
                serde_json::json!({
                    "type": "list_sessions",
                    "req_id": 2,
                    "cursor": "opaque-cursor-value",
                    "limit": 50,
                }),
            ),
        ] {
            assert_eq!(serde_json::to_value(&msg).unwrap(), expected);
            let decoded: ControlMsg = serde_json::from_value(expected).unwrap();
            assert_eq!(decoded, msg);
        }
    }

    /// The golden test above's "absent" case is really an explicit
    /// `"cursor": null, "limit": null` — this crate's own encoder never
    /// omits an `Option` key, so it cannot exercise a real absent-key
    /// decode (a panel review flagged the gap). Hand-written JSON with
    /// both keys left out entirely, matching `hello_json_decodes_with_host_identity_key_entirely_absent`'s
    /// approach for the same reason, is what makes the absence real
    /// instead of a `null` that happens to render the same in this one
    /// crate's own serializer — the shape a genuine v7 sender's
    /// `ListSessions` (before `cursor`/`limit` existed) would produce.
    #[test]
    fn list_sessions_json_decodes_with_cursor_and_limit_keys_entirely_absent() {
        let raw = r#"{"type":"list_sessions","req_id":3}"#;
        let decoded: ControlMsg = serde_json::from_str(raw)
            .expect("a list_sessions JSON object with no cursor/limit keys must still decode");
        let ControlMsg::ListSessions {
            req_id,
            cursor,
            limit,
        } = decoded
        else {
            panic!("expected ControlMsg::ListSessions, got {decoded:?}");
        };
        assert_eq!(req_id, 3);
        assert_eq!(
            cursor, None,
            "an absent cursor key must decode the same as an explicit null"
        );
        assert_eq!(
            limit, None,
            "an absent limit key must decode the same as an explicit null"
        );
    }

    /// The REVERSE direction: a hand-rolled decoder shaped like the
    /// genuine v7 `ListSessions` — before `cursor`/`limit` existed (see the
    /// version history linked from `PROTOCOL_VERSION`'s own docs) — must
    /// still decode a NEW v8 sender's `ListSessions` JSON, silently ignoring
    /// both fields it does not recognize. Same shadow-decoder pattern as
    /// `new_session_list_json_decodes_under_a_legacy_pre_status_decoder`
    /// and `new_hello_json_decodes_under_a_legacy_pre_host_identity_decoder`;
    /// as with those, no real peer exercises this path (the handshake
    /// refuses a version mismatch first), but the tolerance is still part
    /// of the additive contract this file pins directly.
    #[derive(Debug, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum LegacyListSessionsControlMsg {
        ListSessions { req_id: u64 },
    }

    #[test]
    fn new_list_sessions_json_decodes_under_a_legacy_pre_cursor_decoder() {
        let new_msg = ControlMsg::ListSessions {
            req_id: 4,
            cursor: Some("opaque-cursor-value".to_string()),
            limit: Some(50),
        };
        let json = serde_json::to_value(&new_msg).unwrap();

        let LegacyListSessionsControlMsg::ListSessions { req_id } =
            serde_json::from_value(json.clone())
                .expect("a legacy decoder without cursor/limit must still decode new-shape JSON");
        assert_eq!(req_id, 4);

        // The REAL type round-trips the same JSON too — same discipline as
        // the SessionList and Hello siblings, making explicit that `json`
        // is exactly what a real `ControlMsg::ListSessions` produces.
        let real_decoded: ControlMsg = serde_json::from_value(json).unwrap();
        assert_eq!(real_decoded, new_msg);
    }

    /// `SessionInfo::created_at` (PLAN_M6.md item 1), golden-pinned as its
    /// own test rather than only riding along inside the larger
    /// `SessionInfo` golden tests elsewhere in this file: this is the one
    /// field whose exact wire NAME and TYPE (a bare integer, seconds since
    /// the Unix epoch — not a nested object, not an RFC 3339 string) the
    /// pagination cursor's ordering key depends on, per PLAN_M6.md's
    /// "Pagination shape" paragraph, so a rename or a type change here
    /// deserves a failure that points straight at this field rather than
    /// getting lost in a larger struct's diff.
    #[test]
    fn session_info_created_at_json_shape_is_pinned() {
        let info = SessionInfo {
            parent: None,
            archived: false,
            id: "s1".to_string(),
            title: "demo".to_string(),
            created_at: 1_700_000_000,
            last_activity_at: 1_700_000_000,
            creation_seq: None,
            cwd: "/tmp".to_string(),
            invocation: "agent".to_string(),
            status: SessionStatus::default(),
            annotation: None,
            restart_offer: RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        };
        assert_eq!(
            serde_json::to_value(&info).unwrap()["created_at"],
            serde_json::json!(1_700_000_000)
        );
    }

    /// PLAN_M7.md item 2's parent and archive flag, including both default
    /// and populated wire shapes. Archive remains metadata beside status;
    /// it is never serialized as a status variant.
    #[test]
    fn session_info_parent_and_archived_json_shapes_are_pinned() {
        for (parent, archived, expected_parent) in [
            (None, false, serde_json::Value::Null),
            (
                Some("parent-1".to_string()),
                true,
                serde_json::json!("parent-1"),
            ),
        ] {
            let info = SessionInfo {
                id: "child-1".to_string(),
                parent,
                title: "child".to_string(),
                created_at: 1_700_000_000,
                last_activity_at: 1_700_000_000,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Unknown,
                annotation: None,
                restart_offer: RestartOffer::FreshOnly,
                tabs: Vec::new(),
                archived,
                source_profile: None,
            };
            let json = serde_json::to_value(&info).unwrap();
            assert_eq!(json["parent"], expected_parent);
            assert_eq!(json["archived"], serde_json::json!(archived));
            assert_eq!(serde_json::from_value::<SessionInfo>(json).unwrap(), info);
        }
    }

    /// `PROTOCOL_VERSION` is a load-bearing constant (see the version
    /// history linked from the const's own docs for the M2 bump to 3, the
    /// M2.5 bump to 4, the M3 bump to 5, the M4 bump to 6, the M5 bump to 7,
    /// the M6 bump to 8, the non-displacing attach's bump to 9, the M6.75
    /// bump to 10, and M7's vocabulary bump to 11): pinning its value
    /// here makes an accidental re-bump (or a forgotten one, if a later change
    /// needed it) a loud test failure rather than a silent drift discovered only
    /// by two builds refusing to talk to each other.
    ///
    /// The version-skew tests in the helm and the farhelm e2e suite are
    /// deliberately written against `PROTOCOL_VERSION ± 1` rather than
    /// against a literal, so they FOLLOW this constant instead of needing
    /// an edit per bump; this test is the one place the number itself is
    /// asserted.
    #[test]
    fn protocol_version_is_pinned_at_11() {
        assert_eq!(PROTOCOL_VERSION, 11);
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

    /// Archive's request and fresh-session reply are golden-pinned here.
    /// The reply carries the archived row so an idempotent retry returns
    /// the same useful answer as the first request.
    #[test]
    fn archive_session_json_shapes_are_pinned() {
        let session = SessionInfo {
            parent: None,
            archived: true,
            id: "s1".to_string(),
            title: "demo".to_string(),
            created_at: 1_700_000_000,
            last_activity_at: 1_700_000_000,
            creation_seq: Some(7),
            cwd: "/tmp".to_string(),
            invocation: "agent".to_string(),
            status: SessionStatus::Exited { exit_code: None },
            annotation: Some(STOP_ANNOTATION.to_string()),
            restart_offer: RestartOffer::FreshOnly,
            tabs: Vec::new(),
            source_profile: None,
        };
        for (msg, expected) in [
            (
                ControlMsg::ArchiveSession {
                    req_id: 13,
                    session_id: "s1".to_string(),
                },
                serde_json::json!({
                    "type": "archive_session",
                    "req_id": 13,
                    "session_id": "s1",
                }),
            ),
            (
                ControlMsg::SessionArchived {
                    req_id: 13,
                    session: session.clone(),
                },
                serde_json::json!({
                    "type": "session_archived",
                    "req_id": 13,
                    "session": {
                        "id": "s1",
                        "parent": null,
                        "title": "demo",
                        "created_at": 1_700_000_000,
                        "last_activity_at": 1_700_000_000,
                        "creation_seq": 7,
                        "cwd": "/tmp",
                        "invocation": "agent",
                        "status": { "state": "exited", "exit_code": null },
                        "annotation": STOP_ANNOTATION,
                        "restart_offer": "fresh_only",
                        "tabs": [],
                        "archived": true,
                        "source_profile": null,
                    },
                }),
            ),
        ] {
            assert_eq!(serde_json::to_value(&msg).unwrap(), expected);
            assert_eq!(serde_json::from_value::<ControlMsg>(expected).unwrap(), msg);
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

    /// `SessionStatus`'s now SEVEN variants and EIGHT distinct JSON shapes
    /// (`Exited` alone has two: `exit_code` present vs. `null`) are
    /// PLAN_M2.md's, PLAN_M3.md's and PLAN_M6_75.md's "Proto growth" wire
    /// changes, pinned exactly like `ErrorKind`'s variants above: an
    /// `#[serde(tag = ...)]` or variant-naming change here would compile and
    /// round-trip cleanly while quietly producing bytes an unmodified peer
    /// cannot parse. All shapes matter individually because `Exited` and
    /// `Error` both have internally-tagged fields that flatten into the same
    /// object as the `state` tag — a detail `serde_json::to_value` equality
    /// alone makes visible, unlike a bare round-trip. `Error` and
    /// `Interrupted` are the PLAN_M3.md item 3/2 additions that forced
    /// `PROTOCOL_VERSION` to 5; `Running`/`Waiting`/`Idle` are version 10's
    /// live split, and `alive` is gone from this list entirely, which is the
    /// half of that change no round-trip test can show.
    ///
    /// Structured as ONE match per value whose arms ARE the golden
    /// assertions, the shape
    /// `agent_kind_restart_and_terminal_selector_vocabulary_json_shapes_are_pinned`
    /// settled on and for its reason: a separate fixture list plus a
    /// separate exhaustive match lets a new variant be "covered" by an empty
    /// arm with no golden value attached anywhere. Here the arm IS the
    /// pinning, so the compiler's demand for an arm is a demand for a wire
    /// shape.
    #[test]
    fn session_status_json_shapes_are_pinned() {
        for status in [
            SessionStatus::Unknown,
            SessionStatus::Running,
            SessionStatus::Waiting,
            SessionStatus::Idle,
            SessionStatus::Exited { exit_code: Some(3) },
            SessionStatus::Exited { exit_code: None },
            SessionStatus::Error {
                detail: "exec: no such file or directory".to_string(),
            },
            SessionStatus::Interrupted,
        ] {
            let expected = match &status {
                SessionStatus::Unknown => serde_json::json!({ "state": "unknown" }),
                SessionStatus::Running => serde_json::json!({ "state": "running" }),
                SessionStatus::Waiting => serde_json::json!({ "state": "waiting" }),
                SessionStatus::Idle => serde_json::json!({ "state": "idle" }),
                SessionStatus::Exited { exit_code } => {
                    serde_json::json!({ "state": "exited", "exit_code": exit_code })
                }
                SessionStatus::Error { detail } => {
                    serde_json::json!({ "state": "error", "detail": detail })
                }
                SessionStatus::Interrupted => serde_json::json!({ "state": "interrupted" }),
            };
            assert_eq!(serde_json::to_value(&status).unwrap(), expected);
            // Both directions against the golden value itself: encoding it
            // and decoding it are separate claims, and only the pair rules
            // out a coordinated drift that still agrees with itself.
            assert_eq!(
                serde_json::from_value::<SessionStatus>(expected).unwrap(),
                status
            );
        }
    }

    /// [`SessionStatus::is_live`] answers for every variant, once.
    ///
    /// A table rather than scattered asserts, mirroring farhelm-ui's
    /// `status_truth_table`: the compiler already forces the predicate's
    /// match to grow when a status is added, and this is what forces
    /// someone to say out loud what the new status MEANS rather than
    /// picking whichever arm compiles. The three live statuses answering
    /// alike is the whole point — a consumer must never be able to tell
    /// them apart through this predicate.
    #[test]
    fn is_live_answers_for_every_status() {
        for (status, live) in [
            (SessionStatus::Running, true),
            (SessionStatus::Waiting, true),
            (SessionStatus::Idle, true),
            (SessionStatus::Unknown, false),
            (SessionStatus::Exited { exit_code: Some(0) }, false),
            (SessionStatus::Exited { exit_code: None }, false),
            (
                SessionStatus::Error {
                    detail: "exec failed".to_string(),
                },
                false,
            ),
            (SessionStatus::Interrupted, false),
        ] {
            assert_eq!(
                status.is_live(),
                live,
                "{status:?} must{} be live",
                if live { "" } else { " not" }
            );
        }
    }

    /// The live split's REMOVAL half (PLAN_M6_75.md item 3), which the
    /// golden test above cannot state: `alive` is no longer decodable at
    /// all. A v9 peer's `SessionInfo` is the only thing that would ever
    /// carry it, and such a peer is refused at the handshake — so this
    /// pins the failure the bump exists to cause, exactly as
    /// `interrupted_and_error_status_fail_under_a_legacy_v4_decoder` pins
    /// the mirror-image failure for version 5's additions.
    ///
    /// The direction matters: a decoder that silently DEFAULTED an
    /// unrecognized `alive` to `Unknown` would turn a version skew into a
    /// fleet of sessions with no status at all, which reads as a product
    /// bug rather than as the version problem it is.
    #[test]
    fn the_removed_alive_status_no_longer_decodes() {
        serde_json::from_value::<SessionStatus>(serde_json::json!({ "state": "alive" }))
            .expect_err("`alive` was REPLACED at version 10, not kept as a tolerated alias");
    }

    /// The other side of the same skew: a decoder shaped like a genuine v9
    /// peer — one that only ever knew `alive` — must FAIL on each of the
    /// three live statuses version 10 introduced, rather than defaulting or
    /// ignoring them. Without this, "the split forced a bump" would be an
    /// assertion rather than a tested property.
    #[derive(Debug, Deserialize)]
    #[serde(tag = "state", rename_all = "snake_case")]
    enum LegacyV9SessionStatus {
        Unknown,
        Alive,
        #[allow(dead_code)]
        Exited {
            exit_code: Option<i32>,
        },
        Error {
            #[allow(dead_code)]
            detail: String,
        },
        Interrupted,
    }

    #[test]
    fn the_split_live_statuses_fail_under_a_legacy_v9_decoder() {
        for status in [
            SessionStatus::Running,
            SessionStatus::Waiting,
            SessionStatus::Idle,
        ] {
            let json = serde_json::to_value(status).unwrap();
            serde_json::from_value::<LegacyV9SessionStatus>(json).unwrap_err();
        }
        // The unchanged statuses still cross that boundary, which is what
        // makes the failures above about the SPLIT rather than about the
        // shadow decoder being broken.
        for status in [SessionStatus::Unknown, SessionStatus::Interrupted] {
            let json = serde_json::to_value(status).unwrap();
            serde_json::from_value::<LegacyV9SessionStatus>(json)
                .expect("statuses that version 10 did not touch must still decode");
        }
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
    /// `SessionStatus` test in this file decodes through the CURRENT (v6)
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

    /// The v10 error-kind vocabulary, before PLAN_M7.md item 2 added
    /// `Unauthorized`.
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum LegacyV10ErrorKind {
        NotFound,
        InvalidRequest,
        Internal,
        Conflict,
    }

    /// The v10 control slice needed to prove the bump-earning archive
    /// request/reply tags and unauthorized error kind. Unknown enum tags
    /// fail before any handler could accidentally assign them an older
    /// meaning.
    #[derive(Debug, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum LegacyV10ControlMsg {
        #[allow(dead_code)]
        DeleteSession { req_id: u64, session_id: String },
        #[allow(dead_code)]
        SessionDeleted { req_id: u64 },
        #[allow(dead_code)]
        Error {
            req_id: u64,
            message: String,
            kind: LegacyV10ErrorKind,
        },
    }

    /// A v10 decoder cannot mistake v11's authorization failure for an
    /// older error kind; it must reject the tagged value.
    #[test]
    fn unauthorized_error_fails_under_a_legacy_v10_decoder() {
        let error = ControlMsg::Error {
            req_id: 1,
            message: "credential rejected".to_string(),
            kind: ErrorKind::Unauthorized,
        };
        serde_json::from_value::<LegacyV10ControlMsg>(serde_json::to_value(error).unwrap())
            .expect_err("a v10 decoder must fail on the v11 unauthorized error kind");
    }

    /// A v10 decoder has neither archive tag and must reject both messages
    /// rather than ignore either as an additive field on an older operation.
    #[test]
    fn archive_messages_fail_under_a_legacy_v10_decoder() {
        for archive in [
            ControlMsg::ArchiveSession {
                req_id: 2,
                session_id: "s1".to_string(),
            },
            ControlMsg::SessionArchived {
                req_id: 2,
                session: SessionInfo {
                    parent: None,
                    archived: true,
                    id: "s1".to_string(),
                    title: "demo".to_string(),
                    created_at: 0,
                    last_activity_at: 0,
                    creation_seq: Some(1),
                    cwd: "/tmp".to_string(),
                    invocation: "agent".to_string(),
                    status: SessionStatus::Exited { exit_code: None },
                    annotation: Some(STOP_ANNOTATION.to_string()),
                    restart_offer: RestartOffer::FreshOnly,
                    tabs: Vec::new(),
                    source_profile: None,
                },
            },
        ] {
            let decoded = serde_json::from_value::<LegacyV10ControlMsg>(
                serde_json::to_value(&archive).unwrap(),
            );
            assert!(
                decoded.is_err(),
                "a v10 decoder accepted a v11 archive tag: {archive:?}"
            );
        }
    }

    /// Version 11 has one archive-reply shape: the post-teardown session is
    /// required, not an optional field a same-version peer may omit.
    #[test]
    fn original_v11_archive_reply_requires_the_session() {
        #[derive(Debug, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OriginalV11Reply {
            SessionArchived { req_id: u64, session: SessionInfo },
        }

        let value = serde_json::to_value(ControlMsg::SessionArchived {
            req_id: 9,
            session: SessionInfo {
                parent: None,
                archived: true,
                id: "s1".to_string(),
                title: "demo".to_string(),
                created_at: 0,
                last_activity_at: 0,
                creation_seq: Some(1),
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Exited { exit_code: None },
                annotation: Some(STOP_ANNOTATION.to_string()),
                restart_offer: RestartOffer::FreshOnly,
                tabs: Vec::new(),
                source_profile: None,
            },
        })
        .unwrap();
        let OriginalV11Reply::SessionArchived { req_id, session } =
            serde_json::from_value(value).expect("the current reply is the original v11 shape");
        assert_eq!(req_id, 9);
        assert!(session.archived);

        serde_json::from_value::<OriginalV11Reply>(serde_json::json!({
            "type": "session_archived",
            "req_id": 9,
        }))
        .expect_err("a v11 archive reply without its session must fail");
    }

    /// `SessionList`'s `total`/`next_cursor` shape at `PROTOCOL_VERSION` 8
    /// (PLAN_M6.md item 1), golden-pinned in both the exhaustion and
    /// continuation cases: `next_cursor: None` (a page that reached the
    /// end of the order) and `next_cursor: Some(_)` (more pages remain).
    /// `total` itself is unchanged from the pre-8 shape — still additive,
    /// still the full pre-cut count — so it rides along here rather than
    /// getting its own test. `next_cursor` REPLACING `truncated` is the
    /// one part of this shape that is NOT additive: `truncated` simply no
    /// longer exists on the wire, and a decoder built before this PR has
    /// no way to notice that absence, which is exactly the field-removal
    /// case `PROTOCOL_VERSION`'s own docs say forces a bump rather than
    /// riding an additive one — see the two tests below for what that
    /// forced bump buys in exchange (both directions of tolerance
    /// WITHIN version 8, once both peers are known to speak it).
    #[test]
    fn session_list_total_and_next_cursor_json_shape_is_pinned() {
        for (msg, expected) in [
            (
                ControlMsg::SessionList {
                    req_id: 5,
                    sessions: vec![],
                    total: 7,
                    next_cursor: None,
                },
                serde_json::json!({
                    "type": "session_list",
                    "req_id": 5,
                    "sessions": [],
                    "total": 7,
                    "next_cursor": null,
                }),
            ),
            (
                ControlMsg::SessionList {
                    req_id: 6,
                    sessions: vec![],
                    total: 700,
                    next_cursor: Some("opaque-cursor-value".to_string()),
                },
                serde_json::json!({
                    "type": "session_list",
                    "req_id": 6,
                    "sessions": [],
                    "total": 700,
                    "next_cursor": "opaque-cursor-value",
                }),
            ),
        ] {
            assert_eq!(serde_json::to_value(&msg).unwrap(), expected);
        }
    }

    /// The additive-decode half of PLAN_M2.md's "Proto growth" contract,
    /// extended for version 8: JSON shaped like a genuine v7 sender — a
    /// `SessionInfo` with no `state` or `created_at`, a `SessionList`
    /// still carrying the pre-8 `truncated` bool (removed, not merely
    /// unfilled, by this PR's bump — see `PROTOCOL_VERSION`'s own docs)
    /// but no `total`/`next_cursor` — must still decode successfully:
    /// `truncated` is silently dropped as an unrecognized key, and the new
    /// fields default rather than failing. `total` was already
    /// additive-tolerant before this PR (see the version history linked above);
    /// `created_at` is NOT — it is this PR's own addition, decoding to `0`
    /// under the same `#[serde(default)]` tolerance every prior addition
    /// got on the PR that introduced IT. What decodes differently from
    /// every earlier version is `next_cursor`, which takes over
    /// `truncated`'s old job: absence decodes to `None`, the same way
    /// `truncated`'s absence used to decode to `false` — but this
    /// tolerance is intra-version-8 only (both peers already speak v8; the
    /// handshake refused anything older), NOT a claim that a real pre-8
    /// peer's `SessionList` reaches this decoder at all.
    #[test]
    fn old_shape_session_list_json_decodes_with_defaulted_new_fields() {
        let old_shape = serde_json::json!({
            "type": "session_list",
            "req_id": 9,
            "truncated": false,
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
            next_cursor,
        } = decoded
        else {
            panic!("expected ControlMsg::SessionList, got {decoded:?}");
        };
        assert_eq!(req_id, 9);
        assert_eq!(total, 0, "an old sender's reply predates the field");
        assert_eq!(
            next_cursor, None,
            "an old sender's reply predates the field, same as an exhausted page"
        );
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].created_at, 0,
            "an old sender's SessionInfo predates created_at"
        );
        assert_eq!(
            sessions[0].status,
            SessionStatus::Unknown,
            "a SessionInfo with no state field must decode as Unknown, never a guess"
        );
        assert_eq!(sessions[0].parent, None, "a v10 session has no parent");
        assert_eq!(
            sessions[0].creation_seq, None,
            "an older sender has no supervisor creation sequence"
        );
        assert!(!sessions[0].archived, "a v10 session is not archived");
    }

    /// The REVERSE direction from the test above: a hand-rolled decoder
    /// shaped like a much OLDER peer than the one that test models — no
    /// `status`, no `total`, no `created_at`/`next_cursor` (nor, further
    /// back, the `truncated` this version replaced) — must still decode a
    /// NEW sender's JSON successfully, silently dropping every field it
    /// does not know about. This is deliberately older than "the peer
    /// immediately before this PR": a genuine v7 sender already HAD
    /// `status` and `total` (see the version history linked above for when each
    /// arrived), so this shadow type instead stands in for a peer that
    /// predates both — the pre-status/pre-total shape its own test name
    /// says it models. Serde's default of ignoring unrecognized object
    /// keys is what makes this work. The shadow types below deliberately
    /// carry no `#[serde(deny_unknown_fields)]` either, standing in for a real old
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
                parent: None,
                archived: false,
                id: "s1".to_string(),
                title: "demo".to_string(),
                created_at: 1_700_000_000,
                last_activity_at: 1_700_000_000,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Exited { exit_code: Some(1) },
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
                source_profile: None,
            }],
            total: 3,
            next_cursor: Some("cursor-after-s1".to_string()),
        };
        let json = serde_json::to_value(&new_msg).unwrap();

        let LegacyControlMsg::SessionList { req_id, sessions } =
            serde_json::from_value(json.clone()).expect(
                "a legacy decoder without status/total/next_cursor/created_at must still decode \
                 new-shape JSON",
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
    /// elsewhere in this file. `tabs` (PLAN_M4.md item 2, additive within
    /// 5's successor version 6 the same way) rides along in the absent-
    /// shape half below rather than earning its own test, since its
    /// present-shape golden (with actual `TabInfo` entries) is more
    /// informative pinned separately in
    /// `session_info_tabs_json_shape_is_pinned`.
    ///
    /// `last_activity_at` (version 11's addition) rides along in both
    /// halves rather than earning a test of its own — the golden below
    /// already pins its wire name and its bare-integer type, and the
    /// old-shape decode below pins its default. It is given a value
    /// DIFFERENT from `created_at` here for one reason worth stating: the
    /// two are independent fields, and equal values would let a
    /// serialization that derived one from the other pass unnoticed.
    #[test]
    fn session_info_annotation_and_restart_offer_json_shapes_are_pinned() {
        let bare = SessionInfo {
            parent: None,
            archived: false,
            id: "s1".to_string(),
            title: "demo".to_string(),
            created_at: 1_700_000_000,
            last_activity_at: 1_700_000_600,
            creation_seq: None,
            cwd: "/tmp".to_string(),
            invocation: "agent".to_string(),
            status: SessionStatus::default(),
            annotation: None,
            restart_offer: RestartOffer::default(),
            tabs: Vec::new(),
            source_profile: None,
        };
        assert_eq!(
            serde_json::to_value(&bare).unwrap(),
            serde_json::json!({
                "id": "s1",
                "parent": null,
                "title": "demo",
                "created_at": 1_700_000_000,
                "last_activity_at": 1_700_000_600,
                "creation_seq": null,
                "cwd": "/tmp",
                "invocation": "agent",
                "status": { "state": "unknown" },
                "annotation": null,
                "restart_offer": "fresh_only",
                "tabs": [],
                "archived": false,
                "source_profile": null,
            })
        );

        // Dropping ONE of the two timestamps must leave the other alone —
        // the half of the additive contract a whole-object golden cannot
        // express, and the case a real old sender produces.
        let mut without = serde_json::to_value(&bare).unwrap();
        without
            .as_object_mut()
            .expect("a SessionInfo serializes as an object")
            .remove("last_activity_at");
        let decoded: SessionInfo = serde_json::from_value(without).expect("decodes without it");
        assert_eq!(decoded.last_activity_at, 0);
        assert_eq!(
            decoded.created_at, 1_700_000_000,
            "dropping one timestamp must not disturb the other"
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

        // JSON shaped as if none of `annotation`, `restart_offer`, `tabs`,
        // (version 8) `created_at`, or (version 11) `last_activity_at` had
        // been added YET — must still decode, defaulting every one of
        // them. This is intra-version additive
        // discipline, not real cross-build interop: an actual pre-M3 (v4)
        // peer is refused outright at the handshake (see
        // `PROTOCOL_VERSION`'s own docs) and never reaches this decode
        // path at all — `tabs` predates no real peer either, since it
        // shipped in the same version 6 bump as every other M4 addition,
        // and `created_at` likewise predates no real peer, having shipped
        // in version 8 alongside the rest of M6's vocabulary — but pinning
        // both here keeps the same "additive within one version" guarantee
        // `status` needed when THAT field was added within v3 (see
        // `old_shape_session_list_json_decodes_with_defaulted_new_fields`
        // above) explicit for every field this struct has ever grown.
        let old_shape = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
            "status": { "state": "running" },
        });
        let decoded: SessionInfo = serde_json::from_value(old_shape).unwrap();
        assert_eq!(decoded.annotation, None);
        assert_eq!(decoded.parent, None);
        assert_eq!(decoded.creation_seq, None);
        assert!(!decoded.archived);
        assert_eq!(decoded.restart_offer, RestartOffer::FreshOnly);
        assert_eq!(
            decoded.tabs,
            Vec::new(),
            "a sender that predates tabs must decode to \"none known\", not an error"
        );
        assert_eq!(
            decoded.created_at, 0,
            "a sender that predates created_at must default to 0, not fail or guess a real time"
        );
        assert_eq!(
            decoded.last_activity_at, 0,
            "a sender that predates last_activity_at must default to 0, which readers take as \
             \"unknown, fall back to created_at\" rather than as an instant in 1970"
        );
    }

    /// PLAN_M4.md item 2's `SessionInfo::tabs` addition, golden-pinned
    /// with tabs actually present — the sibling test above only pins the
    /// empty-tabs case alongside `annotation`/`restart_offer`. `TabInfo`'s
    /// own wire shape (a bare `{"id": "..."}`, no `kind` tag, unlike
    /// `TerminalSelector`) is pinned here with a LIST of them; it also
    /// appears as a single value nested in `ControlMsg::TabOpened`, which
    /// `tab_open_and_close_json_shapes_are_pinned` pins separately — no
    /// standalone `TabInfo`-only test exists because both call sites
    /// already golden-pin its shape.
    #[test]
    fn session_info_tabs_json_shape_is_pinned() {
        let info = SessionInfo {
            parent: None,
            archived: false,
            id: "s1".to_string(),
            title: "demo".to_string(),
            created_at: 1_700_000_000,
            last_activity_at: 1_700_000_000,
            creation_seq: None,
            cwd: "/tmp".to_string(),
            invocation: "agent".to_string(),
            status: SessionStatus::Running,
            annotation: None,
            restart_offer: RestartOffer::default(),
            tabs: vec![
                TabInfo {
                    id: "t1".to_string(),
                },
                TabInfo {
                    id: "t2".to_string(),
                },
            ],
            source_profile: None,
        };
        assert_eq!(
            serde_json::to_value(&info).unwrap()["tabs"],
            serde_json::json!([{"id": "t1"}, {"id": "t2"}])
        );
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
                        "status": { "state": "running" },
                        "future_field_inside_session": "value from tomorrow",
                    }
                ],
                "total": 1,
                "next_cursor": null,
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
        assert_eq!(sessions[0].status, SessionStatus::Running);
    }

    /// PLAN_M4.md item 1's `TerminalSelector` is itself a nested object
    /// (tagged `kind`), so it needs the same nesting-depth additivity
    /// proof `session_list_with_unknown_field_inside_session_decodes_through_parse_control`
    /// gives `SessionInfo` above: a future field appearing inside the
    /// `terminal` object must still decode through the REAL
    /// `parse_control` path, not just a hand-rolled
    /// `serde_json::from_value::<TerminalSelector>`.
    #[test]
    fn attach_with_unknown_field_inside_terminal_selector_decodes_through_parse_control() {
        let frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: serde_json::json!({
                "type": "attach",
                "req_id": 13,
                "session_id": "s1",
                "channel": 5,
                "cols": 80,
                "rows": 24,
                "terminal": {
                    "kind": "tab",
                    "id": "t1",
                    "future_field_inside_selector": "value from tomorrow",
                },
                "lease": "client-abc",
            })
            .to_string()
            .into_bytes(),
        };
        let msg = crate::io::parse_control(&frame).expect(
            "an unknown field nested inside a TerminalSelector object must decode, not error",
        );
        let ControlMsg::Attach {
            terminal,
            if_unowned,
            ..
        } = msg
        else {
            panic!("expected ControlMsg::Attach, got {msg:?}");
        };
        assert_eq!(
            terminal,
            TerminalSelector::Tab {
                id: "t1".to_string()
            }
        );
        assert!(
            !if_unowned,
            "an attach from a sender that predates PLAN_M6.md item 7 must read as the DISPLACING \
             attach every caller has always sent — defaulting it the other way would turn every \
             legacy reattach into a refusal the moment anyone else held the session"
        );
    }

    /// The refusal string a non-displacing attach comes back with must be
    /// the SAME string a client displaced while attached is told
    /// (`DETACH_REASON_TAKEOVER`, private to the supervisor).
    ///
    /// Pinned as a literal here because the two constants live in
    /// different crates with nothing but this equality holding them
    /// together, and the browser matches on ONE string to decide it lost
    /// the session (terminal.js's `TAKEOVER_DETACH_REASON`). If the
    /// supervisor's copy ever drifts from this one, a client refused a
    /// reconnect would fall through to its generic banner and keep
    /// climbing the ladder against a session it can never have — the exact
    /// eviction loop the refusal exists to end, minus the eviction.
    #[test]
    fn the_refused_attach_reason_is_the_takeover_wording() {
        assert_eq!(ATTACH_REFUSED_TAKEN_OVER, "another client attached");
    }

    /// The `TabInfo` sibling of the two nesting-additivity tests above:
    /// a future field inside a `TabInfo` object (here, the one
    /// `TabOpened` carries) must still decode through `parse_control`,
    /// keeping the tab id intact.
    #[test]
    fn tab_opened_with_unknown_field_inside_tab_info_decodes_through_parse_control() {
        let frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: serde_json::json!({
                "type": "tab_opened",
                "req_id": 20,
                "tab": {
                    "id": "t1",
                    "future_field_inside_tab_info": "value from tomorrow",
                },
            })
            .to_string()
            .into_bytes(),
        };
        let msg = crate::io::parse_control(&frame)
            .expect("an unknown field nested inside a TabInfo object must decode, not error");
        let ControlMsg::TabOpened { tab, .. } = msg else {
            panic!("expected ControlMsg::TabOpened, got {msg:?}");
        };
        assert_eq!(tab.id, "t1");
    }

    /// Unlike an unknown FIELD (tolerated at every nesting level pinned
    /// above), an unrecognized `TerminalSelector` `kind` tag is decode-
    /// fatal — the internally-tagged-enum analogue of
    /// `unknown_control_message_tag_fails_decode`. This is what would
    /// catch a future terminal kind (a shared or observer selector, say)
    /// shipping without its own protocol bump: today's decoder must
    /// refuse an unrecognized `kind` outright, never silently default to
    /// `Agent`, or a real version skew would attach the wrong terminal
    /// instead of failing loudly.
    #[test]
    fn unknown_terminal_selector_kind_fails_decode() {
        let frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: serde_json::json!({
                "type": "attach",
                "req_id": 14,
                "session_id": "s1",
                "channel": 6,
                "cols": 80,
                "rows": 24,
                "terminal": { "kind": "holo" },
            })
            .to_string()
            .into_bytes(),
        };
        crate::io::parse_control(&frame).expect_err(
            "an unrecognized TerminalSelector kind must be a decode error, not a tolerated \
             default",
        );
    }

    /// The decode-direction counterpart to `session_info_tabs_json_shape_is_pinned`
    /// (which only pins the encode side): a `SessionList` frame carrying
    /// multiple tabs must decode through the REAL `parse_control` path
    /// with both tab IDENTITY and ORDER intact — order matters because
    /// `TabInfo`'s own doc comment promises clients derive their
    /// positional labels ("Terminal 1", "Terminal 2", ...) from list
    /// order, so a decoder that silently reordered (a `HashSet`-backed
    /// field, say) would relabel every tab without any field actually
    /// being wrong.
    #[test]
    fn session_list_tabs_decode_in_order_through_parse_control() {
        let frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: serde_json::json!({
                "type": "session_list",
                "req_id": 15,
                "sessions": [
                    {
                        "id": "s1",
                        "title": "demo",
                        "cwd": "/tmp",
                        "invocation": "agent",
                        "status": { "state": "running" },
                        "tabs": [{"id": "t1"}, {"id": "t2"}, {"id": "t3"}],
                    }
                ],
                "total": 1,
                "next_cursor": null,
            })
            .to_string()
            .into_bytes(),
        };
        let msg = crate::io::parse_control(&frame).expect("a SessionInfo with tabs must decode");
        let ControlMsg::SessionList { sessions, .. } = msg else {
            panic!("expected ControlMsg::SessionList, got {msg:?}");
        };
        assert_eq!(
            sessions[0].tabs,
            vec![
                TabInfo {
                    id: "t1".to_string()
                },
                TabInfo {
                    id: "t2".to_string()
                },
                TabInfo {
                    id: "t3".to_string()
                },
            ],
            "tab identity and creation order must both survive the wire"
        );
    }

    /// `AgentKind`, `RestartOffer`, and `RestartMode` are bare snake_case
    /// strings (see their doc comments for why, unlike `SessionStatus`);
    /// `TerminalSelector` (PLAN_M4.md item 1) is internally tagged instead
    /// (see its own doc comment for why — `Tab` carries an id) but is
    /// exactly the same kind of small, closed wire vocabulary, so it gets
    /// the same golden-per-variant treatment here rather than a fourth
    /// near-duplicate test.
    ///
    /// Each enum below is checked by a `match` with no wildcard arm whose
    /// arms ARE the golden assertions, not a separate exhaustive match
    /// paired with a separate `assert_eq!` list. That used to be two
    /// artifacts (an early version of this test, and this comment, both
    /// claimed the pairing was itself exhaustive proof): compilation only
    /// forces a new variant into the bare top-of-test match, which an
    /// empty `{}` arm satisfies with no golden value attached anywhere,
    /// so a variant could be "covered" there while its JSON shape was
    /// simply never pinned. Collapsing the two into one match per enum
    /// closes that gap the only way the type system can: the compiler
    /// still only forces an arm to exist for a new variant, but that arm
    /// is now the one place this test would ever assert its shape, so
    /// there is no separate golden list left to forget populating.
    #[test]
    fn agent_kind_restart_and_terminal_selector_vocabulary_json_shapes_are_pinned() {
        for kind in [AgentKind::Claude, AgentKind::Codex, AgentKind::Generic] {
            let expected = match kind {
                AgentKind::Claude => "claude",
                AgentKind::Codex => "codex",
                AgentKind::Generic => "generic",
            };
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                serde_json::json!(expected)
            );
        }

        for offer in [
            RestartOffer::FreshOnly,
            RestartOffer::Resume,
            RestartOffer::FallbackTemplate,
        ] {
            let expected = match offer {
                RestartOffer::FreshOnly => "fresh_only",
                RestartOffer::Resume => "resume",
                RestartOffer::FallbackTemplate => "fallback_template",
            };
            assert_eq!(
                serde_json::to_value(offer).unwrap(),
                serde_json::json!(expected)
            );
        }

        for mode in [
            RestartMode::Resume,
            RestartMode::Fresh,
            RestartMode::FallbackTemplate,
        ] {
            let expected = match mode {
                RestartMode::Resume => "resume",
                RestartMode::Fresh => "fresh",
                RestartMode::FallbackTemplate => "fallback_template",
            };
            assert_eq!(
                serde_json::to_value(mode).unwrap(),
                serde_json::json!(expected)
            );
        }

        for selector in [
            TerminalSelector::Agent,
            TerminalSelector::Tab {
                id: "t1".to_string(),
            },
        ] {
            let expected = match &selector {
                TerminalSelector::Agent => serde_json::json!({"kind": "agent"}),
                TerminalSelector::Tab { id } => serde_json::json!({"kind": "tab", "id": id}),
            };
            assert_eq!(serde_json::to_value(&selector).unwrap(), expected);
        }
    }

    /// `CreateSession`'s three PLAN_M3.md additions (`intent_key`,
    /// `agent_kind`, `resume_template`) golden-pinned with every one of
    /// them present, matching the treatment every other message shape in
    /// this file gets — now in the RAW mode `PROTOCOL_VERSION` 10 named,
    /// with `profile_id` explicitly null beside a present `invocation`.
    #[test]
    fn create_session_snapshot_override_fields_json_shape_is_pinned() {
        let msg = ControlMsg::CreateSession {
            req_id: 1,
            parent: None,
            profile_name: None,
            cwd: "/some/dir".to_string(),
            invocation: Some("/opt/bin/claude".to_string()),
            profile_id: None,
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
                "parent": null,
                "profile_name": null,
                "cwd": "/some/dir",
                "invocation": "/opt/bin/claude",
                "profile_id": null,
                "title": null,
                "cols": 80,
                "rows": 24,
                "intent_key": "intent-abc",
                "agent_kind": "claude",
                "resume_template": ["/opt/bin/claude", "--resume", "{conversation}"],
            })
        );
    }

    /// The PROFILE mode of the same message (PLAN_M6_75.md item 3),
    /// golden-pinned as its own shape because it is the one this bump
    /// actually introduces: `profile_id` present, and `invocation`,
    /// `agent_kind` and `resume_template` all null — the request a client
    /// sends when the user picked a profile rather than typing a command.
    ///
    /// Pinned in both directions against the golden value, since a
    /// profile-mode create is the shape with no legacy sender to have
    /// established it by precedent: encode must produce exactly this, and
    /// this must decode back.
    #[test]
    fn create_session_profile_mode_json_shape_is_pinned() {
        let msg = ControlMsg::CreateSession {
            req_id: 2,
            parent: None,
            profile_name: None,
            cwd: "/some/dir".to_string(),
            invocation: None,
            profile_id: Some("prof-7".to_string()),
            title: Some("demo".to_string()),
            cols: 80,
            rows: 24,
            intent_key: Some("intent-abc".to_string()),
            agent_kind: None,
            resume_template: None,
        };
        let expected = serde_json::json!({
            "type": "create_session",
            "req_id": 2,
            "parent": null,
            "profile_name": null,
            "cwd": "/some/dir",
            "invocation": null,
            "profile_id": "prof-7",
            "title": "demo",
            "cols": 80,
            "rows": 24,
            "intent_key": "intent-abc",
            "agent_kind": null,
            "resume_template": null,
        });
        assert_eq!(serde_json::to_value(&msg).unwrap(), expected);
        let golden_frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: expected.to_string().into_bytes(),
        };
        assert_eq!(crate::io::parse_control(&golden_frame).unwrap(), msg);
    }

    /// PLAN_M7.md item 2's spawn selector and parent reference, pinned in
    /// both directions. Together with the raw and profile-id goldens above,
    /// this covers every selector present once and null twice.
    #[test]
    fn create_session_profile_name_and_parent_json_shape_is_pinned() {
        let msg = ControlMsg::CreateSession {
            req_id: 3,
            parent: Some("parent-1".to_string()),
            cwd: "/some/dir".to_string(),
            invocation: None,
            profile_id: None,
            profile_name: Some("Claude Code".to_string()),
            title: None,
            cols: 80,
            rows: 24,
            intent_key: Some("spawn-key".to_string()),
            agent_kind: None,
            resume_template: None,
        };
        let expected = serde_json::json!({
            "type": "create_session",
            "req_id": 3,
            "parent": "parent-1",
            "cwd": "/some/dir",
            "invocation": null,
            "profile_id": null,
            "profile_name": "Claude Code",
            "title": null,
            "cols": 80,
            "rows": 24,
            "intent_key": "spawn-key",
            "agent_kind": null,
            "resume_template": null,
        });
        assert_eq!(serde_json::to_value(&msg).unwrap(), expected);
        assert_eq!(serde_json::from_value::<ControlMsg>(expected).unwrap(), msg);
    }

    /// The both-modes request EXISTS on the wire and decodes cleanly — it
    /// is refused by the supervisor's create handler, not by the codec
    /// (see `ControlMsg::CreateSession`'s own docs for why the exclusivity
    /// is a handler rule with a message rather than a type that cannot
    /// express the request).
    ///
    /// Worth pinning precisely because it is easy to "fix" in the wrong
    /// place: a decoder that rejected this shape would move the refusal
    /// from a correlated `InvalidRequest` a client can display into a
    /// decode error that tears down the whole connection, taking every
    /// unrelated session on it along. Same for the names-NEITHER shape.
    #[test]
    fn a_create_naming_both_modes_or_neither_still_decodes_for_the_handler_to_refuse() {
        for (invocation, profile_id) in [
            (Some("agent".to_string()), Some("prof-7".to_string())),
            (None, None),
        ] {
            let msg = ControlMsg::CreateSession {
                req_id: 3,
                parent: None,
                profile_name: None,
                cwd: "/some/dir".to_string(),
                invocation,
                profile_id,
                title: None,
                cols: 80,
                rows: 24,
                intent_key: None,
                agent_kind: None,
                resume_template: None,
            };
            let json = serde_json::to_value(&msg).unwrap();
            let decoded: ControlMsg = serde_json::from_value(json)
                .expect("an ambiguous create is a request to REFUSE, not a frame to reject");
            assert_eq!(decoded, msg);
        }
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
            parent,
            invocation,
            profile_id,
            profile_name,
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
        assert_eq!(parent, None, "a v10 create has no spawn parent");
        assert_eq!(profile_name, None, "a v10 create cannot select by name");
        // Version 10's mode selection read against a request that predates
        // it: a bare `invocation` with no `profile_id` key IS the raw mode,
        // which is what keeps every pre-10 caller meaning exactly what it
        // always meant rather than becoming an ambiguous names-neither
        // request the handler would refuse.
        assert_eq!(
            invocation,
            Some("some-agent".to_string()),
            "a required-then-optional field must still carry the value it always did"
        );
        assert_eq!(
            profile_id, None,
            "an absent profile_id selects raw mode, the only mode an older sender knows"
        );
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
            parent: None,
            profile_name: None,
            cwd: "/some/dir".to_string(),
            // RAW mode deliberately: a legacy decoder's `invocation` is a
            // required `String`, so the PROFILE mode's `null` would fail
            // this decode outright — which is a version-10 skew hazard the
            // handshake closes, not a tolerance this test may claim.
            invocation: Some("/opt/bin/claude".to_string()),
            profile_id: None,
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

    /// A decoder shaped like a genuine v9 peer: every field `CreateSession`
    /// had at `PROTOCOL_VERSION` 9, with `invocation` still a REQUIRED
    /// `String` — which is precisely the field version 10 loosened.
    #[derive(Debug, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum LegacyV9ControlMsg {
        // Every field is spelled out for SHAPE fidelity even though the
        // test below reads only `invocation`: a decoder missing fields a
        // real v9 peer had would be tolerant in ways that peer was not,
        // and the whole point is to model what that peer would actually do
        // with today's bytes.
        #[allow(dead_code)]
        CreateSession {
            req_id: u64,
            cwd: String,
            invocation: String,
            title: Option<String>,
            cols: u16,
            rows: u16,
            intent_key: Option<String>,
            agent_kind: Option<AgentKind>,
            resume_template: Option<Vec<String>>,
        },
    }

    /// The skew hazard version 10's `CreateSession` actually carries, in
    /// both directions (PLAN_M6_75.md item 3).
    ///
    /// A RAW create still decodes under a v9 decoder — that is the control,
    /// and it is what makes the second half meaningful rather than a test
    /// of a broken fixture. A PROFILE-MODE create does NOT: `invocation` is
    /// `null`, and a required `String` cannot take a null, so the decode
    /// fails outright.
    ///
    /// Failing is the RIGHT outcome, and worth pinning precisely because
    /// the alternative sounds harmless: a decoder that tolerated the null
    /// (an `Option` with a default, say) would hand a v9 supervisor a
    /// create with no invocation at all and no profile it knows how to
    /// resolve — a request whose meaning it cannot see. The handshake is
    /// what keeps this unreachable; this test states what would happen
    /// without it, which is the argument for the bump.
    #[test]
    fn a_profile_mode_create_cannot_decode_under_a_legacy_v9_decoder() {
        let raw = ControlMsg::CreateSession {
            req_id: 1,
            parent: None,
            profile_name: None,
            cwd: "/some/dir".to_string(),
            invocation: Some("agent".to_string()),
            profile_id: None,
            title: None,
            cols: 80,
            rows: 24,
            intent_key: None,
            agent_kind: None,
            resume_template: None,
        };
        let LegacyV9ControlMsg::CreateSession { invocation, .. } =
            serde_json::from_value(serde_json::to_value(&raw).unwrap())
                .expect("a RAW create must still decode under a v9 decoder");
        assert_eq!(
            invocation, "agent",
            "the control: version 10 did not change what a raw create looks like on the wire"
        );

        let profile_mode = ControlMsg::CreateSession {
            req_id: 2,
            parent: None,
            profile_name: None,
            cwd: "/some/dir".to_string(),
            invocation: None,
            profile_id: Some("prof-7".to_string()),
            title: None,
            cols: 80,
            rows: 24,
            intent_key: None,
            agent_kind: None,
            resume_template: None,
        };
        serde_json::from_value::<LegacyV9ControlMsg>(serde_json::to_value(&profile_mode).unwrap())
            .expect_err(
                "a v9 decoder must REFUSE a profile-mode create rather than read it as a create \
             with no invocation",
            );
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
                parent: None,
                archived: false,
                id: "s1".to_string(),
                title: "demo".to_string(),
                created_at: 1_700_000_000,
                last_activity_at: 1_700_000_000,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                invocation: "claude".to_string(),
                status: SessionStatus::Running,
                annotation: None,
                restart_offer: RestartOffer::Resume,
                tabs: Vec::new(),
                source_profile: None,
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
                parent: None,
                archived: false,
                id: "s1".to_string(),
                title: "demo".to_string(),
                created_at: 1_700_000_000,
                last_activity_at: 1_700_000_000,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                invocation: "claude".to_string(),
                status: SessionStatus::Running,
                annotation: None,
                restart_offer: RestartOffer::Resume,
                tabs: Vec::new(),
                source_profile: None,
            },
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            serde_json::json!({
                "type": "session_restarted",
                "req_id": 8,
                "session": {
                    "id": "s1",
                    "parent": null,
                    "title": "demo",
                    "created_at": 1_700_000_000,
                    "last_activity_at": 1_700_000_000,
                    "creation_seq": null,
                    "cwd": "/tmp",
                    "invocation": "claude",
                    "status": { "state": "running" },
                    "annotation": null,
                    "restart_offer": "resume",
                    "tabs": [],
                    "archived": false,
                    "source_profile": null,
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

    /// PLAN_M5.md item 1's complete M5 wire vocabulary — `ReplayComplete`,
    /// `RenameSession`, `SessionRenamed` — golden-pinned the same way the
    /// M4 tab-lifecycle and upload families are below: a serde attribute
    /// drift on any one of them would compile and round-trip cleanly
    /// while quietly producing bytes an unmodified peer cannot parse. A
    /// (message, expected JSON) table rather than three separately-named
    /// locals, matching every other multi-variant golden test in this
    /// file. `SessionRenamed`'s nested `session` gets the same
    /// full-shape treatment `session_restarted_json_shape_is_pinned`
    /// gives its own nested `SessionInfo`, for the identical reason: a
    /// bare round-trip could hide a coordinated encode/decode drift that
    /// still agrees with itself but not with an unmodified peer.
    ///
    /// Each row is pinned in BOTH directions against the golden value
    /// itself — encode must produce it, and `parse_control` must decode
    /// it (the literal golden bytes, not a re-serialization) back to the
    /// original message — plus one pass through the real frame
    /// encode/decode path, so a drift between the codec's framing and
    /// serde's JSON shape cannot hide behind a value-level assertion.
    /// One test rather than a golden/roundtrip pair: the earlier split
    /// duplicated the three-variant fixture table and its "decode"
    /// direction only round-tripped bytes it had itself just produced,
    /// which proves self-agreement, not agreement with the pinned shape.
    #[test]
    fn replay_complete_and_rename_json_shapes_are_pinned() {
        for (msg, expected) in [
            (
                ControlMsg::ReplayComplete { channel: 4 },
                serde_json::json!({
                    "type": "replay_complete",
                    "channel": 4,
                }),
            ),
            (
                ControlMsg::RenameSession {
                    req_id: 50,
                    session_id: "s1".to_string(),
                    title: "renamed title".to_string(),
                },
                serde_json::json!({
                    "type": "rename_session",
                    "req_id": 50,
                    "session_id": "s1",
                    "title": "renamed title",
                }),
            ),
            (
                ControlMsg::SessionRenamed {
                    req_id: 50,
                    session: SessionInfo {
                        parent: None,
                        archived: false,
                        id: "s1".to_string(),
                        title: "renamed title".to_string(),
                        created_at: 1_700_000_000,
                        last_activity_at: 1_700_000_000,
                        creation_seq: None,
                        cwd: "/tmp".to_string(),
                        invocation: "claude".to_string(),
                        status: SessionStatus::Running,
                        annotation: None,
                        restart_offer: RestartOffer::Resume,
                        tabs: Vec::new(),
                        source_profile: None,
                    },
                },
                serde_json::json!({
                    "type": "session_renamed",
                    "req_id": 50,
                    "session": {
                        "id": "s1",
                        "parent": null,
                        "title": "renamed title",
                        "created_at": 1_700_000_000,
                        "last_activity_at": 1_700_000_000,
                        "creation_seq": null,
                        "cwd": "/tmp",
                        "invocation": "claude",
                        "status": { "state": "running" },
                        "annotation": null,
                        "restart_offer": "resume",
                        "tabs": [],
                        "archived": false,
                        "source_profile": null,
                    },
                }),
            ),
        ] {
            // Encode direction: the golden value, not just a round trip.
            assert_eq!(serde_json::to_value(&msg).unwrap(), expected);
            // Decode direction: the LITERAL golden bytes back through the
            // real control-frame parser — a re-serialization here would
            // only prove self-agreement.
            let golden_frame = Frame {
                kind: FrameKind::Control,
                channel: 0,
                body: expected.to_string().into_bytes(),
            };
            assert_eq!(crate::io::parse_control(&golden_frame).unwrap(), msg);
            // And once through the real frame codec, where a framing
            // drift would hide from both value-level assertions above.
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

    /// Version 7's additive rule for FIELDS (see `PROTOCOL_VERSION`'s
    /// docs), future-sender → current-decoder half, applied to the one
    /// new REQUEST-shaped M5 variant — mirroring
    /// `restart_session_with_future_extra_field_decodes_through_parse_control`'s
    /// treatment of version 5's own new request. One representative
    /// variant rather than all three, deliberately: every `ControlMsg`
    /// variant shares one `serde(tag = "type")` enum with no
    /// `deny_unknown_fields` anywhere, so unknown-field tolerance is a
    /// property of the enum's derive, not of any single variant — the
    /// generic mechanism is what this pins, on the variant whose decode
    /// path (a request a supervisor must not reject) carries the
    /// highest cost of getting it wrong. The golden test above already
    /// decodes all three variants' exact shapes through the same
    /// parser.
    #[test]
    fn rename_session_with_future_extra_field_decodes_through_parse_control() {
        let frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: br#"{"type":"rename_session","req_id":2,"session_id":"s1","title":"new title","priority":"high"}"#
                .to_vec(),
        };
        let msg = crate::io::parse_control(&frame)
            .expect("a known tag with an unknown extra field must decode, not error");
        assert_eq!(
            msg,
            ControlMsg::RenameSession {
                req_id: 2,
                session_id: "s1".to_string(),
                title: "new title".to_string(),
            }
        );
    }

    /// The OTHER half of version 7's additive rule: current-sender →
    /// future-decoder, mirroring
    /// `current_pause_output_decodes_under_a_future_v4_decoder_with_defaults`'s
    /// shadow-struct technique. A hypothetical later-v7 build that grew
    /// an optional field on `RenameSession` must accept today's rename
    /// bytes and default the absent field — this is what makes "new
    /// optional fields with decode defaults are fine" (the
    /// `PROTOCOL_VERSION` docs' additive discipline) a tested promise
    /// for the M5 vocabulary rather than an asserted one. Same
    /// representative-variant argument as the future-extra-field test
    /// above: the tolerance is the enum derive's property, pinned once
    /// on the variant where a wrong answer costs most.
    #[test]
    fn current_rename_session_decodes_under_a_future_v7_decoder_with_defaults() {
        #[derive(serde::Deserialize)]
        struct FutureRenameSession {
            req_id: u64,
            session_id: String,
            title: String,
            #[serde(default)]
            expected_generation: Option<u64>,
        }
        let mut wire = Vec::new();
        Frame::control(&ControlMsg::RenameSession {
            req_id: 7,
            session_id: "s1".to_string(),
            title: "new title".to_string(),
        })
        .encode(&mut wire)
        .unwrap();
        let (frame, _) = Frame::decode(&wire).unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&frame.body).unwrap();
        assert_eq!(value["type"], "rename_session");
        let decoded: FutureRenameSession = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.req_id, 7);
        assert_eq!(decoded.session_id, "s1");
        assert_eq!(decoded.title, "new title");
        assert_eq!(
            decoded.expected_generation, None,
            "an absent future field must default, never fail the decode"
        );
    }

    /// A profile with every field populated, for the golden tests below.
    ///
    /// A helper rather than a repeated literal because the CRUD family
    /// carries the same record through four messages, and a fixture that
    /// differed between them would let a golden test pass while the shapes
    /// disagreed. `resume_template` is `Some` here deliberately: its `None`
    /// half is pinned separately, since an absent template is what a
    /// generic profile normally has.
    fn a_profile() -> Profile {
        Profile {
            id: "prof-7".to_string(),
            name: "Claude Code".to_string(),
            invocation: "claude".to_string(),
            agent_kind: AgentKind::Claude,
            resume_template: Some(vec!["claude".to_string(), "{conversation}".to_string()]),
        }
    }

    /// The complete profile CRUD vocabulary (PLAN_M6_75.md item 3),
    /// golden-pinned the way the tab-lifecycle and upload families below
    /// are: every variant's exact JSON, since a serde attribute drift on
    /// any one of them would compile and round-trip cleanly while quietly
    /// producing bytes an unmodified peer cannot parse.
    ///
    /// Each row is checked in BOTH directions against the golden value —
    /// encode must produce it, and the LITERAL golden bytes must parse back
    /// through `parse_control` — plus one pass through the real frame
    /// codec, matching `replay_complete_and_rename_json_shapes_are_pinned`'s
    /// discipline and for its reason: a re-serialization would only prove
    /// self-agreement, and a value-level assertion cannot see a framing
    /// drift.
    #[test]
    fn profile_crud_json_shapes_are_pinned() {
        let profile_json = serde_json::json!({
            "id": "prof-7",
            "name": "Claude Code",
            "invocation": "claude",
            "agent_kind": "claude",
            "resume_template": ["claude", "{conversation}"],
        });
        for (msg, expected) in [
            (
                ControlMsg::ListProfiles { req_id: 60 },
                serde_json::json!({
                    "type": "list_profiles",
                    "req_id": 60,
                }),
            ),
            (
                ControlMsg::ProfileList {
                    req_id: 60,
                    profiles: vec![a_profile()],
                },
                serde_json::json!({
                    "type": "profile_list",
                    "req_id": 60,
                    "profiles": [profile_json],
                }),
            ),
            (
                ControlMsg::CreateProfile {
                    req_id: 61,
                    name: "Claude Code".to_string(),
                    invocation: "claude".to_string(),
                    agent_kind: AgentKind::Claude,
                    resume_template: Some(vec!["claude".to_string(), "{conversation}".to_string()]),
                },
                serde_json::json!({
                    "type": "create_profile",
                    "req_id": 61,
                    "name": "Claude Code",
                    "invocation": "claude",
                    "agent_kind": "claude",
                    "resume_template": ["claude", "{conversation}"],
                }),
            ),
            (
                ControlMsg::ProfileCreated {
                    req_id: 61,
                    profile: a_profile(),
                },
                serde_json::json!({
                    "type": "profile_created",
                    "req_id": 61,
                    "profile": profile_json,
                }),
            ),
            (
                ControlMsg::UpdateProfile {
                    req_id: 62,
                    profile: a_profile(),
                },
                serde_json::json!({
                    "type": "update_profile",
                    "req_id": 62,
                    "profile": profile_json,
                }),
            ),
            (
                ControlMsg::ProfileUpdated {
                    req_id: 62,
                    profile: a_profile(),
                },
                serde_json::json!({
                    "type": "profile_updated",
                    "req_id": 62,
                    "profile": profile_json,
                }),
            ),
            (
                ControlMsg::DeleteProfile {
                    req_id: 63,
                    profile_id: "prof-7".to_string(),
                },
                serde_json::json!({
                    "type": "delete_profile",
                    "req_id": 63,
                    "profile_id": "prof-7",
                }),
            ),
            (
                ControlMsg::ProfileDeleted { req_id: 63 },
                serde_json::json!({
                    "type": "profile_deleted",
                    "req_id": 63,
                }),
            ),
        ] {
            assert_eq!(serde_json::to_value(&msg).unwrap(), expected);
            let golden_frame = Frame {
                kind: FrameKind::Control,
                channel: 0,
                body: expected.to_string().into_bytes(),
            };
            assert_eq!(crate::io::parse_control(&golden_frame).unwrap(), msg);
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

    /// The bound that makes `ListProfiles` safe to leave unpaginated
    /// (PLAN_M6_75.md item 3): a catalog at BOTH limits — the maximum
    /// number of profiles, each at the maximum field size — must still
    /// encode to a sendable frame, with real headroom rather than a
    /// technical fit.
    ///
    /// Built as an actual `ProfileList` and actually encoded, not as
    /// arithmetic over the constants: JSON escaping, field names, the
    /// per-record braces and the frame header are all real bytes, and a
    /// hand-multiplied estimate is exactly the kind of reasoning that is
    /// wrong by a factor nobody notices until a user's catalog stops
    /// listing. The headroom assertion mirrors `upload_consts_are_pinned`'s
    /// treatment of `UPLOAD_CHUNK_BYTES` for the same reason: a bound that
    /// only just fits leaves nothing for a future field on `Profile`.
    ///
    /// The worst case is deliberately absurd (16 MiB of text spread over
    /// 256 profiles), which is the point of a bound: it holds for a catalog
    /// no user would ever author, so it holds for every catalog they would.
    #[test]
    fn profile_bounds_keep_a_full_catalog_sendable() {
        // Every byte of the per-profile allowance spent on `name`, the one
        // field with no shape of its own to limit it.
        let fattest = |id: usize| Profile {
            id: format!("prof-{id}"),
            name: "x".repeat(PROFILE_FIELD_CAP),
            invocation: String::new(),
            agent_kind: AgentKind::Generic,
            resume_template: None,
        };
        let full = ControlMsg::ProfileList {
            req_id: 1,
            profiles: (0..MAX_PROFILES_PER_HOST).map(fattest).collect(),
        };
        let frame = Frame::control(&full);
        assert!(
            !frame.exceeds_max_len(),
            "a catalog at both bounds must still be sendable, got {} bytes against a \
             {MAX_FRAME_LEN}-byte limit",
            frame.encoded_len()
        );
        assert!(
            frame.encoded_len() < MAX_FRAME_LEN as usize / 2,
            "the bounds must leave real headroom for future Profile fields, not merely fit: \
             {} bytes",
            frame.encoded_len()
        );
    }

    /// A generic profile — no integration, no resume-template override — is
    /// the other half of [`Profile`]'s shape, and the half a fresh
    /// hand-written profile normally has. Pinned separately because both
    /// values are easy to get wrong in the same direction: `agent_kind`
    /// must be the explicit string `"generic"` rather than a null or an
    /// absent key (a profile always states its kind — see that field's
    /// docs), while `resume_template` must be a real `null` — which for
    /// THIS fixture's `Generic` kind means no resume template at all
    /// (there is no integration to derive a default from; see the field's
    /// two-outcome rule).
    #[test]
    fn a_generic_profile_states_its_kind_and_omits_no_field() {
        let profile = Profile {
            id: "prof-8".to_string(),
            name: "my script".to_string(),
            invocation: "./run-agent.sh".to_string(),
            agent_kind: AgentKind::Generic,
            resume_template: None,
        };
        let expected = serde_json::json!({
            "id": "prof-8",
            "name": "my script",
            "invocation": "./run-agent.sh",
            "agent_kind": "generic",
            "resume_template": null,
        });
        assert_eq!(serde_json::to_value(&profile).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<Profile>(expected).unwrap(),
            profile
        );
    }

    /// `SessionInfo::source_profile` and its [`ProfileExistence`] states
    /// (PLAN_M6_75.md item 3), golden-pinned per state through the arm-IS-
    /// the-assertion shape `session_status_json_shapes_are_pinned` uses, so
    /// a fourth existence state cannot be added without pinning its wire
    /// spelling here.
    ///
    /// The nesting is the point: this rides inside every `SessionInfo` on
    /// every reply, so its exact key (`source_profile`) and its bare
    /// snake_case existence string are what a client's filter and its
    /// no-longer-exists rendering both key off.
    #[test]
    fn session_info_source_profile_json_shapes_are_pinned() {
        for existence in [
            ProfileExistence::Present,
            ProfileExistence::Renamed,
            ProfileExistence::Deleted,
        ] {
            let expected_existence = match existence {
                ProfileExistence::Present => "present",
                ProfileExistence::Renamed => "renamed",
                ProfileExistence::Deleted => "deleted",
            };
            let info = SessionInfo {
                parent: None,
                archived: false,
                id: "s1".to_string(),
                title: "demo".to_string(),
                created_at: 1_700_000_000,
                last_activity_at: 1_700_000_000,
                creation_seq: None,
                cwd: "/tmp".to_string(),
                invocation: "claude".to_string(),
                status: SessionStatus::Running,
                annotation: None,
                restart_offer: RestartOffer::default(),
                tabs: Vec::new(),
                source_profile: Some(SourceProfile {
                    id: "prof-7".to_string(),
                    name: "Claude Code".to_string(),
                    existence,
                }),
            };
            let encoded = serde_json::to_value(&info).unwrap();
            assert_eq!(
                encoded["source_profile"],
                serde_json::json!({
                    "id": "prof-7",
                    "name": "Claude Code",
                    "existence": expected_existence,
                })
            );
            // Decode back through the whole `SessionInfo`, not just the
            // nested value: the field's own key is half of what is being
            // pinned, and a rename of it would leave the nested shape
            // perfectly correct and completely unreachable.
            let decoded: SessionInfo = serde_json::from_value(encoded).unwrap();
            assert_eq!(decoded, info);
        }
    }

    /// [`SourceProfile`] is a nested object on `SessionInfo`, so it needs
    /// the same both-directions field tolerance every other nesting level
    /// in this file has been given (`session_list_with_unknown_field_inside_session_decodes_through_parse_control`
    /// for `SessionInfo`, `attach_with_unknown_field_inside_terminal_selector_decodes_through_parse_control`
    /// for `TerminalSelector`, `tab_opened_with_unknown_field_inside_tab_info_decodes_through_parse_control`
    /// for `TabInfo`). Additivity that holds at the outer level and fails
    /// one object down is not additivity — and this object is the one most
    /// likely to grow, since every future fact about a session's origin
    /// belongs in it.
    ///
    /// Both directions, in one test because they are one property:
    ///
    /// - FUTURE SENDER → today's decoder: an unrecognized field inside
    ///   `source_profile` must decode, through the REAL `parse_control`
    ///   path rather than a hand-rolled `from_value::<SourceProfile>`,
    ///   which would say nothing about whether `deny_unknown_fields` had
    ///   crept in anywhere along the chain from frame bytes to
    ///   `SessionInfo`.
    /// - TODAY'S SENDER → future decoder: a later build that grew an
    ///   optional field on this record must read today's bytes and default
    ///   it, rather than refusing a snapshot that predates it.
    #[test]
    fn source_profile_tolerates_unknown_fields_in_both_directions() {
        let frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: serde_json::json!({
                "type": "session_list",
                "req_id": 80,
                "sessions": [
                    {
                        "id": "s1",
                        "title": "demo",
                        "cwd": "/tmp",
                        "invocation": "claude",
                        "status": { "state": "running" },
                        "source_profile": {
                            "id": "prof-7",
                            "name": "Claude Code",
                            "existence": "present",
                            "created_from_host": "value from tomorrow",
                        },
                    }
                ],
                "total": 1,
                "next_cursor": null,
            })
            .to_string()
            .into_bytes(),
        };
        let msg = crate::io::parse_control(&frame).expect(
            "an unknown field nested inside a SourceProfile must decode, not error — an \
             undecodable session list is a whole page of the fleet gone",
        );
        let ControlMsg::SessionList { sessions, .. } = msg else {
            panic!("expected ControlMsg::SessionList, got {msg:?}");
        };
        assert_eq!(
            sessions[0].source_profile,
            Some(SourceProfile {
                id: "prof-7".to_string(),
                name: "Claude Code".to_string(),
                existence: ProfileExistence::Present,
            }),
            "the fields it DOES know must survive alongside the one it ignored"
        );

        // The other direction: a later build's decoder, modelled by a
        // shadow struct with an added optional field.
        #[derive(serde::Deserialize)]
        struct FutureSourceProfile {
            id: String,
            name: String,
            existence: ProfileExistence,
            #[serde(default)]
            deleted_at: Option<i64>,
        }
        let today = serde_json::to_value(SourceProfile {
            id: "prof-7".to_string(),
            name: "Claude Code".to_string(),
            existence: ProfileExistence::Deleted,
        })
        .unwrap();
        let decoded: FutureSourceProfile = serde_json::from_value(today).unwrap();
        assert_eq!(decoded.id, "prof-7");
        assert_eq!(decoded.name, "Claude Code");
        assert_eq!(decoded.existence, ProfileExistence::Deleted);
        assert_eq!(
            decoded.deleted_at, None,
            "an absent future field must default, never fail the decode"
        );
    }

    /// The absent case — which is EVERY session at this bump, since the
    /// supervisor's profile catalog is a later step and nothing is
    /// profile-created yet (see the field's own docs). Two spellings must
    /// mean the same thing: an explicit `null` (what this crate's encoder
    /// produces) and a key that never appears at all (what a sender
    /// predating the field produces).
    ///
    /// This is the tolerance case the whole vocabulary-first shape rests
    /// on: if absence decoded as anything but "raw-created", every session
    /// in the fleet would acquire a phantom profile the day the field
    /// shipped.
    #[test]
    fn an_absent_source_profile_decodes_as_raw_created_either_spelling() {
        let with_null = serde_json::json!({
            "id": "s1",
            "title": "demo",
            "created_at": 1_700_000_000,
            "last_activity_at": 1_700_000_000,
            "cwd": "/tmp",
            "invocation": "agent",
            "status": { "state": "running" },
            "annotation": null,
            "restart_offer": "fresh_only",
            "tabs": [],
            "source_profile": null,
        });
        let decoded: SessionInfo = serde_json::from_value(with_null).unwrap();
        assert_eq!(decoded.source_profile, None);

        // Hand-written so the key's ABSENCE is real, not an explicit null
        // that merely renders the same in this crate's own serializer —
        // the same reason
        // `hello_json_decodes_with_host_identity_key_entirely_absent`
        // exists.
        let raw = r#"{
            "id": "s1",
            "title": "demo",
            "cwd": "/tmp",
            "invocation": "agent",
            "status": {"state": "running"}
        }"#;
        let decoded: SessionInfo = serde_json::from_str(raw)
            .expect("a SessionInfo with no source_profile key at all must still decode");
        assert_eq!(
            decoded.source_profile, None,
            "an absent key must read as raw-created, exactly as an explicit null does"
        );
    }

    /// Version 10's additive rule for FIELDS, both directions, applied to
    /// the profile family — mirroring
    /// `rename_session_with_future_extra_field_decodes_through_parse_control`
    /// and `current_rename_session_decodes_under_a_future_v7_decoder_with_defaults`
    /// for version 7's vocabulary, and pinned on ONE representative variant
    /// for their reason: unknown-field tolerance is a property of the
    /// enum's derive (no `deny_unknown_fields` anywhere on this path), not
    /// of any single variant.
    ///
    /// `UpdateProfile` is the representative because it nests a whole
    /// [`Profile`], which makes this cover the nesting level too — the
    /// thing `session_list_with_unknown_field_inside_session_decodes_through_parse_control`
    /// had to be written separately to cover for `SessionInfo`.
    #[test]
    fn profile_messages_tolerate_unknown_fields_in_both_directions() {
        // Future sender -> current decoder, with the unknown field nested
        // inside the `Profile` object rather than beside it.
        let frame = Frame {
            kind: FrameKind::Control,
            channel: 0,
            body: serde_json::json!({
                "type": "update_profile",
                "req_id": 70,
                "profile": {
                    "id": "prof-7",
                    "name": "Claude Code",
                    "invocation": "claude",
                    "agent_kind": "claude",
                    "resume_template": ["claude", "{conversation}"],
                    "initial_prompt": "value from tomorrow",
                },
                "dry_run": true,
            })
            .to_string()
            .into_bytes(),
        };
        let msg = crate::io::parse_control(&frame)
            .expect("unknown fields, nested or not, must decode rather than error");
        assert_eq!(
            msg,
            ControlMsg::UpdateProfile {
                req_id: 70,
                profile: a_profile(),
            }
        );

        // Current sender -> future decoder: a later build that grew an
        // optional field must accept today's bytes and default it.
        #[derive(serde::Deserialize)]
        struct FutureProfile {
            id: String,
            name: String,
            #[serde(default)]
            shared_with_hosts: Option<Vec<String>>,
        }
        let mut wire = Vec::new();
        Frame::control(&ControlMsg::ProfileCreated {
            req_id: 71,
            profile: a_profile(),
        })
        .encode(&mut wire)
        .unwrap();
        let (frame, _) = Frame::decode(&wire).unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&frame.body).unwrap();
        assert_eq!(value["type"], "profile_created");
        let decoded: FutureProfile = serde_json::from_value(value["profile"].clone()).unwrap();
        assert_eq!(decoded.id, "prof-7");
        assert_eq!(decoded.name, "Claude Code");
        assert_eq!(
            decoded.shared_with_hosts, None,
            "an absent future field must default, never fail the decode"
        );
    }

    /// The profile family's own version-skew half: a decoder shaped like a
    /// genuine v9 peer — which knew no profile messages at all — must FAIL
    /// on every one of them rather than ignoring it, exactly as
    /// `unknown_control_message_tag_fails_decode` pins for any unrecognized
    /// tag. This is what makes "eight new tagged variants earn the bump" a
    /// tested claim rather than a stated one; a tolerated no-op here would
    /// mean a v9 supervisor silently swallowing profile CRUD and answering
    /// nothing, hanging the caller.
    ///
    /// The POSITIVE CONTROL comes first, and it is what makes the rest of
    /// this test mean anything: a shadow decoder that rejected everything —
    /// a typo in its `rename_all`, a wrong tag name — would satisfy every
    /// `expect_err` below while proving nothing at all. So the fixture is
    /// first shown to decode a message the modelled peer DID know, with its
    /// `req_id` read back out, before being asked to refuse the ones it did
    /// not.
    #[test]
    fn profile_messages_fail_under_a_decoder_that_predates_them() {
        // Scoped to this test and named for what it models: a decoder that
        // knows the v9 message set and nothing else. Distinct from the
        // module-level `LegacyV9ControlMsg`, which models the same peer's
        // view of ONE message's fields rather than of the message set.
        #[derive(Debug, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum V9MessageSet {
            SessionStopped { req_id: u64 },
        }

        let known = serde_json::to_value(ControlMsg::SessionStopped { req_id: 99 }).unwrap();
        let V9MessageSet::SessionStopped { req_id } = serde_json::from_value(known)
            .expect("control: this decoder must accept a message the v9 peer really knew");
        assert_eq!(
            req_id, 99,
            "and must read its fields, not merely match the tag — a decoder that accepted the \
             shape while losing the contents would pass the refusals below just as vacuously"
        );

        for msg in [
            ControlMsg::ListProfiles { req_id: 1 },
            ControlMsg::ProfileList {
                req_id: 1,
                profiles: vec![a_profile()],
            },
            ControlMsg::CreateProfile {
                req_id: 2,
                name: "n".to_string(),
                invocation: "i".to_string(),
                agent_kind: AgentKind::Generic,
                resume_template: None,
            },
            ControlMsg::ProfileCreated {
                req_id: 2,
                profile: a_profile(),
            },
            ControlMsg::UpdateProfile {
                req_id: 3,
                profile: a_profile(),
            },
            ControlMsg::ProfileUpdated {
                req_id: 3,
                profile: a_profile(),
            },
            ControlMsg::DeleteProfile {
                req_id: 4,
                profile_id: "prof-7".to_string(),
            },
            ControlMsg::ProfileDeleted { req_id: 4 },
        ] {
            let json = serde_json::to_value(&msg).unwrap();
            serde_json::from_value::<V9MessageSet>(json).expect_err(
                "a decoder predating the profile vocabulary must reject it, not ignore it",
            );
        }
    }

    /// `OpenTab`/`TabOpened` and `CloseTab`/`TabClosed` are PLAN_M4.md item
    /// 1's tab-lifecycle wire additions, golden-pinned the same way
    /// `stop_and_delete_json_shapes_are_pinned` pins the M2 session-
    /// lifecycle pair: a serde attribute drift here would compile and
    /// round-trip clean while quietly producing bytes an unmodified peer
    /// cannot parse. A (message, expected JSON) table rather than four
    /// separately-named locals, since each is the identical
    /// construct-then-assert shape.
    #[test]
    fn tab_open_and_close_json_shapes_are_pinned() {
        for (msg, expected) in [
            (
                ControlMsg::OpenTab {
                    req_id: 20,
                    session_id: "s1".to_string(),
                },
                serde_json::json!({
                    "type": "open_tab",
                    "req_id": 20,
                    "session_id": "s1",
                }),
            ),
            (
                ControlMsg::TabOpened {
                    req_id: 20,
                    tab: TabInfo {
                        id: "t1".to_string(),
                    },
                },
                serde_json::json!({
                    "type": "tab_opened",
                    "req_id": 20,
                    "tab": { "id": "t1" },
                }),
            ),
            (
                ControlMsg::CloseTab {
                    req_id: 21,
                    session_id: "s1".to_string(),
                    tab_id: "t1".to_string(),
                },
                serde_json::json!({
                    "type": "close_tab",
                    "req_id": 21,
                    "session_id": "s1",
                    "tab_id": "t1",
                }),
            ),
            (
                ControlMsg::TabClosed { req_id: 21 },
                serde_json::json!({
                    "type": "tab_closed",
                    "req_id": 21,
                }),
            ),
        ] {
            assert_eq!(serde_json::to_value(&msg).unwrap(), expected);
        }
    }

    /// Round-trip every tab-lifecycle variant through the real
    /// encode/decode path (not just `serde_json::to_value`), matching how
    /// `stop_and_delete_roundtrip_through_frames` exercises the M2
    /// session-lifecycle pair — this is what would catch a drift between
    /// the codec's framing and serde's JSON shape, which the pure-JSON
    /// test just above cannot see.
    #[test]
    fn tab_open_and_close_roundtrip_through_frames() {
        for msg in [
            ControlMsg::OpenTab {
                req_id: 1,
                session_id: "s1".to_string(),
            },
            ControlMsg::TabOpened {
                req_id: 1,
                tab: TabInfo {
                    id: "t1".to_string(),
                },
            },
            ControlMsg::CloseTab {
                req_id: 2,
                session_id: "s1".to_string(),
                tab_id: "t1".to_string(),
            },
            ControlMsg::TabClosed { req_id: 2 },
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

    /// The full attachment-upload control vocabulary (PLAN_M4.md items 1
    /// and 4), golden-pinned the same way the tab-lifecycle pair above
    /// is: every variant's exact JSON shape, since a serde attribute
    /// drift on any one of them would compile and round-trip cleanly
    /// while quietly producing bytes an unmodified peer cannot parse. A
    /// (message, expected JSON) table rather than seven separately-named
    /// locals, since each is the identical construct-then-assert shape.
    /// `UploadAborted`'s `reason` uses [`UPLOAD_ABORT_REASON_STALLED`]
    /// itself rather than a hand-typed string, but the expected JSON below
    /// still spells the string out literally (mirroring
    /// `stall_detach_reason_json_shape_is_pinned`'s treatment of
    /// `DETACH_REASON_STALLED`) so an accidental edit to the constant
    /// fails this test instead of silently changing what ships.
    #[test]
    fn upload_control_json_shapes_are_pinned() {
        for (msg, expected) in [
            (
                ControlMsg::BeginUpload {
                    req_id: 30,
                    session_id: "s1".to_string(),
                    channel: 5,
                    filename: "screenshot.png".to_string(),
                    size: 12345,
                },
                serde_json::json!({
                    "type": "begin_upload",
                    "req_id": 30,
                    "session_id": "s1",
                    "channel": 5,
                    "filename": "screenshot.png",
                    "size": 12345,
                }),
            ),
            (
                ControlMsg::UploadStarted {
                    req_id: 30,
                    channel: 5,
                },
                serde_json::json!({
                    "type": "upload_started",
                    "req_id": 30,
                    "channel": 5,
                }),
            ),
            (
                ControlMsg::UploadAck {
                    channel: 5,
                    received: 4096,
                },
                serde_json::json!({
                    "type": "upload_ack",
                    "channel": 5,
                    "received": 4096,
                }),
            ),
            (
                ControlMsg::CommitUpload {
                    req_id: 31,
                    channel: 5,
                },
                serde_json::json!({
                    "type": "commit_upload",
                    "req_id": 31,
                    "channel": 5,
                }),
            ),
            (
                ControlMsg::UploadCommitted {
                    req_id: 31,
                    path: "/data/sessions/s1/attachments/screenshot.png".to_string(),
                },
                serde_json::json!({
                    "type": "upload_committed",
                    "req_id": 31,
                    "path": "/data/sessions/s1/attachments/screenshot.png",
                }),
            ),
            (
                ControlMsg::AbortUpload { channel: 5 },
                serde_json::json!({
                    "type": "abort_upload",
                    "channel": 5,
                }),
            ),
            (
                ControlMsg::UploadAborted {
                    channel: 5,
                    reason: UPLOAD_ABORT_REASON_STALLED.to_string(),
                },
                serde_json::json!({
                    "type": "upload_aborted",
                    "channel": 5,
                    "reason": "transfer stopped making progress (stalled)",
                }),
            ),
        ] {
            assert_eq!(serde_json::to_value(&msg).unwrap(), expected);
        }
    }

    /// Round-trip every upload variant through the real encode/decode
    /// path, matching `tab_open_and_close_roundtrip_through_frames`'s
    /// treatment of the tab-lifecycle pair — the deserialize-direction
    /// half of the golden test above.
    #[test]
    fn upload_control_roundtrip_through_frames() {
        for msg in [
            ControlMsg::BeginUpload {
                req_id: 1,
                session_id: "s1".to_string(),
                channel: 1,
                filename: "a.txt".to_string(),
                size: 10,
            },
            ControlMsg::UploadStarted {
                req_id: 1,
                channel: 1,
            },
            ControlMsg::UploadAck {
                channel: 1,
                received: 5,
            },
            ControlMsg::CommitUpload {
                req_id: 2,
                channel: 1,
            },
            ControlMsg::UploadCommitted {
                req_id: 2,
                path: "/tmp/a.txt".to_string(),
            },
            ControlMsg::AbortUpload { channel: 1 },
            ControlMsg::UploadAborted {
                channel: 1,
                reason: UPLOAD_ABORT_REASON_STALLED.to_string(),
            },
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

    /// `BeginUpload::size` and `UploadAck::received` are `u64` specifically
    /// so PLAN_M4.md's "no size cap in v1" promise is not secretly bounded
    /// by a 32-bit wire representation. A value that only fits inside a
    /// `u64` — here, one past `u32::MAX` — golden-pinned and round-tripped
    /// through the real frame codec is what would catch an accidental
    /// narrowing (a serde attribute, or a change to either field's type)
    /// that silently truncated a multi-gigabyte upload's declared or
    /// acknowledged size.
    #[test]
    fn upload_size_and_received_survive_values_above_u32_max() {
        let big: u64 = u32::MAX as u64 + 1;

        let begin = ControlMsg::BeginUpload {
            req_id: 40,
            session_id: "s1".to_string(),
            channel: 7,
            filename: "huge.bin".to_string(),
            size: big,
        };
        assert_eq!(
            serde_json::to_value(&begin).unwrap()["size"],
            serde_json::json!(big)
        );
        let mut wire = Vec::new();
        Frame::control(&begin).encode(&mut wire).unwrap();
        let (frame, used) = Frame::decode(&wire).unwrap().unwrap();
        assert_eq!(used, wire.len());
        let ControlMsg::BeginUpload { size, .. } =
            serde_json::from_slice::<ControlMsg>(&frame.body).unwrap()
        else {
            panic!("expected ControlMsg::BeginUpload");
        };
        assert_eq!(
            size, big,
            "size must survive the wire exactly, not truncate"
        );

        let ack = ControlMsg::UploadAck {
            channel: 7,
            received: big,
        };
        assert_eq!(
            serde_json::to_value(&ack).unwrap()["received"],
            serde_json::json!(big)
        );
        let mut wire = Vec::new();
        Frame::control(&ack).encode(&mut wire).unwrap();
        let (frame, used) = Frame::decode(&wire).unwrap().unwrap();
        assert_eq!(used, wire.len());
        let ControlMsg::UploadAck { received, .. } =
            serde_json::from_slice::<ControlMsg>(&frame.body).unwrap()
        else {
            panic!("expected ControlMsg::UploadAck");
        };
        assert_eq!(
            received, big,
            "received must survive the wire exactly, not truncate"
        );
    }

    /// PLAN_M4.md item 1's three upload constants, pinned the same way
    /// `PROTOCOL_VERSION` is above: `UPLOAD_CHUNK_BYTES` and
    /// `UPLOAD_WINDOW_BYTES` are wire-visible sizing decisions every
    /// sender's framing math depends on, and `UPLOAD_ABORT_REASON_STALLED`
    /// is matched verbatim by client UI and by both of its emitters (see
    /// the constant's own doc comment) — an accidental edit to any of the
    /// three must fail loudly here rather than drift silently. The
    /// headroom assertion is the const's own reason to exist: a chunk
    /// size too close to `MAX_FRAME_LEN` would leave no room for the
    /// frame header before an upload chunk alone could violate the
    /// bounded-frame rule. It compares entirely in `usize` and adds the
    /// 5-byte per-frame header (`kind` plus `channel`, the same
    /// accounting `frame_size_boundary_accepts_the_maximum_and_rejects_one_more`
    /// uses) rather than casting `UPLOAD_CHUNK_BYTES` down to `u32` and
    /// comparing bare byte counts — an earlier version of this test did
    /// the cast, which would silently truncate and pass for a chunk
    /// constant large enough to overflow `u32`, and omitted the header,
    /// which understates how much of a frame a chunk actually occupies.
    #[test]
    fn upload_consts_are_pinned() {
        assert_eq!(UPLOAD_CHUNK_BYTES, 256 * 1024);
        assert!(
            UPLOAD_CHUNK_BYTES + 5 < MAX_FRAME_LEN as usize / 2,
            "a chunk plus its frame header must leave real headroom below MAX_FRAME_LEN, not \
             just technically fit"
        );
        assert_eq!(UPLOAD_WINDOW_BYTES, 4 * 1024 * 1024);
        assert_eq!(
            UPLOAD_ABORT_REASON_STALLED,
            "transfer stopped making progress (stalled)"
        );
    }

    /// [`ControlMsg::reply_req_id`] is what every demultiplexer routes
    /// replies by, and getting its answer wrong has two failure modes that
    /// are both silent at the call site: a reply variant it forgot returns
    /// `None` and the caller waits forever, while a request variant it
    /// admitted lets a peer's echo of a request be delivered as that
    /// request's answer.
    ///
    /// Deliberately a handful of representative cases rather than a value
    /// of every variant. COMPLETENESS is not this test's job and cannot be:
    /// any fixture list here would be a second copy of the enum, kept in
    /// step by the same discipline it claims to enforce. The accessor's own
    /// match is exhaustive with no wildcard, so a new variant is a compile
    /// error at the classification site — the guarantee is structural, and
    /// what is left to assert is that the classification MEANS what the
    /// docs say: the three groups map to Some/None as intended, and a
    /// request is not merely absent from the reply list but actively
    /// answered `None` while carrying a `req_id` of its own.
    #[test]
    fn reply_req_id_is_some_for_exactly_the_reply_variants() {
        // Replies: the `req_id` comes back verbatim, including `Error`'s.
        assert_eq!(
            ControlMsg::SessionStopped { req_id: 7 }.reply_req_id(),
            Some(7)
        );
        assert_eq!(ControlMsg::TabClosed { req_id: 9 }.reply_req_id(), Some(9));
        assert_eq!(
            ControlMsg::ProfileDeleted { req_id: 10 }.reply_req_id(),
            Some(10)
        );
        assert_eq!(
            ControlMsg::UploadStarted {
                req_id: 11,
                channel: 1
            }
            .reply_req_id(),
            Some(11)
        );
        assert_eq!(
            ControlMsg::Error {
                req_id: 13,
                message: "boom".to_string(),
                kind: ErrorKind::Internal,
            }
            .reply_req_id(),
            Some(13)
        );

        // Requests carry a `req_id` too, and must still answer `None` — a
        // peer echoing one back must never complete the pending request it
        // names.
        for echoed in [
            ControlMsg::StopSession {
                req_id: 7,
                session_id: "s1".to_string(),
            },
            ControlMsg::CommitUpload {
                req_id: 11,
                channel: 1,
            },
            ControlMsg::DeleteProfile {
                req_id: 10,
                profile_id: "prof-7".to_string(),
            },
        ] {
            assert_eq!(
                echoed.reply_req_id(),
                None,
                "a request echoed back must never be routed as a reply: {echoed:?}"
            );
        }

        // Unsolicited, channel-correlated events have no request to answer.
        assert_eq!(ControlMsg::Detach { channel: 1 }.reply_req_id(), None);
        assert_eq!(
            ControlMsg::UploadAck {
                channel: 1,
                received: 3
            }
            .reply_req_id(),
            None
        );
        assert_eq!(ControlMsg::hello("supervisor").reply_req_id(), None);
    }
}
