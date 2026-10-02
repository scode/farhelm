//! Agent-kind integrations: everything the supervisor knows about the
//! SPECIFIC agents it launches, behind one seam so that knowledge cannot
//! leak into the rest of the process.
//!
//! `AgentKind` (farhelm-proto's enum) merely NAMES a kind.
//! [`AgentIntegration`] is what knowing the kind buys, and by M6.75 it
//! buys three different things:
//!
//! 1. **The kind seam and its per-session snapshot** (PLAN_M3.md item 7).
//!    At create time a session records its agent kind and how a resume
//!    would be invoked. The kind stays fixed; restart-with can replace the
//!    resume template after a new process spawns. Kind derivation is honestly
//!    dumb — the basename of the invocation's first token — and is done
//!    ONCE, never re-guessed later. Doing it once is not caching but stability:
//!    re-deriving later would consult a PATH, a filesystem, and a heuristic
//!    that may all have changed since, so a session could silently become a
//!    different kind between two restarts and resume through a template
//!    that never matched the agent actually running. A RESUME TEMPLATE may
//!    carry both placeholders; a launch invocation carries only
//!    [`CWD_PLACEHOLDER`], because nothing ever substitutes
//!    [`CONVERSATION_PLACEHOLDER`] into an invocation — written there it
//!    survives as literal text on the agent's command line. The two obey
//!    the same whole-element rule and are filled at different moments:
//!    [`CONVERSATION_PLACEHOLDER`] when the resume argv is built
//!    ([`IntegrationSnapshot::filled_resume_argv`]), and
//!    [`CWD_PLACEHOLDER`] at spawn time in `Supervisor::spawn_agent`,
//!    which is the only place the launch's working directory is known on
//!    every path.
//! 2. **Conversation-identity capture** (item 8). Claude can correlate
//!    discoverable records as a fallback to its per-launch hook.
//!    Codex requires an attributed foreground report and exact root metadata.
//!    Codex, Goose, Pi, and OMP are report-only integrations: they never expose a
//!    record root for Farhelm to scan, and a locator reported under one
//!    vendor's prefix is never accepted for another's. Reporter artifacts
//!    stay in Farhelm's state; Goose alone retains the credential-free
//!    reporter declaration in its conversation metadata. An exact report
//!    always wins
//!    over a scan-derived inference for the kinds that have both.
//! 3. **Activity interpretation** (PLAN_M6_75.md item 2). The generic
//!    classifier can compare successive screens, but it cannot know which
//!    redraws are vendor-owned decoration or which still screen proves a
//!    current question or work indicator. Those narrow interpretations
//!    live here; unfamiliar screen shapes keep the generic behavior.
//!
//! ## Where each agent's behavior lives
//!
//! Everything Farhelm does differently per agent is answered per kind, in a
//! small number of predictable places, so that "all the Codex-specific
//! behavior" is a short list of files and a new kind is a compile error at
//! every decision it needs. The map, by layer:
//!
//! - **Naming the agents.** `farhelm-proto` declares both enums with a
//!   generated `ALL`: `LaunchHarness` (what the user picked; `launch.rs`) and
//!   `AgentKind` (the integration that runs; the crate root). Muse, Cursor,
//!   and OpenCode are harnesses that run as `AgentKind::Generic`.
//! - **What a launch can choose.** Exhaustive `LaunchHarness` methods in
//!   `farhelm-proto/src/launch.rs` (`agent_kind`, `offers_model`,
//!   `offers_effort`, `offers_permission`, `offers_workspace_trust`,
//!   `sole_permission`). The built-in launch profiles (Claude, Codex, Muse,
//!   Cursor and their YOLO variants, with their resume templates) are
//!   `builtin_profiles` in `farhelm-helm/src/store.rs`. The helm's release
//!   catalog and argv compiler stay in
//!   `farhelm-helm/src/launches.rs`: exhaustive matches for the program
//!   name and the model, effort, and YOLO flags, plus harness-specific
//!   branches (Goose's environment and subcommand, Grok's `--no-leader`,
//!   OMP's approval mode, the workspace-trust flags) that were deliberately
//!   left as they are. A new harness gets compile errors for the former and
//!   has to be checked against the latter by hand.
//! - **How the browser shows it.** `farhelm-ui/src/launch_composer.rs` holds
//!   the per-harness vocabulary (labels, display orders checked against
//!   `LaunchHarness::ALL` at compile time, help text); `list/row.rs` maps
//!   harnesses to their marks in `icons.rs`. One exception keeps a direct
//!   comparison: the new-session form's Cursor support notice
//!   (`list/create_form.rs`), which also recognizes Cursor's built-in
//!   profiles by id.
//! - **Pure per-kind decisions in the supervisor.** This module: one
//!   [`AgentIntegration`] impl per kind (resume template, record parsing,
//!   hook argv, [`AgentIntegration::inject_hooks`],
//!   [`AgentIntegration::ambiguous_derived_resume`]), the exhaustive
//!   per-kind functions below (ownership, locators, resume verification,
//!   report vocabularies, executable names), and one file per kind
//!   (`claude.rs`, `codex.rs`, `goose.rs`, `grok.rs`, `omp.rs`, `pi.rs`) for
//!   that kind's own helpers: argv grammar, record and locator parsing, and
//!   the vendor-file verification some locators need. Screen readers:
//!   `screen_reader.rs`.
//! - **Stateful per-kind behavior in the supervisor** (report admission,
//!   capture refresh, resume verification, launch provenance):
//!   `service/core/vendor/<kind>.rs`, dispatched from `service/core.rs` by
//!   exhaustive matches.
//! - **Process-tree attribution:** `procs/<kind>.rs`, each kind's corridor.
//! - **Reporter assets and hook entry points:** `pi_extension.rs` chooses and
//!   materializes which extension a kind loads; the extensions themselves
//!   (`assets/pi-conversation-v1.ts`, `assets/omp-conversation-v1.ts`) own
//!   those agents' event subscriptions, report payloads, and ordering. In
//!   the `farhelm` crate, `hook.rs` (parsing
//!   each vendor's hook callback payload) and `goose_hook.rs` (Goose's MCP
//!   reporter endpoint, which builds Goose's report itself).
//!
//! A new per-agent capability follows the same shape: a required
//! [`AgentIntegration`] method, an exhaustive per-kind function here or on
//! `LaunchHarness`, or a per-kind file, with call sites asking that named
//! question. Not `kind == X`, `matches!(kind, X | Y)`, or a `_` arm over
//! kinds in shared code: each of those silently hands a new kind whatever
//! the branch does for the kinds it does not name.
//!
//! The per-kind decision functions this map points at carry
//! `#[warn(clippy::wildcard_enum_match_arm)]`, so a `_` arm added to one fails
//! the Clippy gate: the free functions and `LocatorVendor`'s methods here,
//! the resume methods on [`IntegrationSnapshot`], the dispatching methods in
//! `service/core.rs`, `LaunchHarness`'s impl, and the UI's per-harness
//! vocabulary. Other exhaustive matches over kinds (the helm's argv compiler
//! and host transport, display names, hook payload parsing) do not carry it
//! and stay exhaustive by review. Give a new per-kind decision its own small
//! function with the attribute rather than burying the match in a larger one.
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
//! ## Screen reading is allowed to be wrong; capture is not
//!
//! The two halves have OPPOSITE failure economics, and reading one with the
//! other's instincts is the mistake this section exists to prevent.
//!
//! Capture's uncertainty is unrecoverable (resuming the wrong conversation
//! is silent and permanent), so it refuses to guess at all — see
//! `capture`'s own docs. A screen reader's uncertainty is a badge in a
//! list: SPEC.md fixes the waiting/idle boundary as heuristic BY CONTRACT
//! and forbids anything about interaction from waiting on a status, so a
//! reader that misses a prompt costs a session that reads idle while it
//! waits, and one that fires early costs the reverse. Both are cosmetic.
//!
//! What neither is allowed to do is cost anything else, and the properties
//! that guarantee it are worth stating exactly rather than loosely. A
//! [`screen_reader::ScreenReader`] takes plain strings and returns a
//! reading: it performs no I/O, awaits nothing, acquires no admission
//! permit, and cannot block on anything a request needs. The sampler calls
//! it while its session's `activity` cell is locked, so the honest claim is
//! "holds one per-entry leaf mutex for a substring search", not "holds no
//! lock at all". What the leaf-lock property guarantees is the part that
//! matters — the mutex is held across no await and alongside no other
//! lock, so the wait is bounded by one screen read or one sample fold and
//! can never participate in a deadlock. Replies never run a reader; they
//! read the stored result.

use farhelm_proto::{AgentKind, RestartOffer};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Locate the executable behind the launcher's simple `env NAME=value` prefix.
/// Option-bearing `env` commands have different parsing rules and are deliberately
/// left unsupported. Injection and resume must agree on this boundary, and so
/// must the helm's YOLO guard, which is why the one copy of the rule lives in
/// `farhelm-proto` beside the YOLO classifier.
pub(crate) use farhelm_proto::yolo::effective_program_index;

mod capture;
pub(crate) mod claude;
pub(crate) mod codex;
pub(crate) mod goose;
pub(crate) mod grok;
pub(crate) mod omp;
pub(crate) mod pi;
#[cfg(test)]
mod screen_fixtures;
pub(crate) mod screen_reader;
pub use capture::{
    CAPTURE_PUBLICATION_GRACE, CAPTURE_WINDOW_AFTER, CAPTURE_WINDOW_BEFORE, Candidate,
    CaptureVerdict, CaptureWindow, CaptureWindowBounds, RecordCorrelators, RecordStamp,
    ScanOutcome, choose, format_rfc3339, now_unix, parse_rfc3339, read_record, scan_records,
    stamp_of,
};
pub(crate) use capture::{
    read_complete as read_complete_bounded_regular_file, read_prefix as read_bounded_regular_file,
};
pub use grok::{
    encode_report as encode_grok_report, validate_event_time as validate_grok_event_time,
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

/// The one argv element an invocation or resume template may use to mean
/// "substitute the session's working directory here" — the directory the
/// launch hands tmux as the pane's cwd, spelled exactly as tmux gets it.
///
/// Exists for wrapper launchers shaped like `wrapper run <dir> <agent...>`,
/// which need the directory as an ARGUMENT rather than as an ambient
/// value. Without it, one profile could only ever launch into a single
/// hardcoded directory — and a profile whose baked-in directory disagreed
/// with the session's real cwd would silently break capture correlation,
/// since the agent would report the wrapper's directory while capture
/// matches against the session's own canonical cwd.
///
/// Same rules as [`CONVERSATION_PLACEHOLDER`], for the same reasons: EXACT
/// whole-element equality (`--dir={cwd}` is literal text, not a match),
/// substitution into the element's own slot so the path is never quoted
/// or word-split, and never as `argv[0]` (see [`ensure_no_cwd_program`]).
pub const CWD_PLACEHOLDER: &str = "{cwd}";

/// Structured Codex launches defer their project-trust override until the
/// target supervisor knows the final working directory. Each marker is one
/// entire argv element after `-c`; raw paths cannot be embedded in the
/// helm's earlier invocation because a fresh checkout has no path yet.
pub const CODEX_TRUSTED_CWD_PLACEHOLDER: &str = "{codex:trusted-cwd}";
pub const CODEX_UNTRUSTED_CWD_PLACEHOLDER: &str = "{codex:untrusted-cwd}";

/// Every whole-element marker a later launch pass substitutes, as the set a
/// VALIDATOR refuses. (Substitution itself matches its own subsets:
/// [`fill_cwd`] only the three directory markers.)
///
/// The helm's model-id check asks [`is_reserved_placeholder`] instead of
/// keeping its own list: a model id that happens to equal a marker would be
/// silently rewritten at spawn, and a hand-copied list is what let that check
/// miss the two Codex markers when they were added. The conversation-id shape
/// check (`is_plausible_conversation_id`) asks it too, for the same reason.
pub const RESERVED_PLACEHOLDERS: &[&str] = &[
    CWD_PLACEHOLDER,
    CONVERSATION_PLACEHOLDER,
    CODEX_TRUSTED_CWD_PLACEHOLDER,
    CODEX_UNTRUSTED_CWD_PLACEHOLDER,
];

/// Whether `value` is exactly one of [`RESERVED_PLACEHOLDERS`].
///
/// Exact equality only, matching how substitution matches: a marker embedded
/// in longer text (`--dir={cwd}`) is literal and is not reserved.
pub fn is_reserved_placeholder(value: &str) -> bool {
    RESERVED_PLACEHOLDERS.contains(&value)
}

/// The short model-visible pointer delivered through every supported vendor's
/// additive instruction channel.
pub const INSTRUCTIONS_POINTER: &str = "farhelm: when the user writes \"$farhelm ...\", run `farhelm agent instructions` and follow its output.";

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

/// Largest vendor session-file path accepted from an injected extension.
const MAX_SESSION_PATH_BYTES: usize = 4 * 1024;

/// Largest encoded vendor locator accepted on the existing conversation field.
///
/// This bound is checked after JSON escaping as well as before decoding, so
/// every value the encoder produces is one the decoder can accept unchanged.
pub const MAX_LOCATOR_BYTES: usize = 8 * 1024;

/// Which vendor's locator spelling a conversation token claims.
///
/// The two harnesses share one locator SHAPE (a versioned JSON object carrying
/// the session id and optional exact file) but never share a wire token: the
/// prefix is part of the identity, and a locator reported for one vendor is
/// never accepted for the other. Closed rather than stringly-typed so a new
/// harness cannot silently inherit both prefixes' rejection rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocatorVendor {
    Pi,
    Omp,
}

#[warn(clippy::wildcard_enum_match_arm)]
impl LocatorVendor {
    /// The exact wire prefix this vendor's locators carry. Pi's bytes are
    /// frozen by every database already holding them; OMP's mirror the shape.
    pub(crate) fn prefix(self) -> &'static str {
        match self {
            LocatorVendor::Pi => "pi:",
            LocatorVendor::Omp => "omp:",
        }
    }

    /// This module's stable spelling of the vendor for human-facing messages —
    /// deliberately not a protocol surface (see [`kind_name`]).
    pub(crate) fn name(self) -> &'static str {
        match self {
            LocatorVendor::Pi => "Pi",
            LocatorVendor::Omp => "OMP",
        }
    }
}

/// Whether a conversation token claims EITHER vendor's locator spelling.
///
/// Used by the plain-id kinds, whose rejection must not depend on the token
/// parsing as anything in particular: a malformed `pi:` or `omp:` prefix is
/// still a claim this build must refuse, not garbage to treat as a bare id.
pub fn is_reserved_locator_token(value: &str) -> bool {
    value.starts_with(LocatorVendor::Pi.prefix())
        || value.starts_with(LocatorVendor::Omp.prefix())
        || value.starts_with(codex::PREFIX)
        || value.starts_with(grok::PREFIX)
}

/// A vendor's exact durable resume target, carried inside the existing
/// conversation column because the vendor needs both values to resume without
/// a scan. One shape for every locator-reporting vendor; the vendor lives in
/// the token's prefix, never in the JSON body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionLocator {
    pub version: u8,
    pub session_id: String,
    pub session_file: Option<String>,
}

/// Encode a validated locator with an unmistakable versioned vendor prefix.
pub fn encode_locator(vendor: LocatorVendor, locator: SessionLocator) -> anyhow::Result<String> {
    validate_locator(vendor, &locator)?;
    let encoded = format!("{}{}", vendor.prefix(), serde_json::to_string(&locator)?);
    if encoded.len() > MAX_LOCATOR_BYTES {
        anyhow::bail!("{} locator exceeds its encoded byte bound", vendor.name());
    }
    Ok(encoded)
}

/// Decode the only conversation-token form accepted for `vendor`'s sessions.
pub fn parse_locator(vendor: LocatorVendor, value: &str) -> anyhow::Result<SessionLocator> {
    if value.len() > MAX_LOCATOR_BYTES {
        anyhow::bail!("{} locator exceeds its encoded byte bound", vendor.name());
    }
    let json = value
        .strip_prefix(vendor.prefix())
        .ok_or_else(|| anyhow::anyhow!("not a {} locator", vendor.name()))?;
    // JSON allows whitespace such as a newline between tokens, but the
    // accepted string itself is stored and logged, so a raw control
    // character would split supervisor log lines. Genuine reports are
    // compact JSON (as `encode_locator` produces) and never contain one.
    if json.chars().any(char::is_control) {
        anyhow::bail!("{} locator contains control characters", vendor.name());
    }
    let locator: SessionLocator = serde_json::from_str(json)?;
    validate_locator(vendor, &locator)?;
    Ok(locator)
}

/// Enforce the parts of a vendor locator that are safe to store before touching
/// the exact file named by it at restart time.
fn validate_locator(vendor: LocatorVendor, locator: &SessionLocator) -> anyhow::Result<()> {
    if locator.version != 1 {
        anyhow::bail!("{} locator version is not supported", vendor.name());
    }
    if !is_plausible_conversation_id(&locator.session_id) {
        anyhow::bail!("{} session id is not plausible", vendor.name());
    }
    if let Some(path) = &locator.session_file
        && (path.len() > MAX_SESSION_PATH_BYTES
            || !Path::new(path).is_absolute()
            || path.chars().any(char::is_control))
    {
        anyhow::bail!(
            "{} session file is not a bounded absolute text path",
            vendor.name()
        );
    }
    Ok(())
}

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
/// Every method is required so each kind makes its capture and resume
/// policy explicit. Returning no scan root is a real policy: report-only
/// kinds must not infer ownership from nearby files. Status recognition is
/// not part of this trait; a kind's screen reader lives in
/// [`screen_reader`], so a kind can have one without an integration and the
/// reverse.
pub trait AgentIntegration: Send + Sync {
    /// The resume invocation this kind gets by default, preserving the
    /// complete original launch argv before Farhelm appends per-launch hook
    /// arguments. Resume arguments are deliberately appended as argv
    /// elements so the user's argument boundaries survive without shell
    /// reconstruction.
    fn default_resume_template(&self, original_argv: &[String]) -> Vec<String>;

    /// Why a DERIVED resume template cannot work for this invocation, or
    /// `None` when it can.
    ///
    /// `original_argv` is the whole launch argv, program first; only kinds
    /// with such a case look past the program. Only consulted when the
    /// create supplied no explicit template: an override is filled verbatim
    /// rather than appended to, so the ambiguity this guards against (where
    /// [`AgentIntegration::default_resume_template`]'s appended selector would
    /// land) does not arise for it. Required rather than defaulted so a new
    /// kind decides whether its derived shape has such a case.
    fn ambiguous_derived_resume(&self, original_argv: &[String]) -> Option<SnapshotError>;

    /// An eligible scan root for this kind and working directory, if scanning
    /// can establish ownership. Claude uses its munged-cwd project directory;
    /// report-only kinds return `None` rather than guessing from nearby files.
    fn record_root(&self, home: &Path, canonical_cwd: &str) -> Option<PathBuf>;

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

    /// Command-line elements that make THIS launch report its conversation
    /// identity through `farhelm internal hook`, appended verbatim after the
    /// user's argv by the caller (`Supervisor::with_hook_argv`). Empty
    /// means "this kind does not use this hook form". Some such kinds use a
    /// different exact reporter; only integrations with a record root may
    /// fall back to scanning.
    ///
    /// Must be PURE: no I/O, no environment reads, and in particular no
    /// consulting the `FARHELM_AGENT_HOOKS` opt-out ([`AgentHooks`]) — the
    /// caller applies that policy before ever calling this method, so an
    /// implementation cannot be asked twice whether its kind is allowed to
    /// be hooked.
    ///
    /// `hook_exe` is the ALREADY-RESOLVED absolute path of the farhelm
    /// binary, as a `str` rather than a [`Path`]: every implementation
    /// embeds it in a vendor's own quoting syntax via `shell_words::quote`,
    /// which only accepts `&str`, so the caller resolves the path to a
    /// `String` once (where the supervisor is constructed) rather than
    /// making every implementation repeat the same fallible
    /// `Path`-to-`str` conversion. Fallible, not lossy: `Path::to_str`
    /// returns `None` for a non-UTF-8 path rather than substituting
    /// replacement characters, so nothing here ever embeds a mangled path
    /// in a vendor's config. A non-UTF-8 `farhelm_exe` is therefore not
    /// this method's problem at all: a supervisor refuses to start on one
    /// (SPEC.md "Paths that are not valid UTF-8"), and the hook policy's
    /// `exe` being `None` makes the caller skip this method and log why.
    ///
    /// `instructions` selects whether the embedded command gets
    /// `--announce`, which makes the hook print one pointer line the agent
    /// reads (`farhelm`'s `hook::POINTER_LINE`). It is a parameter for the
    /// same reason `hook_exe` is one — the policy is resolved once at
    /// supervisor startup and handed down — and it rides INSIDE the
    /// vendor's own quoting rather than beside it, so it cannot be
    /// appended by a caller after the fact.
    fn hook_argv(&self, hook_exe: &str, instructions: AgentInstructions) -> Vec<String> {
        let _ = (hook_exe, instructions);
        Vec::new()
    }

    /// This kind's whole launch-time hook decision: whether and how to
    /// rewrite `argv` so the launch reports its conversation identity, and
    /// what to log about it.
    ///
    /// Pure, like [`AgentIntegration::hook_argv`]: every policy input
    /// (the `FARHELM_AGENT_HOOKS` verdict, the executable path, the vendor
    /// extension artifact, the instructions setting) arrives resolved in
    /// `policy`, and the log line is returned rather than written, so the
    /// caller (`service::core`'s `with_hook_argv_using`) owns the one place
    /// that traces it. Each kind keeps its own check order: Goose, Pi, and
    /// OMP check the invocation's shape before the opt-out, and Goose
    /// rewrites a resume even with hooks off; Claude, Codex, and Grok share
    /// [`inject_hook_argv_tail`]. Required so a new kind states its
    /// decision rather than inheriting another kind's.
    fn inject_hooks(&self, argv: Vec<String>, policy: &HookPolicy<'_>) -> HookInjection;
}

/// The resolved launch-time inputs a hook decision works within (see
/// [`AgentIntegration::inject_hooks`]).
pub struct HookPolicy<'a> {
    /// The `FARHELM_AGENT_HOOKS` setting. Carried whole rather than as a
    /// precomputed verdict so each kind consults it at the same point in its
    /// own check order as it always has (Goose, Pi, and OMP only after their
    /// shape checks).
    pub hooks: &'a AgentHooks,
    /// Whether an injected hook should announce the instructions pointer.
    pub instructions: AgentInstructions,
    /// The farhelm executable path, or `None` when there is none as text to
    /// embed in a vendor's configuration. A supervisor refuses to start on a
    /// path that is not valid UTF-8, so it always passes one; `None` is this
    /// layer's own contract (inject nothing), kept for callers and tests.
    pub exe: Option<&'a str>,
    /// The materialized reporter extension for kinds that load one (Pi,
    /// OMP), or `None` when it is unavailable or the kind has none.
    pub vendor_extension: Option<&'a str>,
}

/// What a hook decision asks its caller to log.
pub enum HookLog {
    /// Nothing: an un-hookable kind or launch shape that is not worth a
    /// line on every launch.
    Silent,
    /// "conversation hook flags not injected", with this reason: a skip that
    /// silently degrades identity capture, so the log is its only evidence.
    Skipped(&'static str),
    /// "conversation hook flags injected". Only the hook-tail kinds
    /// (Claude, Codex) have ever logged this.
    Injected,
}

/// A hook decision's result: the argv to launch, whether it was HOOKED
/// (what the caller's tripwire records), and what to log.
pub struct HookInjection {
    pub argv: Vec<String>,
    pub hooked: bool,
    pub log: HookLog,
}

impl HookInjection {
    /// The unchanged argv, not hooked, with a skip reason to log.
    pub(crate) fn skipped(argv: Vec<String>, reason: &'static str) -> Self {
        HookInjection {
            argv,
            hooked: false,
            log: HookLog::Skipped(reason),
        }
    }

    /// A hooked argv, logged as injected.
    pub(crate) fn injected(argv: Vec<String>) -> Self {
        HookInjection {
            argv,
            hooked: true,
            log: HookLog::Injected,
        }
    }

    /// A hooked argv with no log line: the reporter-extension kinds (Pi,
    /// OMP) have never logged their successful injection, only their skips.
    pub(crate) fn hooked_silently(argv: Vec<String>) -> Self {
        HookInjection {
            argv,
            hooked: true,
            log: HookLog::Silent,
        }
    }
}

/// The hook decision shared by the kinds whose hook is a tail of argv
/// elements from [`AgentIntegration::hook_argv`] (Claude, Codex, Grok),
/// with `vendor_refusal` as the one kind-specific check.
///
/// The order is the contract: the opt-out, then the executable path, then a
/// bare `--`, then the vendor's own refusal, then the tail. A kind whose
/// tail is empty (Grok, which reports through its own configured
/// callbacks) returns silently at the end, after any of the earlier skips
/// has already been logged.
fn inject_hook_argv_tail(
    integration: &dyn AgentIntegration,
    kind: AgentKind,
    mut argv: Vec<String>,
    policy: &HookPolicy<'_>,
    vendor_refusal: fn(&[String]) -> Option<&'static str>,
) -> HookInjection {
    if !policy.hooks.allows(kind) {
        return HookInjection::skipped(argv, "disabled by FARHELM_AGENT_HOOKS");
    }
    let Some(exe) = policy.exe else {
        return HookInjection::skipped(argv, "farhelm executable path is not utf-8");
    };
    // Plan D4: both vendors take a trailing positional prompt, and a bare
    // `--` turns everything after it into that prompt's text. Appending
    // past one would not configure a hook, it would type our flags at the
    // agent.
    if argv.iter().any(|element| element == "--") {
        return HookInjection::skipped(argv, "invocation contains a bare --");
    }
    if let Some(reason) = vendor_refusal(&argv) {
        return HookInjection::skipped(argv, reason);
    }
    let tail = integration.hook_argv(exe, policy.instructions);
    // An integration that offers no tail does not use this hook form.
    // Silently, and WITHOUT the injected line: claiming flags were injected
    // when none were would arm the caller's tripwire against a launch that
    // never had this hook to begin with, and every reader of that log line
    // would be chasing a vendor bug that does not exist.
    if tail.is_empty() {
        return HookInjection {
            argv,
            hooked: false,
            log: HookLog::Silent,
        };
    }
    argv.extend(tail);
    HookInjection::injected(argv)
}

/// Add launch-local reporter controls without persisting them in vendor metadata.
pub(crate) fn with_launch_environment(argv: Vec<String>, assignments: &[String]) -> Vec<String> {
    if argv
        .first()
        .is_some_and(|program| farhelm_proto::yolo::is_env_program(program))
    {
        let mut wrapped = Vec::with_capacity(argv.len() + assignments.len());
        wrapped.push(argv[0].clone());
        wrapped.extend(assignments.iter().cloned());
        wrapped.extend(argv.into_iter().skip(1));
        wrapped
    } else {
        let mut wrapped = Vec::with_capacity(argv.len() + assignments.len() + 1);
        wrapped.push("env".to_string());
        wrapped.extend(assignments.iter().cloned());
        wrapped.extend(argv);
        wrapped
    }
}

/// The command string both integrations embed in their vendor's hook
/// configuration: farhelm's own binary, shell-quoted, running
/// `internal hook`.
///
/// Shared rather than written twice because the two vendors differ only in
/// how they QUOTE this string, never in what it says — and a flag that
/// reached one vendor's launches but not the other's would be a difference
/// nobody chose. Shell-quoted because both vendors run a hook's `command`
/// through a shell rather than exec'ing it, so an unquoted path containing
/// a space would be split into arguments neither can find (verified).
#[warn(clippy::wildcard_enum_match_arm)]
fn hook_command(
    hook_exe: &str,
    instructions: AgentInstructions,
    vendor: farhelm_proto::ReportVendor,
) -> String {
    let vendor = match vendor {
        farhelm_proto::ReportVendor::Claude => "claude",
        farhelm_proto::ReportVendor::Codex => "codex",
        farhelm_proto::ReportVendor::Goose => "goose",
        farhelm_proto::ReportVendor::Pi => "pi",
        farhelm_proto::ReportVendor::Omp => "omp",
        farhelm_proto::ReportVendor::Grok => "grok",
    };
    // The `--vendor` flag is the report envelope's discriminator, sourced
    // from the vendor-specific entry point rather than inferred from the
    // payload: the hook process proves nothing by carrying it (a child can
    // copy argv), but without it the supervisor cannot tell a Claude
    // report from a Codex one before spending vendor I/O. It rides the
    // injected command line so every private entry point declares its
    // adapter; the Goose helper supplies its value internally instead.
    let mut command = format!(
        "{} internal hook --vendor {vendor}",
        farhelm_proto::text::shell_quote(hook_exe)
    );
    if instructions.announces() {
        command.push_str(" --announce");
    }
    command
}

/// The integration for a kind, or `None` for [`AgentKind::Generic`] —
/// which is not an omission but the definition of generic: no record
/// location, no correlators, and therefore no capture, ever.
#[warn(clippy::wildcard_enum_match_arm)]
pub fn integration_for(kind: AgentKind) -> Option<&'static dyn AgentIntegration> {
    match kind {
        AgentKind::Claude => Some(&ClaudeIntegration),
        AgentKind::Codex => Some(&CodexIntegration),
        AgentKind::Goose => Some(&GooseIntegration),
        AgentKind::Pi => Some(&PiIntegration),
        AgentKind::Omp => Some(&OmpIntegration),
        AgentKind::Grok => Some(&GrokIntegration),
        AgentKind::Generic => None,
    }
}

/// Whether `kind`'s screen reader reads `screen` (one visible grid, bottom
/// last) as its agent waiting on the user.
///
/// A cross-crate seam for the fake agent fixture (`farhelm-fixtures`),
/// whose replay mode must draw a menu that the real readers classify as
/// waiting; its test pins that contract against the production reader
/// rather than a copy of it. The supervisor itself classifies through the
/// sampler and never calls this.
pub fn reads_as_waiting(kind: AgentKind, screen: &str) -> bool {
    use screen_reader::{SampleCounts, Screen, ScreenState, reader_for};
    let screen = Screen {
        text: screen,
        title: "",
    };
    reader_for(kind)
        .read(SampleCounts::default(), &screen)
        .state
        == ScreenState::Waiting
}

/// Claude Code: one JSONL record per conversation, under a project
/// directory named after the munged working directory.
struct ClaudeIntegration;

/// Codex binds an attributed foreground report to its exact root rollout.
/// Directory proximity cannot establish ownership when nested invocations exist.
struct CodexIntegration;

/// Goose reports exact identities; it has no record tree Farhelm may scan.
struct GooseIntegration;

/// Pi reports an exact file locator; it has no record tree Farhelm may scan.
struct PiIntegration;

/// OMP reports an exact file locator; it has no record tree Farhelm may scan.
/// Its session files may open with a rewritable title-slot record before the
/// session header, which is the one header-shape difference from Pi.
struct OmpIntegration;

/// Grok is manually hooked and resumes only from an exact verified UUID.
struct GrokIntegration;

impl AgentIntegration for GooseIntegration {
    fn inject_hooks(&self, argv: Vec<String>, policy: &HookPolicy<'_>) -> HookInjection {
        goose::inject_hooks(argv, policy)
    }

    fn ambiguous_derived_resume(&self, _original_argv: &[String]) -> Option<SnapshotError> {
        None
    }

    fn default_resume_template(&self, original_argv: &[String]) -> Vec<String> {
        let mut template = strip_goose_selectors(original_argv);
        if effective_program_index(&template).is_some_and(|index| index + 1 == template.len()) {
            template.push("session".to_string());
        }
        template.extend([
            "--resume".to_string(),
            "--session-id".to_string(),
            CONVERSATION_PLACEHOLDER.to_string(),
        ]);
        template
    }

    fn record_root(&self, _home: &Path, _canonical_cwd: &str) -> Option<PathBuf> {
        None
    }

    fn record_depth(&self) -> usize {
        0
    }

    fn is_record_file(&self, _name: &str) -> bool {
        false
    }

    fn parse_record(&self, _text: &str) -> anyhow::Result<Option<RecordCorrelators>> {
        Ok(None)
    }
}

impl AgentIntegration for PiIntegration {
    fn inject_hooks(&self, argv: Vec<String>, policy: &HookPolicy<'_>) -> HookInjection {
        pi::inject_hooks(argv, policy)
    }

    fn ambiguous_derived_resume(&self, _original_argv: &[String]) -> Option<SnapshotError> {
        None
    }

    fn default_resume_template(&self, original_argv: &[String]) -> Vec<String> {
        let mut template = strip_pi_selectors(original_argv);
        template.extend([
            "--session".to_string(),
            CONVERSATION_PLACEHOLDER.to_string(),
        ]);
        template
    }

    fn record_root(&self, _home: &Path, _canonical_cwd: &str) -> Option<PathBuf> {
        None
    }

    fn record_depth(&self) -> usize {
        0
    }

    fn is_record_file(&self, _name: &str) -> bool {
        false
    }

    fn parse_record(&self, text: &str) -> anyhow::Result<Option<RecordCorrelators>> {
        let first = text
            .lines()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Pi session file has no header"))?;
        let header: serde_json::Value = serde_json::from_str(first)
            .map_err(|_| anyhow::anyhow!("Pi session header is not JSON"))?;
        let object = header
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Pi session header is not an object"))?;
        if object.get("type").and_then(serde_json::Value::as_str) != Some("session") {
            anyhow::bail!("Pi session header has the wrong record type");
        }
        let conversation = object
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Pi session header has no string id"))?;
        if !is_plausible_conversation_id(conversation) {
            anyhow::bail!("Pi session header id is not plausible");
        }
        Ok(Some(RecordCorrelators {
            conversation: conversation.to_string(),
            cwd: String::new(),
            created_at: 0,
        }))
    }
}

impl AgentIntegration for GrokIntegration {
    fn inject_hooks(&self, argv: Vec<String>, policy: &HookPolicy<'_>) -> HookInjection {
        inject_hook_argv_tail(self, AgentKind::Grok, argv, policy, |_| None)
    }

    fn ambiguous_derived_resume(&self, original_argv: &[String]) -> Option<SnapshotError> {
        grok_has_ambiguous_resume_shape(&original_argv[1..])
            .then_some(SnapshotError::GrokAmbiguousResumeBoundary)
    }

    fn default_resume_template(&self, original_argv: &[String]) -> Vec<String> {
        let mut template = original_argv.to_vec();
        let program = effective_program_index(&template).unwrap_or(0);
        if !template[program + 1..]
            .iter()
            .any(|argument| argument == "--no-leader")
        {
            template.insert(program + 1, "--no-leader".to_string());
        }
        template.extend(["--resume".to_string(), CONVERSATION_PLACEHOLDER.to_string()]);
        template
    }

    fn record_root(&self, _home: &Path, _canonical_cwd: &str) -> Option<PathBuf> {
        None
    }

    fn record_depth(&self) -> usize {
        0
    }

    fn is_record_file(&self, _name: &str) -> bool {
        false
    }

    fn parse_record(&self, _text: &str) -> anyhow::Result<Option<RecordCorrelators>> {
        Ok(None)
    }
}

/// OMP's known string flags — each consumes the NEXT argv token as its value
/// unconditionally, even a flag-looking one (`OMP/cli/flag-tables.ts`
/// STRING_SETTERS plus the `--profile`/`--alias` profile surface). ONE table
/// backs everything that walks an OMP argv: the resume-template stripper, the
/// create-time delimiter check, and injection's classifier. Three readers of
/// one vendor grammar that must never drift apart — a consumption rule one
/// walker missed would misplace exactly the tokens the others preserved.
pub(crate) const OMP_STRING_FLAGS: &[&str] = &[
    "--cwd",
    "--config",
    "--add-dir",
    "--mode",
    "--provider",
    "--model",
    "--smol",
    "--slow",
    "--plan",
    "--prewalk-into",
    "--plan-yolo-into",
    "--max-time",
    "--service-tier",
    "--api-key",
    "--system-prompt",
    "--append-system-prompt",
    "--provider-session-id",
    "--prompt-cache-key",
    "--session-dir",
    "--models",
    "--tools",
    "--thinking",
    "--export",
    "--fork",
    "--hook",
    "--extension",
    "-e",
    "--trusted-extension",
    "--plugin-dir",
    "--skills",
    "--approval-mode",
    "--profile",
    "--alias",
];

/// OMP's optional-value flags: the next token is consumed only when it looks
/// like a value — not `-`-prefixed, and not empty, because OMP rejects an
/// empty value for these (`OMP/cli/flag-tables.ts` OPTIONAL_FLAGS).
pub(crate) const OMP_OPTIONAL_FLAGS: &[&str] = &["--resume", "-r", "--session"];

/// OMP's known valueless flags: `VALUELESS_FLAGS` plus the short
/// help/version/print/continue aliases.
pub(crate) const OMP_VALUELESS_FLAGS: &[&str] = &[
    "--help",
    "--version",
    "--allow-home",
    "--continue",
    "--from-claude",
    "--from-codex",
    "--no-session",
    "--no-tools",
    "--no-lsp",
    "--no-pty",
    "--hide-thinking",
    "--advisor",
    "--external-thinking",
    "--prewalk",
    "--no-prewalk",
    "--plan-yolo",
    "--print",
    "--print-thoughts",
    "--no-extensions",
    "--no-skills",
    "--no-rules",
    "--no-title",
    "--auto-approve",
    "--yolo",
    "-h",
    "-v",
    "-c",
    "-p",
];

/// How OMP reads one flag's value from the argv that follows it — the arity
/// half of `flagConsumesValue`, with the unknown-flag cases OMP's bootstrap
/// spells out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OmpFlagArity {
    /// Consumes the next token unconditionally, even a flag-looking one.
    String,
    /// Consumes the next token only when it looks like a value.
    Optional,
    /// Consumes nothing.
    Valueless,
    /// A bare long flag the built-in tables do not know: OMP's bootstrap
    /// reads it as a possible extension string flag that consumes a
    /// value-like successor.
    UnknownLong,
    /// An unknown short flag, which consumes nothing.
    UnknownShort,
}

/// One OMP flag occurrence, named the way OMP's own parsers read it.
pub(crate) struct OmpFlagOccurrence<'a> {
    /// The flag's name: for a long option, everything before an inline
    /// `=value`; short flags stay whole (`-r=x` is its own unknown token,
    /// exactly as OMP's restart rewrite leaves it).
    pub name: &'a str,
    /// Whether the token carries its own inline value (`--model=x`).
    pub inline_value: bool,
    pub arity: OmpFlagArity,
}

/// Classify one argv token as an OMP flag occurrence, or `None` when it is a
/// positional or the end-of-options `--` delimiter.
pub(crate) fn omp_flag_occurrence(argument: &str) -> Option<OmpFlagOccurrence<'_>> {
    if argument == "--" || !argument.starts_with('-') {
        return None;
    }
    let (name, inline_value) = if argument.starts_with("--") {
        match argument.find('=') {
            Some(at) => (&argument[..at], true),
            None => (argument, false),
        }
    } else {
        (argument, false)
    };
    let arity = if OMP_STRING_FLAGS.contains(&name) {
        OmpFlagArity::String
    } else if OMP_OPTIONAL_FLAGS.contains(&name) {
        OmpFlagArity::Optional
    } else if OMP_VALUELESS_FLAGS.contains(&name) {
        OmpFlagArity::Valueless
    } else if name.starts_with("--") {
        OmpFlagArity::UnknownLong
    } else {
        OmpFlagArity::UnknownShort
    };
    Some(OmpFlagOccurrence {
        name,
        inline_value,
        arity,
    })
}

/// Whether `flag` consumes the following token, under OMP's own
/// `flagConsumesValue`: an inline `=value` belongs to the token itself and
/// never consumes a successor; string flags consume any successor;
/// optional-value flags consume only a non-empty value-looking successor; an
/// unknown long flag may be an extension string flag consuming a value-like
/// successor; an unknown short flag consumes nothing.
pub(crate) fn omp_flag_consumes_next(flag: &OmpFlagOccurrence<'_>, next: Option<&str>) -> bool {
    if flag.inline_value {
        return false;
    }
    let value_like = next.is_some_and(|value| !value.starts_with('-'));
    match flag.arity {
        OmpFlagArity::String => next.is_some(),
        OmpFlagArity::Optional => value_like && next != Some(""),
        OmpFlagArity::Valueless => false,
        OmpFlagArity::UnknownLong => value_like,
        OmpFlagArity::UnknownShort => false,
    }
}

/// Whether an OMP argv carries an UNCONSUMED end-of-options delimiter — a
/// `--` in a position where no option claimed it as a value. Walked with the
/// same grammar as the resume-template stripper and injection's classifier,
/// so every boundary that cares agrees on which `--` shapes are genuine:
/// `omp --system-prompt --` carries a prompt literally spelled `--` (no
/// delimiter), while `omp hello --` carries one.
pub(crate) fn omp_has_unconsumed_delimiter(args: &[String]) -> bool {
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        let Some(flag) = omp_flag_occurrence(argument) else {
            if argument == "--" {
                return true;
            }
            index += 1;
            continue;
        };
        if omp_flag_consumes_next(&flag, args.get(index + 1).map(String::as_str)) {
            index += 2;
        } else {
            index += 1;
        }
    }
    false
}

/// Whether appending Grok's exact `--resume <UUID>` selector would be
/// ambiguous or land after an end-of-options boundary.
///
/// The structured Grok launch has neither shape. This check protects the
/// derived-template path used by custom invocations: it refuses an existing
/// selector instead of deleting arguments whose vendor meaning may depend on
/// position, and treats only a whole argv element `--` as the boundary.
fn grok_has_ambiguous_resume_shape(args: &[String]) -> bool {
    args.iter().any(|argument| {
        matches!(
            argument.as_str(),
            "--" | "--resume"
                | "-r"
                | "--continue"
                | "-c"
                | "--session-id"
                | "-s"
                | "--fork-session"
        ) || argument.starts_with("--resume=")
            || argument.starts_with("--session-id=")
    })
}

/// Whether appending Claude's derived `--resume <id>` would collide with a
/// conversation the launch already selects, or land behind `--`.
///
/// Claude's selectors are `--continue`/`-c` (the folder's most recent
/// conversation), `--resume`/`-r`, `--session-id`, `--from-pr` and
/// `--teleport` (checked against Claude Code 2.1.288's `--help`), plus
/// `--fork-session`, which makes a resume fork a copy instead of continuing
/// the conversation it names. Appended beside one of them, the derived
/// `--resume <id>` leaves Claude two answers to "which conversation", and if
/// it honors the original one, Resume opens a different conversation than
/// the one Farhelm captured, whose identity then replaces the valid offer.
/// Behind a whole-element `--`, the appended flag is prompt text, so Resume
/// starts a fresh conversation instead.
///
/// Every argument is scanned, the way [`grok_has_ambiguous_resume_shape`]
/// does, rather than mirroring Claude's option grammar. Two costs follow,
/// of the kind Grok and Codex accept. A wrapper or launcher declared as the
/// Claude kind whose OWN arguments include `-c` or `--` (the documented
/// `sh -c '...' w {cwd} claude` wrapper, `mise exec -- claude`) is
/// refused, though its derived resume would have worked; it needs an
/// explicit resume template, the escape hatch for every refusal here. And clustered or attached short forms (`-pc`,
/// `-r<id>`), which Claude's parser accepts, are not recognized.
fn claude_has_ambiguous_resume_shape(args: &[String]) -> bool {
    args.iter().any(|argument| {
        matches!(
            argument.as_str(),
            "--" | "--continue"
                | "-c"
                | "--resume"
                | "-r"
                | "--session-id"
                | "--from-pr"
                | "--teleport"
                | "--fork-session"
        ) || ["--resume=", "--session-id=", "--from-pr=", "--teleport="]
            .iter()
            .any(|prefix| argument.starts_with(prefix))
    })
}

/// Whether a Codex launch already selects a session, so appending the
/// derived `resume <id>` would produce two selectors.
///
/// Codex selects a session with the `resume` or `fork` SUBCOMMAND, and its
/// argument parser rejects a second one (`codex resume <old> resume <new>`
/// fails with "unexpected argument", verified against codex-cli 0.159.3), so
/// a Restart or Resume of a session launched as `codex resume <old>` would
/// exit with an error instead of continuing. Every argument is scanned, the
/// way [`grok_has_ambiguous_resume_shape`] does, so `codex --yolo resume
/// <id>` is caught too. The cost, the same kind Grok accepts, is that any
/// argument spelled exactly `resume` or `fork` is refused, not only the
/// subcommand: a prompt that is that single word, or an option value such as
/// `-p fork` (a config profile named `fork`), refuses a derived template even
/// though it would have resumed. An explicit resume template (a profile's
/// resume command) remains the escape hatch.
fn codex_has_session_selector(args: &[String]) -> bool {
    args.iter()
        .any(|argument| matches!(argument.as_str(), "resume" | "fork"))
}

/// Remove OMP's session-source flags before inserting its verified file.
///
/// OMP-SPECIFIC, and deliberately not shared with [`strip_pi_selectors`]:
/// OMP's `--resume`/`-r`/`--session` take OPTIONAL values and `--fork` takes a
/// required one (`OMP/cli/flag-tables.ts`), while Pi's equivalents are
/// valueless or differently-shaped — a selector that consumes a value under
/// one vendor's grammar may be valueless under the other's, so the two
/// vendors' stripping cannot share one consumption table without one of them
/// silently mis-stripping. OMP's own restart path
/// (`OMP/cli/flag-tables.ts::restartArgv`) drops these same selectors plus
/// positionals; only the selector half is mirrored here, because Farhelm's
/// policy keeps the original argv (prompt included) intact.
///
/// Long options are NORMALIZED before the selector decision — the name
/// before an inline `=value` is what matches — so every supported inline
/// spelling of every session-source selector drops whole (`--resume=<id>`,
/// `--fork=<id>`, `--continue=x`, `--from-claude=true`), consuming no extra
/// element, while an inline value of a kept flag (`--model=x`) survives as
/// the single element it is. A dropped separate selector consumes its value
/// under its own arity; a kept flag consumes under the shared grammar, which
/// is what keeps an opaque value spelled like a selector (`--model --resume`)
/// from being mistaken for one.
fn strip_omp_selectors(argv: &[String]) -> Vec<String> {
    /// OMP's session-source selectors, by their normalized names, with the
    /// arity each one consumes its separate-form value under.
    fn selector_arity(name: &str) -> Option<OmpFlagArity> {
        match name {
            "--resume" | "-r" | "--session" => Some(OmpFlagArity::Optional),
            "--fork" => Some(OmpFlagArity::String),
            "--continue" | "-c" | "--from-claude" | "--from-codex" => Some(OmpFlagArity::Valueless),
            _ => None,
        }
    }

    let mut kept = Vec::with_capacity(argv.len());
    let mut index = 0;
    while index < argv.len() {
        let argument = &argv[index];
        if index == 0 {
            kept.push(argument.clone());
            index += 1;
            continue;
        }
        // End-of-options: everything after is prompt text for OMP, so the
        // scan stops here and the tail is preserved verbatim. (A genuine
        // delimiter refuses template resolution — see [`SnapshotError`] —
        // but the ORIGINAL argv is still preserved element for element, and
        // a `--` consumed as an option value is just a kept value.)
        if argument == "--" {
            kept.extend(argv[index..].iter().cloned());
            break;
        }
        let next = argv.get(index + 1).map(String::as_str);
        match omp_flag_occurrence(argument) {
            Some(flag) if selector_arity(flag.name).is_some() => {
                if flag.inline_value {
                    // The inline spelling carries its own value: drop the
                    // single token and nothing else.
                    index += 1;
                } else if omp_flag_consumes_next(&flag, next) {
                    index += 2;
                } else {
                    index += 1;
                }
            }
            Some(flag) => {
                kept.push(argument.clone());
                if omp_flag_consumes_next(&flag, next) {
                    kept.push(argv[index + 1].clone());
                    index += 2;
                } else {
                    index += 1;
                }
            }
            None => {
                kept.push(argument.clone());
                index += 1;
            }
        }
    }
    kept
}

/// Read an OMP session file's header id out of its bounded prefix.
///
/// OMP's own parser, deliberately separate from Pi's first-record rule: an
/// OMP session file may open with ONE rewritable title-slot record —
/// `{"type":"title","v":1,"title":...,"updatedAt":...,"pad":"..."}`, a fixed
/// 256-byte line OMP overwrites in place (`OMP/session/session-title-slot.ts`)
/// — before the real session header. The prefix reader hands this function a
/// bounded, possibly mid-line-truncated text, so every malformed or truncated
/// shape refuses (fail closed) rather than being skipped: verification must
/// never accept a file it cannot fully parse as exactly the session it was
/// told the agent is in.
///
/// Refused: a missing first line, unparseable JSON, a leading title slot with
/// the wrong shape (which is NOT skipped — only a well-formed slot may be),
/// a second title-shaped record, any record whose type is not `session`, a
/// missing or non-3 `version`, a missing/non-string/implausible id, and —
/// implicitly — compressed bytes, which are not JSON at all.
fn parse_omp_session_header(text: &str) -> anyhow::Result<String> {
    /// The one record shape a leading title slot may take, checked field by
    /// field (type, v, title, updatedAt, pad, and the optional `source`)
    /// rather than by its type tag alone — a record that merely CLAIMS to be
    /// the title slot is not one. `source` mirrors OMP's own
    /// `parseTitleSlotObject`: it must be absent or exactly `auto`/`user`,
    /// because any other value makes OMP decline the record as a title slot
    /// and reject the whole file as an invalid session header — so a file
    /// Farhelm accepted here would fail inside OMP after the resume launched.
    fn is_title_slot(record: &serde_json::Value) -> bool {
        let Some(object) = record.as_object() else {
            return false;
        };
        object.get("type").and_then(serde_json::Value::as_str) == Some("title")
            && object.get("v").and_then(serde_json::Value::as_u64) == Some(1)
            && object
                .get("title")
                .and_then(serde_json::Value::as_str)
                .is_some()
            && object
                .get("updatedAt")
                .and_then(serde_json::Value::as_str)
                .is_some()
            && object
                .get("pad")
                .and_then(serde_json::Value::as_str)
                .is_some()
            && match object.get("source") {
                None => true,
                Some(source) => matches!(source.as_str(), Some("auto") | Some("user")),
            }
    }

    let mut lines = text.lines().map(str::trim_start);
    let first = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("OMP session file has no header"))?;
    let first: serde_json::Value = serde_json::from_str(first)
        .map_err(|_| anyhow::anyhow!("OMP session prefix is not JSONL"))?;
    let header = if is_title_slot(&first) {
        let second = lines
            .next()
            .ok_or_else(|| anyhow::anyhow!("OMP session file ends after its title slot"))?;
        let second: serde_json::Value = serde_json::from_str(second)
            .map_err(|_| anyhow::anyhow!("the record after OMP's title slot is not JSONL"))?;
        second
    } else {
        first
    };
    let object = header
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("OMP session header is not an object"))?;
    if object.get("type").and_then(serde_json::Value::as_str) != Some("session") {
        anyhow::bail!("OMP session header has the wrong record type");
    }
    if object.get("version").and_then(serde_json::Value::as_u64) != Some(3) {
        anyhow::bail!("OMP session header is not session-record version 3");
    }
    let conversation = object
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("OMP session header has no string id"))?;
    if !is_plausible_conversation_id(conversation) {
        anyhow::bail!("OMP session header id is not plausible");
    }
    Ok(conversation.to_string())
}

impl AgentIntegration for OmpIntegration {
    fn inject_hooks(&self, argv: Vec<String>, policy: &HookPolicy<'_>) -> HookInjection {
        omp::inject_hooks(argv, policy)
    }

    fn ambiguous_derived_resume(&self, original_argv: &[String]) -> Option<SnapshotError> {
        omp_has_unconsumed_delimiter(&original_argv[1..])
            .then_some(SnapshotError::OmpAmbiguousResumeBoundary)
    }

    fn default_resume_template(&self, original_argv: &[String]) -> Vec<String> {
        let mut template = strip_omp_selectors(original_argv);
        template.extend(["--resume".to_string(), CONVERSATION_PLACEHOLDER.to_string()]);
        template
    }

    fn record_root(&self, _home: &Path, _canonical_cwd: &str) -> Option<PathBuf> {
        None
    }

    fn record_depth(&self) -> usize {
        0
    }

    fn is_record_file(&self, _name: &str) -> bool {
        false
    }

    fn parse_record(&self, text: &str) -> anyhow::Result<Option<RecordCorrelators>> {
        let conversation = parse_omp_session_header(text)?;
        Ok(Some(RecordCorrelators {
            conversation,
            cwd: String::new(),
            created_at: 0,
        }))
    }
}

/// Remove only Goose's documented identity selectors and their values.
fn strip_goose_selectors(argv: &[String]) -> Vec<String> {
    strip_selectors(
        argv,
        &["--name", "-n", "--session-id", "--id", "--path"],
        &["--resume", "-r", "--fork", "--edit"],
        &["--name=", "--session-id=", "--id=", "--path=", "-n"],
        &[
            "--provider",
            "--model",
            "--system",
            "--max-turns",
            "--with-extension",
            "--with-builtin",
            "--with-streamable-http-extension",
            "--mode",
        ],
    )
}

/// Remove Pi's session-selection flags before inserting its verified file.
fn strip_pi_selectors(argv: &[String]) -> Vec<String> {
    strip_selectors(
        argv,
        &["--session", "--session-id", "--fork"],
        &["--continue", "-c", "--resume", "-r"],
        &["--session=", "--session-id=", "--fork="],
        &[
            "--provider",
            "--model",
            "--thinking",
            "--append-system-prompt",
            "--system-prompt",
            "--tools",
            "--exclude-tools",
            "--session-dir",
            "-e",
            "--extension",
        ],
    )
}

/// Preserve every unrelated argv boundary while removing selector options.
fn strip_selectors(
    argv: &[String],
    valued: &[&str],
    flags: &[&str],
    joined_prefixes: &[&str],
    preserved_valued: &[&str],
) -> Vec<String> {
    let mut kept = Vec::with_capacity(argv.len());
    let mut index = 0;
    while index < argv.len() {
        let argument = &argv[index];
        if index > 0 && argument == "--" {
            kept.extend(argv[index..].iter().cloned());
            break;
        }
        if index > 0 && valued.contains(&argument.as_str()) {
            index += usize::from(index + 1 < argv.len()) + 1;
            continue;
        }
        if index > 0 && preserved_valued.contains(&argument.as_str()) {
            kept.push(argument.clone());
            if let Some(value) = argv.get(index + 1) {
                kept.push(value.clone());
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if index > 0
            && (flags.contains(&argument.as_str())
                || joined_prefixes
                    .iter()
                    .any(|prefix| argument.starts_with(prefix) && argument.len() > prefix.len()))
        {
            index += 1;
            continue;
        }
        kept.push(argument.clone());
        index += 1;
    }
    kept
}

impl AgentIntegration for ClaudeIntegration {
    fn inject_hooks(&self, argv: Vec<String>, policy: &HookPolicy<'_>) -> HookInjection {
        inject_hook_argv_tail(self, AgentKind::Claude, argv, policy, claude::hook_refusal)
    }

    fn ambiguous_derived_resume(&self, original_argv: &[String]) -> Option<SnapshotError> {
        claude_has_ambiguous_resume_shape(&original_argv[1..])
            .then_some(SnapshotError::ClaudeAmbiguousResumeSelector)
    }

    fn default_resume_template(&self, original_argv: &[String]) -> Vec<String> {
        let mut template = original_argv.to_vec();
        template.extend(["--resume".to_string(), CONVERSATION_PLACEHOLDER.to_string()]);
        template
    }

    fn record_root(&self, home: &Path, canonical_cwd: &str) -> Option<PathBuf> {
        Some(
            home.join(".claude")
                .join("projects")
                .join(munge_cwd(canonical_cwd)),
        )
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

    /// `--settings <json>` carrying one SessionStart hook. Claude Code
    /// MERGES an inline `--settings` JSON's hooks with whatever the user's
    /// own settings files already declare — both fire — so this never
    /// displaces a hook the user configured for themselves, and nothing
    /// under `~/.claude` is ever written: the JSON lives only in this
    /// process's argv and is gone the moment the launch ends (plan §1,
    /// verified against Claude Code 2.1.241). `with_hook_argv` is what
    /// refuses to inject when the user's OWN argv already contains a
    /// `--settings` element — Claude keeps only the LAST such flag, so
    /// appending a second one would silently discard theirs (plan D3);
    /// that skip decision does not belong here, which is why this method
    /// never inspects its caller's argv.
    ///
    /// The `command` string comes from [`hook_command`], which owns the
    /// shell quoting and the `--announce` decision. The JSON is
    /// built with `serde_json::json!` rather than string formatting so a
    /// path that happens to need JSON escaping (a quote, a backslash) can
    /// never produce malformed JSON, only a correctly escaped string. The
    /// `timeout` of 5 seconds is the OUTER bound Claude itself enforces on
    /// the hook process; `farhelm internal hook` budgets 2 seconds
    /// internally (`hook.rs`, a later step), so this is scheduling margin,
    /// not an expectation that the hook will ever need it.
    fn hook_argv(&self, hook_exe: &str, instructions: AgentInstructions) -> Vec<String> {
        let command = hook_command(hook_exe, instructions, farhelm_proto::ReportVendor::Claude);
        let settings = serde_json::json!({
            "hooks": {
                "SessionStart": [{
                    "hooks": [{
                        "type": "command",
                        "command": command,
                        "timeout": 5
                    }]
                }]
            }
        });
        vec!["--settings".to_string(), settings.to_string()]
    }
}

impl AgentIntegration for CodexIntegration {
    fn inject_hooks(&self, argv: Vec<String>, policy: &HookPolicy<'_>) -> HookInjection {
        inject_hook_argv_tail(self, AgentKind::Codex, argv, policy, codex::hook_refusal)
    }

    fn ambiguous_derived_resume(&self, original_argv: &[String]) -> Option<SnapshotError> {
        codex_has_session_selector(&original_argv[1..])
            .then_some(SnapshotError::CodexAmbiguousResumeSelector)
    }

    /// `codex resume <id>`, the audited shape — a SUBCOMMAND rather than a
    /// flag, which is exactly why the default template is per-kind
    /// knowledge instead of one shared string with the command swapped in.
    fn default_resume_template(&self, original_argv: &[String]) -> Vec<String> {
        let mut template = original_argv.to_vec();
        template.extend(["resume".to_string(), CONVERSATION_PLACEHOLDER.to_string()]);
        template
    }

    // A lone nested conversation can be the only file in a capture window.
    // Only an attributed foreground report can select a Codex record.
    fn record_root(&self, _home: &Path, _canonical_cwd: &str) -> Option<PathBuf> {
        None
    }

    fn record_depth(&self) -> usize {
        0
    }

    fn is_record_file(&self, _name: &str) -> bool {
        false
    }

    fn parse_record(&self, text: &str) -> anyhow::Result<Option<RecordCorrelators>> {
        Ok(codex::parse_record(text)?.map(|(record, _)| record))
    }

    /// Five argv elements: the per-launch hook-trust bypass, then two `-c`
    /// overrides that both land in Codex's `SessionFlags` config layer
    /// (`codex-rs/config/src/config_layer_source.rs`, audited in plan §1)
    /// — one turning on the hooks feature gate, one declaring the
    /// SessionStart hook itself.
    ///
    /// `--dangerously-bypass-hook-trust` is the ONLY per-launch trust
    /// bypass Codex offers (plan D2): without it, an untrusted hook
    /// triggers a startup review dialog the TUI cannot get past
    /// unattended. Accepting it costs two things, both documented
    /// user-facing rather than hidden here: Codex prints a warning line
    /// above the composer on every launch (the one line this plan accepts
    /// onto the agent's own terminal), and any hook already sitting
    /// untrusted in the user's own `~/.codex/config.toml` runs during a
    /// farhelm-launched session too, not just ours. `features.hooks=true`
    /// is required because Codex gates the entire hooks subsystem behind
    /// it; passing it again when the user's own config already sets it is
    /// harmless.
    ///
    /// Unlike Claude, Codex's `SessionStart` fires at FIRST PROMPT
    /// SUBMISSION in the TUI, not at process start (verified against
    /// 0.149.1) — so a freshly created session's identity report lags
    /// behind Claude's by however long the user takes to type, and a
    /// session that is created but never prompted never reports at all.
    /// Nothing here compensates for that; it is a property of the
    /// resulting report's TIMING that the supervisor-side handler (a
    /// later step) has to tolerate, not something this argv can fix. The
    /// same lag applies to the `--announce` pointer: on Codex the agent
    /// reads it as developer context alongside the user's first prompt,
    /// not before it.
    ///
    /// The command is rendered as a TOML basic string
    /// ([`toml_basic_string`]) because Codex's `-c` value is TOML, not
    /// JSON, and Codex runs a hook's `command` through a shell exactly as
    /// Claude does — so [`hook_command`]'s shell quoting comes first and
    /// the RESULT of that quoting is what gets TOML-escaped.
    fn hook_argv(&self, hook_exe: &str, instructions: AgentInstructions) -> Vec<String> {
        let command = toml_basic_string(&hook_command(
            hook_exe,
            instructions,
            farhelm_proto::ReportVendor::Codex,
        ));
        vec![
            "--dangerously-bypass-hook-trust".to_string(),
            "-c".to_string(),
            "features.hooks=true".to_string(),
            "-c".to_string(),
            format!(
                "hooks.SessionStart=[{{hooks=[{{type=\"command\",command={command},timeout=5}}]}}]"
            ),
        ]
    }
}

// ---------------------------------------------------------------------
// Hook-argv support (plan §2.2): TOML-string rendering for Codex's `-c`.
//
// A crate-local helper rather than a `toml` runtime dependency, because
// the only thing this crate ever needs to PRODUCE in TOML is one quoted
// string, and pulling in a whole TOML writer for that would be a dependency
// the rest of the crate never touches (`Cargo.lock` already carries
// `toml`/`toml_edit` transitively, through `dx`'s own tooling — not through
// anything this crate links). The `toml` crate is still used, as a
// DEV-dependency only, to round-trip-test the output below.
// ---------------------------------------------------------------------

/// Render `s` as a TOML basic string — the quoted literal that embeds the
/// hook command inside Codex's `-c hooks.SessionStart=...` value
/// ([`CodexIntegration::hook_argv`]).
///
/// ## Why `serde_json::to_string` does almost all the work
///
/// A TOML basic string accepts nearly the exact same escapes JSON does:
/// `\"`, `\\`, `\n`, `\t`, and `\uXXXX` for the other control characters
/// below 0x20. `serde_json::to_string` already produces exactly that
/// escaping, plus the surrounding quotes, for any Rust `&str` — so this
/// function is mostly just reusing a JSON encoder as a TOML encoder for the
/// (large) subset of syntax the two formats happen to share.
///
/// Non-ASCII text is where that reuse could have gone wrong and does not:
/// both formats leave non-ASCII characters as raw UTF-8 bytes rather than
/// escaping them, and TOML allows that unescaped. TOML would also accept
/// an escaped spelling of its own — `\u` with four hex digits for the BMP,
/// `\U` with eight for anything above it, so `😀` may legally be written
/// `\U0001F600` — so raw UTF-8 is a choice between two valid encodings
/// rather than the only one there is. What TOML does NOT accept is the
/// JSON-style SURROGATE PAIR — that same emoji written as two `\u`
/// escapes in the `D800`–`DFFF` range, the way UTF-16 encodes it — since
/// neither half is a Unicode scalar value. That is the one spelling that
/// would break the launch, and it is why this function deliberately does
/// not ASCII-escape non-ASCII characters (some JSON encoders can be
/// configured to do that). `serde_json::to_string` never emits surrogate
/// pairs for a `&str` anyway, so the raw-UTF-8 path is both the simpler
/// and the safer of the two.
///
/// ## The one gap, found by testing rather than by reading a spec
///
/// TOML forbids a raw DEL byte (U+007F) inside a basic string; JSON does
/// not require DEL to be escaped (it is not one of the mandatory
/// below-0x20 control characters), so `serde_json::to_string` emits it
/// raw. This function therefore does one more pass after the JSON
/// encoding: every raw DEL is replaced with the six-character escape
/// `\u007F`. See `toml_basic_string_round_trips_through_escaping` for the
/// exact character set this was verified against, including DEL and a
/// multi-byte emoji.
pub(crate) fn toml_basic_string(s: &str) -> String {
    serde_json::to_string(s)
        .expect("serializing a &str to JSON cannot fail: no float, no map key, no cycle")
        .replace('\u{7f}', "\\u007F")
}

// ---------------------------------------------------------------------
// Codex working-widget recognition, used by `screen_reader`'s Codex reader
//
// The `Working (elapsed • esc to interrupt)` widget counts only where Codex
// draws its CURRENT status: directly above the bottom composer. A copy of
// the same text anywhere else — scrolled history, quoted output — says
// nothing about now. Recognizing "directly above the composer" needs the
// composer's exact geometry, sparkle animation and wrapped drafts
// included, which is what the helpers below establish.
//
// Deliberately not regular expressions, and not a dependency: plain `str`
// methods do exact substring and character-wise prefix work without adding
// a crate to a supervisor whose dependency set is kept small enough to
// cross-compile to musl, and without byte-offset arithmetic that could
// panic on a lossily decoded screen.
// ---------------------------------------------------------------------

/// The only Braille cells Codex's audited sparkle renderer draws.
///
/// These are deliberately individual characters rather than the Braille
/// block. Terminal output, progress spinners, and user text may legitimately
/// contain other Braille, and treating it as composer padding is only valid
/// inside a proven Codex composer.
const CODEX_SPARKLE_DOTS: [char; 8] = [
    '\u{2801}', '\u{2802}', '\u{2804}', '\u{2808}', '\u{2810}', '\u{2820}', '\u{2840}', '\u{2880}',
];

/// Whether a Codex capture shows its current `Working (…)` widget adjoining a
/// recognized bottom composer. A widget anywhere else on the screen is
/// history or quoted text, not the current state.
fn codex_working(raw: &str) -> bool {
    let lines: Vec<&str> = raw.lines().collect();
    codex_composer_top(&lines)
        .is_some_and(|upper_padding| codex_status_region_start(&lines, upper_padding).is_some())
}

/// Locate a current Codex status widget directly above the composer.
///
/// The widget renders its header first, followed by an optional hook row
/// and up to three detail rows. All continuation rows carry the audited
/// four-column branch indent. Capping that region matters: an unbounded
/// reverse search could turn a status line retained in output history into
/// evidence about the current task.
fn codex_status_region_start(lines: &[&str], upper_padding: usize) -> Option<usize> {
    let mut header = upper_padding.checked_sub(1)?;
    let mut continuations = 0;
    while is_codex_status_continuation(lines[header]) {
        continuations += 1;
        if continuations > 4 {
            return None;
        }
        header = header.checked_sub(1)?;
    }
    is_codex_running_status(lines[header]).then_some(header)
}

/// Match the indentation shared by wrapped details and hook overflow.
fn is_codex_status_continuation(line: &str) -> bool {
    line.strip_prefix("  └ ")
        .is_some_and(|text| !text.is_empty())
        || line
            .strip_prefix("    ")
            .is_some_and(|text| !text.is_empty())
}

/// How many padding rows may separate the status widget from the composer's
/// prompt row. The captured 0.159.0 working screens show two; one is the
/// layout the synthetic tests below use. Each extra row widens the window in
/// which a status line left in history above an idle composer's blank rows
/// would count as current, so the cap is the smallest the captures need.
const CODEX_MAX_PADDING_ABOVE_PROMPT: usize = 2;

/// Find the topmost padding row above a bottom Codex composer.
///
/// The composer is the last prompt row (`›` at column zero) with a padding
/// row right above it and nothing below it down to the last row but rows in
/// Codex's two-column indent: wrapped draft lines, padding, and the footer
/// rows (on 0.159.0 a status line such as the model and context use, then
/// the key hints). The last row must not be padding, which ties the match
/// to the bottom of the screen; it may be unindented only because the
/// synthetic layouts in the tests use an unindented footer, while real
/// 0.159.0 footers are always indented.
///
/// This geometry alone does not keep dialogs out: Codex's numbered menus
/// (`› 1. Yes, proceed`) can match it. They are kept out by the reader's
/// dialog checks, which run before the working check, and by
/// [`codex_status_region_start`] finding no status row above them.
///
/// `screen_reader::codex_has_composer` answers the neighbouring question
/// for the idle reading with a looser rule (any `›` row, anything indented
/// or padding below it, no padding required above). The two differ on
/// purpose: this one has to find the padding above the prompt to locate the
/// widget, and it may be stricter because a false "working" pins a finished
/// task. A Codex layout change likely needs both updated.
///
/// An earlier version required exactly one footer row with a padding row
/// right above it, which no real 0.159.0 screen has (they end in two footer
/// rows), so the status backstop this feeds never fired.
fn codex_composer_top(lines: &[&str]) -> Option<usize> {
    if lines.len() < 4 || is_codex_composer_padding(lines.last()?) {
        return None;
    }
    let last = lines.len() - 1;
    let prompt = lines.iter().rposition(|line| is_codex_prompt_row(line))?;
    if prompt == last
        || !lines[prompt + 1..last]
            .iter()
            .all(|line| is_codex_wrapped_row(line))
    {
        return None;
    }
    let mut top = prompt.checked_sub(1)?;
    if !is_codex_composer_padding(lines[top]) {
        return None;
    }
    while prompt - top < CODEX_MAX_PADDING_ABOVE_PROMPT
        && top > 0
        && is_codex_composer_padding(lines[top - 1])
    {
        top -= 1;
    }
    Some(top)
}

/// Whether a row is either blank or contains only the known Codex particles.
fn is_codex_composer_padding(line: &str) -> bool {
    line.chars()
        .all(|character| character.is_whitespace() || CODEX_SPARKLE_DOTS.contains(&character))
}

/// Whether a row has Codex's left-edge composer prompt.
///
/// No ASCII space is required after the prompt: a sparkle is allowed in its
/// column-one blank cell, and requiring a space would reject the observed
/// idle frame this normalization exists to handle. An empty first draft row
/// captures as the bare prompt because tmux removes trailing blank cells.
fn is_codex_prompt_row(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('›') else {
        return false;
    };
    rest.chars().next().is_none_or(|character| {
        character.is_whitespace() || CODEX_SPARKLE_DOTS.contains(&character)
    })
}

/// Whether a wrapped textarea row has only blank/sparkle cells in its indent.
///
/// Tmux trims trailing blanks: a blank draft row can therefore contain zero
/// characters, or just one sparkle. Missing captured cells are blank here;
/// a present non-padding character in either indent cell still rejects it.
fn is_codex_wrapped_row(line: &str) -> bool {
    line.chars()
        .take(2)
        .all(|character| character.is_whitespace() || CODEX_SPARKLE_DOTS.contains(&character))
}

/// Match the pinned Codex status header, including its reduced-motion form.
///
/// The audited source defaults to `Working` and permits either no activity
/// indicator or `•`/`◦` before it. Unknown labels and symbols deliberately
/// do not count as work: without that bound arbitrary prose would be
/// indistinguishable from the widget.
fn is_codex_running_status(line: &str) -> bool {
    let Some(header) = line
        .strip_prefix("Working ")
        .or_else(|| strip_codex_spinner(line))
    else {
        return false;
    };
    let Some(rest) = header.strip_prefix('(') else {
        return false;
    };
    let Some((elapsed, suffix)) = rest.split_once(')') else {
        return false;
    };
    let Some(elapsed) = elapsed.strip_suffix(" • esc to interrupt") else {
        return false;
    };
    is_codex_elapsed(elapsed)
        && (suffix.is_empty()
            || suffix
                .strip_prefix(" · ")
                .is_some_and(|context| !context.is_empty()))
}

/// Remove one animated activity cell before the fixed status header.
fn strip_codex_spinner(line: &str) -> Option<&str> {
    line.strip_prefix("• ")
        .or_else(|| line.strip_prefix("◦ "))?
        .strip_prefix("Working ")
}

/// Validate the exact compact elapsed forms emitted by `fmt_elapsed_compact`.
fn is_codex_elapsed(elapsed: &str) -> bool {
    fn unpadded(value: &str) -> Option<u64> {
        let parsed = value.parse::<u64>().ok()?;
        (parsed.to_string() == value).then_some(parsed)
    }

    fn two_digits_below_sixty(value: &str) -> bool {
        value.len() == 2
            && value.bytes().all(|byte| byte.is_ascii_digit())
            && value.parse::<u8>().is_ok_and(|parsed| parsed < 60)
    }

    let Some(without_seconds_unit) = elapsed.strip_suffix('s') else {
        return false;
    };
    if let Some((hours, rest)) = without_seconds_unit.split_once("h ") {
        let Some((minutes, seconds)) = rest.split_once("m ") else {
            return false;
        };
        return unpadded(hours).is_some_and(|hours| hours > 0)
            && two_digits_below_sixty(minutes)
            && two_digits_below_sixty(seconds);
    }
    if let Some((minutes, seconds)) = without_seconds_unit.split_once("m ") {
        return unpadded(minutes).is_some_and(|minutes| (1..60).contains(&minutes))
            && two_digits_below_sixty(seconds);
    }
    unpadded(without_seconds_unit).is_some_and(|seconds| seconds < 60)
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
/// TWO untrusted sources, and TWO downstream re-checks, all sharing this
/// one predicate — which is the whole reason it is `pub(crate)` rather
/// than private to this module. Splitting it would let the four drift, and
/// the only way that drift shows up is a resume that runs the wrong
/// command.
///
/// The sources are where a value first arrives from outside: the record
/// parse above, which reads ids out of files the supervisor did not write,
/// and the supervisor's `ReportConversation` handler
/// (`service/handlers.rs` — plan §2.4), which takes an agent-REPORTED id
/// off the wire before anything is retained. The re-checks are the two
/// points where a stored value becomes user-facing again:
/// [`IntegrationSnapshot::restart_offer`], so an id this build would refuse
/// to substitute is never OFFERED, and
/// [`IntegrationSnapshot::filled_resume_argv`], the last point before the
/// value becomes an argv element. The re-checks are not redundant with the
/// sources: a column written by an older build, or edited by hand, reaches
/// them without ever passing a source check.
///
/// Slot substitution is the defence that lets this stay a SHAPE check
/// rather than a sanitizer. `filled_resume_argv` replaces a whole argv
/// ELEMENT, so an id is never quoted, escaped, or word-split on its way
/// into a command line, and shell metacharacters in one are inert. What
/// that does not buy is the paragraph below.
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
///
/// ## The placeholders themselves
///
/// An id equal to any of [`RESERVED_PLACEHOLDERS`] is refused even though
/// it is graphic ASCII. Substitution runs in two passes — identity first,
/// the working directory later in `spawn_agent` — and an id spelled `{cwd}`
/// (or one of the Codex trust markers, which that later pass turns into a
/// Codex project-trust setting) would be written into the template by the
/// first pass and then rewritten by the second, so a record file (which any
/// local process can write) could steer what the resume argv carries.
/// Refusing the literals keeps the passes from reinterpreting each other's
/// output; asking the shared set means a marker added later is covered too.
pub(crate) fn is_plausible_conversation_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_CONVERSATION_ID_LEN
        && !id.starts_with('-')
        && !is_reserved_placeholder(id)
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

/// The per-session integration settings (PLAN_M3.md item 7): a fixed agent
/// kind and the template the next resume will use. Restart-with may replace
/// the template after its new process spawns, without re-deriving the kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationSnapshot {
    pub kind: AgentKind,
    /// The resume invocation as an argv VECTOR, so a path with spaces
    /// survives without quoting. `None` means this session has no resume
    /// invocation at all. An active integration always has at least its
    /// derived default; a newly introduced kind may retain an explicit
    /// template before its capture integration is enabled.
    pub resume_template: Option<Vec<String>>,
}

/// Why a create's integration snapshot could not be resolved.
///
/// One variant today, and it stays an enum rather than a bare string
/// because the caller has to map it to a wire `ErrorKind` — a decision
/// that belongs at the boundary, not here.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SnapshotError {
    /// A generic kind has no conversation-identity capture, so its fallback
    /// command cannot substitute this placeholder. Refuse it at create time
    /// rather than storing a restart command that can never be used.
    #[error(
        "a generic session cannot supply conversation identity to its resume template; remove \
         the {CONVERSATION_PLACEHOLDER} placeholder or use an integrated agent kind"
    )]
    GenericTemplateHasPlaceholder,
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
    /// An OMP launch whose DERIVED resume template would have to be appended
    /// behind a GENUINE end-of-options delimiter: OMP reads everything after
    /// an unconsumed `--` as prompt text, so the appended
    /// `--resume <verified-file>` could only ever be read as more prompt,
    /// and the derived template would silently launch a fresh conversation
    /// instead of resuming. Three shapes are deliberately NOT this error:
    /// a `--` consumed as an option value (`--system-prompt --` is a prompt
    /// spelled `--`, and appended flags are still options), an explicit
    /// template override (filled verbatim, never appended), and every other
    /// kind. Refusing the create is the fail-closed outcome; the error text
    /// is what the user reads.
    #[error(
        "an OMP invocation containing a bare \"--\" cannot carry a verified resume: OMP reads \
         everything after \"--\" as prompt text, so the appended --resume flag would never \
         resume the captured conversation"
    )]
    OmpAmbiguousResumeBoundary,
    /// A derived Grok template cannot safely add an exact selector beside
    /// an existing session selector or beyond `--`.
    #[error(
        "a Grok invocation already contains a session selector or end-of-options boundary; \
         Farhelm cannot append an unambiguous exact --resume target"
    )]
    GrokAmbiguousResumeBoundary,
    /// A derived Codex template cannot append `resume <id>` to a launch that
    /// already selects a session with the `resume` or `fork` subcommand (or
    /// carries an argument spelled that way): Codex rejects a second
    /// selector, so the Resume would fail.
    #[error(
        "a Codex invocation that already contains \"resume\" or \"fork\" cannot be resumed by \
         Farhelm: Codex accepts only one session selector, so the resume command Farhelm would \
         add could never start; launch it without that argument, or from a profile that sets \
         its own resume command"
    )]
    CodexAmbiguousResumeSelector,
    /// A derived Claude template cannot append `--resume <id>` to a launch
    /// that already selects a conversation (`--continue`/`-c`,
    /// `--resume`/`-r`, `--session-id`, `--from-pr`, `--teleport`,
    /// `--fork-session`, or an argument spelled that way) or carries a
    /// whole-element `--`: the first could resume a different conversation
    /// than the captured one, the second would read the appended flag as
    /// prompt text.
    #[error(
        "a Claude invocation that already contains --continue, -c, --resume, -r, --session-id, \
         --from-pr, --teleport, --fork-session or a bare \"--\" cannot be resumed by Farhelm: \
         the --resume flag Farhelm would add could open a different conversation or be read as \
         prompt text; launch it without that argument, or from a profile that sets its own \
         resume command"
    )]
    ClaudeAmbiguousResumeSelector,
}

/// This module's stable spelling of a kind for human-facing messages.
/// Deliberately not the wire serde representation: an error string is not
/// a protocol surface and must not start depending on one.
#[warn(clippy::wildcard_enum_match_arm)]
fn kind_name(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Claude => "claude",
        AgentKind::Codex => "codex",
        AgentKind::Goose => "goose",
        AgentKind::Pi => "pi",
        AgentKind::Omp => "omp",
        AgentKind::Grok => "grok",
        AgentKind::Generic => "generic",
    }
}

impl IntegrationSnapshot {
    /// Resolve a create's snapshot from the parsed invocation argv and the request's
    /// optional overrides (PLAN_M3.md item 7).
    ///
    /// The precedence is: an explicit override always wins over derivation,
    /// and derivation is basename recognition of `argv0` — nothing more.
    /// `env claude`, a wrapper script, or a shell alias all classify as
    /// `Generic`, which is honest rather than clever: the override fields
    /// exist precisely because this heuristic cannot be made smart without
    /// becoming wrong in ways nobody could predict.
    ///
    /// The default template preserves the ORIGINAL argv, not a canonical
    /// command name or a shell reconstruction, so `/opt/bin/claude
    /// --dangerously-skip-permissions` resumes through that original argv
    /// before its per-kind suffix.
    ///
    /// Callers must provide a non-empty executable argv. Create validation
    /// establishes that precondition before resolution; an empty slice has
    /// no program from which to derive a kind and is therefore invalid.
    ///
    /// Validation rejects templates whose conversation-identity requirements
    /// do not match the resolved kind: integrated kinds need the placeholder,
    /// while generic kinds cannot use it because they have no identity capture.
    /// A derived template must also have an unambiguous place for its resume
    /// selector. OMP refuses a genuine end-of-options delimiter; Grok and
    /// Claude also refuse an existing selector because their derived form
    /// owns that argument, and Codex refuses an argument spelled `resume` or
    /// `fork` because it accepts only one session selector (see
    /// [`SnapshotError`] for the exact cases).
    pub fn resolve(
        original_argv: &[String],
        kind_override: Option<AgentKind>,
        template_override: Option<Vec<String>>,
    ) -> Result<IntegrationSnapshot, SnapshotError> {
        let kind = kind_override.unwrap_or_else(|| derive_kind(&original_argv[0]));
        let integration = integration_for(kind);
        let explicit_override = template_override.is_some();
        let resume_template = template_override
            .or_else(|| integration.map(|i| i.default_resume_template(original_argv)));
        // The refusal is scoped to the shape that cannot work: the DERIVED
        // template appends `--resume` at the tail, which a genuine delimiter
        // would turn into prompt text. A `--` consumed as an option value is
        // not a delimiter (`omp --system-prompt --` carries a prompt spelled
        // `--`, and appended flags remain options), and an explicit override
        // is filled verbatim rather than appended, so neither refuses here.
        if !explicit_override
            && let Some(error) = integration.and_then(|i| i.ambiguous_derived_resume(original_argv))
        {
            return Err(error);
        }
        if integration.is_none() && template_has_placeholder(resume_template.as_deref()) {
            return Err(SnapshotError::GenericTemplateHasPlaceholder);
        }
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
    #[warn(clippy::wildcard_enum_match_arm)]
    /// offering a garbled command line.
    pub fn restart_offer(&self, captured: Option<&str>, ownership_version: i64) -> RestartOffer {
        // Provenance gate: kinds with an implemented ownership proof offer
        // exact Resume only for bindings admitted under this contract
        // (version 1). The deliberate Codex exception keeps existing valid
        // `codex:` v1 tokens — produced under the two-proof contract long
        // before the version column existed — resumable at version 0;
        // bare IDs stay excluded exactly as before, and no row is ever
        // backfilled. Unknown or future versions preserve their data but
        // refuse exact Resume and readiness promotion, and a later
        // attributed report may replace them under the usual CAS. Kinds
        // without an implemented proof keep today's offer behavior until
        // their PR flips the predicate above; the gate shape does not
        // change when they do.
        if ownership_proof_implemented(self.kind)
            && ownership_version != 1
            && !(ownership_version == 0 && accepts_unversioned_ownership(self.kind))
        {
            return RestartOffer::FreshOnly;
        }
        // Each kind reads its captured identity in its own vocabulary, the
        // same one `filled_resume_argv` substitutes from, so an offer and the
        // command it promises cannot disagree.
        match self.kind {
            AgentKind::Codex => {
                match captured.and_then(|value| codex::CodexLocator::parse(value).ok()) {
                    Some(locator)
                        if locator.resume_id().is_some() && self.resume_template.is_some() =>
                    {
                        RestartOffer::Resume
                    }
                    _ => RestartOffer::FreshOnly,
                }
            }
            AgentKind::Grok => {
                match captured.and_then(|value| grok::GrokLocator::parse(value).ok()) {
                    Some(locator)
                        if locator.resume_id().is_some()
                            && locator.selected_at.is_some()
                            && self.resume_template.is_some() =>
                    {
                        RestartOffer::Resume
                    }
                    _ => RestartOffer::FreshOnly,
                }
            }
            AgentKind::Pi => self.typed_locator_offer(LocatorVendor::Pi, captured),
            AgentKind::Omp => self.typed_locator_offer(LocatorVendor::Omp, captured),
            AgentKind::Claude | AgentKind::Goose | AgentKind::Generic => {
                self.plain_id_offer(captured)
            }
        }
    }

    /// The offer for a kind whose identity is a typed locator (Pi, OMP):
    /// Resume only when the locator names its exact saved file.
    fn typed_locator_offer(&self, vendor: LocatorVendor, captured: Option<&str>) -> RestartOffer {
        match captured
            .and_then(|value| parse_locator(vendor, value).ok())
            .and_then(|locator| locator.session_file)
        {
            Some(_) if self.resume_template.is_some() => RestartOffer::Resume,
            _ => RestartOffer::FreshOnly,
        }
    }

    /// The offer for a kind whose identity is a plain conversation id
    /// (Claude, Goose), and for Generic sessions, which never have an
    /// identity but may carry an explicit fallback template.
    fn plain_id_offer(&self, captured: Option<&str>) -> RestartOffer {
        // An identity this build would refuse to substitute
        // (`is_plausible_conversation_id` — an option-shaped id being the
        // case that matters) is not something to OFFER a resume for either:
        // the offer would be one `filled_resume_argv` then declines to
        // honor, which is a confusing refusal at the worst moment. Judged
        // here so the offer and the command it promises can never disagree.
        let captured = captured
            .filter(|id| is_plausible_conversation_id(id) && !is_reserved_locator_token(id));
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
    /// The substitution LOOP itself now lives in [`fill_slots`], shared
    /// with [`fill_cwd`]'s `{cwd}` handling, so both placeholders have
    /// exactly one implementation of the whole-element rule. `fill_slots`
    /// skips slot 0 as a backstop only; the actual refusal of a
    /// `{conversation}`-first template is [`ensure_resume_template`]'s job,
    /// enforced long before a template can reach this method.
    ///
    /// PLAN_M3.md item 9 is what RUNS this; it exists here so the capture
    /// tests can assert the end-to-end promise ("resume this exact
    #[warn(clippy::wildcard_enum_match_arm)]
    /// conversation") rather than only the id in isolation.
    pub fn filled_resume_argv(&self, conversation: &str) -> Option<Vec<String>> {
        let replacement = match self.kind {
            AgentKind::Codex => codex::CodexLocator::parse(conversation)
                .ok()?
                .resume_id()?
                .to_string(),
            AgentKind::Pi => {
                parse_locator(LocatorVendor::Pi, conversation)
                    .ok()?
                    .session_file?
            }
            AgentKind::Omp => {
                parse_locator(LocatorVendor::Omp, conversation)
                    .ok()?
                    .session_file?
            }
            AgentKind::Grok => grok::GrokLocator::parse(conversation)
                .ok()?
                .resume_id()?
                .to_string(),
            AgentKind::Claude | AgentKind::Goose | AgentKind::Generic => {
                if is_reserved_locator_token(conversation)
                    || !is_plausible_conversation_id(conversation)
                {
                    return None;
                }
                conversation.to_string()
            }
        };
        // Re-validated at the boundary it actually matters at, not only
        // where the value was captured: a durable column written by an
        // older build (or edited by hand) reaches this function too, and
        // this is the last point before the value becomes an argv element.
        // See `is_plausible_conversation_id` for why a leading dash is the
        // case worth being paranoid about.
        let mut filled = self.resume_template.clone()?;
        fill_slots(&mut filled, CONVERSATION_PLACEHOLDER, &replacement);
        Some(filled)
    }
}

/// Whether this kind's ownership proofs (foreground attribution plus its own
/// kind-specific record check) are implemented, so its admissions write
/// versioned ownership provenance and its exact-resume offers require it.
///
/// Codex, Grok, and OMP have complete proofs. Goose, Claude, and Pi keep
/// their arm false until their own proof lands. `false` does not mean "no
/// check": Claude's legacy admission still refuses a report unless the pane
/// process or its direct child ran the hook (a positional check, with no
/// record proof and no versioned provenance), so flipping Claude here would
/// be a separate decision that withdraws every existing Claude resume
/// offer until its next proven report. Admission, durable writers,
/// the refresh mirror, and every offer surface consult this one predicate,
/// so a later kind needs one deliberate flip rather than scattered match
/// changes. New framework entry points default to deny; legacy paths are
/// preserved, not re-blessed, until their kind flips.
#[warn(clippy::wildcard_enum_match_arm)]
pub fn ownership_proof_implemented(kind: AgentKind) -> bool {
    match kind {
        AgentKind::Codex | AgentKind::Grok | AgentKind::Omp => true,
        AgentKind::Claude | AgentKind::Goose | AgentKind::Pi => false,
        AgentKind::Generic => false,
    }
}

/// Whether this kind's exact-resume offers also accept an ownership binding
/// written before the ownership version column existed (version 0).
///
/// Only Codex: its `codex:` tokens were already produced under the two-proof
/// contract long before the column existed, so they stay resumable. Every
/// other kind with an implemented proof requires version 1. Exhaustive so a
/// kind that flips [`ownership_proof_implemented`] answers this too.
#[warn(clippy::wildcard_enum_match_arm)]
fn accepts_unversioned_ownership(kind: AgentKind) -> bool {
    match kind {
        AgentKind::Codex => true,
        AgentKind::Claude
        | AgentKind::Goose
        | AgentKind::Pi
        | AgentKind::Omp
        | AgentKind::Grok
        | AgentKind::Generic => false,
    }
}

/// The typed-locator vocabulary a kind's reported identity uses, or `None`
/// for a kind whose identity is not a typed locator (a plain id, Codex's and
/// Grok's own locators, or no identity at all).
#[warn(clippy::wildcard_enum_match_arm)]
pub fn locator_vendor(kind: AgentKind) -> Option<LocatorVendor> {
    match kind {
        AgentKind::Pi => Some(LocatorVendor::Pi),
        AgentKind::Omp => Some(LocatorVendor::Omp),
        AgentKind::Claude
        | AgentKind::Codex
        | AgentKind::Goose
        | AgentKind::Grok
        | AgentKind::Generic => None,
    }
}

/// Whether this kind's reported binding is re-verified against its exact
/// vendor evidence when readiness is refreshed (Codex's root record, Grok's
/// record pair). Other kinds' bindings are taken as reported until a restart
/// asks.
#[warn(clippy::wildcard_enum_match_arm)]
pub fn refreshes_reported_capture(kind: AgentKind) -> bool {
    match kind {
        AgentKind::Codex | AgentKind::Grok => true,
        AgentKind::Claude
        | AgentKind::Goose
        | AgentKind::Pi
        | AgentKind::Omp
        | AgentKind::Generic => false,
    }
}

/// Whether a Resume restart of this kind verifies its captured target on
/// disk (the exact saved file or record pair) before relaunching.
#[warn(clippy::wildcard_enum_match_arm)]
pub fn verifies_resume_target(kind: AgentKind) -> bool {
    match kind {
        AgentKind::Pi | AgentKind::Omp | AgentKind::Grok => true,
        AgentKind::Claude | AgentKind::Codex | AgentKind::Goose | AgentKind::Generic => false,
    }
}

/// The refusal a Resume restart of this kind gets when its offer is not
/// Resume, for a kind that must never resume without a verified target;
/// `None` for kinds whose unverified resume is left to the offer alone.
///
/// Only Codex: a legacy Codex identity is unattributed, so resuming it could
/// select another transcript, and the restart refuses outright instead.
#[warn(clippy::wildcard_enum_match_arm)]
pub fn unverified_resume_refusal(kind: AgentKind) -> Option<&'static str> {
    match kind {
        AgentKind::Codex => Some(
            "this Codex conversation has no verified foreground resume target: its legacy identity is unattributed, \
             or its exact record is unavailable; nothing was relaunched and no other transcript was selected",
        ),
        AgentKind::Claude
        | AgentKind::Goose
        | AgentKind::Pi
        | AgentKind::Omp
        | AgentKind::Grok
        | AgentKind::Generic => None,
    }
}

/// The doorway refusal for a report whose foreground-transition `source` is
/// outside its vendor's vocabulary, or `None` when it is inside it (or the
/// vendor has no such vocabulary).
///
/// Codex and OMP report named transitions (OMP's four subscribed event
/// tags, `session_switch` carrying its opaque upstream reason); admission
/// re-checks the same allowlists.
#[warn(clippy::wildcard_enum_match_arm)]
pub fn foreground_source_refusal(
    vendor: farhelm_proto::ReportVendor,
    source: &str,
) -> Option<&'static str> {
    use farhelm_proto::ReportVendor;
    match vendor {
        ReportVendor::Codex => (!codex::is_foreground_source(source))
            .then_some("Codex reported an unsupported foreground transition"),
        ReportVendor::Omp => (!omp::is_omp_foreground_source(source))
            .then_some("OMP reported an unsupported foreground transition"),
        ReportVendor::Claude | ReportVendor::Goose | ReportVendor::Pi | ReportVendor::Grok => None,
    }
}

/// The durable kind a report discriminator must name. The destination
/// row's kind stays authoritative; this is the comparison the doorway
/// applies before any vendor I/O.
#[warn(clippy::wildcard_enum_match_arm)]
pub fn agent_kind_of_vendor(vendor: farhelm_proto::ReportVendor) -> AgentKind {
    match vendor {
        farhelm_proto::ReportVendor::Claude => AgentKind::Claude,
        farhelm_proto::ReportVendor::Codex => AgentKind::Codex,
        farhelm_proto::ReportVendor::Goose => AgentKind::Goose,
        farhelm_proto::ReportVendor::Pi => AgentKind::Pi,
        farhelm_proto::ReportVendor::Omp => AgentKind::Omp,
        farhelm_proto::ReportVendor::Grok => AgentKind::Grok,
    }
}

/// Validate a reported identity against the durable kind before any write.
#[warn(clippy::wildcard_enum_match_arm)]
pub fn accepts_reported_conversation(kind: AgentKind, value: &str) -> bool {
    match kind {
        AgentKind::Pi => parse_locator(LocatorVendor::Pi, value).is_ok(),
        AgentKind::Omp => parse_locator(LocatorVendor::Omp, value).is_ok(),
        AgentKind::Grok => grok::GrokLocator::parse(value).is_ok(),
        AgentKind::Codex => codex::CodexLocator::parse(value).is_ok(),
        AgentKind::Claude | AgentKind::Goose => {
            !is_reserved_locator_token(value) && is_plausible_conversation_id(value)
        }
        AgentKind::Generic => false,
    }
}

/// Replace every argv element equal to `placeholder` with `value`, in
/// place, leaving `argv[0]` alone. The shared body of both placeholder
/// substitutions ([`CONVERSATION_PLACEHOLDER`], [`CWD_PLACEHOLDER`]), so
/// the whole-element rule has exactly one implementation for either to
/// drift away from.
///
/// Skipping slot 0 here is a backstop, not the rule: every boundary that
/// accepts an unfilled vector already refuses a placeholder in the
/// program slot ([`ensure_no_cwd_program`], and the `{conversation}` check
/// in [`ensure_resume_template`]), so this function should never actually
/// see one there in practice. If a vector somehow arrives with one anyway
/// — a build with looser validation wrote the row and a decode check was
/// bypassed — skipping it means the eventual exec fails loudly on a
/// program literally named `{cwd}` or `{conversation}`, rather than
/// quietly running a directory or a UUID.
fn fill_slots(argv: &mut [String], placeholder: &str, value: &str) {
    for element in argv.iter_mut().skip(1) {
        if element == placeholder {
            *element = value.to_string();
        }
    }
}

/// Substitute the launch's working directory into each supported whole-element
/// marker. Codex project-trust markers use the target's canonical directory
/// when available; the ordinary `{cwd}` marker keeps the path tmux receives.
/// Meant for exactly one caller, `Supervisor::spawn_agent` — the
/// single seam where an argv becomes a process for create, retry, and
/// every restart mode alike — as the first of the two transformations that
/// seam applies, ahead of hook-flag injection, so the injected tail is
/// never itself a substitution target. Filling anywhere else would need a
/// second, subtly different copy of this substitution for whichever path
/// was missed.
///
/// Caller precondition: `cwd` is the directory tmux is handed for this
/// launch, so the wrapper and the pane end up agreeing on the same string,
/// and it has already passed `ensure_cwd_usable` (service/core.rs). That
/// precondition is why the filled vector is deliberately NOT re-validated
/// here: `ensure_cwd_usable`'s `is_absolute()` check rules out an empty or
/// dash-leading value, while its `tokio::fs::metadata(cwd)` call fails
/// with `InvalidInput` on a path holding a NUL byte — the one property
/// [`ensure_executable_argv`] would otherwise need to check. (A Rust
/// `String` CAN hold a NUL — `shell_words` carries one through unmodified,
/// which is exactly why `ensure_executable_argv` exists — so "it's just a
/// `String`" is not why this is safe; `ensure_cwd_usable` having already
/// rejected one is.)
pub fn fill_cwd(mut argv: Vec<String>, cwd: &str) -> Vec<String> {
    fill_slots(&mut argv, CWD_PLACEHOLDER, cwd);
    if !argv.iter().skip(1).any(|element| {
        matches!(
            element.as_str(),
            CODEX_TRUSTED_CWD_PLACEHOLDER | CODEX_UNTRUSTED_CWD_PLACEHOLDER
        )
    }) {
        return argv;
    }
    // Codex matches project trust by the path it sees after chdir. Resolve
    // symlinks here, on the target host, so the per-run override names that
    // same directory. If the path changes after the earlier usability check,
    // retain the original spelling: the subsequent exec will decide whether
    // the directory still exists, without accidentally trusting another key.
    let resolved = std::fs::canonicalize(cwd)
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| cwd.to_string());
    for element in argv.iter_mut().skip(1) {
        let level = match element.as_str() {
            CODEX_TRUSTED_CWD_PLACEHOLDER => "trusted",
            CODEX_UNTRUSTED_CWD_PLACEHOLDER => "untrusted",
            _ => continue,
        };
        // The existing TOML encoder also escapes DEL, which JSON leaves raw
        // even though TOML forbids it. Keep any directory name inside one
        // project key rather than letting it change the config's structure.
        let path = toml_basic_string(&resolved);
        *element = format!("projects={{{path}={{trust_level=\"{level}\"}}}}");
    }
    argv
}

/// Whether `argv` carries a working-directory marker as a whole element.
/// The one place that comparison is spelled out, so the log line in
/// `spawn_agent` and this module's own tests cannot drift from either the
/// ordinary `{cwd}` or Codex project-trust substitution rule.
pub fn has_cwd_placeholder(argv: &[String]) -> bool {
    argv.iter().any(|element| {
        matches!(
            element.as_str(),
            CWD_PLACEHOLDER | CODEX_TRUSTED_CWD_PLACEHOLDER | CODEX_UNTRUSTED_CWD_PLACEHOLDER
        )
    })
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

/// Refuse a vector whose PROGRAM (`argv[0]`) is [`CWD_PLACEHOLDER`]:
/// substituting there would make the session's working directory the
/// thing this session tries to exec, rather than an argument passed to it.
///
/// `subject` follows [`ensure_executable_argv`]'s naming convention, and
/// the `Err` text is user-facing verbatim for the same reason: callers
/// wrap it in whichever `ErrorKind` their boundary uses rather than
/// reformatting it.
///
/// Kept separate from [`ensure_executable_argv`] rather than folded into
/// it: that rule is placeholder-agnostic (emptiness and NUL are wrong in
/// any argv, filled or not) and its wording says nothing about
/// placeholders, whereas this one only has meaning where a vector is
/// accepted with its placeholders still unfilled. Folding them together
/// would put placeholder wording into every executability refusal, and
/// would tie a generic rule to a concept only some of its callers have.
///
/// An empty `argv` is `Ok` here — there is no program slot to refuse, and
/// refusing emptiness itself is [`ensure_executable_argv`]'s job.
pub fn ensure_no_cwd_program(subject: &str, argv: &[String]) -> Result<(), String> {
    if argv.first().map(String::as_str) == Some(CWD_PLACEHOLDER) {
        return Err(format!(
            "{subject}'s first element is {CWD_PLACEHOLDER}, so substituting the working \
             directory would make it the PROGRAM this session tries to run; the placeholder \
             belongs in an argument slot"
        ));
    }
    Ok(())
}

/// [`ensure_executable_argv`] plus the three rules that are about a RESUME
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
///
/// The third rule is the same shape, for the other placeholder: delegated
/// to [`ensure_no_cwd_program`] so a template starting with
/// [`CWD_PLACEHOLDER`] gets the identical wording a wrapper invocation
/// gets at every other boundary that checks it, rather than a
/// resume-template-specific paraphrase of the same fact.
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
    ensure_no_cwd_program("resume template", template)?;
    Ok(())
}

/// The `FARHELM_AGENT_HOOKS` opt-out (plan D5): which agent kinds get the
/// per-launch identity hook ([`AgentIntegration::hook_argv`]) appended to
/// their argv at all.
///
/// Grok is deliberately outside this switch because Farhelm does not install
/// its callbacks per launch. The user controls those entries in Grok's own
/// hook configuration.
///
/// This is a SEAM value, not a live environment lookup.
/// [`crate::service::SupervisorSeams::agent_hooks`] carries exactly one of
/// these, set ONCE when the
/// supervisor process starts — `farhelm supervisor run`'s CLI arm reads
/// the environment variable and calls [`parse_agent_hooks`] exactly once,
/// never here and never per-launch — so [`Default`] below is
/// UNCONDITIONALLY `All` rather than a live read of the environment.
/// Keeping the environment out of this type entirely is what lets every
/// seam-level test set the value directly instead of mutating the test
/// process's environment, which this repo's tests never do.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AgentHooks {
    /// Every integration with an automatic reporter gets it. The default,
    /// and the only
    /// value a supervisor started with the environment variable unset (or
    /// set to `all`, or the empty string) ever produces. `#[default]`
    /// spells the same "unconditionally `All`, never a live environment
    /// read" contract this type's own doc comment states — a derived
    /// `Default` cannot accidentally grow a side effect the way a hand-
    /// written `fn default()` could.
    #[default]
    All,
    /// No automatically installed reporter runs. Claude falls back to its
    /// record scan; Codex, Goose, Pi, and OMP gain no new exact target.
    /// Manually configured Grok callbacks are unaffected.
    None,
    /// Exactly these automatically configured kinds get their reporter. A
    /// disabled Claude falls back to scanning; a disabled Codex, Goose, Pi,
    /// or OMP does not. An [`AgentKind::Generic`] entry would be inert rather
    /// than rejected —
    /// `allows` is never asked about it because the caller skips kinds with
    /// no integration before consulting this value.
    Only(Vec<AgentKind>),
}

impl AgentHooks {
    /// Whether `kind` should have the identity hook appended to its argv.
    ///
    /// Searches `Only` with [`Vec::contains`] rather than a set: `AgentKind`
    /// derives `Eq` but neither `Hash` nor `Ord` (farhelm-proto's `lib.rs`),
    /// and adding either derive to a wire-protocol enum just to back a set
    /// here would be a proto-crate change in service of a supervisor-crate
    /// convenience. The configured list is short by construction, and it is
    /// a `Vec`, so a value like
    /// `claude,claude` holds a duplicate; `contains` answers the same
    /// either way, which is why the parser does not bother de-duplicating.
    pub fn allows(&self, kind: AgentKind) -> bool {
        match self {
            AgentHooks::All => true,
            AgentHooks::None => false,
            AgentHooks::Only(kinds) => kinds.contains(&kind),
        }
    }
}

/// Parse the `FARHELM_AGENT_HOOKS` environment variable's value into an
/// [`AgentHooks`]. THE ONLY PARSER of that variable in this codebase —
/// every other reader of the opt-out consults the seam value this
/// produces, never the environment again. That is also why the
/// environment READ itself lives in `farhelm supervisor run`'s CLI arm
/// rather than here: keeping this function pure (a `&str` in, an
/// `AgentHooks` out, no side effects) is what lets it be unit-tested
/// without a test process ever setting an environment variable
/// (CLAUDE.md's testability rule forbids that).
///
/// ## Grammar
///
/// - `all` — also what an EMPTY string means, so a variable that is SET
///   but blank behaves the same as one that is unset — maps to
///   [`AgentHooks::All`].
/// - `none` maps to [`AgentHooks::None`].
/// - Anything else is read as a comma-separated list of kind names
///   (`claude`, `codex`, `goose`, `pi`, `omp` — the automatically configured
///   kinds, using this module's own canonical
///   spelling, from [`kind_name`], rather than a spelling invented for this
///   variable). Whitespace around each token is trimmed, and matching is
///   case-insensitive throughout this grammar: this is a value a human
///   types into a shell profile, not a wire format, so tolerating `Claude`
///   or `ALL` costs nothing and saves a support question.
///
/// An EMPTY element is not tolerated: `claude,,codex`, `,claude` and
/// `codex,` all trim to a token that is no kind name at all and therefore
/// take the unrecognized-token path below. That is deliberate rather than
/// incidental — a stray comma is a typo in an opt-out, and the warning
/// that names it is worth more than silently accepting a value the person
/// who typed it may have meant differently.
///
/// ## Failure mode: fail open, not partially
///
/// A token outside `all`, `none`, `claude`, `codex`, `goose`, `pi`, and
/// `omp` invalidates the WHOLE value, not just that token. `grok` is not a
/// token because this switch cannot remove a manually installed callback. A
/// `tracing::warn!` names the bad
/// token and the full offending value, and the result is `All`. The
/// reasoning is that this variable is an opt-OUT — a typo in it must not
/// silently turn into "opt out of everything" (which is what an
/// unrecognized-token-means-None reading would do), so the safe failure
/// direction is falling back to behaving as if the variable were never
/// set.
pub fn parse_agent_hooks(value: &str) -> AgentHooks {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        return AgentHooks::All;
    }
    if trimmed.eq_ignore_ascii_case("none") {
        return AgentHooks::None;
    }
    let mut kinds = Vec::new();
    for token in trimmed.split(',') {
        let token = token.trim();
        match token.to_ascii_lowercase().as_str() {
            "claude" => kinds.push(AgentKind::Claude),
            "codex" => kinds.push(AgentKind::Codex),
            "goose" => kinds.push(AgentKind::Goose),
            "pi" => kinds.push(AgentKind::Pi),
            "omp" => kinds.push(AgentKind::Omp),
            _ => {
                tracing::warn!(
                    token,
                    value,
                    "FARHELM_AGENT_HOOKS contains an unrecognized token; falling back to \
                     the default (every kind hooked) rather than guessing what was meant"
                );
                return AgentHooks::All;
            }
        }
    }
    AgentHooks::Only(kinds)
}

/// The `FARHELM_AGENT_INSTRUCTIONS` switch: whether an injected integration
/// also delivers the pointer that tells the agent `farhelm agent
/// instructions` exists.
///
/// A SEAM value on exactly the terms [`AgentHooks`] above is one — set
/// once from `farhelm supervisor run`'s CLI arm, never read from the
/// environment below that line — and for one reason beyond consistency:
/// Codex fires `SessionStart` at the user's first prompt, which can be
/// hours after the launch. A live environment read would let a session
/// launched under one setting announce under another.
///
/// It is deliberately NOT folded into `AgentHooks`. The two answer
/// different questions and fail in different directions: turning hooks off
/// costs identity capture (a scan fallback for Claude, no new target for
/// Codex, Goose, Pi, or OMP, and no effect on manual Grok callbacks),
/// while turning instructions off costs an
/// agent knowing the CLI exists and nothing else. Someone who wants a silent
/// launch but working resume must be able to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentInstructions {
    /// The injected integration delivers the pointer. The default, and what
    /// an unset or empty variable means.
    #[default]
    On,
    /// The injected integration omits the pointer. Identity capture is
    /// untouched: the report still happens, and the only difference is a
    /// session whose agent was never told about `farhelm agent`.
    Off,
}

impl AgentInstructions {
    /// Whether a launch's injected hook should be given `--announce`.
    pub fn announces(self) -> bool {
        matches!(self, AgentInstructions::On)
    }
}

/// Parse the `FARHELM_AGENT_INSTRUCTIONS` environment variable's value.
/// THE ONLY PARSER of that variable, on the same terms as
/// [`parse_agent_hooks`]: pure, `&str` in and a value out, so the
/// environment READ can live in the CLI arm and no test ever has to mutate
/// its own process's environment.
///
/// ## Grammar
///
/// `on` — also what an EMPTY string means, so a variable that is set but
/// blank behaves like one that is unset — and `off`. Surrounding
/// whitespace is trimmed and matching is case-insensitive, because this is
/// a value someone types into a shell profile rather than a wire format.
///
/// ## Failure mode: fail open
///
/// Anything else warns, names the value, and yields [`AgentInstructions::On`].
/// Same direction as its neighbour and same reasoning: this is a switch
/// whose OFF position removes a feature, so a typo must not silently turn
/// into "off". A user who meant to disable it and mistyped gets a warning
/// naming what they wrote; a user who gets no warning got what they asked
/// for.
pub fn parse_agent_instructions(value: &str) -> AgentInstructions {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("on") {
        return AgentInstructions::On;
    }
    if trimmed.eq_ignore_ascii_case("off") {
        return AgentInstructions::Off;
    }
    tracing::warn!(
        value,
        "FARHELM_AGENT_INSTRUCTIONS is neither on nor off; falling back to the default (on) \
         rather than guessing what was meant"
    );
    AgentInstructions::On
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
    AgentKind::ALL
        .iter()
        .copied()
        .find(|kind| executable_basename(*kind) == Some(basename))
        .unwrap_or(AgentKind::Generic)
}

/// The executable basename [`derive_kind`] recognizes as this kind, or
/// `None` for a kind no basename derives (Generic is what everything else
/// becomes).
///
/// Exhaustive so a new kind decides whether an invocation can be recognized
/// as it; the same names are what the process-tree checks treat as another
/// integrated agent's runtime.
#[warn(clippy::wildcard_enum_match_arm)]
pub fn executable_basename(kind: AgentKind) -> Option<&'static str> {
    match kind {
        AgentKind::Claude => Some("claude"),
        AgentKind::Codex => Some("codex"),
        AgentKind::Goose => Some("goose"),
        AgentKind::Pi => Some("pi"),
        AgentKind::Omp => Some("omp"),
        AgentKind::Grok => Some("grok"),
        AgentKind::Generic => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: a successful hook injection is logged as "injected" only for
    /// the hook-tail kinds (Claude, Codex); the reporter-extension kinds
    /// (Goose, Pi, OMP) succeed silently, a kind with no tail (Grok) returns
    /// silently unhooked, and a skip carries its reason.
    ///
    /// Why: the log decision moved out of the supervisor's service core into
    /// each kind's `inject_hooks`, where the argv/hooked characterization
    /// tests in `service::core` cannot see it. A kind that started logging
    /// success (as Pi and OMP briefly did during that move) changes the log
    /// stream operators read without failing any of them.
    #[test]
    fn each_kind_logs_its_hook_decision_as_before() {
        let hooks = AgentHooks::All;
        let policy = HookPolicy {
            hooks: &hooks,
            instructions: AgentInstructions::On,
            exe: Some("/opt/farhelm"),
            vendor_extension: Some("/state/extension.js"),
        };
        let argv = |program: &str| vec![program.to_string()];
        let decide = |kind: AgentKind, program: &str| {
            let injection = integration_for(kind)
                .expect("integrated kind")
                .inject_hooks(argv(program), &policy);
            (injection.hooked, injection.log)
        };
        for (kind, program, hooked, logged_injected) in [
            (AgentKind::Claude, "claude", true, true),
            (AgentKind::Codex, "codex", true, true),
            (AgentKind::Goose, "goose", true, false),
            (AgentKind::Pi, "pi", true, false),
            (AgentKind::Omp, "omp", true, false),
            (AgentKind::Grok, "grok", false, false),
        ] {
            let (actual_hooked, log) = decide(kind, program);
            assert_eq!(actual_hooked, hooked, "{kind:?} hooked");
            match log {
                HookLog::Injected => assert!(logged_injected, "{kind:?} logged injected"),
                HookLog::Silent => assert!(!logged_injected, "{kind:?} was silent"),
                HookLog::Skipped(reason) => panic!("{kind:?} skipped: {reason}"),
            }
        }

        let off = AgentHooks::None;
        let disabled = HookPolicy {
            hooks: &off,
            ..policy
        };
        let injection = integration_for(AgentKind::Claude)
            .expect("claude integration")
            .inject_hooks(argv("claude"), &disabled);
        assert!(!injection.hooked);
        assert!(matches!(
            injection.log,
            HookLog::Skipped("disabled by FARHELM_AGENT_HOOKS")
        ));
    }

    /// The positive interrupt hint prevents a still long-running Codex task
    /// from decaying to idle. These fixtures mirror the pinned widget's
    /// reduced-motion, animated, inline-context, and detail-row layouts.
    #[farhelm_testtrace::test]
    fn codex_working_hint_accepts_the_current_adjoining_status_widget() {
        for status in [
            "Working (3s • esc to interrupt)",
            "• Working (1m 02s • esc to interrupt)",
            "◦ Working (59m 59s • esc to interrupt)",
            "Working (2h 03m 09s • esc to interrupt) · compacting",
            "Working (4s • esc to interrupt)\n  └ Running a hook\n  └ Reading files\n    and checking output",
        ] {
            let busy = format!("agent output\n{status}\n\n› draft\n\ncustom footer");
            assert!(
                codex_working(&busy),
                "current status was not recognized: {status:?}"
            );
            // The 0.159.0 layout: two padding rows above the prompt, and two
            // indented footer rows under it.
            let real = format!(
                "agent output\n{status}\n\n\n› draft\n\n  model · context\n  ← for agents · ? for shortcuts"
            );
            assert!(
                codex_working(&real),
                "current status was not recognized in the 0.159.0 layout: {status:?}"
            );
        }
    }

    /// Status words in output history, quoted prose, synthetic labels, and
    /// malformed elapsed values must not pin a completed task at running.
    #[farhelm_testtrace::test]
    fn codex_working_hint_rejects_historical_and_unproved_status_shapes() {
        for screen in [
            "Working (3s • esc to interrupt)\ncompleted output\n\n› draft\n\ncustom footer",
            "agent output\nEarlier: Working (3s • esc to interrupt)\n\n› draft\n\ncustom footer",
            "agent output\nThinking (3s • esc to interrupt)\n\n› draft\n\ncustom footer",
            "agent output\n✻ Working (3s • esc to interrupt)\n\n› draft\n\ncustom footer",
            "agent output\nWorking (3 minutes • esc to interrupt)\n\n› draft\n\ncustom footer",
            "agent output\nWorking (60s • esc to interrupt)\n\n› draft\n\ncustom footer",
            "agent output\nWorking (+1m 02s • esc to interrupt)\n\n› draft\n\ncustom footer",
            "agent output\nWorking (1m +2s • esc to interrupt)\n\n› draft\n\ncustom footer",
            "agent output\nWorking (1h +2m 03s • esc to interrupt)\n\n› draft\n\ncustom footer",
            "agent output\nWorking (3s • esc to interrupt)\n  arbitrary transcript\n\n› draft\n\ncustom footer",
            "Working (3s • esc to interrupt)\n› draft",
            // History above an idle composer's run of blank rows, past the
            // padding a current widget is drawn with.
            "Working (3s • esc to interrupt)\n\n\n\n› draft\n\n  footer\n  ? for shortcuts",
            // An unindented row between the prompt and the last row is not
            // part of the composer.
            "agent output\nWorking (3s • esc to interrupt)\n\n› draft\nreply\n  footer",
        ] {
            assert!(
                !codex_working(screen),
                "unproved status claimed current work: {screen:?}"
            );
        }
    }

    /// Derivation is the DEFAULT every session gets when a caller sends no
    /// override, so its exact reach is a product decision, not an
    /// implementation detail: a path prefix must not defeat it (a session
    /// launched as `/opt/bin/claude` is still claude), and a wrapper must
    /// not accidentally acquire it (`env claude` classifies as generic, so
    /// the user is told to override rather than silently getting a
    /// session that looks integrated and never captures).
    #[farhelm_testtrace::test]
    fn kind_derivation_is_basename_equality_and_nothing_more() {
        assert_eq!(derive_kind("claude"), AgentKind::Claude);
        assert_eq!(derive_kind("/opt/bin/claude"), AgentKind::Claude);
        assert_eq!(derive_kind("codex"), AgentKind::Codex);
        assert_eq!(derive_kind("/usr/local/bin/codex"), AgentKind::Codex);
        assert_eq!(derive_kind("pi"), AgentKind::Pi);
        assert_eq!(derive_kind("/opt/bin/pi"), AgentKind::Pi);
        assert_eq!(derive_kind("omp"), AgentKind::Omp);
        assert_eq!(derive_kind("/opt/bin/omp"), AgentKind::Omp);
        assert_eq!(derive_kind("grok"), AgentKind::Grok);
        assert_eq!(derive_kind("/opt/bin/grok"), AgentKind::Grok);
        assert_eq!(derive_kind("env"), AgentKind::Generic);
        assert_eq!(derive_kind("claude-wrapper"), AgentKind::Generic);
        assert_eq!(derive_kind("my-claude"), AgentKind::Generic);
        assert_eq!(derive_kind("omp-format"), AgentKind::Generic);
        assert_eq!(derive_kind(""), AgentKind::Generic);
    }

    /// A derived template preserves every original launch argument before
    /// appending the kind's resume syntax, so permission flags survive a
    /// restart without shell rejoining. Codex's shape is a subcommand, not a
    /// flag — pinned here because it is audited vendor behavior, not a choice.
    #[farhelm_testtrace::test]
    fn default_templates_keep_the_original_launch_argv() {
        let claude_argv = vec![
            "/opt/bin/claude".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ];
        let claude = IntegrationSnapshot::resolve(&claude_argv, None, None).unwrap();
        assert_eq!(claude.kind, AgentKind::Claude);
        assert_eq!(
            claude.resume_template.unwrap(),
            vec![
                "/opt/bin/claude",
                "--dangerously-skip-permissions",
                "--resume",
                "{conversation}"
            ]
        );
        let codex_argv = vec!["codex".to_string(), "--yolo".to_string()];
        let codex = IntegrationSnapshot::resolve(&codex_argv, None, None).unwrap();
        assert_eq!(
            codex.resume_template.unwrap(),
            vec!["codex", "--yolo", "resume", "{conversation}"]
        );
        let generic = IntegrationSnapshot::resolve(&["bash".into()], None, None).unwrap();
        assert_eq!(generic.kind, AgentKind::Generic);
        assert_eq!(generic.resume_template, None);
    }

    /// A path with spaces is exactly what the structural argv template
    /// exists for (PLAN_M3.md item 7 names this case), so it gets its own
    /// assertion: the token stays ONE element and no quoting is invented
    /// around it, which is also what makes the filled resume argv safe to
    /// hand to an exec without a shell.
    #[farhelm_testtrace::test]
    fn a_first_token_with_spaces_survives_as_one_argv_element() {
        let snapshot =
            IntegrationSnapshot::resolve(&["/opt/my agents/claude".into()], None, None).unwrap();
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
    #[farhelm_testtrace::test]
    fn explicit_overrides_win_over_derivation_in_both_directions() {
        let promoted =
            IntegrationSnapshot::resolve(&["my-wrapper".into()], Some(AgentKind::Claude), None)
                .unwrap();
        assert_eq!(promoted.kind, AgentKind::Claude);
        assert_eq!(
            promoted.resume_template.unwrap(),
            vec!["my-wrapper", "--resume", "{conversation}"],
            "an overridden kind still derives its template from the real first token"
        );
        let demoted =
            IntegrationSnapshot::resolve(&["claude".into()], Some(AgentKind::Generic), None)
                .unwrap();
        assert_eq!(demoted.kind, AgentKind::Generic);
        assert_eq!(
            demoted.resume_template, None,
            "a generic session has no resume invocation unless one is supplied"
        );
    }

    /// Snapshot validation enforces the two directions of the kind/template
    /// contract at CREATE. Integrated kinds require a placeholder; generic
    /// kinds cannot use one because they cannot supply identity. Once
    /// capture has succeeded, a placeholder-free template on an integrated
    /// kind could only ever throw the captured identity away, which is
    /// SPEC.md's exact-conversation restart promise quietly becoming false.
    /// A generic placeholder-bearing template is equally unusable and must
    /// be rejected instead of stored as a verbatim fallback.
    #[farhelm_testtrace::test]
    fn an_integrated_kind_refuses_a_placeholder_free_template() {
        let refused = IntegrationSnapshot::resolve(
            &["claude".into()],
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
                &["claude".into()],
                None,
                Some(vec![
                    "claude".to_string(),
                    "--resume={conversation}".to_string()
                ]),
            )
            .is_err()
        );
        // A generic fallback is valid only when it does not ask Farhelm for
        // conversation identity; embedded text is ordinary literal argv.
        assert_eq!(
            IntegrationSnapshot::resolve(
                &["bash".into()],
                None,
                Some(vec!["bash".to_string(), "{conversation}".to_string()]),
            ),
            Err(SnapshotError::GenericTemplateHasPlaceholder)
        );
        assert!(
            IntegrationSnapshot::resolve(
                &["bash".into()],
                None,
                Some(vec![
                    "bash".to_string(),
                    "--resume={conversation}".to_string()
                ]),
            )
            .is_ok()
        );
        // Generic keeps placeholder-free fallback templates and no-template
        // sessions; integrated kinds keep exact-placeholder templates.
        assert!(
            IntegrationSnapshot::resolve(
                &["bash".into()],
                None,
                Some(vec!["bash".to_string(), "--restore".to_string()]),
            )
            .is_ok()
        );
        assert!(
            IntegrationSnapshot::resolve(&["bash".into()], Some(AgentKind::Generic), None).is_ok()
        );
        assert!(
            IntegrationSnapshot::resolve(
                &["claude".into()],
                None,
                Some(vec![
                    "claude".to_string(),
                    "--resume".to_string(),
                    "{conversation}".to_string(),
                ]),
            )
            .is_ok()
        );
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
    #[farhelm_testtrace::test]
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
        let snapshot =
            IntegrationSnapshot::resolve(&["sh".into()], Some(AgentKind::Claude), Some(wrapper))
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

    /// `{cwd}` is meaningless unless EVERY occurrence in an argument slot
    /// gets filled — a wrapper is free to take the directory twice as two
    /// standalone arguments — while `--dir={cwd}` must stay untouched: the
    /// whole-element rule (shared with `{conversation}`) is what keeps
    /// substitution from splicing into the middle of an argument the user
    /// wrote. Also pins that substitution never introduces quoting: a path
    /// with a space survives as one element. `has_cwd_placeholder` is
    /// asserted on the same vectors so a substring-matching or always-false
    /// implementation would fail here rather than pass by accident.
    #[farhelm_testtrace::test]
    fn cwd_fills_every_matching_slot_and_only_whole_elements() {
        let argv = vec![
            "w".to_string(),
            "run".to_string(),
            CWD_PLACEHOLDER.to_string(),
            "claude".to_string(),
            "--dir={cwd}".to_string(),
            CWD_PLACEHOLDER.to_string(),
        ];
        assert!(has_cwd_placeholder(&argv));
        assert!(
            !has_cwd_placeholder(&["claude".to_string(), "--dir={cwd}".to_string()]),
            "an embedded {{cwd}} is not a placeholder occurrence"
        );
        let filled = fill_cwd(argv, "/a b/c");
        assert!(
            !has_cwd_placeholder(&filled),
            "nothing is left to substitute once every whole-element match is filled"
        );
        assert_eq!(
            filled,
            ["w", "run", "/a b/c", "claude", "--dir={cwd}", "/a b/c"],
            "slots 2 and 5 are whole-element matches and must be replaced; slot 4 is `{{cwd}}` \
             embedded in a longer flag and must not be"
        );
    }

    /// Codex's project key must be built on the target after checkout
    /// resolution. Keep both trust levels and a quoted path in one `-c`
    /// element, or shell/config parsing could trust a different directory.
    #[farhelm_testtrace::test]
    fn codex_project_trust_uses_the_final_cwd_as_one_config_argument() {
        let cwd = "/not-present/a \"quoted\" \\ dir \u{7f}";
        for (marker, level) in [
            (CODEX_TRUSTED_CWD_PLACEHOLDER, "trusted"),
            (CODEX_UNTRUSTED_CWD_PLACEHOLDER, "untrusted"),
        ] {
            let argv = vec!["codex".to_string(), "-c".to_string(), marker.to_string()];
            assert!(has_cwd_placeholder(&argv));
            let filled = fill_cwd(argv, cwd);
            assert!(!has_cwd_placeholder(&filled));
            assert_eq!(filled.len(), 3);
            assert_eq!(
                filled[2],
                format!(
                    "projects={{{}={{trust_level=\"{level}\"}}}}",
                    toml_basic_string(cwd)
                )
            );
            assert!(filled[2].contains("\\u007F"));
            assert!(!filled[2].contains('\u{7f}'));
            let parsed: toml::Value = filled[2].parse().expect("Codex override is valid TOML");
            assert_eq!(
                parsed["projects"][cwd]["trust_level"].as_str(),
                Some(level),
                "the escaped path must remain one exact project key"
            );
        }
    }

    /// A symlink spelling of the selected directory must authorize the
    /// directory Codex sees after chdir, rather than leave the prompt active
    /// because the config key names only the alias.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn codex_project_trust_resolves_a_symlinked_cwd() {
        let root = tempfile::tempdir().expect("workdir");
        let target = root.path().join("target");
        std::fs::create_dir(&target).expect("target directory");
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(&target, &alias).expect("directory alias");
        let filled = fill_cwd(
            vec![
                "codex".into(),
                "-c".into(),
                CODEX_TRUSTED_CWD_PLACEHOLDER.into(),
            ],
            alias.to_str().expect("fixture path is UTF-8"),
        );
        assert_eq!(
            filled[2],
            format!(
                "projects={{{}={{trust_level=\"trusted\"}}}}",
                serde_json::to_string(
                    target
                        .canonicalize()
                        .expect("canonical target")
                        .to_str()
                        .expect("fixture path is UTF-8")
                )
                .unwrap()
            )
        );
    }

    /// `{cwd}` may not be `argv[0]`: substitution would make the working
    /// directory the PROGRAM this session execs. `fill_slots` skips slot 0
    /// unconditionally as a backstop behind `ensure_no_cwd_program`, so a
    /// vector that reaches the fill with the placeholder first comes out
    /// unchanged rather than with a directory in the program name.
    #[farhelm_testtrace::test]
    fn cwd_never_fills_the_program_slot() {
        let argv = vec![CWD_PLACEHOLDER.to_string(), "x".to_string()];
        assert_eq!(fill_cwd(argv.clone(), "/tmp"), argv);
    }

    /// A template with no `{cwd}` at all is the common case (every profile
    /// that does not use a wrapper), and it has to be a true no-op: no
    /// spurious element added, no existing element rewritten. Paired with
    /// `has_cwd_placeholder` returning `false`, since that predicate is
    /// what a caller consults to decide whether a fill is worth logging.
    #[farhelm_testtrace::test]
    fn cwd_fill_is_a_no_op_without_a_placeholder() {
        let argv = vec!["claude".to_string(), "--resume".to_string()];
        assert_eq!(fill_cwd(argv.clone(), "/tmp"), argv);
        assert!(!has_cwd_placeholder(&argv));
    }

    /// Pins the ORDER contract: identity is substituted when the restart
    /// snapshot is built (`filled_resume_argv`, a per-restart call), `{cwd}`
    /// is substituted at SPAWN time (`fill_cwd`, called only from
    /// `Supervisor::spawn_agent`). A template that uses both
    /// placeholders must let `filled_resume_argv` touch only
    /// `{conversation}` and leave `{cwd}` for the later, separate call, and
    /// the second call must then find exactly that element left to fill.
    /// The test exercises the two pure substitution passes in that order;
    /// it does not drive `spawn_agent` itself.
    #[farhelm_testtrace::test]
    fn conversation_and_cwd_placeholders_coexist_in_one_template() {
        let snapshot = IntegrationSnapshot::resolve(
            &["w".into()],
            Some(AgentKind::Claude),
            Some(vec![
                "w".to_string(),
                "run".to_string(),
                CWD_PLACEHOLDER.to_string(),
                "claude".to_string(),
                "--resume".to_string(),
                CONVERSATION_PLACEHOLDER.to_string(),
            ]),
        )
        .expect("a template containing {conversation} satisfies an integrated kind");

        let after_resume = snapshot
            .filled_resume_argv("0199a4d2-9c1a-7bd6-9d18-2c0f2f1c7f31")
            .expect("a plausible id fills the template");
        assert_eq!(
            after_resume,
            [
                "w",
                "run",
                "{cwd}",
                "claude",
                "--resume",
                "0199a4d2-9c1a-7bd6-9d18-2c0f2f1c7f31"
            ],
            "{{conversation}} is filled and {{cwd}} is left exactly as it was"
        );

        let after_spawn = fill_cwd(after_resume, "/work/dir");
        assert_eq!(
            after_spawn,
            [
                "w",
                "run",
                "/work/dir",
                "claude",
                "--resume",
                "0199a4d2-9c1a-7bd6-9d18-2c0f2f1c7f31"
            ],
            "the later, separate spawn-time fill picks up exactly where the resume-time fill left off"
        );
    }

    /// `{cwd}` in the program slot is refused by `ensure_no_cwd_program`
    /// (the check invocation boundaries call) and by
    /// `ensure_resume_template` (which folds it in alongside the
    /// pre-existing `{conversation}`-first refusal so a resume template
    /// gets both rules from one call). The accepted shapes are asserted
    /// too, for both functions, so a rule that quietly started refusing
    /// `{cwd}` anywhere in the vector — not just slot 0 — would fail here
    /// instead of silently disabling every wrapper resume.
    #[farhelm_testtrace::test]
    fn a_cwd_placeholder_may_not_be_the_program() {
        ensure_resume_template(&[
            "w".to_string(),
            "run".to_string(),
            CWD_PLACEHOLDER.to_string(),
            "claude".to_string(),
            "--resume".to_string(),
            CONVERSATION_PLACEHOLDER.to_string(),
        ])
        .expect(
            "a wrapper template with {cwd} past the program slot is exactly the supported shape",
        );
        ensure_no_cwd_program("wrapper invocation", &[CWD_PLACEHOLDER.to_string()])
            .expect_err("a bare {cwd} names no program other than a directory");
        ensure_no_cwd_program(
            "wrapper invocation",
            &[CWD_PLACEHOLDER.to_string(), "claude".to_string()],
        )
        .expect_err("{cwd} first is still the program slot regardless of what follows");
        ensure_no_cwd_program(
            "wrapper invocation",
            &["claude".to_string(), CWD_PLACEHOLDER.to_string()],
        )
        .expect("{cwd} in an argument slot is exactly what the placeholder is for");

        let refused = ensure_resume_template(&[
            CWD_PLACEHOLDER.to_string(),
            "--resume".to_string(),
            CONVERSATION_PLACEHOLDER.to_string(),
        ])
        .expect_err("a resume template whose program is {cwd} is unexecutable");
        assert!(
            refused.contains(CWD_PLACEHOLDER) && refused.contains("PROGRAM"),
            "the refusal must name the placeholder and say what would go wrong: {refused}"
        );
    }

    /// A conversation id spelled like a placeholder is refused, because the
    /// two substitution passes run in sequence: an id of `{cwd}` would be
    /// written into the template by `filled_resume_argv` and then rewritten
    /// into the working directory by `fill_cwd`, letting a record file —
    /// which any local process can write — steer the resume argv through
    /// the second pass. Asserted end to end through `filled_resume_argv`,
    /// not only on the shape check, so the property survives a refactor
    /// that moves where plausibility is enforced.
    #[farhelm_testtrace::test]
    fn a_conversation_id_spelled_like_a_placeholder_is_refused() {
        assert!(!is_plausible_conversation_id(CWD_PLACEHOLDER));
        assert!(!is_plausible_conversation_id(CONVERSATION_PLACEHOLDER));
        let snapshot = IntegrationSnapshot::resolve(
            &["w".into()],
            Some(AgentKind::Claude),
            Some(vec![
                "w".to_string(),
                "run".to_string(),
                CWD_PLACEHOLDER.to_string(),
                "claude".to_string(),
                "--resume".to_string(),
                CONVERSATION_PLACEHOLDER.to_string(),
            ]),
        )
        .expect("a template containing {conversation} satisfies an integrated kind");
        assert_eq!(
            snapshot.filled_resume_argv(CWD_PLACEHOLDER),
            None,
            "an id equal to {{cwd}} must not become a second {{cwd}} element for the spawn-time fill"
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
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
    fn an_option_shaped_identity_neither_fills_a_template_nor_is_offered() {
        let snapshot =
            IntegrationSnapshot::resolve(&["claude".into()], None, None).expect("resolve");
        assert_eq!(
            snapshot.filled_resume_argv("--dangerously-bypass-approvals-and-sandbox"),
            None,
            "an option-shaped id must never be substituted into an argv"
        );
        assert_eq!(
            snapshot.restart_offer(Some("--dangerously-bypass-approvals-and-sandbox"), 0),
            RestartOffer::FreshOnly,
            "and must not be advertised as resumable either, or the offer would promise a \
             command the substitution then refuses to build"
        );
        // The honest case still works, or this test would pass for the
        // wrong reason.
        let good = "0199a4d2-9c1a-7bd6-9d18-2c0f2f1c7f31";
        assert_eq!(snapshot.restart_offer(Some(good), 0), RestartOffer::Resume);
        assert_eq!(
            snapshot
                .filled_resume_argv(good)
                .expect("a plausible id fills the template")
                .last()
                .map(String::as_str),
            Some(good)
        );
    }

    /// Grok resume keeps the original argv, establishes a private backend,
    /// and substitutes only the exact UUID carried by a ready locator.
    #[farhelm_testtrace::test]
    fn grok_resume_argv_adds_no_leader_and_the_verified_uuid() {
        let original =
            ["env", "GROK_HOME=/tmp/grok", "/opt/bin/grok", "--no-plan"].map(str::to_string);
        let snapshot = IntegrationSnapshot::resolve(&original, Some(AgentKind::Grok), None)
            .expect("the native Grok invocation resolves");
        assert_eq!(snapshot.kind, AgentKind::Grok);
        let expected_template = [
            "env",
            "GROK_HOME=/tmp/grok",
            "/opt/bin/grok",
            "--no-leader",
            "--no-plan",
            "--resume",
            "{conversation}",
        ]
        .map(str::to_string)
        .to_vec();
        assert_eq!(snapshot.resume_template.as_ref(), Some(&expected_template));

        let mut locator = grok::GrokLocator::reported(
            "grok-session-1".to_string(),
            Some("/tmp/grok-session-1/updates.jsonl".to_string()),
            Some("2026-09-22T12:00:00Z"),
        )
        .expect("valid locator");
        locator.resumable = true;
        let token = locator.encode().expect("ready locator encodes");
        assert_eq!(
            snapshot.restart_offer(Some(&token), 1),
            RestartOffer::Resume
        );
        assert_eq!(
            snapshot
                .filled_resume_argv(&token)
                .expect("the exact UUID fills the template"),
            [
                "env",
                "GROK_HOME=/tmp/grok",
                "/opt/bin/grok",
                "--no-leader",
                "--no-plan",
                "--resume",
                "grok-session-1",
            ]
        );
    }

    /// A changed Grok permission must reach its explicit resume template.
    ///
    /// Unlike Claude, Grok's resumed argv is supplied by the helm rather
    /// than derived from its invocation. This pins the supervisor's template
    /// resolution and identity fill for a default-to-YOLO restart-with.
    #[farhelm_testtrace::test]
    fn grok_restart_with_permission_uses_replacement_resume_template() {
        let previous = IntegrationSnapshot::resolve(
            &["grok".into(), "--no-leader".into()],
            Some(AgentKind::Grok),
            Some(vec![
                "grok".into(),
                "--no-leader".into(),
                "--resume".into(),
                "{conversation}".into(),
            ]),
        )
        .expect("default Grok template resolves");
        let replacement = IntegrationSnapshot::resolve(
            &[
                "grok".into(),
                "--no-leader".into(),
                "--always-approve".into(),
            ],
            Some(previous.kind),
            Some(vec![
                "grok".into(),
                "--no-leader".into(),
                "--always-approve".into(),
                "--resume".into(),
                "{conversation}".into(),
            ]),
        )
        .expect("YOLO Grok template resolves against the fixed kind");
        let mut locator = grok::GrokLocator::reported(
            "grok-session-1".to_string(),
            Some("/tmp/grok-session-1/updates.jsonl".to_string()),
            Some("2026-09-22T12:00:00Z"),
        )
        .expect("valid captured Grok locator");
        locator.resumable = true;
        let resumed = replacement
            .filled_resume_argv(&locator.encode().expect("resumable locator encodes"))
            .expect("replacement template fills the captured identity");
        assert_eq!(
            resumed,
            [
                "grok",
                "--no-leader",
                "--always-approve",
                "--resume",
                "grok-session-1"
            ]
        );
        assert_ne!(replacement.resume_template, previous.resume_template);
    }

    /// Spec: a Codex launch that already selects a session (`codex resume
    /// <id>`, `codex --yolo resume <id>`, `codex fork <id>`) refuses a derived
    /// resume template, an explicit template still resolves for it, and a
    /// plain `codex` launch still derives `resume {conversation}`.
    ///
    /// Why: Codex accepts one session selector. Derivation appended a second,
    /// so a Restart or Resume of such a session exited with "unexpected
    /// argument" instead of continuing the conversation. Refusing at create
    /// time, as Grok does, tells the user to supply a resume command instead.
    #[farhelm_testtrace::test]
    fn codex_derived_resume_refuses_an_existing_session_selector() {
        let argv = |words: &[&str]| {
            words
                .iter()
                .map(|word| word.to_string())
                .collect::<Vec<_>>()
        };
        for refused in [
            argv(&["codex", "resume", "old-id"]),
            argv(&["codex", "--yolo", "resume", "old-id"]),
            argv(&["codex", "fork", "old-id"]),
        ] {
            assert_eq!(
                IntegrationSnapshot::resolve(&refused, None, None),
                Err(SnapshotError::CodexAmbiguousResumeSelector),
                "derived argv must refuse: {refused:?}"
            );
            let template = argv(&["codex", "resume", CONVERSATION_PLACEHOLDER]);
            let explicit = IntegrationSnapshot::resolve(&refused, None, Some(template.clone()))
                .unwrap_or_else(|error| {
                    panic!("an explicit template must still resolve for {refused:?}: {error}")
                });
            assert_eq!(
                explicit.resume_template,
                Some(template),
                "the explicit template is the one kept for {refused:?}"
            );
        }
        let plain = IntegrationSnapshot::resolve(&argv(&["codex", "--yolo"]), None, None)
            .expect("a plain codex launch derives");
        assert_eq!(
            plain.resume_template,
            Some(argv(&[
                "codex",
                "--yolo",
                "resume",
                CONVERSATION_PLACEHOLDER
            ]))
        );
    }

    /// Spec: a Claude launch that already selects a conversation
    /// (`--continue`/`-c`, `--resume`/`-r`, `--session-id`, `--from-pr`,
    /// `--teleport`, `--fork-session`, inline `=` forms included) or carries a
    /// bare `--` refuses a derived resume template;
    /// an explicit template still resolves for each, and a plain Claude
    /// launch, flags and prompt included, still derives `--resume
    /// {conversation}`.
    ///
    /// Why: the derived template appends `--resume <id>`. Beside an existing
    /// selector, Claude can resume a different conversation than the
    /// captured one, which then replaces the valid Resume offer; behind
    /// `--`, the flag becomes prompt text and Resume starts afresh. Refusing
    /// at create time, as Grok and Codex do, tells the user to supply a
    /// resume command instead.
    #[farhelm_testtrace::test]
    fn claude_derived_resume_refuses_an_existing_selector_or_boundary() {
        let argv = |words: &[&str]| {
            words
                .iter()
                .map(|word| word.to_string())
                .collect::<Vec<_>>()
        };
        for refused in [
            argv(&["claude", "--continue"]),
            argv(&["claude", "--model", "opus", "-c"]),
            argv(&["claude", "--resume", "old-id"]),
            argv(&["claude", "-r", "old-id"]),
            argv(&["claude", "--resume=old-id"]),
            argv(&["claude", "--session-id", "old-id"]),
            argv(&["claude", "--session-id=old-id"]),
            argv(&["claude", "--", "fix the build"]),
            argv(&["claude", "--from-pr", "123"]),
            argv(&["claude", "--from-pr=123"]),
            argv(&["claude", "--teleport"]),
            argv(&["claude", "--fork-session"]),
        ] {
            assert_eq!(
                IntegrationSnapshot::resolve(&refused, None, None),
                Err(SnapshotError::ClaudeAmbiguousResumeSelector),
                "derived argv must refuse: {refused:?}"
            );
            let template = argv(&["claude", "--resume", CONVERSATION_PLACEHOLDER]);
            let explicit = IntegrationSnapshot::resolve(&refused, None, Some(template.clone()))
                .unwrap_or_else(|error| {
                    panic!("an explicit template must still resolve for {refused:?}: {error}")
                });
            assert_eq!(
                explicit.resume_template,
                Some(template),
                "the explicit template is the one kept for {refused:?}"
            );
        }
        let plain = IntegrationSnapshot::resolve(
            &argv(&["claude", "--model", "opus", "fix the build"]),
            None,
            None,
        )
        .expect("a plain claude launch derives");
        assert_eq!(
            plain.resume_template,
            Some(argv(&[
                "claude",
                "--model",
                "opus",
                "fix the build",
                "--resume",
                CONVERSATION_PLACEHOLDER
            ]))
        );
    }

    /// Derivation cannot safely append a selector after `--` or beside an
    /// existing session choice. Explicit templates remain the escape hatch
    /// because the user supplies their complete argv contract.
    #[farhelm_testtrace::test]
    fn grok_derived_resume_refuses_ambiguous_selector_boundaries() {
        for tail in [
            vec!["--"],
            vec!["--resume", "old"],
            vec!["--resume=old"],
            vec!["--session-id", "old"],
            vec!["--continue"],
            vec!["--fork-session", "old"],
        ] {
            let mut argv = vec!["grok".to_string()];
            argv.extend(tail.into_iter().map(str::to_string));
            assert_eq!(
                IntegrationSnapshot::resolve(&argv, None, None),
                Err(SnapshotError::GrokAmbiguousResumeBoundary),
                "derived argv must refuse: {argv:?}"
            );
        }

        let explicit = IntegrationSnapshot::resolve(
            &["grok".to_string(), "--".to_string()],
            None,
            Some(vec![
                "wrapper".to_string(),
                "resume-exact".to_string(),
                CONVERSATION_PLACEHOLDER.to_string(),
            ]),
        )
        .expect("an explicit complete template bypasses derivation");
        assert_eq!(explicit.kind, AgentKind::Grok);
    }

    /// Goose's derived restart keeps the structured launcher's literal
    /// `env` prefix and model choices while replacing every old identity
    /// selector with the one exact session ID Farhelm captured.
    #[farhelm_testtrace::test]
    fn goose_resume_template_preserves_structured_launch_arguments() {
        let original = [
            "env",
            "GOOSE_MODE=auto",
            "GOOSE_THINKING_EFFORT=off",
            "goose",
            "session",
            "--provider",
            "openrouter",
            "--model",
            "x-ai/grok-4.6",
            "--name",
            "draft",
        ]
        .map(str::to_string);
        let snapshot = IntegrationSnapshot::resolve(&original, Some(AgentKind::Goose), None)
            .expect("Goose integration");
        assert_eq!(
            snapshot.resume_template.unwrap(),
            [
                "env",
                "GOOSE_MODE=auto",
                "GOOSE_THINKING_EFFORT=off",
                "goose",
                "session",
                "--provider",
                "openrouter",
                "--model",
                "x-ai/grok-4.6",
                "--resume",
                "--session-id",
                "{conversation}",
            ]
        );
    }

    /// Fresh bare Goose is normalized during injection, but its durable argv
    /// remains bare. Resume must add the subcommand even behind an env prefix.
    #[farhelm_testtrace::test]
    fn goose_resume_template_normalizes_bare_env_launches() {
        for original in [vec!["goose"], vec!["env", "FOO=1", "/opt/bin/goose"]] {
            let argv: Vec<String> = original.iter().map(|s| s.to_string()).collect();
            let snapshot = IntegrationSnapshot::resolve(&argv, Some(AgentKind::Goose), None)
                .expect("Goose integration");
            let mut expected = argv;
            expected.extend(
                ["session", "--resume", "--session-id", "{conversation}"].map(str::to_string),
            );
            assert_eq!(snapshot.resume_template, Some(expected));
        }
    }

    /// OMP's default resume template strips every session-source selector
    /// under OMP'S OWN consumption rules — which differ from Pi's — and
    /// appends `--resume {conversation}`. The subtle cases are the
    /// optional-value selectors: a dropped `--resume`/`-r`/`--session` takes
    /// the next token only when that token is value-like, `--fork` takes its
    /// value unconditionally, and unrelated option values survive opaquely
    /// even when they are themselves spelled like selectors.
    #[farhelm_testtrace::test]
    fn omp_resume_template_strips_session_selectors_under_omp_consumption_rules() {
        let strip = |argv: &[&str]| -> Vec<String> {
            IntegrationSnapshot::resolve(
                &argv.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                Some(AgentKind::Omp),
                None,
            )
            .expect("an OMP template resolves")
            .resume_template
            .expect("OMP has a derived template")
        };

        // Optional-value consumption: the bare form of `--resume` consumes a
        // value-like successor, keeps a flag-looking one, and handles its own
        // inline spelling.
        assert_eq!(
            strip(&["omp", "--resume", "old-id"]),
            vec!["omp", "--resume", "{conversation}"]
        );
        assert_eq!(
            strip(&["omp", "--resume", "--model", "x"]),
            vec!["omp", "--model", "x", "--resume", "{conversation}"]
        );
        assert_eq!(
            strip(&["omp", "--resume=old-id"]),
            vec!["omp", "--resume", "{conversation}"]
        );
        assert_eq!(
            strip(&["omp", "-r", "old-id"]),
            vec!["omp", "--resume", "{conversation}"]
        );
        assert_eq!(
            strip(&["omp", "--session", "old-id"]),
            vec!["omp", "--resume", "{conversation}"]
        );
        // `--fork` takes a value unconditionally, even a flag-looking one —
        // so `--model` is consumed as the fork value and `x` survives as the
        // prompt, exactly as OMP's own parser reads that argv.
        assert_eq!(
            strip(&["omp", "--fork", "branch-1"]),
            vec!["omp", "--resume", "{conversation}"]
        );
        assert_eq!(
            strip(&["omp", "--fork", "--model", "x"]),
            vec!["omp", "x", "--resume", "{conversation}"]
        );
        // Valueless import selectors drop alone.
        assert_eq!(
            strip(&["omp", "--from-claude", "--from-codex", "--continue", "-c"]),
            vec!["omp", "--resume", "{conversation}"]
        );
        // Their supported INLINE spellings drop whole too — the name before
        // `=` is what matches — consuming no extra element, so the prompt
        // after them survives. OMP's own equals handling makes
        // `--from-claude=true` the same import selector as the bare flag.
        assert_eq!(
            strip(&["omp", "--from-claude=true", "hello"]),
            vec!["omp", "hello", "--resume", "{conversation}"]
        );
        assert_eq!(
            strip(&["omp", "--from-codex=false", "--continue=x", "hello"]),
            vec!["omp", "hello", "--resume", "{conversation}"]
        );
        // Inline values of KEPT flags survive as the single elements they are.
        assert_eq!(
            strip(&["omp", "--model=openrouter/x", "--session-dir=/s", "hello"]),
            vec![
                "omp",
                "--model=openrouter/x",
                "--session-dir=/s",
                "hello",
                "--resume",
                "{conversation}"
            ]
        );
        // Unrelated option values survive opaquely, even selector-shaped.
        assert_eq!(
            strip(&["omp", "--system-prompt", "--continue", "--model", "x"]),
            vec![
                "omp",
                "--system-prompt",
                "--continue",
                "--model",
                "x",
                "--resume",
                "{conversation}"
            ],
            "a prompt value that merely looks like a selector is data, not a selector"
        );
        // An unknown long flag keeps a value-like successor, as OMP's own
        // restart rewrite does.
        assert_eq!(
            strip(&["omp", "--ext-flag", "value"]),
            vec!["omp", "--ext-flag", "value", "--resume", "{conversation}"]
        );
        // The end-of-options boundary is preserved verbatim in the ORIGINAL
        // argv; nothing is stripped behind it. (Template resolution REFUSES
        // this argv overall — see the test below — so this pins the stripper
        // itself rather than going through `resolve`.)
        assert_eq!(
            strip_omp_selectors(
                &["omp", "--", "--resume", "prompt-text"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            ),
            vec!["omp", "--", "--resume", "prompt-text"]
        );
    }

    /// A GENUINE end-of-options delimiter refuses an OMP template: OMP reads
    /// everything after an unconsumed `--` as prompt text, so the appended
    /// `--resume <verified-file>` could never resume, and the honest failure
    /// is a refused create rather than a resume that silently launches fresh.
    /// Three neighbouring shapes deliberately do NOT refuse, each for its own
    /// reason: a `--` consumed as an option value (the prompt is literally
    /// spelled `--`, and appended flags remain options), an explicit template
    /// override (filled verbatim, never appended), and every other kind.
    #[farhelm_testtrace::test]
    fn an_omp_argv_with_a_bare_double_dash_refuses_its_template() {
        // A genuine delimiter with the DERIVED template refuses.
        let argv = ["omp".to_string(), "--".to_string(), "hello".to_string()];
        let error = IntegrationSnapshot::resolve(&argv, Some(AgentKind::Omp), None)
            .expect_err("a genuine -- delimiter must refuse OMP template resolution");
        assert!(format!("{error:?}").contains("OmpAmbiguousResumeBoundary"));

        // A `--` consumed as an option value is not a delimiter: the prompt
        // value is literally `--`, and appended flags are still options.
        let consumed = [
            "omp".to_string(),
            "--system-prompt".to_string(),
            "--".to_string(),
        ];
        let snapshot = IntegrationSnapshot::resolve(&consumed, Some(AgentKind::Omp), None)
            .expect("a -- consumed as an option value resolves");
        assert_eq!(
            snapshot.resume_template.expect("derived template"),
            vec![
                "omp".to_string(),
                "--system-prompt".to_string(),
                "--".to_string(),
                "--resume".to_string(),
                CONVERSATION_PLACEHOLDER.to_string(),
            ],
            "the consumed -- survives verbatim and the resume flag lands AFTER it, in \
             option position"
        );
        // Same for a consumed `--` behind a prompt.
        let consumed_after_prompt = [
            "omp".to_string(),
            "hello".to_string(),
            "--system-prompt".to_string(),
            "--".to_string(),
        ];
        assert!(
            IntegrationSnapshot::resolve(&consumed_after_prompt, Some(AgentKind::Omp), None)
                .is_ok()
        );

        // A genuine delimiter plus an EXPLICIT, independently valid template
        // override still resolves: the override REPLACES the derived
        // template verbatim — the original argv (delimiter included) is not
        // carried into it, so nothing is ever appended behind the delimiter.
        // The override here is an exact-file resume command OMP itself
        // honors (`--resume` with an absolute path opens that session file
        // directly); filling it with an OMP locator yields exactly the argv
        // OMP would run, with no delimiter and no prompt inside it.
        let override_argv = ["omp".to_string(), "--".to_string(), "hello".to_string()];
        let snapshot = IntegrationSnapshot::resolve(
            &override_argv,
            Some(AgentKind::Omp),
            Some(vec![
                "omp".to_string(),
                "--resume".to_string(),
                CONVERSATION_PLACEHOLDER.to_string(),
            ]),
        )
        .expect("an explicit template override needs no appending");
        let verified = encode_locator(
            LocatorVendor::Omp,
            SessionLocator {
                version: 1,
                session_id: "conv-b".to_string(),
                session_file: Some("/sessions/conv-b.jsonl".to_string()),
            },
        )
        .expect("a verified locator encodes");
        assert_eq!(
            snapshot
                .filled_resume_argv(&verified)
                .expect("the override fills"),
            vec![
                "omp".to_string(),
                "--resume".to_string(),
                "/sessions/conv-b.jsonl".to_string(),
            ],
            "the filled override is exactly the exact-file resume command, with no \
             delimiter behind which a target could be misread"
        );

        // The refusal is OMP-specific: Pi's grammar treats `--` differently
        // and its template derivation is unchanged.
        let pi_argv = ["pi".to_string(), "--".to_string(), "hello".to_string()];
        assert!(IntegrationSnapshot::resolve(&pi_argv, Some(AgentKind::Pi), None).is_ok());
    }

    /// A locator with a raw control character is refused even where JSON
    /// would accept it as whitespace.
    ///
    /// Why it matters: the accepted locator string is stored and logged as
    /// reported, so a newline between JSON tokens split supervisor log
    /// lines. Genuine reporters send compact JSON, which still parses.
    #[farhelm_testtrace::test]
    fn locators_with_control_characters_are_refused() {
        for vendor in [LocatorVendor::Pi, LocatorVendor::Omp] {
            let encoded = encode_locator(
                vendor,
                SessionLocator {
                    version: 1,
                    session_id: "session-1".to_string(),
                    session_file: Some("/tmp/session.jsonl".to_string()),
                },
            )
            .expect("encode");
            assert!(parse_locator(vendor, &encoded).is_ok());
            let spread = encoded.replacen('{', "{\n", 1);
            assert!(
                serde_json::from_str::<SessionLocator>(
                    spread.strip_prefix(vendor.prefix()).unwrap()
                )
                .is_ok(),
                "fixture premise: the newline is valid JSON whitespace"
            );
            let refusal = parse_locator(vendor, &spread).expect_err("a raw newline is refused");
            assert!(
                refusal.to_string().contains("control characters"),
                "{refusal}"
            );
        }
    }

    /// Each locator vendor's durable token round-trips paths that are hostile
    /// to shell-word parsing because the path is later substituted as one
    /// argv element. A fileless report remains valid but deliberately removes
    /// Resume. The two vendors' tokens are also cross-rejected: neither
    /// spelling parses as the other's locator, and neither passes a plain-id
    /// kind's acceptance.
    #[farhelm_testtrace::test]
    fn locator_round_trips_hostile_paths_and_controls_the_offer() {
        for (vendor, argv0) in [(LocatorVendor::Pi, "pi"), (LocatorVendor::Omp, "omp")] {
            let locator = SessionLocator {
                version: 1,
                session_id: "session-1".to_string(),
                session_file: Some("/tmp/a b/quote-\"-\\-雪.jsonl".to_string()),
            };
            let encoded = encode_locator(vendor, locator.clone()).expect("encode");
            assert!(encoded.starts_with(vendor.prefix()), "{encoded:?}");
            assert_eq!(parse_locator(vendor, &encoded).expect("decode"), locator);

            let snapshot =
                IntegrationSnapshot::resolve(&[argv0.to_string()], None, None).expect("integrated");
            // OMP's ownership proof is implemented, so an unproven
            // (version 0) binding offers fresh-only until its first proven
            // report; Pi keeps today's offer until its own proof flips the
            // predicate.
            let unproven_offer = if vendor == LocatorVendor::Omp {
                RestartOffer::FreshOnly
            } else {
                RestartOffer::Resume
            };
            assert_eq!(snapshot.restart_offer(Some(&encoded), 0), unproven_offer);
            if vendor == LocatorVendor::Omp {
                assert_eq!(
                    snapshot.restart_offer(Some(&encoded), 1),
                    RestartOffer::Resume,
                    "a proven OMP binding resumes"
                );
            }
            let fileless = encode_locator(
                vendor,
                SessionLocator {
                    session_file: None,
                    ..locator.clone()
                },
            )
            .expect("encode fileless locator");
            assert_eq!(
                snapshot.restart_offer(Some(&fileless), 0),
                RestartOffer::FreshOnly
            );
            assert!(
                snapshot.filled_resume_argv(&fileless).is_none(),
                "a fileless locator fills nothing"
            );
            let filled = snapshot
                .filled_resume_argv(&encoded)
                .expect("a verified locator fills the template");
            let mut template = snapshot.resume_template.clone().expect("derived template");
            assert_eq!(template.pop().as_deref(), Some(CONVERSATION_PLACEHOLDER));
            assert_eq!(filled.len(), template.len() + 1);
            assert_eq!(&filled[..template.len()], &template[..]);
            assert_eq!(
                filled.last().map(String::as_str),
                Some("/tmp/a b/quote-\"-\\-雪.jsonl"),
                "the captured path lands exactly in the placeholder slot"
            );

            // Cross-vendor rejection is symmetrical, and a locator is also
            // never accepted as a plain id by the id-reporting kinds.
            let other = match vendor {
                LocatorVendor::Pi => LocatorVendor::Omp,
                LocatorVendor::Omp => LocatorVendor::Pi,
            };
            assert!(parse_locator(other, &encoded).is_err());
            for plain_kind in [AgentKind::Claude, AgentKind::Codex, AgentKind::Goose] {
                assert!(!accepts_reported_conversation(plain_kind, &encoded));
            }
            let own_kind = match vendor {
                LocatorVendor::Pi => AgentKind::Pi,
                LocatorVendor::Omp => AgentKind::Omp,
            };
            assert!(accepts_reported_conversation(own_kind, &encoded));
            // The OTHER vendor's snapshot refuses this token at the OFFER and
            // SUBSTITUTION boundaries too — the durable decision points, not
            // just the parser.
            let other_argv0 = match other {
                LocatorVendor::Pi => "pi",
                LocatorVendor::Omp => "omp",
            };
            let other_snapshot =
                IntegrationSnapshot::resolve(&[other_argv0.to_string()], None, None)
                    .expect("integrated");
            assert_eq!(
                other_snapshot.restart_offer(Some(&encoded), 0),
                RestartOffer::FreshOnly,
                "a {vendor:?} locator cannot make a {other:?} session offer a resume"
            );
            assert!(
                other_snapshot.filled_resume_argv(&encoded).is_none(),
                "a {vendor:?} locator never substitutes into a {other:?} template"
            );
            // A malformed reserved token is likewise refused as a claim at
            // both boundaries, for its own vendor as well as the other's.
            for malformed in [
                format!("{}{{", vendor.prefix()),
                "pi:{".to_string(),
                "omp:nonsense".to_string(),
            ] {
                assert_eq!(
                    snapshot.restart_offer(Some(&malformed), 0),
                    RestartOffer::FreshOnly,
                    "{malformed:?} cannot make a {vendor:?} session offer a resume"
                );
                assert!(
                    snapshot.filled_resume_argv(&malformed).is_none(),
                    "{malformed:?} never substitutes into a {vendor:?} template"
                );
            }
        }
    }

    /// A malformed reserved prefix is refused as a claim, not parsed as a
    /// plain conversation id — the rejection must not depend on successful
    /// JSON decoding, so truncated or corrupted locator tokens cannot pass
    /// as ids for Claude, Codex, or Goose.
    #[farhelm_testtrace::test]
    fn malformed_reserved_prefixes_reject_without_decoding() {
        for malformed in [
            "pi:",
            "omp:",
            "pi:{",
            "omp:not json",
            "pi:{\"version\":1}",
            "omp:{\"version\":2,\"session_id\":\"s\",\"session_file\":null}",
        ] {
            assert!(
                is_reserved_locator_token(malformed),
                "{malformed:?} claims a reserved prefix"
            );
            for plain_kind in [AgentKind::Claude, AgentKind::Codex, AgentKind::Goose] {
                assert!(
                    !accepts_reported_conversation(plain_kind, malformed),
                    "{malformed:?} must not pass as a plain id for {plain_kind:?}"
                );
            }
            // And each locator vendor refuses the other's malformed token.
            assert!(parse_locator(LocatorVendor::Pi, malformed).is_err());
            assert!(parse_locator(LocatorVendor::Omp, malformed).is_err());
        }
    }

    /// The fixtures an OMP session header can present, and the one answer
    /// each must get. Mirrors the shapes OMP 18.2.4 writes: a fixed-width
    /// 256-byte title slot that may precede the version-3 session record, or
    /// no slot at all for legacy files — and nothing else, ever. Every
    /// non-header shape refuses rather than being skipped, because pre-resume
    /// verification may only accept a file it parsed completely.
    ///
    /// Every fixture is built from NEWLINE-SEPARATED, independently valid
    /// JSONL records, and every fixture that CLAIMS to be well-formed
    /// records-with-wrong-content asserts that structure FIRST, so a refusal
    /// is attributable to the mechanism under test (the version check, the
    /// type check, the title-slot shape check) and not to malformed JSON. The
    /// genuinely malformed fixtures are the exceptions, and their premise IS
    /// the malformation.
    #[farhelm_testtrace::test]
    fn omp_header_verification_accepts_only_the_session_shapes_it_parses() {
        /// The records a fixture claims to carry, asserted before the parser
        /// runs: every line parses on its own, and each premise names one
        /// record's field and the value the fixture's premise is about.
        fn assert_records(text: &str, premises: &[(usize, &str, serde_json::Value)]) {
            let records: Vec<serde_json::Value> = text
                .lines()
                .map(|line| {
                    serde_json::from_str(line)
                        .expect("fixture line is an independently valid JSON record")
                })
                .collect();
            for (index, field, expected) in premises {
                let record = records
                    .get(*index)
                    .unwrap_or_else(|| panic!("fixture record {index} exists in {text:?}"));
                assert_eq!(
                    record.get(*field),
                    Some(expected),
                    "fixture record {index} premise on field {field:?}"
                );
            }
        }

        let session = r#"{"type":"session","version":3,"id":"omp-id-1","timestamp":"2026-09-17T00:00:00.000Z","cwd":"/work"}
{"type":"user","text":"hello"}
"#;
        // A REAL title slot: exactly 256 UTF-8 bytes including its newline,
        // the fixed width OMP rewrites in place — not an arbitrary-width
        // look-alike.
        let title = {
            let base = r#"{"type":"title","v":1,"title":"my session","updatedAt":"2026-09-17T00:00:00.000Z","pad":""#;
            let spaces = 256 - base.len() - "\"}\n".len();
            assert!(spaces > 0, "fixture sanity: room for the pad");
            format!("{base}{}\"}}\n", " ".repeat(spaces))
        };
        assert_eq!(
            title.len(),
            256,
            "the title slot is the real 256-byte width"
        );
        let wrong_version = session.replace("\"version\":3", "\"version\":2");
        let foreign_record = session.replace("\"type\":\"session\"", "\"type\":\"turn\"");
        let wrong_title_shape = format!(
            "{}{}",
            r#"{"type":"title","v":2,"title":"t","updatedAt":"u","pad":"p"}"#, "\n"
        );
        let leading_summary = format!("{}\n{}", r#"{"type":"summary","text":"earlier"}"#, session);

        // The two real shapes. Their premises are asserted first: an
        // independently valid session record, and a well-formed 256-byte
        // title slot in front of one.
        assert_eq!(
            parse_omp_session_header(session).expect("a plain session header parses"),
            "omp-id-1"
        );
        assert_records(
            &format!("{title}{session}"),
            &[(0, "title", serde_json::json!("my session"))],
        );
        assert_eq!(
            parse_omp_session_header(&format!("{title}{session}"))
                .expect("exactly one well-formed title slot is skipped"),
            "omp-id-1"
        );

        // The optional `source` field mirrors OMP's own parseTitleSlotObject:
        // absent, "auto", or "user" is a title slot; every other value —
        // including explicit null and numbers — makes OMP decline the record,
        // so the file stops being a valid session header and Farhelm refuses
        // it instead of resuming into an agent-side load failure.
        let with_source = |source: &str| {
            format!(
                "{}{}",
                title.replace(
                    "\"updatedAt\":\"2026-09-17T00:00:00.000Z\"",
                    &format!("\"updatedAt\":\"2026-09-17T00:00:00.000Z\",\"source\":{source}"),
                ),
                session
            )
        };
        for (source, premise) in [
            ("\"auto\"", serde_json::json!("auto")),
            ("\"user\"", serde_json::json!("user")),
        ] {
            let fixture = with_source(source);
            assert_records(&fixture, &[(0usize, "source", premise)]);
            assert_eq!(
                parse_omp_session_header(&fixture)
                    .expect("a well-formed title slot with this source is skipped"),
                "omp-id-1"
            );
        }
        for (source, premise, why) in [
            (
                "\"bogus\"",
                serde_json::json!("bogus"),
                "an unknown source string",
            ),
            ("null", serde_json::json!(null), "an explicit null source"),
            ("42", serde_json::json!(42), "a numeric source"),
        ] {
            let fixture = with_source(source);
            assert_records(&fixture, &[(0usize, "source", premise)]);
            assert!(
                parse_omp_session_header(&fixture).is_err(),
                "{why} must refuse: OMP does not treat the record as a title slot"
            );
        }

        // Fixtures whose records are individually valid JSON: each asserts
        // its premise (wrong field values, right shape) so the refusal is
        // attributable to the mechanism named.
        for (fixture, premises, why) in [
            (
                format!("{title}{title}{session}"),
                vec![
                    (0usize, "title", serde_json::json!("my session")),
                    (1usize, "title", serde_json::json!("my session")),
                ],
                "a second title-shaped record is refused, not skipped",
            ),
            (
                format!("{wrong_title_shape}{session}"),
                vec![(0usize, "v", serde_json::json!(2))],
                "a leading record with the wrong title-slot SHAPE (v:2) is not a title slot",
            ),
            (
                wrong_version.clone(),
                vec![(0usize, "version", serde_json::json!(2))],
                "a session record of the wrong version is refused",
            ),
            (
                foreign_record.clone(),
                vec![(0usize, "type", serde_json::json!("turn"))],
                "any other record type is refused in the session slot",
            ),
            (
                leading_summary.clone(),
                vec![(0usize, "type", serde_json::json!("summary"))],
                "a leading non-title, non-session record is refused, not skipped",
            ),
        ] {
            assert_records(&fixture, &premises);
            assert!(
                parse_omp_session_header(&fixture).is_err(),
                "{why}: fixture must refuse"
            );
        }

        // Field-shape fixtures built by replacement, whose premises are the
        // replaced values themselves.
        for (fixture, why) in [
            (
                session.replace("\"version\":3,", ""),
                "a session record with no version is refused",
            ),
            (
                session.replace("\"id\":\"omp-id-1\"", "\"id\":42"),
                "a non-string id is refused",
            ),
            (
                session.replace("omp-id-1", "-option-shaped"),
                "an implausible (option-shaped) id is refused",
            ),
        ] {
            serde_json::from_str::<serde_json::Value>(fixture.lines().next().expect("line"))
                .expect("fixture record stays independently valid JSON");
            assert!(
                parse_omp_session_header(&fixture).is_err(),
                "{why}: fixture must refuse"
            );
        }

        // Fixtures whose premise IS malformation: no structure to assert.
        for (fixture, why) in [
            (
                format!(
                    "{title}{}",
                    "{\"type\":\"session\",\"version\":3,\"id\":\"omp-id-1\""
                ),
                "a prefix truncated mid-record is refused",
            ),
            (
                "{\"type\":\"session\",\"version\":3,\"id\":\"omp-id-1\"\n".to_string(),
                "a prefix ending mid-line without a title slot is refused",
            ),
            (
                "not json at all\n".to_string(),
                "a non-JSON prefix is refused (compressed bytes refuse here too)",
            ),
            (
                format!("\x1f\u{8b}\u{8}{session}"),
                "gzip magic bytes are refused rather than decompressed",
            ),
            (
                title.clone(),
                "a file that ends after its title slot is refused",
            ),
        ] {
            assert!(
                parse_omp_session_header(&fixture).is_err(),
                "{why}: fixture must refuse"
            );
        }
    }

    /// Verification goes through the shared no-follow regular-file reader, so
    /// a symlink — or any non-regular file — placed where the reported
    /// session file should be refuses at the read, and `OmpIntegration`'s
    /// parser rides on the OMP header rules rather than Pi's first-record
    /// rule. Each refusal is bounded by a completion oracle: the reader opens
    /// `O_NONBLOCK`, so none of these futures may park.
    #[farhelm_testtrace::test]
    async fn omp_record_reads_refuse_symlinks_and_non_regular_files() {
        use std::time::Duration;

        const ORACLE: Duration = Duration::from_secs(5);
        let dir = farhelm_teststate::tempdir().expect("tempdir");
        let session = r#"{"type":"session","version":3,"id":"omp-id-1"}
{"type":"user","text":"hello"}
"#;
        let real = dir.path().join("session.jsonl");
        std::fs::write(&real, session).expect("fixture file");
        let integration = integration_for(AgentKind::Omp).expect("OMP integration");

        let parsed =
            tokio::time::timeout(ORACLE, crate::agent_kind::read_record(&real, integration))
                .await
                .expect("a regular file's read completes, bounded")
                .expect("a regular file reads")
                .expect("a well-formed OMP header is a record");
        assert_eq!(parsed.0.conversation, "omp-id-1");

        let link = dir.path().join("link.jsonl");
        std::os::unix::fs::symlink(&real, &link).expect("symlink fixture");
        let read = tokio::time::timeout(ORACLE, crate::agent_kind::read_record(&link, integration))
            .await
            .expect("the symlink case completes, bounded");
        assert!(read.is_err(), "a symlink is refused through O_NOFOLLOW");

        // A FIFO named where a session file should be: opened without
        // blocking (O_NONBLOCK), then refused by the regular-file check —
        // never read, never parked on.
        let fifo = dir.path().join("pipe.jsonl");
        let fifo_c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes())
            .expect("fixture path has no NUL");
        assert_eq!(unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) }, 0);
        let read = tokio::time::timeout(ORACLE, crate::agent_kind::read_record(&fifo, integration))
            .await
            .expect("the FIFO case completes, bounded");
        assert!(read.is_err(), "a FIFO is not a regular session file");

        // An owned directory named where a session file should be.
        let directory = dir.path().join("directory.jsonl");
        std::fs::create_dir(&directory).expect("directory fixture");
        let read = tokio::time::timeout(
            ORACLE,
            crate::agent_kind::read_record(&directory, integration),
        )
        .await
        .expect("the directory case completes, bounded");
        assert!(read.is_err(), "a directory is not a regular session file");
    }

    /// `restart_offer` is what the UI (PR8) turns into an affordance, so
    /// every one of its three answers is pinned against the state that
    /// produces it. The subtle one is the last: a `{conversation}` template
    /// with nothing captured is FreshOnly, never FallbackTemplate, because
    /// SPEC.md forbids ever running the placeholder unfilled.
    #[farhelm_testtrace::test]
    fn the_restart_offer_reflects_exactly_what_could_honestly_be_run() {
        let claude = IntegrationSnapshot::resolve(&["claude".into()], None, None).unwrap();
        assert_eq!(claude.restart_offer(None, 0), RestartOffer::FreshOnly);
        assert_eq!(
            claude.restart_offer(Some("conv-1"), 0),
            RestartOffer::Resume
        );

        let fallback = IntegrationSnapshot::resolve(
            &["some-agent".into()],
            None,
            Some(vec!["some-agent".to_string(), "--continue".to_string()]),
        )
        .unwrap();
        assert_eq!(
            fallback.restart_offer(None, 0),
            RestartOffer::FallbackTemplate,
            "a placeholder-free template is the one thing that can be run verbatim"
        );

        let generic = IntegrationSnapshot::resolve(&["bash".into()], None, None).unwrap();
        assert_eq!(generic.restart_offer(None, 0), RestartOffer::FreshOnly);

        // This models a legacy stored snapshot. New generic creates reject
        // the unfillable template, but previously stored state still needs
        // to avoid advertising a restart command that cannot be run.
        let unfillable = IntegrationSnapshot {
            kind: AgentKind::Generic,
            resume_template: Some(vec![
                "bash".to_string(),
                CONVERSATION_PLACEHOLDER.to_string(),
            ]),
        };
        assert_eq!(unfillable.restart_offer(None, 0), RestartOffer::FreshOnly);
    }

    /// The munging is the audited reason correlation cannot use directory
    /// names, so the collision is pinned as a PROPERTY rather than left as
    /// prose: `/tmp/a.b` and `/tmp/a-b` genuinely land in one directory,
    /// and any change that made this function injective would silently
    /// stop matching the real agent's own layout.
    #[farhelm_testtrace::test]
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
    #[farhelm_testtrace::test]
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

    /// Codex's transcript contains events from internal work as well as the
    /// root conversation. A resumable locator needs the root session metadata
    /// payload; accepting a flat or mixed-level record would fabricate an
    /// attribution that the transcript never made.
    #[farhelm_testtrace::test]
    fn codex_requires_root_session_metadata_in_one_payload() {
        let nested = "{\"timestamp\":\"2026-07-29T12:00:05Z\",\"type\":\"session_meta\",\
                      \"payload\":{\"source\":\"cli\",\"id\":\"roll-1\",\"session_id\":\"runtime-1\",\"cwd\":\"/work\"}}\n";
        assert_eq!(
            CodexIntegration.parse_record(nested).unwrap().unwrap(),
            RecordCorrelators {
                conversation: "roll-1".to_string(),
                cwd: "/work".to_string(),
                created_at: parse_rfc3339("2026-07-29T12:00:05Z").unwrap(),
            }
        );

        // An ordinary event carrying root-looking fields is not a record.
        let event = "{\"timestamp\":\"2026-07-29T12:00:05Z\",\"type\":\"turn_context\",\
                     \"payload\":{\"source\":\"cli\",\"id\":\"nope\",\"session_id\":\"runtime-2\",\"cwd\":\"/work\"}}\n";
        assert!(CodexIntegration.parse_record(event).is_err());

        // A payload without its root source is not a resumable conversation.
        let mixed = "{\"timestamp\":\"2026-07-29T12:00:05Z\",\"type\":\"session_meta\",\
                     \"cwd\":\"/work\",\"payload\":{\"id\":\"roll-3\",\"session_id\":\"runtime-3\"}}\n";
        assert!(CodexIntegration.parse_record(mixed).is_err());
    }

    /// A Codex locator reads only the transcript's root header. It must not
    /// scan forward for a later identity, because that is the directory-scan
    /// inference removed to prevent unrelated conversations being claimed.
    #[farhelm_testtrace::test]
    fn a_codex_header_without_complete_root_metadata_stays_unresumable() {
        let text = "{\"type\":\"session_meta\",\"payload\":{\"id\":\"early\",\"cwd\":\"/work\"}}\n\
                    {\"timestamp\":\"2026-07-29T12:00:05Z\",\"type\":\"session_meta\",\
                    \"payload\":{\"source\":\"cli\",\"id\":\"real\",\"session_id\":\"runtime-real\",\"cwd\":\"/work\"}}\n";
        assert!(CodexIntegration.parse_record(text).is_err());
    }

    /// A conversation id crosses from an on-disk file into a durable
    /// column, a log line, and eventually an agent's argv. Anything that
    /// is not an identifier under any plausible vendor format is refused —
    /// and refused LOUDLY (an error, marking the scan incomplete) rather
    /// than dropped, since a dropped candidate is exactly the second one
    /// whose absence would turn an ambiguity into a wrong claim.
    #[farhelm_testtrace::test]
    fn implausible_conversation_identifiers_are_refused() {
        assert!(is_plausible_conversation_id("0b0a3d65-a742-4b0e-bda5-c59"));
        assert!(!is_plausible_conversation_id(""));
        assert!(!is_plausible_conversation_id("has space"));
        assert!(!is_plausible_conversation_id("has\nnewline"));
        assert!(!is_plausible_conversation_id("has\"quote"));
        // Every marker a later substitution pass rewrites, including the two
        // Codex trust markers the check once missed.
        for placeholder in RESERVED_PLACEHOLDERS {
            assert!(!is_plausible_conversation_id(placeholder), "{placeholder}");
        }
        assert!(!is_plausible_conversation_id(
            &"x".repeat(MAX_CONVERSATION_ID_LEN + 1)
        ));
        let long = format!(
            "{{\"sessionId\":\"{}\",\"cwd\":\"/w\",\"timestamp\":\"2026-07-29T12:00:05Z\"}}",
            "x".repeat(MAX_CONVERSATION_ID_LEN + 1)
        );
        assert!(ClaudeIntegration.parse_record(&long).is_err());
    }

    // -------------------------------------------------------------------
    // Hook injection (plan §2.2): `hook_argv`, `toml_basic_string`, and the
    // `FARHELM_AGENT_HOOKS` grammar.
    // -------------------------------------------------------------------

    /// [`AgentKind::Generic`] has no integration and therefore no hook —
    /// not a gap, but the definition of generic (see [`integration_for`]'s
    /// own doc comment). Pinned directly because every other test in this
    /// file that exercises `hook_argv` does so through a concrete
    /// `ClaudeIntegration`/`CodexIntegration` value and would never notice
    /// if this property broke.
    #[farhelm_testtrace::test]
    fn generic_kind_has_no_integration_and_therefore_no_hook() {
        assert!(
            integration_for(AgentKind::Generic).is_none(),
            "a generic session must fall through to the scan unconditionally; there is no \
             hook_argv to even ask"
        );
    }

    /// The one property that actually matters about `hook_argv`: a path
    /// hostile to EITHER quoting layer survives being embedded through
    /// BOTH of them and comes back out as the exact six argv elements
    /// `farhelm internal hook` was launched with (the `--vendor`
    /// discriminator included).
    ///
    /// `hostile_path` is chosen to hit every character each layer is
    /// responsible for: a space (breaks an unquoted shell word), a single
    /// quote (the character shell quoting itself must escape), a double
    /// quote and a backslash (the characters JSON/TOML string escaping
    /// must handle). The two vendors are checked in the same test, against
    /// the same path, because the property under test is that the SAME
    /// underlying command survives two structurally different renderings
    /// — a property a single-vendor test could not distinguish from "this
    /// vendor's quoting happens to work".
    #[farhelm_testtrace::test]
    fn hook_argv_survives_a_path_hostile_to_both_quoting_layers() {
        let hostile_path = r#"/tmp/a b's "q" \dir/farhelm"#;
        // The default (announcing) shape, which is what ships. The `off`
        // shape is checked at the end, against the same path.
        // Each vendor's command names its own adapter after `hook`: the
        // envelope discriminator rides the installed entry point, so the
        // supervisor can gate on it before any vendor I/O.
        let expected_words_for = |vendor: &str| {
            vec![
                hostile_path.to_string(),
                "internal".to_string(),
                "hook".to_string(),
                "--vendor".to_string(),
                vendor.to_string(),
                "--announce".to_string(),
            ]
        };
        let expected_words = expected_words_for("claude");

        // --- Claude: `["--settings", <json>]` ---
        let claude_argv = ClaudeIntegration.hook_argv(hostile_path, AgentInstructions::On);
        assert_eq!(
            claude_argv.len(),
            2,
            "Claude's tail is exactly the flag and its value: a third element would be an \
             extra token appended to the user's command line, and on a vendor that takes a \
             trailing positional prompt that is text typed at the agent"
        );
        assert_eq!(claude_argv[0], "--settings");
        let claude_json: serde_json::Value = serde_json::from_str(&claude_argv[1])
            .expect("Claude's --settings value must be valid JSON");
        let claude_hook = &claude_json["hooks"]["SessionStart"][0]["hooks"][0];
        assert_eq!(claude_hook["type"], "command");
        assert_eq!(claude_hook["timeout"], 5);
        let claude_command = claude_hook["command"]
            .as_str()
            .expect("command must be a JSON string");
        let claude_words = shell_words::split(claude_command)
            .expect("Claude's rendered command must be one valid shell command line");
        assert_eq!(claude_words, expected_words);

        // --- Codex: five argv elements, order and identity pinned. ---
        let codex_argv = CodexIntegration.hook_argv(hostile_path, AgentInstructions::On);
        assert_eq!(codex_argv.len(), 5);
        assert_eq!(
            codex_argv[0], "--dangerously-bypass-hook-trust",
            "the bypass flag must lead the injected tail: it is what makes every -c override \
             after it actually take effect without an interactive trust dialog"
        );
        assert_eq!(codex_argv[1], "-c");
        assert_eq!(codex_argv[2], "features.hooks=true");
        assert_eq!(codex_argv[3], "-c");
        let hook_value = codex_argv[4]
            .strip_prefix("hooks.SessionStart=")
            .expect("the fifth element must be the SessionStart declaration");
        // The stripped value is a valid TOML *value*, not a document, so
        // it is wrapped in a throwaway `v = ...` assignment before
        // `toml::from_str` — the same trick `toml_basic_string`'s own
        // round-trip test below uses.
        let document: toml::Value = toml::from_str(&format!("v = {hook_value}"))
            .expect("Codex's rendered -c value must be valid TOML");
        let codex_command = document["v"][0]["hooks"][0]["command"]
            .as_str()
            .expect("command must be a TOML string");
        assert_eq!(
            document["v"][0]["hooks"][0]["type"].as_str(),
            Some("command")
        );
        assert_eq!(
            document["v"][0]["hooks"][0]["timeout"].as_integer(),
            Some(5)
        );
        let codex_words = shell_words::split(codex_command)
            .expect("Codex's rendered command must be one valid shell command line");
        assert_eq!(codex_words, expected_words_for("codex"));

        // --- The same path with the pointer turned off. ---
        //
        // `--announce` is appended AFTER the shell-quoted path, so it is
        // the one part of the command that is not itself quoted. Checking
        // both settings against the same hostile path is what pins that
        // the flag rides outside the quoting rather than getting swallowed
        // into it — a mistake that would produce a path argument nobody
        // can exec, and only for users with a space in their install
        // directory.
        for (kind, argv) in [
            (
                "claude",
                ClaudeIntegration.hook_argv(hostile_path, AgentInstructions::Off),
            ),
            (
                "codex",
                CodexIntegration.hook_argv(hostile_path, AgentInstructions::Off),
            ),
        ] {
            let rendered = argv.join(" ");
            assert!(
                !rendered.contains("--announce"),
                "{kind}: instructions off must not carry the flag: {rendered}"
            );
            let command = match kind {
                "claude" => serde_json::from_str::<serde_json::Value>(&argv[1])
                    .expect("valid JSON")["hooks"]["SessionStart"][0]["hooks"][0]["command"]
                    .as_str()
                    .expect("a command string")
                    .to_string(),
                _ => {
                    let value = argv[4]
                        .strip_prefix("hooks.SessionStart=")
                        .expect("the SessionStart declaration");
                    toml::from_str::<toml::Value>(&format!("v = {value}")).expect("valid TOML")["v"]
                        [0]["hooks"][0]["command"]
                        .as_str()
                        .expect("a command string")
                        .to_string()
                }
            };
            let silent_words: Vec<String> = expected_words_for(kind)
                .iter()
                .filter(|word| *word != "--announce")
                .cloned()
                .collect();
            assert_eq!(
                shell_words::split(&command).expect("one valid shell command line"),
                silent_words,
                "{kind}: the quoting must survive with the flag absent too"
            );
        }
    }

    /// `toml_basic_string`'s escaping is only as good as its ability to
    /// survive TOML's own parser, so this test uses the `toml` crate (a
    /// dev-dependency only — see the function's doc comment for why it is
    /// not a runtime one) as an independent oracle rather than re-deriving
    /// the escaping rules by hand.
    ///
    /// The character set is exactly what the function's doc comment
    /// discusses: a space, both quote characters, a backslash, a tab, a
    /// newline, the one gap between JSON's and TOML's escaping rules (a
    /// raw DEL byte), and two non-ASCII cases — a two-byte accented letter
    /// and a four-byte emoji, since the doc comment's surrogate-pair claim
    /// is specifically about characters outside the Basic Multilingual
    /// Plane.
    #[farhelm_testtrace::test]
    fn toml_basic_string_round_trips_through_escaping() {
        let original = "a space, a 'quote', a \"quote\", a \\backslash, a\ttab, a\nnewline, \
                         DEL:\u{7f}:, and non-ASCII: é😀";
        let rendered = toml_basic_string(original);
        let document: toml::Value = toml::from_str(&format!("v = {rendered}"))
            .unwrap_or_else(|e| panic!("toml rejected the rendered string {rendered:?}: {e}"));
        assert_eq!(
            document["v"].as_str(),
            Some(original),
            "the value TOML parsed back must equal the original input before it was escaped"
        );
    }

    /// `parse_agent_hooks`'s full documented grammar, pinned case by case.
    /// This function is the ONLY parser of `FARHELM_AGENT_HOOKS` in the
    /// codebase (its own doc comment), so every accepted and rejected
    /// shape belongs in a test here rather than being re-derived, and
    /// possibly re-diverged, at the one call site that actually reads the
    /// environment variable.
    #[farhelm_testtrace::test]
    fn parse_agent_hooks_covers_the_documented_grammar() {
        assert_eq!(parse_agent_hooks("all"), AgentHooks::All);
        assert_eq!(parse_agent_hooks("none"), AgentHooks::None);
        assert_eq!(
            parse_agent_hooks("claude"),
            AgentHooks::Only(vec![AgentKind::Claude])
        );
        assert_eq!(
            parse_agent_hooks("omp"),
            AgentHooks::Only(vec![AgentKind::Omp])
        );
        assert_eq!(
            parse_agent_hooks("codex,claude"),
            AgentHooks::Only(vec![AgentKind::Codex, AgentKind::Claude]),
            "input order is preserved rather than normalized; `allows` does not care, but \
             nothing in the parser should silently reorder it either"
        );
        assert_eq!(
            parse_agent_hooks(" claude , codex "),
            AgentHooks::Only(vec![AgentKind::Claude, AgentKind::Codex]),
            "whitespace around each token, and around the whole value, is trimmed"
        );
        assert_eq!(
            parse_agent_hooks(""),
            AgentHooks::All,
            "an empty string means the same as an absent variable, so a profile that SETS the \
             variable to nothing does not accidentally disable every hook"
        );
        assert_eq!(
            parse_agent_hooks("bogus"),
            AgentHooks::All,
            "an unrecognized token falls back to the safe default (all kinds hooked) rather \
             than to None — this variable is an opt-OUT, and a typo must not silently turn \
             into opting out of everything"
        );
        assert_eq!(
            parse_agent_hooks("claude,bogus"),
            AgentHooks::All,
            "one bad token invalidates the WHOLE value rather than being dropped from the \
             list — a partially-applied list would be a second, undocumented grammar"
        );

        // Case folding applies to every branch of the grammar, not just to
        // the kind names: this is a value typed into a shell profile, and
        // a user who capitalizes one word capitalizes all of them.
        assert_eq!(parse_agent_hooks("ALL"), AgentHooks::All);
        assert_eq!(parse_agent_hooks("NONE"), AgentHooks::None);
        assert_eq!(
            parse_agent_hooks("Claude,CODEX"),
            AgentHooks::Only(vec![AgentKind::Claude, AgentKind::Codex])
        );

        // An empty element is an unrecognized token, not a skipped one.
        // Pinned in all three positions a stray comma can occupy because
        // the tempting "tidy" fix — filtering empties out before matching
        // — would change this behavior silently and in the dangerous
        // direction: it would make `,` alone parse as an empty `Only`
        // list, i.e. as `none`, out of what is almost certainly a typo.
        for value in ["claude,,codex", ",claude", "codex,"] {
            assert_eq!(
                parse_agent_hooks(value),
                AgentHooks::All,
                "{value:?}: an empty element takes the unrecognized-token path, warning and \
                 falling back to the default rather than narrowing the opt-out"
            );
        }
    }

    /// `parse_agent_instructions`'s full documented grammar, case by case.
    ///
    /// The same reasoning as its neighbour above: this is the only parser
    /// of `FARHELM_AGENT_INSTRUCTIONS`, so the grammar its doc comment
    /// promises is only real if it is pinned here. The half that matters
    /// most is the fallback DIRECTION — an unreadable value has to land on
    /// `On`, because the switch's `off` position removes a feature and a
    /// typo must not remove it silently. A test that only checked `on` and
    /// `off` would pass just as happily with the fallback inverted.
    #[farhelm_testtrace::test]
    fn parse_agent_instructions_covers_the_documented_grammar() {
        assert_eq!(parse_agent_instructions("on"), AgentInstructions::On);
        assert_eq!(parse_agent_instructions("off"), AgentInstructions::Off);
        assert_eq!(
            parse_agent_instructions(""),
            AgentInstructions::On,
            "an empty string means the same as an absent variable, so a profile that SETS \
             the variable to nothing does not accidentally silence the pointer"
        );
        assert_eq!(
            parse_agent_instructions("  off  "),
            AgentInstructions::Off,
            "surrounding whitespace is trimmed; a shell profile is not a wire format"
        );
        assert_eq!(parse_agent_instructions("OFF"), AgentInstructions::Off);
        assert_eq!(parse_agent_instructions("On"), AgentInstructions::On);
        for value in ["false", "0", "no", "none", "disabled", "of"] {
            assert_eq!(
                parse_agent_instructions(value),
                AgentInstructions::On,
                "{value:?}: an unrecognized value warns and falls back to the default rather \
                 than being read as an attempt to turn the pointer off"
            );
        }
        assert_eq!(
            AgentInstructions::default(),
            AgentInstructions::On,
            "the default is unconditional, never a live environment read"
        );
    }

    /// `AgentInstructions::announces` is the one question the injection
    /// asks of this value, pinned directly so a future variant cannot
    /// quietly change what `On` means.
    #[farhelm_testtrace::test]
    fn agent_instructions_announces_only_when_on() {
        assert!(AgentInstructions::On.announces());
        assert!(!AgentInstructions::Off.announces());
    }

    /// `AgentHooks::allows` is what every hook-injection call site actually
    /// consults; this pins its three-way behavior directly, independent of
    /// how the value was constructed, plus the unconditional `All` default
    /// that [`AgentHooks`]'s own doc comment promises.
    #[farhelm_testtrace::test]
    fn agent_hooks_allows_reflects_its_variant() {
        assert!(AgentHooks::All.allows(AgentKind::Claude));
        assert!(AgentHooks::All.allows(AgentKind::Codex));
        assert!(AgentHooks::All.allows(AgentKind::Generic));

        assert!(!AgentHooks::None.allows(AgentKind::Claude));
        assert!(!AgentHooks::None.allows(AgentKind::Codex));

        let only_claude = AgentHooks::Only(vec![AgentKind::Claude]);
        assert!(only_claude.allows(AgentKind::Claude));
        assert!(!only_claude.allows(AgentKind::Codex));

        assert_eq!(
            AgentHooks::default(),
            AgentHooks::All,
            "the seam's default must never consult the environment (module doc comment); it \
             is unconditionally All"
        );
    }
}
