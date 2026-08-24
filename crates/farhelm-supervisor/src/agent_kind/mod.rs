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
//!    in `service::status` can only see whether successive sampled screens
//!    differed from each other; recognizing that an agent is BLOCKED ON A
//!    QUESTION means recognizing that agent's own prompt and approval
//!    shapes, which is per-kind knowledge and nothing else's business.
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
//!
//! What neither is allowed to do is cost anything else, and the properties
//! that guarantee it are worth stating exactly rather than loosely.
//! [`AgentIntegration::sharpen`] takes a plain `&str` and returns a status:
//! it performs no I/O, awaits nothing, acquires no admission permit, and
//! cannot block on anything a request needs. It IS called while its
//! session's `activity` cell is locked, so the honest claim is "holds one
//! per-entry leaf mutex for a substring search", not "holds no lock at
//! all" — and not "uncontended" either, which an earlier version of this
//! paragraph claimed: the sampler WRITES that cell every tick, so a reply
//! and a tick can genuinely contend for it. What the leaf-lock property
//! guarantees is the part that matters — the mutex is held across no await
//! and alongside no other lock, so the wait is bounded by one substring
//! search or one sample fold and can never participate in a deadlock.

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
    /// `baseline` is what `service::status` concluded from this session's
    /// own run of unchanged samples alone — `Running` or `Idle`, never a
    /// dead status, because a session with no live pane is never sharpened.
    /// `tail` is the pane's visible grid as of the last SUCCESSFUL capture
    /// (`ticker::ActivitySample::tail`), bottom-anchored and lossily
    /// decoded; a session whose captures have started failing has no tail
    /// at all rather than a stale one, so nothing here is ever asked to
    /// judge a screen of unknown age.
    ///
    /// ## What an implementation may do
    ///
    /// Exactly one thing is worth doing here and it is the whole point of
    /// the method: PROMOTE a live baseline to [`SessionStatus::Waiting`]
    /// when the tail shows this agent's own unanswered question or approval
    /// prompt, or leave `baseline` alone. Two rules bound that, and both
    /// are enforced rather than trusted:
    ///
    /// - A NON-LIVE baseline must come back untouched. A screen is not
    ///   evidence about a process, so a stale prompt on the pane of a
    ///   session that has exited must not resurrect it as `Waiting`.
    ///   [`promote_if_waiting`] is where the implementations in this module
    ///   enforce that, at the seam, because the consumer downstream can
    ///   only check that an ANSWER is live — not that the promotion was
    ///   legitimate.
    /// - Anything OTHER than `Waiting` is discarded, and this is a
    ///   whitelist rather than a rejection of dead statuses:
    ///   `service::status::waiting_or_baseline` takes exactly `Waiting`
    ///   through and keeps the baseline for everything else — `Running` and
    ///   `Idle` included. So an implementation cannot flip a session's
    ///   activity classification either, not just its liveness. Returning
    ///   `Idle` because a screen looks still is precisely the mistake that
    ///   would otherwise pass, since the sampler's own count of unchanged
    ///   looks is the only thing entitled to that answer.
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
    /// answers. [`promote_if_waiting`] is that shape plus the baseline
    /// gate, and [`CLAUDE_QUESTION_PHRASES`] is the vocabulary it is
    /// required to appear with.
    fn sharpen(&self, baseline: SessionStatus, tail: &str) -> SessionStatus {
        promote_if_waiting(baseline, tail, CLAUDE_QUESTION_PHRASES)
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
        promote_if_waiting(baseline, tail, CODEX_QUESTION_PHRASES)
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
// Hence the shape below is a CONJUNCTION of independent signals, and
// every one of them has been tightened at least once after a reviewer
// found ordinary prose that satisfied the previous version. As it stands,
// a match needs all of:
//
//   - SUFFIX ORDERING. Read from the bottom up: optional chrome, then a
//     contiguous run of numbered answers, then the question above them.
//     A dialog with even one line of the agent's own output below it has
//     been answered, and must not read as pending.
//   - EVERY OPTION ANSWER-SHAPED. Not "at least one" — a numbered
//     explanation whose first item happens to begin "No" is prose, and is
//     the single most ordinary thing an agent prints.
//   - THE ANSWER-WORD GRAMMAR. The word alone, or the word plus a
//     terminator the recorded dialogs actually use. Anything looser takes
//     "No migration is required" and "No-code path".
//   - MARKER GRAMMAR. A selection pointer belongs to the menu LINE it
//     prefixes rather than to the box's chrome, because the same glyphs
//     are what an empty composer draws — and a composer that trimmed away
//     to nothing would hide an answered dialog above it.
//   - A BOUNDED LOOKBACK for the question, so the search stays inside the
//     dialog box instead of finding questions in the transcript above.
//
// `looks_like_a_choice_prompt` carries each argument in full, including
// the concrete false positive that motivated it.
//
// Deliberately not regular expressions, and not a dependency. What these
// patterns need is exact substring recognition plus a little
// character-wise prefix work, which plain `str` methods do without adding
// a crate to a supervisor whose dependency set is kept small enough to
// cross-compile to musl (see `capture`'s note on the hand-rolled
// timestamp parser for the same trade). It also keeps the no-panic
// property trivial to see: there is no byte-offset arithmetic anywhere in
// here.
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

/// How many numbered answers a menu needs before it counts as one.
///
/// Two, because a real choice always offers at least "yes" and "no", while
/// a single `1.` line is the ordinary shape of an agent enumerating steps
/// in its own prose.
const MIN_MENU_CHOICES: usize = 2;

/// How many numbered answers a menu may have before it stops looking like
/// one.
///
/// Neither vendor offers more than four (yes / yes-and-remember /
/// no-with-feedback, plus the occasional variant), so a run of a dozen
/// numbered lines is an agent enumerating something in prose, not a
/// question. Bounding the run is also what keeps the backward scan from
/// walking a whole screen of numbered output.
const MAX_MENU_CHOICES: usize = 8;

/// The words a menu option starts with when it is an ANSWER rather than a
/// list item.
///
/// The discriminator that ordering and phrasing alone do not provide.
/// Every approval, trust, and plan dialog either vendor draws is a
/// yes/no question whose options are spelled that way — "1. Yes", "2. Yes,
/// and don't ask again", "3. No, and tell Claude what to do differently",
/// "1. Yes, proceed" / "2. No, exit" — while a numbered list an agent
/// writes into its own output ("1. Read the module docs") is not. Without
/// this, prose that happens to ask a question above a numbered list
/// matches perfectly, which is a false `Waiting` produced by the single
/// most ordinary thing an agent does.
///
/// EVERY option in the run has to match, not merely one — see
/// [`looks_like_a_choice_prompt`] for the numbered-prose block that the
/// weaker rule let through, and for why the conjunction was chosen over
/// "the set must contain both a yes and a no".
///
/// A menu with even one option spelled some other way ("3. Cancel") is
/// therefore missed, as is one spelled entirely differently ("1. Proceed" /
/// "2. Abort"). That is the cheap direction, deliberately chosen: see
/// [`CLAUDE_QUESTION_PHRASES`] for the same trade on the question half.
const MENU_ANSWER_WORDS: &[&str] = &["yes", "no"];

/// What may follow an answer word and still leave the option an ANSWER
/// rather than a sentence that happens to start with one.
///
/// Comma alone, because that is what all three recorded dialogs use and
/// nothing else appears in any of them — see [`starts_with_answer_word`],
/// which quotes every option of all three. Written as a set rather than a
/// literal `','` so a vendor's variant can be added beside it with the
/// audit that justified it, exactly as the phrase lists grow.
///
/// Deliberately NOT whitespace: "No migration is required" is prose, and
/// admitting a space is what let a numbered explanation read as a menu.
/// Deliberately not `-` either: "No-code path" is a compound word, not an
/// answer.
const ANSWER_WORD_TERMINATORS: &[char] = &[','];

/// How many substantive lines above the menu are read while looking for
/// the question it answers.
///
/// Not zero, because a real dialog puts body text between the two: the
/// folder-trust dialog this repo has observed explains what trusting the
/// directory means over several lines before offering its options. Small,
/// because the search must stay inside the dialog box — read far enough up
/// and it starts finding questions in the transcript above, which is
/// exactly the unanchored matching this shape replaced.
const PROMPT_QUESTION_LOOKBACK: usize = 8;

/// Whether `tail` ENDS in an unanswered question-plus-numbered-answers
/// dialog written in `phrases`' vocabulary.
///
/// ## The shape, and why it is a suffix rather than a search
///
/// Read from the bottom up, a pending dialog is: optional chrome (the
/// box's closing border, blank padding), then a contiguous run of numbered
/// answers, then the question. Anything else at the bottom of the screen
/// means the dialog is not what the pane is currently showing.
///
/// An earlier version of this searched a window of trailing lines for a
/// phrase and any two numbered lines, which is not the same thing at all:
/// a dialog the user ANSWERED, followed by one line of the agent getting
/// on with the work, still matched — so a session would read `Waiting`
/// while its agent was visibly running. Requiring the block to be the
/// suffix is what makes "the question is still on screen, unanswered" the
/// thing being recognized rather than "a question was asked at some point
/// recently".
///
/// ## The four independent signals
///
/// A match needs all of: the suffix shape above, between
/// [`MIN_MENU_CHOICES`] and [`MAX_MENU_CHOICES`] numbered answers of which
/// EVERY ONE reads as an answer ([`MENU_ANSWER_WORDS`]), at least one
/// option carrying a selection pointer ([`MENU_SELECTION_MARKERS`]), and a
/// question line in the vendor's own wording. Each alone is common in
/// ordinary output; together, at the bottom of the screen, they are what
/// neither an agent's prose nor a shell prompt produces by accident.
///
/// ## Why a selection pointer is required, and not merely tolerated
///
/// The other three signals are all things ORDINARY PROSE can satisfy, and
/// the case that proves it is a numbered explanation whose every item opens
/// with an answer word and a comma:
///
/// ```text
/// Do you want to know why the migration is a no-op?
/// 1. No, migration is required only for rows written before the bump.
/// 2. Yes, and the defaults backfill everything else.
/// ```
///
/// That is a suffix, it is a menu-sized run, every option satisfies
/// [`starts_with_answer_word`], and the line above it is in
/// [`CLAUDE_QUESTION_PHRASES`]' vocabulary — so the conjunction of the
/// other three signals says `Waiting` at a session whose agent is in the
/// middle of writing. Tightening the answer grammar further cannot fix
/// this: the options really ARE answer-shaped, because prose is allowed to
/// be.
///
/// A selection pointer is the one signal that is not prose. It is drawn by
/// the widget, not written by the model, and every dialog this repo has
/// recorded carries one — Claude renders `❯ 1. Yes`, Codex renders
/// `▌ › 1. Yes, run it`. An agent enumerating something in its own output
/// has no reason to emit one, so requiring it is what separates "a menu" from
/// "a numbered list that reads like one".
///
/// The cost is this module's standing trade (see [`CLAUDE_QUESTION_PHRASES`]):
/// a vendor that stops drawing a pointer, or draws one this build does not
/// know, costs a missed `Waiting` until someone re-audits and adds the
/// glyph. That is the cheap direction; a false `Waiting` that persists for
/// as long as the screen does is not.
///
/// ## Why every option, and not just one
///
/// An earlier version asked only that SOME option in the run be
/// answer-shaped, and that is far too weak for the thing it is guarding
/// against. An agent writing prose reaches for numbered lists constantly,
/// and a single line beginning "No" or "Yes" is an ordinary way for one to
/// start — so a block like
///
/// ```text
/// Do you want to know why?
/// 1. No migration is required for existing rows
/// 2. The defaults backfill everything else
/// ```
///
/// satisfied every part of the shape: a vendor question phrase, two
/// numbered lines at the bottom of the screen, one of them beginning with
/// an answer word. That reads `Waiting` at a session whose agent is
/// working, which is the expensive direction this whole module is built to
/// avoid.
///
/// Requiring every option to be answer-shaped is a CONJUNCTION over the
/// run, so an ordinary numbered list disqualifies itself at whichever of
/// its items is not an answer — which for real prose is almost always the
/// first one. The alternative considered was "the set must contain both a
/// yes-shaped and a no-shaped option", which every recorded dialog also
/// satisfies; it was rejected as the weaker of the two, since it is an
/// existence test that a two-item list beginning "No..." and "Yes..." still
/// passes.
///
/// Checked against the dialogs this repo has actually recorded, because a
/// rule that excluded one of them would be a rule that lost real prompts:
/// Claude's command approval ("1. Yes" / "2. Yes, and don't ask again" /
/// "3. No, and tell Claude what to do differently"), Claude's folder-trust
/// dialog ("1. Yes, proceed" / "2. No, exit"), and Codex's command approval
/// ("1. Yes, run it" / "2. Yes, and don't ask again this session" / "3. No,
/// and tell Codex what to do instead"). All three are answer-shaped
/// throughout. A vendor that adds a "3. Cancel" costs us that dialog's
/// `Waiting` until someone re-audits — the cheap direction, and the same
/// one [`CLAUDE_QUESTION_PHRASES`] already chooses.
///
/// ## What counts as chrome, and what deliberately does not
///
/// Only blank lines and lines made entirely of border characters
/// ([`is_line_decoration`]). Footer hints are NOT tolerated below the
/// menu, and that is a decision rather than an omission: the hint-shaped
/// line most likely to appear under an answered dialog is the working
/// spinner itself ("esc to interrupt"), so any vocabulary broad enough to
/// skip footers is broad enough to skip the very line that proves the
/// dialog is gone. A vendor that draws a real hint below its options costs
/// us a missed `Waiting`; the alternative costs a wrong one that persists.
///
/// Case-sensitive on the question phrases (fixed strings a TUI renders
/// from its own source) and case-INSENSITIVE on the answer words, which
/// are ordinary prose a vendor may capitalize either way.
///
/// ## Why the question is looked for above the menu rather than beside it
///
/// Claude's folder-trust dialog — the one shape this repo has actually
/// observed — puts explanatory body text between its question and its
/// options ("Claude Code'll be able to read, edit, and execute files
/// here."), so "the line immediately above the menu" would miss it. The
/// search upward is bounded by [`PROMPT_QUESTION_LOOKBACK`] instead, which
/// keeps it inside the dialog box without needing to know what a vendor
/// chose to explain there. Note what this does NOT relax: the menu itself
/// still has to be the suffix, which is the property the answered-dialog
/// case turns on.
///
/// One backward pass over the lines, allocating nothing at all: this runs
/// once per integrated session per reply, under that session's activity
/// lock.
///
/// Two phases over ONE reverse iterator, which is also why the lookback
/// cannot drift: the body phase is a `take(PROMPT_QUESTION_LOOKBACK)`, so
/// the number of lines examined IS the declared bound rather than a
/// counter compared against it (an earlier shape tested each line and then
/// checked the counter, which examined nine lines where eight were
/// declared).
fn looks_like_a_choice_prompt(tail: &str, phrases: &[&str]) -> bool {
    let asks = |content: &str| phrases.iter().any(|phrase| content.contains(phrase));
    // Blank padding and the box's own borders — above and below the menu
    // alike — are dropped once, here, so neither phase has to think about
    // them. A line made entirely of decoration trims to nothing.
    let mut lines = tail
        .lines()
        .rev()
        .map(|line| line.trim_matches(is_line_decoration))
        .filter(|content| !content.is_empty());

    // Phase 1: the menu, which must be the suffix. Reaching a non-menu line
    // on the FIRST substantive line means the bottom of the screen was
    // never a menu at all, which is what the suffix rule is.
    let mut choices = 0usize;
    // The widget-drawn signal, tracked across the whole run rather than
    // demanded of each line: exactly one option is selected at a time, so
    // only one of them carries the pointer.
    let mut pointed_at = false;
    let first_body_line = loop {
        // A screen that is nothing but a menu has no question on it.
        let Some(content) = lines.next() else {
            return false;
        };
        match menu_choice_text(content) {
            Some(choice) => {
                // EVERY option, not merely one: see this function's docs
                // for the numbered-prose block that "at least one" let
                // through.
                if !starts_with_answer_word(choice.answer) {
                    return false;
                }
                pointed_at |= choice.selected;
                choices += 1;
                if choices > MAX_MENU_CHOICES {
                    return false;
                }
            }
            None => break content,
        }
    };
    if choices < MIN_MENU_CHOICES {
        return false;
    }
    // The signal an agent's own prose does not produce; see this function's
    // docs for the answer-shaped numbered explanation that satisfies
    // everything else.
    if !pointed_at {
        return false;
    }

    // Phase 2: the question, somewhere in the bounded body above the menu.
    // The line that ENDED the menu is the first of those lines, not an
    // extra one beyond the bound.
    std::iter::once(first_body_line)
        .chain(lines)
        .take(PROMPT_QUESTION_LOOKBACK)
        .any(asks)
}

/// One parsed numbered menu option, as [`menu_choice_text`] reads it off a
/// line.
///
/// The `selected` flag is carried out rather than being consumed inside the
/// parser because it answers a question about the RUN, not about the line:
/// [`looks_like_a_choice_prompt`] requires at least one pointed-at option
/// anywhere in the menu, which is the signal that separates a widget from
/// an agent's numbered prose.
struct MenuChoice<'a> {
    /// The option's text with its number, separator, and any selection
    /// pointer stripped — `❯ 1. Yes, proceed` yields `Yes, proceed`.
    answer: &'a str,
    /// Whether this line carried one of [`MENU_SELECTION_MARKERS`].
    selected: bool,
}

/// The parsed form of a numbered menu option — `1. Yes` yields `Yes` — or
/// `None` for any other line.
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
fn menu_choice_text(content: &str) -> Option<MenuChoice<'_>> {
    // The pointer at the currently selected option is part of a menu
    // line's grammar, not of the box's chrome, so it is skipped HERE rather
    // than trimmed as decoration — see [`MENU_SELECTION_MARKERS`] for the
    // answered-dialog bug that distinction fixes.
    let (content, selected) = match content.strip_prefix(MENU_SELECTION_MARKERS) {
        Some(rest) => (rest.trim_start(), true),
        None => (content, false),
    };
    let mut chars = content.chars();
    let digit = chars.next()?;
    if !digit.is_ascii_digit() || digit == '0' {
        return None;
    }
    if chars.next() != Some('.') {
        return None;
    }
    let rest = chars.as_str();
    // A menu option is `1. Yes`, never `1.5`: requiring the separator to be
    // followed by space (or by nothing, on a truncated capture) is what
    // keeps a decimal number at the start of a prose line from counting.
    if rest.starts_with(|c: char| !c.is_whitespace()) {
        return None;
    }
    Some(MenuChoice {
        answer: rest.trim_start(),
        selected,
    })
}

/// Whether a menu option IS an answer rather than a list item: its text is
/// one of [`MENU_ANSWER_WORDS`], either alone or followed by
/// [`ANSWER_WORD_TERMINATORS`].
///
/// ## Why the terminator, and not "any word boundary"
///
/// Requiring only that the first WORD be an answer word — with whitespace
/// ending it like any other separator — is too loose in exactly the
/// direction that matters, because "No migration is required" and "Yes it
/// does" are ordinary English sentences an agent writes into a numbered
/// list. So is a hyphenated compound: "No-code path", "Yes-style wording".
/// Every one of those passes a word-boundary test and none of them is an
/// answer to anything.
///
/// The grammar encoded here is the one the three recorded dialogs actually
/// use, and they are unanimous — in every option of all three, the answer
/// word either IS the whole option or is immediately followed by a comma:
///
/// - Claude command approval: "Yes", "Yes, and don't ask again for rm
///   commands", "No, and tell Claude what to do differently".
/// - Claude folder trust: "Yes, proceed", "No, exit".
/// - Codex command approval: "Yes, run it", "Yes, and don't ask again this
///   session", "No, and tell Codex what to do instead".
///
/// So the rule is: nothing after the word, or a terminator. A vendor that
/// writes "Yes — proceed" costs a missed `Waiting` until someone adds its
/// separator here, which is this module's standing trade (see
/// [`CLAUDE_QUESTION_PHRASES`]) and the cheap direction of it.
///
/// Borrows rather than building: every answer word is ASCII, so the prefix
/// that could match one is exactly the leading run of ASCII letters, and
/// `eq_ignore_ascii_case` compares it in place.
fn starts_with_answer_word(text: &str) -> bool {
    let end = text
        .char_indices()
        .find(|(_, c)| !c.is_ascii_alphabetic())
        .map_or(text.len(), |(index, _)| index);
    let (word, rest) = text.split_at(end);
    if !MENU_ANSWER_WORDS
        .iter()
        .any(|answer| word.eq_ignore_ascii_case(answer))
    {
        return false;
    }
    rest.chars()
        .next()
        .is_none_or(|next| ANSWER_WORD_TERMINATORS.contains(&next))
}

/// Apply one vendor's prompt vocabulary to a baseline, promoting only a
/// LIVE baseline and only to [`SessionStatus::Waiting`].
///
/// The shared body of both implementations' [`AgentIntegration::sharpen`],
/// and the place the trait's "never invent liveness" rule is actually
/// enforced. It has to be enforced HERE, at the seam, rather than only
/// where `service::status` consumes the result: that consumer can check
/// that an ANSWER is live, but not that a promotion was legitimate, so a
/// sharpener handed `Exited` and returning `Waiting` would sail straight
/// through — a screen full of a stale prompt claiming a dead session is
/// blocked on a human.
///
/// `sharpen` is a public method on a public trait, so "the only caller
/// passes a live baseline" is a property of today's tree rather than of
/// the API. The gate costs one comparison and removes the question.
///
/// Considered and rejected: narrowing the parameter to a `LiveStatus`
/// newtype, which would make the invariant unrepresentable. It would also
/// introduce a second status vocabulary next to the wire enum every caller
/// already holds, and force a conversion at both ends of a seam whose
/// entire subject matter is cosmetic. A guard plus tests buys the same
/// property here without that.
fn promote_if_waiting(baseline: SessionStatus, tail: &str, phrases: &[&str]) -> SessionStatus {
    if baseline.is_live() && looks_like_a_choice_prompt(tail, phrases) {
        return SessionStatus::Waiting;
    }
    baseline
}

/// The pointers a TUI draws at the currently selected option.
///
/// Skipped by [`menu_choice_text`] as part of a menu line's grammar rather
/// than trimmed by [`is_line_decoration`] as chrome, and the distinction is
/// a real bug rather than taxonomy. These glyphs are ALSO what a composer
/// prompt draws when it is empty, and an empty composer is precisely the
/// screen that means "no question is pending". As decoration, a bare `›`
/// line would trim to nothing, become indistinguishable from a dialog's
/// blank padding, and let [`looks_like_a_choice_prompt`] scan straight past
/// it into an ANSWERED menu above — reporting `Waiting` at a session whose
/// user has already answered, for as long as that screen stays put. It is
/// the same failure the ASCII `>` is excluded from the decoration set to
/// avoid; Claude's composer draws that one, and Codex's draws `›`.
///
/// Arbitrated on the repo's own evidence, since one reviewer read the risk
/// the other way. The observation that settles the SKIP being necessary is
/// Codex's recorded approval modal, whose selected option renders as
/// `▌ › 1. Yes, run it` — treat `›` as an ordinary character and that line
/// stops parsing as a menu option, and the real dialog stops being
/// recognized at all. What the repo does NOT contain is any capture of an
/// EMPTY Codex composer, so "a bare `›` cannot occur, because the composer
/// always renders placeholder text" could not be written down with a
/// citation; the only in-repo sighting of that line
/// (`real_agent_capture.rs`, on the flake it documents) has the user's
/// typed prompt sitting on it, which says nothing about the empty case.
/// Handling the marker here costs nothing and does not depend on which
/// reading of the vendor's composer is right.
///
/// Since the prose-hardening pass these glyphs also carry a third job:
/// [`looks_like_a_choice_prompt`] REQUIRES one of them somewhere in the run
/// it is about to call a menu. That makes this list load-bearing for
/// recognition rather than only for parsing — a vendor whose pointer is not
/// here now costs a missed `Waiting` for its whole dialog, not merely a
/// mis-parsed selected line. See that function's docs for why a
/// widget-drawn signal is the only thing that separates a real menu from an
/// answer-shaped numbered explanation.
const MENU_SELECTION_MARKERS: &[char] = &['❯', '›', '⏵'];

/// Characters a TUI wraps a line in that carry no meaning for matching:
/// whitespace, box-drawing borders, and bullets.
///
/// Trimmed from BOTH ends, since a boxed dialog closes every line with the
/// same border it opened with. A line made ENTIRELY of these — the box's
/// top and bottom rules, an empty row inside it — therefore trims to
/// nothing, which is how [`looks_like_a_choice_prompt`] tells a dialog's
/// own chrome from something the agent printed.
///
/// The two Unicode blocks are taken wholesale rather than character by
/// character: Box Drawing (U+2500–U+257F) is where `│ ─ ╭ ╰ ┃` and every
/// variant a vendor might switch to live, and Block Elements
/// (U+2580–U+259F) covers the solid bars Codex rules its panes with. Any
/// character in either block is decoration by construction, so enumerating
/// them would only invite a miss when a vendor changes its border style.
/// Deliberately NOT included are the status glyphs both agents prefix real
/// output with (`⏺`, `✻`) — those mark lines that carry meaning, and
/// trimming them away would let a working screen look empty.
///
/// Also deliberately excluded, and for a sharper reason than style: the
/// ASCII `>` and every glyph in [`MENU_SELECTION_MARKERS`]. All of them are
/// composer prompts as well as pointers, and a composer line that trimmed
/// to nothing would hide an answered dialog. See that constant.
fn is_line_decoration(c: char) -> bool {
    c.is_whitespace() || ('\u{2500}'..='\u{259f}').contains(&c) || matches!(c, '|' | '*' | '•')
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

/// Whether a conversation identifier is something this crate is willing
/// to store, log, and eventually place on an agent's command line.
///
/// Two callers, both feeding it the same kind of value from a different
/// direction: the record parse below, which reads ids off disk, and the
/// supervisor's `ReportConversation` handler, which takes one off the wire
/// from inside a session's own process. Neither source is this process's
/// own writing, and the argv they both end at is identical, so they get
/// the identical predicate rather than two that could drift apart.
///
/// ## Option injection is the threat, not exotic characters
///
/// This value comes off DISK — out of a file the supervisor did not write,
/// in a directory any process running as this user can create files in —
/// or off a session-authenticated connection, which every process in the
/// agent's tree can open. Either way it ends up as an argv element in
/// `<agent> --resume <id>`. An id
/// beginning with `-` is therefore not a weird id: it is a FLAG. A record
/// whose id reads `--last` turns a resume of one conversation into a
/// resume of whichever the vendor calls last; one reading
/// `--dangerously-bypass-approvals-and-sandbox` turns it into a permission
/// escalation. Neither needs a quote, a space, or a control character, so
/// the shape check alone (below) never sees them coming — which is why the
/// leading dash is refused outright and unconditionally.
///
/// Slot substitution is not a defence against this and never was, which is
/// exactly the limit of the guarantee described above: keeping the id in
/// ONE argument says nothing about whether that argument is a flag. `--`
/// separators are not one either, since neither vendor's CLI is documented
/// to accept one where the template puts the id.
///
/// ## Shape
///
/// Both vendors use UUIDs, so a UUID is what a valid id looks like today.
/// The check stays SHAPE-based rather than UUID-exact — a vendor is free to
/// change its id format, and rejecting a valid new one would break capture
/// silently — but everything a legitimate identifier has no business
/// containing is refused: whitespace, control characters, quotes,
/// backslashes, and anything past a bounded length.
pub(crate) fn is_plausible_conversation_id(id: &str) -> bool {
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

/// Whether `argv` is a vector this supervisor could actually hand to
/// `execvp` — the ONE rule, applied everywhere an executable vector is
/// accepted, built, or read back.
///
/// `subject` names the thing being checked in the returned message ("agent
/// invocation", "profile invocation", "resume template", ...); the `Err` is
/// the user-facing text verbatim, so callers wrap it in whichever
/// `ErrorKind` their boundary uses rather than reformatting it.
///
/// It lives here, beside [`CONVERSATION_PLACEHOLDER`], because the rule is
/// about what an argv IS rather than about which request produced one. It
/// used to exist only in the profile-write validator, which meant a raw
/// create, a pending-retry takeover, and a restart each accepted vectors
/// that profile CRUD refused — the same unexecutable command line, reached
/// by a different door.
///
/// The three refusals, and why each:
///
/// - **An empty vector** names no program at all. Note that
///   `shell_words::split("''")` yields `[""]` and not `[]`, so this alone
///   never was enough.
/// - **An empty `argv[0]`** is the `''` case above: a command line that
///   exists and names nothing.
/// - **A NUL byte anywhere** cannot survive the C string every exec
///   ultimately builds. It TRUNCATES the argument at the NUL rather than
///   failing, which is the worst of the three because something still runs
///   — just not what was asked for.
///
/// What this deliberately does NOT refuse is an empty element AFTER
/// `argv[0]`. That is the ordinary way to write a safe resume wrapper:
///
/// ```text
/// ["sh", "-c", "exec claude --resume \"$1\"", "", "{conversation}"]
/// ```
///
/// The empty element there is `$0` for the inner shell — a positional slot
/// that exists precisely so the captured identity lands in `$1` rather than
/// being spliced into the script text. An earlier version rejected those
/// and forced users into exactly the substitution the argv-vector design
/// exists to avoid.
pub fn ensure_executable_argv(subject: &str, argv: &[String]) -> Result<(), String> {
    let Some(program) = argv.first() else {
        return Err(format!("{subject} is empty"));
    };
    if program.is_empty() {
        return Err(format!(
            "{subject}'s first element is empty, so it names no program to run; only the \
             ARGUMENTS after it may be empty"
        ));
    }
    if argv.iter().any(|element| element.contains('\0')) {
        return Err(format!(
            "{subject} contains a NUL byte, which cannot survive being passed to a program"
        ));
    }
    Ok(())
}

/// [`ensure_executable_argv`] plus the two rules that are about a RESUME
/// template specifically.
///
/// A present-but-empty template gets its own wording rather than the
/// generic "is empty", because omitting the field entirely is a different
/// request (this kind's default, or no resume invocation at all) and the
/// message has to say which one the caller probably meant.
///
/// The second rule is the sharp one: [`CONVERSATION_PLACEHOLDER`] may not
/// be the PROGRAM. Substitution replaces that element with a captured
/// conversation id, so a template shaped `["{conversation}", ...]` turns
/// into an argv whose `argv[0]` is a UUID read off disk — a restart that
/// tries to execute the conversation identity. It passes every other check
/// (the vector is non-empty, `argv[0]` is non-empty, the placeholder is
/// present so an integrated kind is satisfied), which is exactly why it
/// needs naming here rather than being caught by accident.
pub fn ensure_resume_template(template: &[String]) -> Result<(), String> {
    if template.is_empty() {
        return Err(
            "resume template is present but empty; omit it entirely to mean \"this kind's \
             default\" or \"no resume invocation\""
                .to_string(),
        );
    }
    ensure_executable_argv("resume template", template)?;
    if template[0] == CONVERSATION_PLACEHOLDER {
        return Err(format!(
            "resume template's first element is {CONVERSATION_PLACEHOLDER}, so substituting the \
             captured conversation identity would make it the PROGRAM this session tries to run; \
             the placeholder belongs in an argument slot"
        ));
    }
    Ok(())
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

    /// The placeholder may not be the PROGRAM, and the shapes that put it
    /// anywhere else keep working.
    ///
    /// Substitution replaces the placeholder element with an identifier read
    /// off disk, so a template shaped `["{conversation}", ...]` resolves to
    /// an argv whose `argv[0]` is a conversation id — a restart that tries
    /// to execute the identity it was supposed to resume. It passes every
    /// other rule: the vector is non-empty, `argv[0]` is non-empty, and the
    /// placeholder IS present, so an integrated kind's own validation is
    /// satisfied by the very element that breaks it.
    ///
    /// The accepted half is not filler. The refusal has to be narrow enough
    /// that the wrapper idiom — `sh -c 'exec … "$1"' '' {conversation}` —
    /// still works, and the filled argv is asserted rather than merely the
    /// acceptance, so a rule that quietly stopped substituting would fail
    /// here too.
    #[test]
    fn the_conversation_placeholder_may_not_be_the_program() {
        let refused =
            ensure_resume_template(&[CONVERSATION_PLACEHOLDER.to_string(), "--resume".to_string()])
                .expect_err("a template whose program slot is the placeholder is unexecutable");
        assert!(
            refused.contains("PROGRAM"),
            "the refusal must say what would go wrong: {refused}"
        );

        let wrapper = vec![
            "sh".to_string(),
            "-c".to_string(),
            "exec claude --resume \"$1\"".to_string(),
            String::new(),
            CONVERSATION_PLACEHOLDER.to_string(),
        ];
        ensure_resume_template(&wrapper).expect("the placeholder in an ARGUMENT slot is the point");
        let snapshot = IntegrationSnapshot::resolve("sh", Some(AgentKind::Claude), Some(wrapper))
            .expect("an integrated kind is satisfied by a placeholder anywhere in the vector");
        assert_eq!(
            snapshot
                .filled_resume_argv("0199a4d2-9c1a-7bd6-9d18-2c0f2f1c7f31")
                .expect("a plausible id fills the template"),
            [
                "sh",
                "-c",
                "exec claude --resume \"$1\"",
                "",
                "0199a4d2-9c1a-7bd6-9d18-2c0f2f1c7f31"
            ],
            "the program stays the program and the identity lands in its own slot"
        );
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
    /// leaves the screen sitting still, so once enough consecutive samples
    /// have found it unchanged the generic classifier has already decayed
    /// the session — meaning `Waiting` can only ever arrive by promotion,
    /// never by a sharpener happening to agree with a baseline that was
    /// going to say `Running` anyway.
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
        // question is the more specific fact. A `Running` baseline here
        // only means the unchanged-sample streak has not yet reached the
        // idle threshold — under a budgeted round robin that can outlast
        // the dialog being drawn by a long way, so the sharpener must not
        // wait for the baseline to decay before it says anything.
        assert_eq!(
            ClaudeIntegration.sharpen(SessionStatus::Running, CLAUDE_COMMAND_APPROVAL),
            SessionStatus::Waiting
        );
    }

    /// Ordinary screens are left exactly as the baseline classified them.
    ///
    /// This is the test that would fail first if the recognizer were ever
    /// loosened, and it covers the five ways a screen can look like a
    /// prompt without being one: an agent working, an agent finished and
    /// idle, a numbered list with no question, a question with no menu,
    /// and — the case that defeats a plain conjunction — a question phrase
    /// sitting directly above a numbered list in the agent's OWN prose.
    ///
    /// That last one is why a menu has to read as an ANSWER
    /// ([`MENU_ANSWER_WORDS`]) rather than merely as a numbered run: it
    /// satisfies every other requirement, including the suffix shape, and
    /// an agent laying out a plan when asked to is about as ordinary as
    /// output gets.
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
        let question_over_numbered_prose = "\
⏺ Do you want to see the plan before I start? Here it is:
  1. Read the module docs
  2. Extract the classifier
  3. Wire it into the reply path";

        for tail in [
            CLAUDE_WORKING,
            CLAUDE_COMPOSER,
            numbered_prose,
            question_prose,
            question_over_numbered_prose,
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

    /// The adversarial case the "at least one answer-shaped option" rule
    /// let through: a numbered PROSE block whose first item happens to
    /// begin with an answer word, under a line the vendor phrase list
    /// matches.
    ///
    /// This is not a contrived shape. "Do you want to know why?" is
    /// ordinary agent prose, "1. No migration is required" is an ordinary
    /// way to open a numbered explanation, and the two together satisfied
    /// every part of the old shape — vendor phrase, suffix menu, two
    /// numbered lines, one of them answer-shaped. The result was `Waiting`
    /// at a session whose agent was in the middle of explaining something,
    /// which teaches users the column lies.
    ///
    /// Both orders and both answer words, because the rule is a
    /// conjunction over the whole run and would be equally wrong if it
    /// happened to check only the first option or only the last.
    ///
    /// The fixtures carry a selection pointer they would never have in real
    /// prose, and that is deliberate: the pointer requirement added later
    /// (see [`looks_like_a_choice_prompt`]) would otherwise reject every row
    /// here before the answer grammar was ever consulted, leaving this test
    /// green while the rule it names went untested. Realism belongs to
    /// `an_answer_shaped_numbered_explanation_is_not_a_menu`, which is the
    /// pointer rule's own regression.
    #[test]
    fn a_numbered_prose_block_with_one_answer_shaped_line_is_not_a_menu() {
        let answer_word_first = "\
⏺ Do you want to know why the schema needed no migration?
❯ 1. No migration is required for existing rows
  2. The defaults backfill everything else";
        let answer_word_last = "\
⏺ Do you want to know why the schema needed no migration?
❯ 1. The defaults backfill everything else
  2. Yes-style wording is what makes this a menu, and this line is not one";
        let answer_word_in_the_middle = "\
⏺ Do you want to see what changed?
❯ 1. The column is nullable
  2. No backfill was needed
  3. The version bump is atomic with it";

        for tail in [
            answer_word_first,
            answer_word_last,
            answer_word_in_the_middle,
        ] {
            assert_eq!(
                ClaudeIntegration.sharpen(SessionStatus::Running, tail),
                SessionStatus::Running,
                "every option must read as an answer, or this is prose:\n{tail}"
            );
        }
    }

    /// A numbered explanation whose EVERY item opens with an answer word and
    /// a comma is still prose, and must not read as a pending question.
    ///
    /// This is the case that survived the "every option must be
    /// answer-shaped" tightening, and it survived because the tightening was
    /// aimed at the wrong axis. Answer grammar cannot separate these two
    /// things: an agent answering "why?" with a numbered list genuinely does
    /// write "No, ..." and "Yes, ..." at the head of its items, and a
    /// recognizer strict enough to exclude that would have to exclude
    /// Claude's own "Yes, proceed" / "No, exit" as well.
    ///
    /// What separates them is that a menu is drawn by a WIDGET, so exactly
    /// one of its options carries a selection pointer, and prose carries
    /// none. Every fixture here satisfies the suffix shape, the size bounds,
    /// the answer grammar, and the vendor question phrase — everything
    /// except the pointer — so a build that dropped that requirement fails
    /// here and nowhere else.
    ///
    /// Both vendors, since each has its own phrase list and each would be
    /// independently wrong.
    #[test]
    fn an_answer_shaped_numbered_explanation_is_not_a_menu() {
        let two_items = "\
⏺ Do you want to know why the migration is a no-op?
  1. No, migration is required only for rows written before the bump.
  2. Yes, and the defaults backfill everything else.";
        let three_items = "\
⏺ Do you trust the summary above?
  1. Yes, the counts match the fixture.
  2. No, the third column was renamed since.
  3. Yes, once the rename is accounted for.";
        // The bare-word shape too, which is the closest an explanation ever
        // gets to a real dialog's option text.
        let bare_words = "\
⏺ Allow command? Here is how I read the two flags:
  1. Yes
  2. No";

        for tail in [two_items, three_items, bare_words] {
            assert_eq!(
                ClaudeIntegration.sharpen(SessionStatus::Running, tail),
                SessionStatus::Running,
                "prose draws no selection pointer, so this is not a menu:\n{tail}"
            );
            assert_eq!(
                CodexIntegration.sharpen(SessionStatus::Idle, tail),
                SessionStatus::Idle,
                "codex must not promote it either:\n{tail}"
            );
        }

        // ...and the same block WITH a pointer is recognized, so this test
        // cannot pass by a recognizer that stopped working altogether.
        let pointed_at = "\
⏺ Do you want to know why the migration is a no-op?
❯ 1. No, migration is required only for rows written before the bump.
  2. Yes, and the defaults backfill everything else.";
        assert_eq!(
            ClaudeIntegration.sharpen(SessionStatus::Running, pointed_at),
            SessionStatus::Waiting,
            "premise: the pointer is the only difference"
        );
    }

    /// The two numeric boundaries of a menu, at and just past each end.
    ///
    /// Both bounds exist for the same reason and fail in opposite
    /// directions: below [`MIN_MENU_CHOICES`] a single numbered line is the
    /// ordinary shape of an agent enumerating one thing, and above
    /// [`MAX_MENU_CHOICES`] a long numbered run is an agent enumerating
    /// many. Off by one at either end is invisible in production until the
    /// day a real dialog sits on the boundary, so both are pinned at the
    /// exact value rather than "a few" and "lots".
    #[test]
    fn a_menu_is_recognized_at_its_size_bounds_and_not_past_them() {
        let menu = |count: usize| {
            let mut tail = String::from("Do you want to proceed?\n");
            for i in 1..=count {
                // Every option answer-shaped and the first one pointed at,
                // so SIZE is the only variable.
                let pointer = if i == 1 { "❯ " } else { "  " };
                tail.push_str(&format!("{pointer}{i}. Yes, option {i}\n"));
            }
            tail
        };
        for (count, expected) in [
            (MIN_MENU_CHOICES - 1, SessionStatus::Running),
            (MIN_MENU_CHOICES, SessionStatus::Waiting),
            (MAX_MENU_CHOICES, SessionStatus::Waiting),
            (MAX_MENU_CHOICES + 1, SessionStatus::Running),
        ] {
            assert_eq!(
                ClaudeIntegration.sharpen(SessionStatus::Running, &menu(count)),
                expected,
                "a menu of {count} options"
            );
        }
    }

    /// The LEXICAL boundary beside the numeric one: an option must BE an
    /// answer word, alone or followed by [`ANSWER_WORD_TERMINATORS`].
    ///
    /// Three families of near-miss, all of them ordinary English an agent
    /// writes into numbered lists, and all of them accepted by weaker
    /// versions of this rule:
    ///
    /// - Prefix matching takes "Note the following" and "Yesterday's run".
    /// - Word-boundary matching (any whitespace ends the word) additionally
    ///   takes "No migration is required" and "Yes it does" — a numbered
    ///   EXPLANATION, which is about the most ordinary thing an agent
    ///   prints.
    /// - Either takes hyphenated compounds: "No-code path", "Yes-style
    ///   wording".
    ///
    /// The accepted rows are the shapes the three recorded dialogs actually
    /// use, so a tightening that broke a real prompt fails here rather than
    /// in the field.
    #[test]
    fn an_option_must_be_an_answer_word_rather_than_merely_start_like_one() {
        for (first, second, expected) in [
            // The recorded shapes: the bare word, and the word plus comma.
            ("Yes", "No", SessionStatus::Waiting),
            ("Yes, proceed", "No, exit", SessionStatus::Waiting),
            ("YES", "no", SessionStatus::Waiting),
            (
                "Yes, and don't ask again for rm commands",
                "No, and tell Claude what to do differently",
                SessionStatus::Waiting,
            ),
            // Prefixes of an answer word.
            (
                "Note the following",
                "Yesterday's run failed",
                SessionStatus::Running,
            ),
            ("Yes, proceed", "Nothing to do", SessionStatus::Running),
            ("Nope", "Yes", SessionStatus::Running),
            // Answer-shaped PROSE: the word is genuinely there, as a word,
            // and the option is still a sentence.
            (
                "No migration is required for existing rows",
                "Yes it does apply here",
                SessionStatus::Running,
            ),
            ("Yes", "No migration is required", SessionStatus::Running),
            // Hyphenated compounds, including the all-hyphenated menu that
            // has no ordinary-prose option to disqualify it.
            ("No-code path", "Yes-style wording", SessionStatus::Running),
            ("Yes-and", "No-but", SessionStatus::Running),
        ] {
            // Pointed at, so the ANSWER grammar is the only variable here;
            // the menu-only signal has its own test.
            let tail = format!("Do you want to proceed?\n❯ 1. {first}\n  2. {second}");
            assert_eq!(
                ClaudeIntegration.sharpen(SessionStatus::Running, &tail),
                expected,
                "options {first:?} / {second:?}"
            );
        }
    }

    /// Every declared selection marker, on both sides of the grammar it
    /// participates in.
    ///
    /// [`MENU_SELECTION_MARKERS`] is a list, and a list is where a glyph
    /// gets added with no test and quietly does nothing — `⏵` sat in the
    /// decoration set for a long time with nothing asserting what it
    /// MEANT. Each marker has two opposite jobs, so each is checked for
    /// both: prefixing a selected option without stopping it from parsing
    /// as one, and — alone on a line, which is what an empty composer
    /// prompt renders as — ending the scan, so an ANSWERED dialog above it
    /// does not read as pending.
    #[test]
    fn every_selection_marker_prefixes_an_option_and_ends_a_scan_alone() {
        for marker in MENU_SELECTION_MARKERS {
            let selected = format!("Do you want to proceed?\n{marker} 1. Yes\n  2. No");
            assert_eq!(
                ClaudeIntegration.sharpen(SessionStatus::Running, &selected),
                SessionStatus::Waiting,
                "{marker:?} must not stop the option it points at from parsing as one"
            );

            let answered = format!("{selected}\n{marker}");
            assert_eq!(
                ClaudeIntegration.sharpen(SessionStatus::Running, &answered),
                SessionStatus::Running,
                "{marker:?} alone is a composer prompt, and must end the scan rather than \
                 trimming away to nothing"
            );
        }
    }

    /// The question is found up to exactly [`PROMPT_QUESTION_LOOKBACK`]
    /// substantive lines above the menu, and not one line further.
    ///
    /// Both sides of the boundary, because the bound is the only thing
    /// keeping the search inside the dialog box: read further up and it
    /// starts matching questions in the TRANSCRIPT above, which is the
    /// unanchored matching this whole shape replaced. An earlier version
    /// examined nine lines while declaring eight — the counter was compared
    /// after the line had already been tested — which is the kind of drift
    /// that only a boundary test catches.
    #[test]
    fn the_question_lookback_reaches_exactly_as_far_as_it_declares() {
        let dialog = |body_lines: usize| {
            let mut tail = String::from("Do you want to proceed?\n");
            for i in 1..=body_lines {
                tail.push_str(&format!("explanatory body line {i}\n"));
            }
            tail.push_str("❯ 1. Yes\n  2. No");
            tail
        };
        // The question sits `body_lines + 1` lines above the menu, so a
        // body of LOOKBACK-1 lines puts it on the last line the search is
        // allowed to read.
        assert_eq!(
            ClaudeIntegration.sharpen(
                SessionStatus::Running,
                &dialog(PROMPT_QUESTION_LOOKBACK - 1)
            ),
            SessionStatus::Waiting,
            "a question on the {PROMPT_QUESTION_LOOKBACK}th body line is still inside the bound"
        );
        assert_eq!(
            ClaudeIntegration.sharpen(SessionStatus::Running, &dialog(PROMPT_QUESTION_LOOKBACK)),
            SessionStatus::Running,
            "one line further is outside it, and must not be found"
        );
    }

    /// Codex's composer, empty, below an ANSWERED approval modal.
    ///
    /// The selection marker is what makes this case sharp. Codex draws `›`
    /// at its selected option AND at its composer prompt, so treating that
    /// glyph as box chrome made an empty composer line trim to nothing —
    /// indistinguishable from the dialog's blank padding — and let the scan
    /// run straight past it into the menu above, reporting `Waiting` at a
    /// session whose user had already answered and whose agent was waiting
    /// on nothing. Skipping the marker inside
    /// [`menu_choice_text`] instead keeps the real modal recognized while
    /// leaving a bare `›` as a substantive line that ends the scan.
    ///
    /// The real modal is asserted alongside it, in the same test, because
    /// the two are the halves of one trade: a fix for either that broke the
    /// other would be no fix at all.
    #[test]
    fn an_empty_codex_composer_below_an_answered_modal_ends_the_scan() {
        let answered = format!("{CODEX_COMMAND_APPROVAL}\n▌ ›");
        assert_eq!(
            CodexIntegration.sharpen(SessionStatus::Running, &answered),
            SessionStatus::Running,
            "an empty composer under the modal means the question was answered:\n{answered}"
        );
        assert_eq!(
            CodexIntegration.sharpen(SessionStatus::Running, CODEX_COMMAND_APPROVAL),
            SessionStatus::Waiting,
            "and the real modal — whose selected option is prefixed with the same glyph — must \
             still be recognized"
        );
    }

    /// A dialog that is still on screen but no longer the BOTTOM of it has
    /// been answered, and must not hold the session at `Waiting`.
    ///
    /// The sharpest case first, and it is the one that killed the previous
    /// design: ONE line of the agent getting on with the work is enough.
    /// A window-based search ("a phrase and two numbered lines somewhere in
    /// the last N lines") matches the answered box for as long as it stays
    /// visible, so a session would read `Waiting` while its agent was
    /// visibly running — the "the column is lying, ignore it" outcome the
    /// whole heuristic exists to avoid. The suffix shape is the mechanism,
    /// and one trailing line is what pins it: nothing weaker distinguishes
    /// the two designs.
    ///
    /// Both vendors, because each has its own phrase list and its own
    /// fixture, and the shape is the only thing they share.
    #[test]
    fn an_answered_dialog_that_is_no_longer_the_bottom_of_the_screen_does_not_count() {
        for (what, dialog, sharpen) in [
            (
                "claude",
                CLAUDE_COMMAND_APPROVAL,
                &ClaudeIntegration as &dyn AgentIntegration,
            ),
            ("codex", CODEX_COMMAND_APPROVAL, &CodexIntegration),
        ] {
            // Exactly one line of ordinary work below the answered dialog.
            let one_line_later = format!("{dialog}\n✻ Thinking… (3s · esc to interrupt)");
            assert_eq!(
                sharpen.sharpen(SessionStatus::Running, &one_line_later),
                SessionStatus::Running,
                "{what}: one line of progress under an answered dialog is enough to mean it \
                 was answered"
            );

            // The composer coming back is the same fact wearing the other
            // vendor-neutral shape (S11's boundary): the prompt block is
            // still on screen, but something else is now bottom-most.
            let composer_back = format!("{dialog}\n╭────────────╮\n│ >          │\n╰────────────╯");
            assert_eq!(
                sharpen.sharpen(SessionStatus::Idle, &composer_back),
                SessionStatus::Idle,
                "{what}: a composer under the block means the question is no longer pending"
            );

            // ...and the dialog DOES still count while it is the
            // bottom-most thing on the screen, or the assertions above
            // would pass with the recognizer removed entirely.
            assert_eq!(
                sharpen.sharpen(SessionStatus::Running, dialog),
                SessionStatus::Waiting,
                "{what}: premise"
            );
        }
    }

    /// A NON-LIVE baseline comes back untouched, whatever the screen says.
    ///
    /// `sharpen` is a public method on a public trait, so "the only caller
    /// passes a live baseline" is a fact about today's tree and not about
    /// the API — and the guard downstream
    /// (`status::waiting_or_baseline`) cannot help here, because `Waiting`
    /// is exactly what that guard lets through. The
    /// failure without this is a session that exited hours ago being
    /// reported as blocked on a human, on the strength of the last thing
    /// its pane happened to be showing.
    ///
    /// Every non-live variant against both vendors, with the tail that
    /// WOULD promote a live baseline, so nothing passes by accident of the
    /// fixture.
    #[test]
    fn a_non_live_baseline_is_never_promoted_by_any_screen() {
        for (what, tail, sharpen) in [
            (
                "claude",
                CLAUDE_COMMAND_APPROVAL,
                &ClaudeIntegration as &dyn AgentIntegration,
            ),
            ("codex", CODEX_COMMAND_APPROVAL, &CodexIntegration),
        ] {
            for baseline in [
                SessionStatus::Exited { exit_code: Some(0) },
                SessionStatus::Exited { exit_code: None },
                SessionStatus::Error {
                    detail: "Permission denied".to_string(),
                },
                SessionStatus::Interrupted,
                SessionStatus::Unknown,
            ] {
                assert_eq!(
                    sharpen.sharpen(baseline.clone(), tail),
                    baseline,
                    "{what} promoted a {baseline:?} session on the strength of its screen"
                );
            }
            // The live baselines it IS allowed to act on, so this test
            // cannot pass by refusing everything.
            assert_eq!(
                sharpen.sharpen(SessionStatus::Idle, tail),
                SessionStatus::Waiting,
                "{what}: premise"
            );
        }
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
