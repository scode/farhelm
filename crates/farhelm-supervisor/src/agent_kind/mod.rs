//! Agent-kind integrations: everything the supervisor knows about the
//! SPECIFIC agents it launches, behind one seam so that knowledge cannot
//! leak into the rest of the process.
//!
//! `AgentKind` (farhelm-proto's enum) merely NAMES a kind.
//! [`AgentIntegration`] is what knowing the kind buys, and by M6.75 it
//! buys three different things:
//!
//! 1. **The kind seam and its per-session snapshot** (PLAN_M3.md item 7).
//!    At create time a session records, immutably, which agent kind it is
//!    and how a resume would be invoked. Derivation is honestly dumb — the
//!    basename of the invocation's first token — and is done ONCE, never
//!    re-guessed later. Doing it once is not caching but stability:
//!    re-deriving later would consult a PATH, a filesystem, and a heuristic
//!    that may all have changed since, so a session could silently become a
//!    different kind between two restarts and resume through a template
//!    that never matched the agent actually running.
//! 2. **Conversation-identity capture** (item 8). Both supported agents
//!    write discoverable on-disk records; the supervisor reads them and
//!    nothing else. Capture is observation-only per SPEC.md — no hooks, no
//!    agent configuration, no file in the agent's own directories is ever
//!    written.
//! 3. **Status sharpening** (PLAN_M6_75.md item 2). The generic classifier
//!    in `service::status` can only see whether a pane produced output
//!    recently; recognizing that an agent is BLOCKED ON A QUESTION means
//!    recognizing that agent's own prompt and approval shapes, which is
//!    per-kind knowledge and nothing else's business.
//!
//! ## Where the line between this file and `capture` is drawn
//!
//! The submodule is not "the second half". The split is by AXIS: `capture`
//! holds what is the same no matter which agent wrote the record — window
//! arithmetic, the bounded directory walk and its budgets, the ambiguity
//! rule, timestamp parsing — and this file holds everything a reader has to
//! check against a VENDOR. That is why [`AgentIntegration::parse_record`]
//! is here while `scan_records` is not, and why a new agent kind is an
//! `impl` in this file rather than an edit spread across both.
//!
//! It also means the two files fail differently, which is worth knowing
//! before touching either. A bug in `capture` is a correctness bug about
//! which conversation gets resumed; a bug here is usually a bug about
//! whether this build still recognizes what the vendor currently emits —
//! silent, version-dependent, and fixed by re-auditing the agent rather
//! than by reasoning about the code.
//!
//! ## The audited constraints this module is shaped by
//!
//! SPEC_impl.md's "Supervisor internals" records four facts about the real
//! agents that were established by audit, and each one shows up here as a
//! design decision rather than as a comment:
//!
//! - **The record appears at first prompt submission, not at launch.** So
//!   correlation keys on FIRST-INPUT time ([`CaptureWindow`]), and the
//!   launch-to-first-input gap is unbounded and simply tolerated. There is
//!   no timeout anywhere in this module measured from a session's creation.
//! - **The cwd munging is non-injective** (`/`, `.`, and `_` all become
//!   `-`). So the munged directory name is only ever used to FIND candidate
//!   files cheaply; whether a record belongs to a session is decided by the
//!   `cwd` FIELD inside it ([`RecordCorrelators::cwd`]), never by the
//!   directory it was found in.
//! - **Per-line JSON fields are the reliable correlators.** File birth
//!   times can postdate content after rewrites, so nothing here derives a
//!   record's creation time from the filesystem; the timestamp comes out of
//!   the record's own leading JSON. Filesystem mtime is used, but only as
//!   a monotone LOWER BOUND that lets a scan skip files it could not
//!   possibly need to open (see [`scan_records`]) — never as an answer.
//! - **A plain resume appends under the same id; a new id appears only on
//!   an explicit fork.** So an append is treated as a re-verification
//!   signal ([`read_record`]) rather than as a new conversation, and a
//!   fork's new file never displaces an identity already claimed.
//!
//! ## Sharpening is allowed to be wrong; capture is not
//!
//! The two halves have OPPOSITE failure economics, and reading one with the
//! other's instincts is the mistake this section exists to prevent.
//!
//! Capture's uncertainty is unrecoverable (resuming the wrong conversation
//! is silent and permanent), so it refuses to guess at all — see
//! `capture`'s own docs. Sharpening's uncertainty is a badge in a list:
//! SPEC.md fixes the waiting/idle boundary as heuristic BY CONTRACT and
//! forbids anything about interaction from waiting on a status, so a
//! sharpener that misses a prompt costs a session that reads idle while it
//! waits, and one that fires early costs the reverse. Both are cosmetic.
//! What neither is allowed to do is cost anything else, which is why
//! [`AgentIntegration::sharpen`] takes a plain `&str`, returns a status,
//! and is called from a place that has no lock, no I/O, and no way to
//! delay a keystroke.

use farhelm_proto::{AgentKind, RestartOffer, SessionStatus};
use std::path::{Path, PathBuf};

mod capture;
pub use capture::{
    CAPTURE_PUBLICATION_GRACE, CAPTURE_WINDOW_AFTER, CAPTURE_WINDOW_BEFORE, Candidate,
    CaptureVerdict, CaptureWindow, CaptureWindowBounds, RecordCorrelators, RecordStamp,
    ScanOutcome, choose, format_rfc3339, now_unix, parse_rfc3339, read_record, scan_records,
    stamp_of,
};

/// The one argv element a resume template may use to mean "substitute the
/// captured conversation identity here".
///
/// Matched by EXACT, whole-element equality — never as a substring — which
/// is PR3's wire contract (`ControlMsg::CreateSession::resume_template`'s
/// own docs) and is what keeps `--resume={conversation}` from silently
/// looking like it works. A structural argv vector is also why quoting
/// never enters into it: a path with spaces survives as one element.
pub const CONVERSATION_PLACEHOLDER: &str = "{conversation}";

/// How much of a record file is read while looking for its correlators.
///
/// Both agents put the identifying fields in the record's first line, and
/// a long-running conversation's file grows without bound — so reading the
/// whole thing to learn something the first kilobyte already said would
/// make every rescan proportional to conversation length. A record whose
/// correlators are not inside this prefix is a PARSE FAILURE, which marks
/// the whole scan incomplete rather than quietly dropping one candidate:
/// see the module docs on why incomplete evidence may not produce a claim.
const RECORD_PREFIX_BYTES: usize = 64 * 1024;

/// How many leading lines of a record are examined for correlators. Same
/// bound as [`RECORD_PREFIX_BYTES`] from the other direction — a file of
/// many tiny lines must not turn a scan into a JSON-parsing marathon.
const RECORD_PREFIX_LINES: usize = 64;

/// Longest conversation identifier this module will retain.
///
/// A record's id comes off disk rather than from this process, and it ends
/// up in a durable column, in log lines, and eventually on an agent's
/// command line. Both vendors use UUIDs; 128 bytes is generous headroom
/// while still being a bound. An id over it is a parse failure, which —
/// like every other parse failure — marks the scan incomplete rather than
/// silently dropping a candidate that might have been the ambiguity.
const MAX_CONVERSATION_ID_LEN: usize = 128;

/// One agent's knowledge of itself: where its conversation records live,
/// how to read them, how a resume is invoked, and what its screen looks
/// like when it is waiting for a human — SPEC_impl.md's `AgentKind` trait.
///
/// Object-safe and implemented by unit structs with `'static` instances
/// ([`integration_for`]) because there is nothing per-session to carry: a
/// session's own state (cwd, first-input time, captured identity, sampled
/// tail) lives with the session, and what remains here is pure per-KIND
/// knowledge.
///
/// Every method except [`AgentIntegration::sharpen`] is required, and that
/// asymmetry is the contract SPEC_impl.md's two halves imply: an
/// integration that cannot say where its records live has no business
/// existing, while one that cannot recognize its own prompts is simply an
/// agent whose status stays at the generic baseline. See `sharpen`'s own
/// docs for why the default is "no sharpening" and never "no status".
pub trait AgentIntegration: Send + Sync {
    /// The resume invocation this kind gets by default, built from the
    /// session's ORIGINAL first token rather than from a bare command
    /// name: a session launched as `/opt/bin/claude` must resume through
    /// `/opt/bin/claude`, because that is the binary the user chose and
    /// possibly the only one reachable from the supervisor's own service
    /// environment.
    fn default_resume_template(&self, argv0: &str) -> Vec<String>;

    /// The directory beneath which records for `canonical_cwd` can be
    /// found. For Claude this is the munged-cwd project directory; for
    /// Codex it is the whole (date-nested) rollout tree, since Codex does
    /// not partition by working directory at all.
    fn record_root(&self, home: &Path, canonical_cwd: &str) -> PathBuf;

    /// How many directory levels below [`AgentIntegration::record_root`]
    /// records may be nested. Bounds the walk so a stray deep tree cannot
    /// turn a rescan into a filesystem crawl.
    fn record_depth(&self) -> usize;

    /// Whether a file name could be a record at all — a cheap pre-filter
    /// applied before anything is opened.
    fn is_record_file(&self, name: &str) -> bool;

    /// Pull the correlators out of a record's leading text.
    ///
    /// `Ok(None)` means "this is a well-formed file that is positively not
    /// a record of mine"; anything the implementation cannot make sense of
    /// is an `Err`, which marks the whole scan incomplete. The asymmetry
    /// is the module's no-guessing rule applied at the parse boundary: a
    /// file this build cannot read might be the second candidate that
    /// should have forced an ambiguity bail.
    ///
    /// `text` is a bounded PREFIX of the file (see [`RECORD_PREFIX_BYTES`]),
    /// not necessarily the whole of it, and may end mid-line — so an
    /// implementation must tolerate a truncated final line rather than
    /// treating it as corruption.
    fn parse_record(&self, text: &str) -> anyhow::Result<Option<RecordCorrelators>>;

    /// Refine the generic activity classification for one session using
    /// the last screen the sampler captured from its pane (PLAN_M6_75.md
    /// item 2).
    ///
    /// `baseline` is what `service::status` concluded from output recency
    /// alone — `Running` or `Idle`, never a dead status, because a session
    /// with no live pane is never sharpened. `tail` is the pane's visible
    /// grid as of the last tick (`ticker::ActivitySample::tail`), bottom-
    /// anchored and lossily decoded.
    ///
    /// ## What an implementation may do
    ///
    /// Exactly one thing is worth doing here and it is the whole point of
    /// the method: PROMOTE a live baseline to
    /// [`SessionStatus::Waiting`](farhelm_proto::SessionStatus::Waiting)
    /// when the tail shows this agent's own unanswered question or approval
    /// prompt, or leave `baseline` alone. Returning a status that is not
    /// live at all is not merely discouraged — `service::status` DISCARDS
    /// it and keeps the baseline, because a sharpener is looking at a
    /// screen and a screen is not evidence that a process ended. Nothing
    /// here may invent liveness in either direction.
    ///
    /// ## Why it is defaulted, and what the default means
    ///
    /// The default returns `baseline` unchanged, so adding a kind never
    /// silently costs it a status. That matters because the OTHER shape —
    /// `integration_for` returning `None` — already carries a meaning:
    /// [`AgentKind::Generic`] has no integration at all, and generic
    /// sessions still get the baseline classification. "No sharpening" and
    /// "no status" must therefore be different things, and a required
    /// method would have made every new integration write a stub that
    /// looked like a contract.
    ///
    /// ## Robustness
    ///
    /// Wrong is cosmetic (this module's own docs); PANICKING is not.
    /// `tail` is arbitrary terminal output that survived a lossy UTF-8
    /// decode, so it can contain control bytes, replacement characters,
    /// half-drawn escape sequences, and multi-byte characters at any
    /// offset. An implementation must therefore never index or slice by
    /// byte offset — this is a classification running on the reply path of
    /// a supervisor that is also serving live terminals.
    fn sharpen(&self, baseline: SessionStatus, tail: &str) -> SessionStatus {
        let _ = tail;
        baseline
    }
}

/// The integration for a kind, or `None` for [`AgentKind::Generic`] —
/// which is not an omission but the definition of generic: no record
/// location, no correlators, and therefore no capture, ever.
pub fn integration_for(kind: AgentKind) -> Option<&'static dyn AgentIntegration> {
    match kind {
        AgentKind::Claude => Some(&ClaudeIntegration),
        AgentKind::Codex => Some(&CodexIntegration),
        AgentKind::Generic => None,
    }
}

/// Claude Code: one JSONL record per conversation, under a project
/// directory named after the munged working directory.
struct ClaudeIntegration;

/// Codex: one JSONL rollout file per conversation, under a date-nested
/// sessions tree that is NOT partitioned by working directory.
struct CodexIntegration;

impl AgentIntegration for ClaudeIntegration {
    fn default_resume_template(&self, argv0: &str) -> Vec<String> {
        vec![
            argv0.to_string(),
            "--resume".to_string(),
            CONVERSATION_PLACEHOLDER.to_string(),
        ]
    }

    fn record_root(&self, home: &Path, canonical_cwd: &str) -> PathBuf {
        home.join(".claude")
            .join("projects")
            .join(munge_cwd(canonical_cwd))
    }

    fn record_depth(&self) -> usize {
        0
    }

    fn is_record_file(&self, name: &str) -> bool {
        name.ends_with(".jsonl")
    }

    /// Claude puts `sessionId`, `cwd`, and `timestamp` at the TOP level of
    /// every line, so the first line carrying all three answers all three
    /// questions at once. Lines are scanned rather than only the first
    /// taken because a record can legitimately open with a line that
    /// carries only some of them (a summary or a meta entry) — and a line
    /// missing one of the three CONTINUES to the next rather than failing
    /// the file, since that is the ordinary shape rather than corruption.
    ///
    /// What does fail: a file whose prefix contains no such line at all
    /// (`Ok(None)` would claim positively that this is not a Claude
    /// record, which no amount of a 64 KiB prefix can establish), and a
    /// line whose fields are present but unusable — an unparseable
    /// timestamp or an implausible id. Both mark the scan incomplete.
    fn parse_record(&self, text: &str) -> anyhow::Result<Option<RecordCorrelators>> {
        for line in leading_json_lines(text) {
            let Some(object) = line.as_object() else {
                continue;
            };
            let (Some(conversation), Some(cwd), Some(timestamp)) = (
                object.get("sessionId").and_then(|v| v.as_str()),
                object.get("cwd").and_then(|v| v.as_str()),
                object.get("timestamp").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            return Ok(Some(correlators_from(conversation, cwd, timestamp)?));
        }
        anyhow::bail!(
            "no line in this file's first {RECORD_PREFIX_BYTES} bytes carries Claude's \
             sessionId/cwd/timestamp correlators"
        )
    }

    /// Claude Code asks for permission through a bordered dialog at the
    /// bottom of the screen: a question line, then a numbered list of
    /// answers. [`looks_like_a_choice_prompt`] is that shape, and
    /// [`CLAUDE_QUESTION_PHRASES`] is the vocabulary it is required to
    /// appear with.
    fn sharpen(&self, baseline: SessionStatus, tail: &str) -> SessionStatus {
        if looks_like_a_choice_prompt(tail, CLAUDE_QUESTION_PHRASES) {
            return SessionStatus::Waiting;
        }
        baseline
    }
}

impl AgentIntegration for CodexIntegration {
    /// `codex resume <id>`, the audited shape — a SUBCOMMAND rather than a
    /// flag, which is exactly why the default template is per-kind
    /// knowledge instead of one shared string with the command swapped in.
    fn default_resume_template(&self, argv0: &str) -> Vec<String> {
        vec![
            argv0.to_string(),
            "resume".to_string(),
            CONVERSATION_PLACEHOLDER.to_string(),
        ]
    }

    /// Every session, regardless of working directory: Codex partitions
    /// its rollout files by DATE, not by cwd, so there is no per-cwd
    /// directory to narrow to and `canonical_cwd` is unused here.
    /// Narrowing falls entirely to the recorded `cwd` field plus the mtime
    /// lower bound — which is also why `service`'s scan cache is keyed on
    /// the ROOT PATH: every Codex session on a host shares this one root
    /// and must not scan it once each.
    fn record_root(&self, home: &Path, _canonical_cwd: &str) -> PathBuf {
        home.join(".codex").join("sessions")
    }

    /// `YYYY/MM/DD` beneath the sessions root.
    fn record_depth(&self) -> usize {
        3
    }

    fn is_record_file(&self, name: &str) -> bool {
        name.ends_with(".jsonl")
    }

    /// Only a `session_meta` line is read, and its identity fields must
    /// come from ONE schema level.
    ///
    /// Both restrictions are about not fabricating a record out of parts.
    /// A rollout file carries many event types, several of which carry a
    /// `cwd` or an `id` of their own meaning something else entirely;
    /// accepting "any line with an id, a cwd and a timestamp" would let an
    /// arbitrary event supply a conversation identity. And taking `id`
    /// from the nested payload while taking `cwd` from the top level (or
    /// the reverse) would assemble a correlator pair that no single record
    /// ever asserted.
    ///
    /// The flat spelling — `id` and `cwd` directly on a `session_meta`
    /// line — is accepted alongside the audited nested one. Honestly, that
    /// is forward-tolerance for a vendor that promotes those fields, not a
    /// shape observed in the wild: the audited form nests them under
    /// `payload`. It is kept because the failure it guards against is
    /// silent (capture would simply stop happening, with no error
    /// anywhere), and it cannot admit a foreign record, since
    /// `type == "session_meta"` is still required.
    ///
    /// A `session_meta` line lacking a usable timestamp CONTINUES to the
    /// next line rather than failing the file: a rollout may legitimately
    /// open with a meta line whose timestamp lives elsewhere, and aborting
    /// there would hide the real meta line further down.
    fn parse_record(&self, text: &str) -> anyhow::Result<Option<RecordCorrelators>> {
        for line in leading_json_lines(text) {
            let Some(object) = line.as_object() else {
                continue;
            };
            if object.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
                continue;
            }
            // One level or the other, never a mixture: the nested payload
            // is the audited shape and is consulted as a whole.
            let (conversation, cwd, nested_timestamp) =
                match object.get("payload").and_then(|v| v.as_object()) {
                    Some(payload) => (
                        payload.get("id").and_then(|v| v.as_str()),
                        payload.get("cwd").and_then(|v| v.as_str()),
                        payload.get("timestamp").and_then(|v| v.as_str()),
                    ),
                    None => (
                        object.get("id").and_then(|v| v.as_str()),
                        object.get("cwd").and_then(|v| v.as_str()),
                        None,
                    ),
                };
            let (Some(conversation), Some(cwd)) = (conversation, cwd) else {
                continue;
            };
            let Some(timestamp) = object
                .get("timestamp")
                .and_then(|v| v.as_str())
                .or(nested_timestamp)
            else {
                continue;
            };
            return Ok(Some(correlators_from(conversation, cwd, timestamp)?));
        }
        anyhow::bail!(
            "no session_meta line in this file's first {RECORD_PREFIX_BYTES} bytes carries \
             Codex's id/cwd/timestamp correlators"
        )
    }

    /// Codex asks the same way Claude does — a question followed by
    /// numbered answers at the bottom of the screen — so it reuses the same
    /// recognizer with its own vocabulary ([`CODEX_QUESTION_PHRASES`])
    /// rather than a second hand-rolled matcher. The shape is shared
    /// because both are TUIs built around the same interaction, not because
    /// one vendor copied the other; if either diverges, the phrase list is
    /// what changes, and only for that kind.
    fn sharpen(&self, baseline: SessionStatus, tail: &str) -> SessionStatus {
        if looks_like_a_choice_prompt(tail, CODEX_QUESTION_PHRASES) {
            return SessionStatus::Waiting;
        }
        baseline
    }
}

// ---------------------------------------------------------------------
// Prompt-shape recognition (PLAN_M6_75.md item 2)
//
// The one mechanism both sharpeners run on, and the vendor vocabulary
// each one runs it with. Kept together, and kept small, because the
// pressure on this code is always in the same direction: someone notices
// a prompt that was not caught and loosens a test. A recognizer that
// fires on ordinary agent prose is far worse than one that misses a
// dialog — a status that is occasionally late is boring, while a status
// that says WAITING at a session that is happily working teaches users to
// ignore the column, which is the one failure that makes the whole
// milestone pointless.
//
// Hence the shape below is a CONJUNCTION of two independent signals
// (vendor question wording AND a rendered menu of numbered answers),
// restricted to the bottom of the screen. Every part of that is load-
// bearing; see `looks_like_a_choice_prompt`.
//
// Deliberately not regular expressions, and not a dependency. What these
// patterns need is exact substring recognition plus a two-character
// prefix test, which plain `str` methods do without adding a crate to a
// supervisor whose dependency set is kept small enough to cross-compile
// to musl (see `capture`'s note on the hand-rolled timestamp parser for
// the same trade). It also keeps the no-panic property trivial to see:
// there is no byte-offset arithmetic anywhere in here.
// ---------------------------------------------------------------------

/// Question wording that, together with a menu of numbered answers, means
/// Claude Code is blocked on a human.
///
/// Provenance is uneven and saying so is the point:
///
/// - `Do you trust` is the folder-trust dialog this repo has ACTUALLY
///   observed under tmux — `crates/farhelm/tests/e2e/real_agent_capture.rs`
///   waits on that exact text during its (manually run) real-agent runs,
///   against Claude Code v2.1.220 on 2026-07-31.
/// - `Do you want to` is the tool-approval family — running a command,
///   making an edit, creating a file — from the same audit. Nothing in CI
///   pins it, because the fixture suite deliberately has no real agent.
/// - `Would you like to` is the plan-mode confirmation, same standing.
///
/// A vendor rewording any of these costs a `Waiting` that reads `Idle`
/// until someone re-audits, which is the cheap direction and is exactly
/// why the list is allowed to be under-inclusive. The expensive direction
/// — a phrase generic enough to appear in the agent's own output — is what
/// the conjunction below protects against, so entries are only ever half
/// of a match and must not be "fixed" by relaxing that.
const CLAUDE_QUESTION_PHRASES: &[&str] = &["Do you want to", "Do you trust", "Would you like to"];

/// The same vocabulary for Codex, whose approval and trust modals ask in
/// the same shape (a question, then numbered answers).
///
/// `Do you trust` is repo-observed: codex's folder-trust modal is the one
/// this suite had to work around because it ignores input under tmux (see
/// `real_codex_session_captures_its_conversation_identity`'s docs). The
/// approval wordings are from the vendor's own TUI as audited alongside
/// it, and carry the same caveat as Claude's — under-inclusive by choice,
/// never pinned by an automated real-agent test, and cheap when stale.
const CODEX_QUESTION_PHRASES: &[&str] = &[
    "Do you want to",
    "Do you trust",
    "Would you like to",
    "Allow command",
];

/// How many trailing lines of a sampled tail count as "the bottom of the
/// screen".
///
/// Both agents draw a pending dialog as the bottom-most element of the
/// pane, so restricting the match here is what separates a LIVE question
/// from one that was answered and scrolled up — the transcript above is
/// full of text the agent has already moved past, and matching it would
/// pin a session at `Waiting` for as long as the answered dialog stayed on
/// screen. Sized for a whole dialog box (a question, a rule, three or four
/// options, a border) rather than for the question alone, since both
/// halves of the conjunction have to fall inside it.
const PROMPT_TAIL_LINES: usize = 14;

/// How many numbered answers a menu needs before it counts as one.
///
/// Two, because a real choice always offers at least "yes" and "no", while
/// a single `1.` line is the ordinary shape of an agent enumerating steps
/// in its own prose. This is the cheapest available defence against the
/// expensive failure direction.
const MIN_MENU_CHOICES: usize = 2;

/// Whether the bottom of `tail` looks like an unanswered
/// question-plus-numbered-answers dialog written in `phrases`' vocabulary.
///
/// The conjunction is the whole design. A question phrase alone appears in
/// ordinary agent output constantly ("Do you want to keep the old name?"
/// written INTO a reply); a numbered menu alone is how any agent formats a
/// list. Together, at the bottom of the screen, they are what neither an
/// agent's prose nor a shell prompt produces by accident.
///
/// Blank lines are ignored rather than counted, so the padding rows inside
/// a dialog box cannot push the question out of the window
/// ([`PROMPT_TAIL_LINES`]) while it is still on screen.
///
/// Case-sensitive: these are fixed strings a TUI renders from its own
/// source, not user input, and lowercasing every line would spend
/// allocations on a robustness nobody needs.
fn looks_like_a_choice_prompt(tail: &str, phrases: &[&str]) -> bool {
    let lines: Vec<&str> = tail
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let bottom = &lines[lines.len().saturating_sub(PROMPT_TAIL_LINES)..];
    let choices = bottom
        .iter()
        .filter(|line| is_numbered_choice(line))
        .count();
    choices >= MIN_MENU_CHOICES
        && bottom
            .iter()
            .any(|line| phrases.iter().any(|phrase| line.contains(phrase)))
}

/// Whether a rendered line is one option of a numbered menu — `1.`, `2.`,
/// once the box drawing and selection markers around it are stripped.
///
/// Character-wise rather than by byte offset, which is not stylistic: the
/// input is a lossily decoded terminal screen, so a `&line[..2]` would
/// panic the moment a dialog's border character landed at the split (see
/// [`AgentIntegration::sharpen`]'s robustness note).
///
/// Single digits only. Every menu either vendor shows has a handful of
/// options, so `10.` is not a case worth admitting — and refusing it also
/// refuses years, versions, and section numbers that would otherwise start
/// a prose line.
fn is_numbered_choice(line: &str) -> bool {
    let mut chars = line.trim_matches(is_line_decoration).chars();
    let Some(digit) = chars.next() else {
        return false;
    };
    if !digit.is_ascii_digit() || digit == '0' {
        return false;
    }
    if chars.next() != Some('.') {
        return false;
    }
    // A menu option is `1. Yes`, never `1.5`: requiring the separator to be
    // followed by space (or by nothing, on a truncated capture) is what
    // keeps a decimal number at the start of a prose line from counting.
    chars.next().is_none_or(char::is_whitespace)
}

/// Characters a TUI wraps a line in that carry no meaning for matching:
/// whitespace, box-drawing borders, bullets, and the marker that points at
/// the currently selected option.
///
/// Trimmed from BOTH ends, since a boxed dialog closes every line with the
/// same border it opened with.
fn is_line_decoration(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '│' | '┃' | '┆' | '┊' | '▌' | '▏' | '|' | '❯' | '›' | '>' | '*' | '•' | '⏵'
        )
}

/// Assemble validated correlators, refusing anything this module is not
/// willing to retain.
///
/// The refusals are `Err`, not a silent skip, because both of them mean
/// "there is a record here that I cannot represent" — and a record that
/// goes unseen is exactly the second candidate whose absence turns an
/// ambiguity into a wrong claim.
fn correlators_from(
    conversation: &str,
    cwd: &str,
    timestamp: &str,
) -> anyhow::Result<RecordCorrelators> {
    if !is_plausible_conversation_id(conversation) {
        anyhow::bail!(
            "a conversation record carries an identifier this build will not retain \
             ({} bytes; must be 1..={MAX_CONVERSATION_ID_LEN} printable ASCII characters \
             without spaces or quotes)",
            conversation.len()
        );
    }
    let created_at = parse_rfc3339(timestamp).ok_or_else(|| {
        anyhow::anyhow!("a conversation record's timestamp is not an RFC 3339 instant")
    })?;
    Ok(RecordCorrelators {
        conversation: conversation.to_string(),
        cwd: cwd.to_string(),
        created_at,
    })
}

/// Whether a conversation identifier is something this module is willing
/// to store, log, and eventually place on an agent's command line.
///
/// ## Option injection is the threat, not exotic characters
///
/// This value comes off DISK — out of a file the supervisor did not write,
/// in a directory any process running as this user can create files in —
/// and ends up as an argv element in `<agent> --resume <id>`. An id
/// beginning with `-` is therefore not a weird id: it is a FLAG. A record
/// whose id reads `--last` turns a resume of one conversation into a
/// resume of whichever the vendor calls last; one reading
/// `--dangerously-bypass-approvals-and-sandbox` turns it into a permission
/// escalation. Neither needs a quote, a space, or a control character, so
/// the shape check alone (below) never sees them coming — which is why the
/// leading dash is refused outright and unconditionally.
///
/// Slot substitution is not a defence against this and never was: it
/// guarantees the id stays ONE argument, not that the argument is not a
/// flag. `--` separators are not one either, since neither vendor's CLI is
/// documented to accept one where the template puts the id.
///
/// ## Shape
///
/// Both vendors use UUIDs, so a UUID is what a valid id looks like today.
/// The check stays SHAPE-based rather than UUID-exact — a vendor is free to
/// change its id format, and rejecting a valid new one would break capture
/// silently — but everything a legitimate identifier has no business
/// containing is refused: whitespace, control characters, quotes,
/// backslashes, and anything past a bounded length.
fn is_plausible_conversation_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_CONVERSATION_ID_LEN
        && !id.starts_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_graphic() && c != '"' && c != '\'' && c != '\\')
}

/// The leading lines of a record prefix, parsed as JSON, skipping anything
/// unparseable.
///
/// A truncated trailing line (the prefix may end mid-line) simply fails to
/// parse and is skipped, which is why this never needs to know whether the
/// text it was handed was complete. `serde_json` skips surrounding
/// whitespace itself, so nothing is trimmed here.
fn leading_json_lines(text: &str) -> impl Iterator<Item = serde_json::Value> + '_ {
    text.lines()
        .take(RECORD_PREFIX_LINES)
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
}

/// Claude's project-directory name for a working directory.
///
/// NON-INJECTIVE by construction (`/tmp/a.b` and `/tmp/a-b` both become
/// `-tmp-a-b`), which is the whole reason this function's result is only
/// ever used to LOCATE files and never to decide that one belongs to a
/// session. Always applied to a session's CANONICAL cwd, because that is
/// what the agent itself munges: it munges its own `getcwd()`, which the
/// kernel has already resolved.
pub fn munge_cwd(canonical_cwd: &str) -> String {
    canonical_cwd
        .chars()
        .map(|c| match c {
            '/' | '.' | '_' => '-',
            other => other,
        })
        .collect()
}

/// The immutable per-session integration snapshot (PLAN_M3.md item 7):
/// which kind this session is, and how a resume would be invoked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationSnapshot {
    pub kind: AgentKind,
    /// The resume invocation as an argv VECTOR, so a path with spaces
    /// survives without quoting. `None` means this session has no resume
    /// invocation at all, which only a [`AgentKind::Generic`] session can
    /// be: an integrated kind always has at least its derived default.
    pub resume_template: Option<Vec<String>>,
}

/// Why a create's integration snapshot could not be resolved.
///
/// One variant today, and it stays an enum rather than a bare string
/// because the caller has to map it to a wire `ErrorKind` — a decision
/// that belongs at the boundary, not here.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SnapshotError {
    /// An integrated kind (derived or overridden) was given a template
    /// with no `{conversation}` element. Refused at create rather than at
    /// resume, because by resume time the only honest thing left to do
    /// would be to DISCARD a successfully captured identity — the exact
    /// promise SPEC.md makes ("restart resumes exactly that conversation")
    /// turned into a silent no-op. Placeholder-free templates belong to
    /// non-integrated kinds, where they are SPEC.md's verbatim fallback.
    #[error(
        "an explicit resume template for the integrated agent kind {kind} must contain a \
         {CONVERSATION_PLACEHOLDER} argv element; a placeholder-free template could only ever \
         discard the conversation identity this session captures"
    )]
    IntegratedTemplateHasNoPlaceholder { kind: &'static str },
}

/// This module's stable spelling of a kind for human-facing messages.
/// Deliberately not the wire serde representation: an error string is not
/// a protocol surface and must not start depending on one.
fn kind_name(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Claude => "claude",
        AgentKind::Codex => "codex",
        AgentKind::Generic => "generic",
    }
}

impl IntegrationSnapshot {
    /// Resolve a create's snapshot from the invocation and the request's
    /// optional overrides (PLAN_M3.md item 7).
    ///
    /// The precedence is: an explicit override always wins over derivation,
    /// and derivation is basename recognition of `argv0` — nothing more.
    /// `env claude`, a wrapper script, or a shell alias all classify as
    /// `Generic`, which is honest rather than clever: the override fields
    /// exist precisely because this heuristic cannot be made smart without
    /// becoming wrong in ways nobody could predict.
    ///
    /// The default template is built from the ORIGINAL `argv0`, not from a
    /// canonical command name, so `/opt/bin/claude` resumes through
    /// `/opt/bin/claude`.
    ///
    /// One validation invariant, and it is the only thing that can fail
    /// here: an integrated kind must end up with a template containing the
    /// placeholder. See [`SnapshotError`].
    pub fn resolve(
        argv0: &str,
        kind_override: Option<AgentKind>,
        template_override: Option<Vec<String>>,
    ) -> Result<IntegrationSnapshot, SnapshotError> {
        let kind = kind_override.unwrap_or_else(|| derive_kind(argv0));
        let integration = integration_for(kind);
        let resume_template =
            template_override.or_else(|| integration.map(|i| i.default_resume_template(argv0)));
        if integration.is_some() && !template_has_placeholder(resume_template.as_deref()) {
            return Err(SnapshotError::IntegratedTemplateHasNoPlaceholder {
                kind: kind_name(kind),
            });
        }
        Ok(IntegrationSnapshot {
            kind,
            resume_template,
        })
    }

    /// This session's integration, or `None` when it has none — the one
    /// gate every capture path passes through.
    pub fn integration(&self) -> Option<&'static dyn AgentIntegration> {
        integration_for(self.kind)
    }

    /// What restarting this session would do to its conversation
    /// (PLAN_M3.md item 7's third clause), given whatever identity is
    /// DURABLY claimed for it.
    ///
    /// The `FallbackTemplate` test is "a template that exists and does NOT
    /// mention the placeholder", which is exactly equivalent to "an
    /// explicitly overridden placeholder-free template" without needing a
    /// column to record explicitness: a DERIVED template exists only for
    /// integrated kinds and always contains the placeholder, and an
    /// integrated kind with a placeholder-free template can neither be
    /// created nor loaded (`store`'s decode enforces the same invariant at
    /// the trust boundary). So the only way to reach this state is a
    /// Generic session whose caller supplied a verbatim resume invocation
    /// — SPEC.md's fallback shape.
    ///
    /// A template that DOES mention the placeholder with nothing captured
    /// is `FreshOnly`, never `FallbackTemplate`: SPEC.md forbids running a
    /// `{conversation}` invocation unfilled, so offering it would be
    /// offering a garbled command line.
    pub fn restart_offer(&self, captured: Option<&str>) -> RestartOffer {
        // An identity this build would refuse to substitute
        // (`is_plausible_conversation_id` — an option-shaped id being the
        // case that matters) is not something to OFFER a resume for either:
        // the offer would be one `filled_resume_argv` then declines to
        // honor, which is a confusing refusal at the worst moment. Judged
        // here so the offer and the command it promises can never disagree.
        let captured = captured.filter(|id| is_plausible_conversation_id(id));
        match (&self.resume_template, captured) {
            (Some(_), Some(_)) if self.integration().is_some() => RestartOffer::Resume,
            (Some(template), _)
                if !template.is_empty() && !template_has_placeholder(Some(template)) =>
            {
                RestartOffer::FallbackTemplate
            }
            _ => RestartOffer::FreshOnly,
        }
    }

    /// The resume argv with `{conversation}` replaced by `conversation`, or
    /// `None` when this session has no template to fill.
    ///
    /// Substitutes into the element's own slot rather than into a command
    /// STRING — an id is never quoted, escaped, or word-split on its way
    /// in, which is why a resume can never be turned into a different
    /// command by an id that happens to contain shell metacharacters (and
    /// why [`is_plausible_conversation_id`] can afford to be a shape check
    /// rather than a sanitizer).
    ///
    /// PLAN_M3.md item 9 is what RUNS this; it exists here so the capture
    /// tests can assert the end-to-end promise ("resume this exact
    /// conversation") rather than only the id in isolation.
    pub fn filled_resume_argv(&self, conversation: &str) -> Option<Vec<String>> {
        // Re-validated at the boundary it actually matters at, not only
        // where the value was captured: a durable column written by an
        // older build (or edited by hand) reaches this function too, and
        // this is the last point before the value becomes an argv element.
        // See `is_plausible_conversation_id` for why a leading dash is the
        // case worth being paranoid about.
        if !is_plausible_conversation_id(conversation) {
            return None;
        }
        let mut filled = self.resume_template.clone()?;
        for element in &mut filled {
            if element == CONVERSATION_PLACEHOLDER {
                *element = conversation.to_string();
            }
        }
        Some(filled)
    }
}

/// Whether a resume template carries the placeholder as a whole element.
/// The one place that rule is spelled out, so `resolve`, `restart_offer`,
/// and `store`'s decode-time check cannot drift apart.
pub fn template_has_placeholder(template: Option<&[String]>) -> bool {
    template.is_some_and(|template| template.iter().any(|e| e == CONVERSATION_PLACEHOLDER))
}

/// Recognize an agent kind from the basename of an invocation's first
/// token — PLAN_M3.md item 7's deliberately dumb default.
///
/// Exact basename equality, not a prefix or substring match, and the
/// asymmetry is the point: a false NEGATIVE (`claude-wrapper` classified
/// generic) costs an honest fresh-launch offer and is fixable with an
/// explicit override, while a false POSITIVE gives a session Claude's
/// record layout and correlators when its agent will never write them —
/// producing either no capture at all or, worse, a correlation against
/// some other process's records in the same directory. When in doubt this
/// function says generic.
pub fn derive_kind(argv0: &str) -> AgentKind {
    let basename = Path::new(argv0)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(argv0);
    match basename {
        "claude" => AgentKind::Claude,
        "codex" => AgentKind::Codex,
        _ => AgentKind::Generic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Derivation is the DEFAULT every session gets when a caller sends no
    /// override, so its exact reach is a product decision, not an
    /// implementation detail: a path prefix must not defeat it (a session
    /// launched as `/opt/bin/claude` is still claude), and a wrapper must
    /// not accidentally acquire it (`env claude` classifies as generic, so
    /// the user is told to override rather than silently getting a
    /// session that looks integrated and never captures).
    #[test]
    fn kind_derivation_is_basename_equality_and_nothing_more() {
        assert_eq!(derive_kind("claude"), AgentKind::Claude);
        assert_eq!(derive_kind("/opt/bin/claude"), AgentKind::Claude);
        assert_eq!(derive_kind("codex"), AgentKind::Codex);
        assert_eq!(derive_kind("/usr/local/bin/codex"), AgentKind::Codex);
        assert_eq!(derive_kind("env"), AgentKind::Generic);
        assert_eq!(derive_kind("claude-wrapper"), AgentKind::Generic);
        assert_eq!(derive_kind("my-claude"), AgentKind::Generic);
        assert_eq!(derive_kind(""), AgentKind::Generic);
    }

    /// The default template must be built from the ORIGINAL first token,
    /// which is the whole reason PLAN_M3.md item 7 spells it out: resuming
    /// a session launched as `/opt/bin/claude` through a bare `claude`
    /// would depend on a PATH the supervisor's own service environment may
    /// not have. Codex's shape is a subcommand, not a flag — pinned here
    /// because it is audited vendor behavior, not a choice.
    #[test]
    fn default_templates_keep_the_original_first_token() {
        let claude = IntegrationSnapshot::resolve("/opt/bin/claude", None, None).unwrap();
        assert_eq!(claude.kind, AgentKind::Claude);
        assert_eq!(
            claude.resume_template.unwrap(),
            vec!["/opt/bin/claude", "--resume", "{conversation}"]
        );
        let codex = IntegrationSnapshot::resolve("codex", None, None).unwrap();
        assert_eq!(
            codex.resume_template.unwrap(),
            vec!["codex", "resume", "{conversation}"]
        );
        let generic = IntegrationSnapshot::resolve("bash", None, None).unwrap();
        assert_eq!(generic.kind, AgentKind::Generic);
        assert_eq!(generic.resume_template, None);
    }

    /// A path with spaces is exactly what the structural argv template
    /// exists for (PLAN_M3.md item 7 names this case), so it gets its own
    /// assertion: the token stays ONE element and no quoting is invented
    /// around it, which is also what makes the filled resume argv safe to
    /// hand to an exec without a shell.
    #[test]
    fn a_first_token_with_spaces_survives_as_one_argv_element() {
        let snapshot = IntegrationSnapshot::resolve("/opt/my agents/claude", None, None).unwrap();
        assert_eq!(
            snapshot.resume_template.as_deref().unwrap(),
            ["/opt/my agents/claude", "--resume", "{conversation}"]
        );
        assert_eq!(
            snapshot.filled_resume_argv("abc").unwrap(),
            ["/opt/my agents/claude", "--resume", "abc"]
        );
    }

    /// Overrides are the escape hatch for derivation's deliberate dumbness
    /// (PLAN_M3.md item 7), so both directions have to work: a wrapper
    /// declared claude gains the integration AND a derived default
    /// template, and a genuinely-claude invocation declared generic loses
    /// it. Without the second direction a user could never opt OUT of an
    /// integration that misbehaves for them.
    #[test]
    fn explicit_overrides_win_over_derivation_in_both_directions() {
        let promoted =
            IntegrationSnapshot::resolve("my-wrapper", Some(AgentKind::Claude), None).unwrap();
        assert_eq!(promoted.kind, AgentKind::Claude);
        assert_eq!(
            promoted.resume_template.unwrap(),
            vec!["my-wrapper", "--resume", "{conversation}"],
            "an overridden kind still derives its template from the real first token"
        );
        let demoted =
            IntegrationSnapshot::resolve("claude", Some(AgentKind::Generic), None).unwrap();
        assert_eq!(demoted.kind, AgentKind::Generic);
        assert_eq!(
            demoted.resume_template, None,
            "a generic session has no resume invocation unless one is supplied"
        );
    }

    /// The validation invariant PLAN_M3.md item 7 makes the only failure
    /// mode of snapshot resolution. It has to be refused at CREATE: once
    /// capture has succeeded, a placeholder-free template on an integrated
    /// kind could only ever throw the captured identity away, which is
    /// SPEC.md's "restart resumes exactly that conversation" quietly
    /// becoming false. Generic sessions are the opposite case and must
    /// keep accepting exactly such templates — that is SPEC.md's verbatim
    /// fallback shape.
    #[test]
    fn an_integrated_kind_refuses_a_placeholder_free_template() {
        let refused = IntegrationSnapshot::resolve(
            "claude",
            None,
            Some(vec!["claude".to_string(), "--continue".to_string()]),
        );
        assert_eq!(
            refused,
            Err(SnapshotError::IntegratedTemplateHasNoPlaceholder { kind: "claude" })
        );
        // An EMBEDDED placeholder is not a placeholder: PR3's contract is
        // whole-element equality, and accepting this would produce a
        // literal `--resume={conversation}` on the command line.
        assert!(
            IntegrationSnapshot::resolve(
                "claude",
                None,
                Some(vec![
                    "claude".to_string(),
                    "--resume={conversation}".to_string()
                ]),
            )
            .is_err()
        );
        // Generic keeps every shape, including none at all.
        assert!(
            IntegrationSnapshot::resolve(
                "bash",
                None,
                Some(vec!["bash".to_string(), "--restore".to_string()]),
            )
            .is_ok()
        );
        assert!(IntegrationSnapshot::resolve("bash", Some(AgentKind::Generic), None).is_ok());
    }

    /// The option-injection case, which is the reason this validation
    /// exists at all (fix-batch items 12 and 19): a conversation record is
    /// a file on disk that any process running as this user can create, and
    /// its id becomes an argv element in `<agent> --resume <id>`. An id
    /// that IS a flag — `--last`, or the demonstration case
    /// `--dangerously-bypass-approvals-and-sandbox` — would turn a resume
    /// of one conversation into a resume of another, or into a permission
    /// escalation, without containing a single character the shape check
    /// would otherwise object to.
    #[test]
    fn an_option_shaped_conversation_id_is_refused() {
        for hostile in [
            "--last",
            "--dangerously-bypass-approvals-and-sandbox",
            "-r",
            "--resume=other",
        ] {
            assert!(
                !is_plausible_conversation_id(hostile),
                "{hostile:?} is a flag, not an identifier"
            );
        }
        // ...and the shapes a legitimate vendor id actually takes are
        // still accepted, so this is a refusal of flags rather than a
        // narrowing to one vendor's format.
        for ok in [
            "0199a4d2-9c1a-7bd6-9d18-2c0f2f1c7f31",
            "rollout-2026-07-30T08-15-00-0199a4d2",
        ] {
            assert!(is_plausible_conversation_id(ok), "{ok:?} must be accepted");
        }
    }

    /// The two places that value can reach an agent's command line refuse
    /// it independently, because they are reached independently: a durable
    /// column written by an older build (or edited by hand) never passes
    /// through capture's own validation again.
    #[test]
    fn an_option_shaped_identity_neither_fills_a_template_nor_is_offered() {
        let snapshot = IntegrationSnapshot::resolve("claude", None, None).expect("resolve");
        assert_eq!(
            snapshot.filled_resume_argv("--dangerously-bypass-approvals-and-sandbox"),
            None,
            "an option-shaped id must never be substituted into an argv"
        );
        assert_eq!(
            snapshot.restart_offer(Some("--dangerously-bypass-approvals-and-sandbox")),
            RestartOffer::FreshOnly,
            "and must not be advertised as resumable either, or the offer would promise a \
             command the substitution then refuses to build"
        );
        // The honest case still works, or this test would pass for the
        // wrong reason.
        let good = "0199a4d2-9c1a-7bd6-9d18-2c0f2f1c7f31";
        assert_eq!(snapshot.restart_offer(Some(good)), RestartOffer::Resume);
        assert_eq!(
            snapshot
                .filled_resume_argv(good)
                .expect("a plausible id fills the template")
                .last()
                .map(String::as_str),
            Some(good)
        );
    }

    /// `restart_offer` is what the UI (PR8) turns into an affordance, so
    /// every one of its three answers is pinned against the state that
    /// produces it. The subtle one is the last: a `{conversation}` template
    /// with nothing captured is FreshOnly, never FallbackTemplate, because
    /// SPEC.md forbids ever running the placeholder unfilled.
    #[test]
    fn the_restart_offer_reflects_exactly_what_could_honestly_be_run() {
        let claude = IntegrationSnapshot::resolve("claude", None, None).unwrap();
        assert_eq!(claude.restart_offer(None), RestartOffer::FreshOnly);
        assert_eq!(claude.restart_offer(Some("conv-1")), RestartOffer::Resume);

        let fallback = IntegrationSnapshot::resolve(
            "some-agent",
            None,
            Some(vec!["some-agent".to_string(), "--continue".to_string()]),
        )
        .unwrap();
        assert_eq!(
            fallback.restart_offer(None),
            RestartOffer::FallbackTemplate,
            "a placeholder-free template is the one thing that can be run verbatim"
        );

        let generic = IntegrationSnapshot::resolve("bash", None, None).unwrap();
        assert_eq!(generic.restart_offer(None), RestartOffer::FreshOnly);

        // A generic session whose template DOES mention the placeholder can
        // never have an identity to fill it with, so it must not advertise
        // a fallback it could not run.
        let unfillable = IntegrationSnapshot::resolve(
            "bash",
            None,
            Some(vec![
                "bash".to_string(),
                CONVERSATION_PLACEHOLDER.to_string(),
            ]),
        )
        .unwrap();
        assert_eq!(unfillable.restart_offer(None), RestartOffer::FreshOnly);
    }

    /// The munging is the audited reason correlation cannot use directory
    /// names, so the collision is pinned as a PROPERTY rather than left as
    /// prose: `/tmp/a.b` and `/tmp/a-b` genuinely land in one directory,
    /// and any change that made this function injective would silently
    /// stop matching the real agent's own layout.
    #[test]
    fn cwd_munging_is_non_injective_by_construction() {
        assert_eq!(munge_cwd("/tmp/a.b"), "-tmp-a-b");
        assert_eq!(munge_cwd("/tmp/a-b"), "-tmp-a-b");
        assert_eq!(munge_cwd("/tmp/a_b"), "-tmp-a-b");
        assert_eq!(munge_cwd("/home/u/work"), "-home-u-work");
    }
    /// Claude's correlators are top-level per-line JSON fields, and the
    /// FIRST line need not carry all of them — real records open with
    /// summary/meta lines. Pinned because taking line 1 unconditionally is
    /// the obvious-looking implementation that silently captures nothing.
    /// A file with no correlator line at all is an ERROR, not `Ok(None)`:
    /// a 64 KiB prefix cannot establish that a file is not a record.
    #[test]
    fn claude_records_are_parsed_from_the_first_line_carrying_all_correlators() {
        let text = "{\"type\":\"summary\",\"summary\":\"x\"}\n\
                    {\"sessionId\":\"conv-7\",\"cwd\":\"/work\",\
                    \"timestamp\":\"2026-07-29T12:00:05.123Z\"}\n";
        let parsed = ClaudeIntegration.parse_record(text).unwrap().unwrap();
        assert_eq!(
            parsed,
            RecordCorrelators {
                conversation: "conv-7".to_string(),
                cwd: "/work".to_string(),
                created_at: parse_rfc3339("2026-07-29T12:00:05Z").unwrap(),
            }
        );
        assert!(ClaudeIntegration.parse_record("not json at all").is_err());
        assert!(
            ClaudeIntegration
                .parse_record("{\"sessionId\":\"a\",\"cwd\":\"/w\",\"timestamp\":\"nope\"}")
                .is_err(),
            "a correlator line with an unusable timestamp is a failure, not a skip"
        );
    }

    /// Codex's rollout files carry many event types, so accepting "any
    /// line with an id, a cwd and a timestamp" would let an arbitrary
    /// event supply a conversation identity — and taking one field from
    /// the nested payload and the other from the top level would fabricate
    /// a pair no record ever asserted. Both refusals are pinned here,
    /// along with the flat form this build accepts as forward-tolerance.
    #[test]
    fn codex_requires_a_session_meta_line_with_same_level_correlators() {
        let nested = "{\"timestamp\":\"2026-07-29T12:00:05Z\",\"type\":\"session_meta\",\
                      \"payload\":{\"id\":\"roll-1\",\"cwd\":\"/work\"}}\n";
        assert_eq!(
            CodexIntegration.parse_record(nested).unwrap().unwrap(),
            RecordCorrelators {
                conversation: "roll-1".to_string(),
                cwd: "/work".to_string(),
                created_at: parse_rfc3339("2026-07-29T12:00:05Z").unwrap(),
            }
        );

        let flat = "{\"timestamp\":\"2026-07-29T12:00:05Z\",\"type\":\"session_meta\",\
                    \"id\":\"roll-2\",\"cwd\":\"/work\"}\n";
        assert_eq!(
            CodexIntegration.parse_record(flat).unwrap().unwrap(),
            RecordCorrelators {
                conversation: "roll-2".to_string(),
                cwd: "/work".to_string(),
                created_at: parse_rfc3339("2026-07-29T12:00:05Z").unwrap(),
            }
        );

        // An ordinary event carrying the same fields is NOT a record.
        let event = "{\"timestamp\":\"2026-07-29T12:00:05Z\",\"type\":\"turn_context\",\
                     \"payload\":{\"id\":\"nope\",\"cwd\":\"/work\"}}\n";
        assert!(CodexIntegration.parse_record(event).is_err());

        // Mixed levels fabricate a pair: the payload exists, so the top
        // level's `cwd` must not be borrowed to complete it.
        let mixed = "{\"timestamp\":\"2026-07-29T12:00:05Z\",\"type\":\"session_meta\",\
                     \"cwd\":\"/work\",\"payload\":{\"id\":\"roll-3\"}}\n";
        assert!(CodexIntegration.parse_record(mixed).is_err());
    }

    /// A `session_meta` line with no usable timestamp must CONTINUE rather
    /// than fail the file: a rollout may legitimately open with a meta
    /// line whose timestamp lives on a later one, and aborting at the
    /// first would hide the record entirely — a capture that silently
    /// stops happening, with no error anywhere.
    #[test]
    fn a_codex_meta_line_without_a_timestamp_does_not_hide_a_later_one() {
        let text = "{\"type\":\"session_meta\",\"payload\":{\"id\":\"early\",\"cwd\":\"/work\"}}\n\
                    {\"timestamp\":\"2026-07-29T12:00:05Z\",\"type\":\"session_meta\",\
                    \"payload\":{\"id\":\"real\",\"cwd\":\"/work\"}}\n";
        assert_eq!(
            CodexIntegration
                .parse_record(text)
                .unwrap()
                .unwrap()
                .conversation,
            "real"
        );
    }

    /// A conversation id crosses from an on-disk file into a durable
    /// column, a log line, and eventually an agent's argv. Anything that
    /// is not an identifier under any plausible vendor format is refused —
    /// and refused LOUDLY (an error, marking the scan incomplete) rather
    /// than dropped, since a dropped candidate is exactly the second one
    /// whose absence would turn an ambiguity into a wrong claim.
    #[test]
    fn implausible_conversation_identifiers_are_refused() {
        assert!(is_plausible_conversation_id("0b0a3d65-a742-4b0e-bda5-c59"));
        assert!(!is_plausible_conversation_id(""));
        assert!(!is_plausible_conversation_id("has space"));
        assert!(!is_plausible_conversation_id("has\nnewline"));
        assert!(!is_plausible_conversation_id("has\"quote"));
        assert!(!is_plausible_conversation_id(
            &"x".repeat(MAX_CONVERSATION_ID_LEN + 1)
        ));
        let long = format!(
            "{{\"sessionId\":\"{}\",\"cwd\":\"/w\",\"timestamp\":\"2026-07-29T12:00:05Z\"}}",
            "x".repeat(MAX_CONVERSATION_ID_LEN + 1)
        );
        assert!(ClaudeIntegration.parse_record(&long).is_err());
    }

    // -----------------------------------------------------------------
    // Status sharpening (PLAN_M6_75.md item 2)
    //
    // Fixture tails, written to look like what the vendors actually draw
    // at the bottom of a pane. They are transcriptions, not captures —
    // nothing in CI can run a real agent — so they are the same class of
    // evidence as the phrase lists themselves, and the NEGATIVE cases are
    // what carry most of the value: a recognizer can only be made too
    // eager by someone chasing a missed prompt, and these are what would
    // fail when they did.
    // -----------------------------------------------------------------

    /// Claude Code's command-approval dialog.
    const CLAUDE_COMMAND_APPROVAL: &str = "\
⏺ I'll remove the stale build directory.

╭──────────────────────────────────────────────────────╮
│ Bash command                                         │
│                                                      │
│ rm -rf build                                         │
│ Remove the stale build directory                     │
│                                                      │
│ Do you want to proceed?                              │
│ ❯ 1. Yes                                             │
│   2. Yes, and don't ask again for rm commands        │
│   3. No, and tell Claude what to do differently      │
╰──────────────────────────────────────────────────────╯";

    /// Claude Code's folder-trust dialog — the one shape this repo has
    /// genuinely observed under tmux (`real_agent_capture.rs` waits on it).
    const CLAUDE_TRUST_DIALOG: &str = "\
╭──────────────────────────────────────────────────────╮
│ Do you trust the files in this folder?               │
│                                                      │
│ /tmp/scratch                                         │
│                                                      │
│ Claude Code'll be able to read, edit, and execute    │
│ files here.                                          │
│                                                      │
│ ❯ 1. Yes, proceed                                    │
│   2. No, exit                                        │
╰──────────────────────────────────────────────────────╯";

    /// Codex's command-approval modal.
    const CODEX_COMMAND_APPROVAL: &str = "\
▌ Codex wants to run a command
▌
▌   cargo test --workspace
▌
▌ Allow command?
▌ › 1. Yes, run it
▌   2. Yes, and don't ask again this session
▌   3. No, and tell Codex what to do instead";

    /// A working agent: a spinner line and a tool result, no question.
    const CLAUDE_WORKING: &str = "\
⏺ Read(src/service/status.rs)
  ⎿  Read 412 lines

✻ Thinking… (14s · ↑ 1.4k tokens · esc to interrupt)";

    /// The idle composer, which is what a Claude pane shows when it has
    /// finished and is waiting for nothing in particular. The distinction
    /// this fixture defends is the whole reason `Waiting` and `Idle` are
    /// separate statuses.
    const CLAUDE_COMPOSER: &str = "\
⏺ Done — the tests pass.

╭──────────────────────────────────────────────────────╮
│ >                                                    │
╰──────────────────────────────────────────────────────╯
  ? for shortcuts";

    /// Each kind recognizes its own agent's pending question and promotes
    /// a quiet session to `Waiting`.
    ///
    /// The baseline is passed as `Idle` on purpose: a pending approval
    /// produces no output, so by the time it matters the generic
    /// classifier has already decayed the session — meaning `Waiting` can
    /// only ever arrive by promotion, never by a sharpener happening to
    /// agree with recency.
    #[test]
    fn each_kind_recognizes_its_own_pending_question() {
        for tail in [CLAUDE_COMMAND_APPROVAL, CLAUDE_TRUST_DIALOG] {
            assert_eq!(
                ClaudeIntegration.sharpen(SessionStatus::Idle, tail),
                SessionStatus::Waiting,
                "claude should have recognized:\n{tail}"
            );
        }
        assert_eq!(
            CodexIntegration.sharpen(SessionStatus::Idle, CODEX_COMMAND_APPROVAL),
            SessionStatus::Waiting
        );
        // A running session with a question on screen is waiting too: the
        // question is the more specific fact, and the recency that made it
        // "running" is at most a few seconds of the dialog being drawn.
        assert_eq!(
            ClaudeIntegration.sharpen(SessionStatus::Running, CLAUDE_COMMAND_APPROVAL),
            SessionStatus::Waiting
        );
    }

    /// Ordinary screens are left exactly as the baseline classified them.
    ///
    /// This is the test that would fail first if the recognizer were ever
    /// loosened, and it covers the four ways a screen can look like a
    /// prompt without being one: an agent working, an agent finished and
    /// idle, a numbered list with no question, and a question with no
    /// menu. The last two are the halves of the conjunction, checked
    /// separately so a change that dropped either requirement cannot pass.
    #[test]
    fn ordinary_output_is_never_mistaken_for_a_pending_question() {
        let numbered_prose = "\
⏺ Here is the plan:
  1. Read the module docs
  2. Extract the classifier
  3. Wire it into the reply path";
        let question_prose = "\
⏺ I renamed the field to `budget`. Do you want to keep the old name as an
  alias, or is the rename fine?";

        for tail in [
            CLAUDE_WORKING,
            CLAUDE_COMPOSER,
            numbered_prose,
            question_prose,
        ] {
            assert_eq!(
                ClaudeIntegration.sharpen(SessionStatus::Idle, tail),
                SessionStatus::Idle,
                "claude must not have promoted:\n{tail}"
            );
            assert_eq!(
                CodexIntegration.sharpen(SessionStatus::Running, tail),
                SessionStatus::Running,
                "codex must not have promoted:\n{tail}"
            );
        }
    }

    /// A dialog that has been ANSWERED and scrolled up the transcript must
    /// not hold the session at `Waiting`.
    ///
    /// The failure this pins is not hypothetical: a whole-screen search
    /// would keep matching the answered box for as long as it stayed
    /// visible, so a session would sit at `Waiting` for minutes after the
    /// user replied — which is precisely the "the column is lying, ignore
    /// it" outcome the heuristic has to avoid. [`PROMPT_TAIL_LINES`] is
    /// the mechanism, and this is what says why it exists.
    #[test]
    fn an_answered_dialog_scrolled_out_of_the_bottom_no_longer_counts() {
        let mut tail = CLAUDE_COMMAND_APPROVAL.to_string();
        for i in 0..PROMPT_TAIL_LINES {
            tail.push_str(&format!("\n⏺ Update(src/file-{i}.rs)"));
        }
        assert_eq!(
            ClaudeIntegration.sharpen(SessionStatus::Running, &tail),
            SessionStatus::Running
        );
        // ...and it DOES still count while it is the bottom-most thing on
        // the screen, or the test above would pass with the recognizer
        // removed entirely.
        assert_eq!(
            ClaudeIntegration.sharpen(SessionStatus::Running, CLAUDE_COMMAND_APPROVAL),
            SessionStatus::Waiting
        );
    }

    /// Sharpening survives arbitrary bytes without panicking.
    ///
    /// Status is cosmetic by contract; a panic is not. The tail is a
    /// lossily decoded terminal screen, so it routinely carries control
    /// bytes, escape fragments, replacement characters from a capture that
    /// split a multi-byte sequence, and box-drawing characters at exactly
    /// the offsets a naive `&line[..2]` would slice. This runs the
    /// generated garbage through both sharpeners and asserts only that
    /// they RETURN — the verdict on nonsense is nonsense, and that is
    /// fine.
    ///
    /// Deterministic (a fixed-seed LCG) rather than randomized: a fuzz
    /// test that finds a panic only on some runs is a flake, and the
    /// interesting inputs here are structural rather than rare.
    #[test]
    fn sharpening_tolerates_arbitrary_tail_bytes() {
        // Bytes chosen to land on the boundaries that matter: control
        // characters, ASCII the matcher looks for, lone continuation and
        // lead bytes (which `from_utf8_lossy` turns into replacement
        // characters mid-line), and the multi-byte box drawing and marker
        // characters the decoration trim knows about.
        let alphabet: Vec<u8> = (0u8..=0x20)
            .chain(*b"1.2Do ?\n")
            .chain([0x80, 0xbf, 0xc3, 0xe2, 0xf0, 0xff])
            .chain("│❯›⏵".bytes())
            .collect();
        let mut seed = 0x5eed_1234_u64;
        let mut next = move || {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            (seed >> 33) as usize
        };
        for _ in 0..500 {
            let len = next() % 512;
            let bytes: Vec<u8> = (0..len)
                .map(|_| alphabet[next() % alphabet.len()])
                .collect();
            let tail = String::from_utf8_lossy(&bytes);
            for baseline in [
                SessionStatus::Running,
                SessionStatus::Idle,
                SessionStatus::Waiting,
            ] {
                for integration in [
                    &ClaudeIntegration as &dyn AgentIntegration,
                    &CodexIntegration,
                ] {
                    let status = integration.sharpen(baseline.clone(), &tail);
                    assert!(
                        status.is_live(),
                        "a sharpener may only ever answer with a live status; got {status:?} \
                         for {tail:?}"
                    );
                }
            }
        }
    }

    /// The DEFAULT `sharpen` returns the baseline untouched — "no
    /// sharpening", which is a different thing from "no status".
    ///
    /// Worth a stub implementation of its own because the distinction is
    /// the reason the method is defaulted at all (see its docs): a new
    /// integration that says nothing about prompts must still leave its
    /// sessions with the generic classification, and the shape that would
    /// have broken that — a required method inviting a stub — is exactly
    /// what this proves unnecessary.
    #[test]
    fn the_default_sharpener_leaves_every_baseline_alone() {
        struct Unsharpened;
        impl AgentIntegration for Unsharpened {
            fn default_resume_template(&self, _argv0: &str) -> Vec<String> {
                unreachable!("this fixture exists only to exercise the defaulted method")
            }
            fn record_root(&self, _home: &Path, _canonical_cwd: &str) -> PathBuf {
                unreachable!("this fixture exists only to exercise the defaulted method")
            }
            fn record_depth(&self) -> usize {
                unreachable!("this fixture exists only to exercise the defaulted method")
            }
            fn is_record_file(&self, _name: &str) -> bool {
                unreachable!("this fixture exists only to exercise the defaulted method")
            }
            fn parse_record(&self, _text: &str) -> anyhow::Result<Option<RecordCorrelators>> {
                unreachable!("this fixture exists only to exercise the defaulted method")
            }
        }

        for baseline in [
            SessionStatus::Running,
            SessionStatus::Idle,
            SessionStatus::Waiting,
        ] {
            assert_eq!(
                Unsharpened.sharpen(baseline.clone(), CLAUDE_COMMAND_APPROVAL),
                baseline,
                "a kind that declares no prompt knowledge must not lose its baseline"
            );
        }
    }
}
