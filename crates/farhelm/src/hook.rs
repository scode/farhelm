//! `farhelm internal hook`: a bounded vendor callback that reports the
//! conversation identity the agent is currently using.
//!
//! Most adapters report a session-start event. Grok uses three manually
//! configured callbacks (`SessionStart`, `UserPromptSubmit`, and `Stop`) so
//! the first event can select a UUID and later events can supply its exact
//! record path. In every case this module runs as a short-lived child below
//! the agent process, inside the agent's terminal, with the session's
//! identity already in its environment. It reads one JSON payload from
//! stdin, records its own process ancestry, and drops the report as a file
//! into the session's drop directory under the supervisor's state directory
//! (`farhelm_supervisor::hook_report`), where the supervisor applies it on
//! its next reconciliation pass. It never talks to the supervisor and never
//! waits for it, so a report made while no supervisor is running (on the
//! Mac, whenever the desktop app is closed) waits on disk instead of being
//! lost, and the agent is never held up by a slow or absent supervisor.
//! Everything else about it is a consequence of *where* it runs.
//!
//! ## The contract
//!
//! These rules are not stylistic; each one exists because the alternative
//! is visible to the human using the agent.
//!
//! 1. **Silence on stdout and stderr, except for one deliberate line.**
//!    The identity work itself writes nothing to either descriptor. Claude
//!    and Codex feed `SessionStart` stdout into the model's context, and hook
//!    failures may surface in the vendor UI, so a
//!    diagnostic printed to either stream here is at best noise in
//!    someone's terminal and at worst text the model reads as instruction.
//!    (The per-session hook log below is the one place this run DOES
//!    write, on purpose, precisely because it is not either of those
//!    streams.) The single stdout exception is [`POINTER_LINE`], written
//!    by [`announce`] when the supervisor injected `--announce`; it is
//!    written on purpose, precisely BECAUSE the model reads it. See that
//!    constant for what the vendors do with it and why it is one line.
//! 2. **Exit 0, always** — including on panic. The caller ([`crate`]'s
//!    `InternalCmd::Hook` arm) installs a no-op panic hook so a panic
//!    prints nothing, and [`run_with`] catches unwinds so no failure can
//!    turn into a non-zero status the vendor surfaces as a hook error.
//! 3. **A bounded stdin read.** Everything after the read is local and
//!    quick (a few `/proc` reads, one small file written and renamed), but
//!    the read itself waits on the vendor, which may hold the pipe open.
//!    Overrunning the vendor's own hook timeout is exactly the failure that
//!    shows up in the agent's UI, so the read is bounded by [`HOOK_BUDGET`];
//!    see [`run_with`] for why that forces a detached reader thread.
//! 4. **No credential, no IDENTITY work.** Without the three injected
//!    environment values (session id, session token, supervisor socket)
//!    this is not a Farhelm launch (someone ran the agent outside Farhelm
//!    with its callback still configured); the run logs `no-credential` and
//!    stops without writing a report. The token is not written anywhere —
//!    the report file sits in the supervisor's private state directory and
//!    needs none — but its presence is still what marks a real launch. This
//!    rule is scoped to identity capture only: [`announce`] needs no
//!    credential at all and still prints [`POINTER_LINE`] whenever
//!    `--announce` was passed — the pointer is a fact about the launch, not
//!    about whether a report can be made.
//! 5. **Nothing about the payload is trusted.** Unknown fields are
//!    ignored, and the reported id is an opaque string this side merely
//!    length-checks — the supervisor owns plausibility and every admission
//!    check (`farhelm_supervisor::hook_report`'s "Trust" section).
//!
//! ## The hook log
//!
//! Because nothing may be printed, the per-session log file is the only
//! place a failure is ever visible. Every run that reaches its logger appends
//! **exactly one line** and then stops; a run never writes two lines, so
//! counting this hook's lines counts completed runs. A vendor that kills the child first
//! can prevent that final diagnostic write. Pi and OMP reporters do exactly
//! that at their published two-second child timer (see `pi_extension.rs` for
//! why that timer stays). The file lives at
//! `<state_dir>/hook-log/<session-id>.log`, where
//! `<state_dir>` is the parent of the supervisor socket.
//!
//! That path needs only the session id and the socket — not the token —
//! and the caller derives it that way ON PURPOSE. A half-configured
//! environment is exactly the case a human comes to this file for, so a
//! run missing only its token still leaves its `no-credential` line
//! behind. Only a run with no session id or no socket at all has nowhere
//! to write, and then there is nothing to say about which session it
//! belonged to either.
//!
//! The line says what THIS run did: whether it wrote a report, not whether
//! the supervisor accepted it. The supervisor judges the report later, on its
//! next reconciliation pass, and appends its own verdict line to the same file
//! (`acked`, or `refused <error kind> <reason>`, with the same identity pair).
//! The report is in place before this run appends its own line, so a pass
//! that lands in between can, rarely, put the verdict above the `written`
//! line it answers.
//! The shape and the sanitizing of both writers' lines are defined once, in
//! `farhelm_supervisor::hook_report`.
//!
//! ```text
//! <unix-seconds> <outcome> [<outcome detail> ]<conversation-id> <source>
//! ```
//!
//! The trailing `<conversation-id> <source>` pair is present only once the
//! payload has parsed — i.e. from the moment there is an id to name — and
//! `<source>` is `-` when the vendor sent no usable `source` field. Usable
//! means a JSON string: a `source` that is `null`, a number, or any other
//! shape renders as `-` exactly like an absent one, because the field is
//! diagnostic-only and refusing a report over its TYPE would trade a
//! working resume for a log nicety.
//!
//! Grok's payloads are the exception. Its `SessionStart` must carry a
//! `source` of `new` or `load`, and one without it (absent or `null`) is
//! `bad-payload unsupported-session-source`. Separately, `parse_grok_payload`
//! refuses a `source` that is present but not a string (a number, an object)
//! on every Grok event as `bad-payload source-not-a-string`; an absent or
//! `null` one on the other events still renders as `-`.
//!
//! Outcome words, and the detail each carries:
//!
//! | Outcome | Detail |
//! | --- | --- |
//! | `written` | —, or `no-ancestry: <error>` when the process ancestry could not be recorded |
//! | `write-failed` | the I/O error |
//! | `no-credential` | — |
//! | `bad-payload` | a one-word reason (`unparsable`, `missing-session-id`, `subagent-report`, …), or `no-reader: <io error>` |
//! | `timeout` | `stdin` |
//! | `panic` | — |
//!
//! A report written without its ancestry is still worth writing — the
//! supervisor will refuse it, since it cannot tie the report to the
//! session's current launch, but its refusal names that — and the detail
//! says why the evidence was missing.
//!
//! ```text
//! 1724470000 written conv-1 startup
//! 1724470002 acked conv-1 startup
//! 1724470000 write-failed No such file or directory (os error 2) conv-1 -
//! 1724470000 timeout stdin
//! ```
//!
//! Every value interpolated into a line is sanitized and length-capped
//! before it is written: control characters and the Unicode
//! direction-and-line controls always become `_`, and the two
//! trailing identity fields additionally lose their spaces so they stay
//! single positional tokens. Free-form detail keeps its spaces, since an
//! I/O error is a sentence. The conversation id and source come from the
//! agent — the same process that could otherwise embed a newline and forge
//! a log line — and a log nobody can trust to be one-line-per-run is worse
//! than no log.
//!
//! Every failure of logging itself — a missing directory that cannot be
//! created, an unwritable path, a full disk — is ignored. The log exists
//! to explain a broken run, never to become one.

use farhelm_proto::ReportVendor;
use farhelm_supervisor::hook_report::{self, HookReport};
use std::io::Read;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Largest stdin payload accepted. A hook payload is normally a few hundred
/// bytes of JSON; the cap exists so a vendor (or anything else
/// holding our stdin) cannot make the hook allocate without bound while
/// the budget runs down.
const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

/// Longest `session_id` this side will forward. Purely a sanity bound —
/// the supervisor makes the real plausibility judgement — but forwarding
/// a megabyte "id" would only convert a bad payload into a bad report.
const MAX_SESSION_ID_BYTES: usize = 128;

/// The production bound on reading the vendor's payload, shared by every
/// vendor entry point.
///
/// Reading stdin is the one step of a run that waits on anyone else: a
/// vendor normally closes the pipe right after writing a few hundred bytes,
/// but one that holds it open must not hold the hook past the vendor's own
/// timer. Farhelm sets or documents 60-second timers for its hook
/// declarations, and sessions launched before an upgrade may still carry
/// Claude's or Codex's old five-second one, where a vendor that never
/// closes the pipe kills the hook first and its log line is lost. The
/// published Pi and OMP reporters keep their own two-second child timers;
/// tests may pass a lower value through the child-only environment seam in
/// `main.rs`.
pub const HOOK_BUDGET: Duration = Duration::from_secs(30);

/// The one line the hook is allowed to say out loud — the pointer that
/// tells an agent `farhelm agent instructions` exists.
///
/// ## Why stdout reaches the model at all
///
/// Claude and Codex, the two integrations that enable this announcement,
/// treat plain-text stdout from `SessionStart` as model context. Both were
/// checked rather than assumed:
///
/// - Claude Code (<https://code.claude.com/docs/en/hooks>): "For most
///   events, Claude Code writes stdout to the debug log and doesn't show
///   it in the transcript. The exceptions are `UserPromptSubmit`,
///   `UserPromptExpansion`, `SessionStart`, and `PostModelSwitch`, where
///   Claude Code adds plain-text stdout as context that Claude can see and
///   act on."
/// - Codex (<https://learn.chatgpt.com/docs/hooks>), under `SessionStart`:
///   "Plain text on `stdout` is added as extra developer context." Most
///   other Codex hook events say the opposite ("Plain text on `stdout` is
///   ignored"), so this is an event-specific guarantee, not a general one.
///
/// That symmetry is why there is no Codex-specific fallback here — no file
/// under farhelm's state directory, no `model_instructions_file` override.
/// One mechanism serves both announcement-enabled vendors. Grok's manually
/// configured callbacks stay silent and never request this line.
///
/// ## Why the wording is constrained
///
/// Both vendors decide plain-text-versus-JSON by SHAPE: stdout that starts
/// with `{` and ends with `}` is parsed as JSON, and Codex fails the hook
/// run outright when that parse does not succeed. So this line must not
/// begin with `{`. It also stays ASCII and single-line: it is spliced into
/// a context window by a vendor that wraps it in machinery of its own, and
/// there is nothing to gain from making that splice interesting.
///
/// It is short on purpose. Every session pays for it whether or not the
/// user ever writes `$farhelm ...`, so the line buys exactly one thing —
/// knowing the command exists — and the instructions themselves are paid
/// for only by a session that goes and runs it.
pub const POINTER_LINE: &str = farhelm_supervisor::agent_kind::INSTRUCTIONS_POINTER;

/// Write [`POINTER_LINE`] and nothing else, ignoring any failure.
///
/// Failure is IGNORED rather than reported, and the distinction matters
/// more than it looks: `println!` panics when the write fails, and a hook
/// whose stdout is a closed pipe (a vendor that gave up on us, a
/// `--announce` run outside any agent) would then unwind out of `main` and
/// exit non-zero — turning the nicety into exactly the visible hook error
/// the whole module exists to avoid. There is also nowhere to report to:
/// stderr belongs to the agent's terminal.
///
/// Takes the sink as a parameter so the bytes can be asserted in a unit
/// test without a process and without touching the real stdout.
pub fn announce(out: &mut impl std::io::Write) {
    let _ = writeln!(out, "{POINTER_LINE}");
}

/// Where a report goes, derived by the caller from a complete session
/// credential in the environment.
///
/// A struct rather than values read from the environment inside
/// [`run_with`] because this repo's tests never mutate the process
/// environment — and, more sharply, because a test process running inside
/// a real farhelm session already carries those variables and would
/// otherwise drop reports into a live supervisor's state directory. The
/// environment read stays in the `main.rs` arm; everything testable takes
/// the destination as a value. It exists only for a complete credential
/// (contract rule 4), although the token itself is not carried.
pub struct HookCredential {
    /// The farhelm session this hook is reporting for — the identity whose
    /// drop directory receives the report, not the vendor's conversation id.
    pub session_id: String,
    /// The supervisor's state directory: the parent of its socket.
    pub state_dir: PathBuf,
}

/// Run one hook report to completion and record the outcome in `hook_log`.
/// `budget` bounds the stdin read, the only step that waits on anyone.
///
/// Never returns an error and never panics out: the caller's only job
/// after this returns is to exit 0. `payload` is taken by value because it
/// is moved onto a reader thread that outlives this call.
///
/// ## Why the payload is read on a detached thread
///
/// `budget` covers reading stdin, and a blocking `std::io::Read` cannot be
/// interrupted once it is waiting on a pipe the vendor keeps open. So the
/// read happens on a plain `std::thread` that is spawned and never joined,
/// handing bytes back over a channel; the main thread waits with
/// `recv_timeout`. A stuck reader thread is simply abandoned, and the
/// caller exits the process shortly afterwards, which is what makes the
/// abandonment immediate instead of merely eventual.
pub fn run_with(
    credential: Option<HookCredential>,
    payload: impl Read + Send + 'static,
    budget: Duration,
    hook_log: Option<PathBuf>,
    entry_vendor: ReportVendor,
) {
    // Two nested catches, for two different failures. The inner one turns
    // a panic in the work into the `panic` outcome, so the log still gets
    // its one line; the outer one guarantees that even a panic while
    // logging cannot escape into the caller, which must reach `exit(0)`.
    let outcome = match std::panic::catch_unwind(AssertUnwindSafe(|| {
        run_inner(credential, payload, budget, entry_vendor)
    })) {
        Ok(outcome) => outcome,
        Err(_) => Outcome::word("panic"),
    };
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
        // The supervisor's clock reading: a pre-epoch clock reads as 0, and a
        // nonsense timestamp is still better than losing the line.
        append_log(
            hook_log.as_deref(),
            &outcome.render(farhelm_supervisor::store::now_unix().max(0) as u64),
        );
    }));
}

/// The hook's actual work, minus the panic and logging shell.
///
/// Returns the single outcome the run is described by. Phase order is the
/// contract's priority order: a missing credential short-circuits before
/// stdin is even read, because there is nowhere to drop a payload and the
/// vendor's `SessionStart` payload is far smaller than a pipe buffer, so
/// declining to drain it cannot block the agent.
fn run_inner(
    credential: Option<HookCredential>,
    payload: impl Read + Send + 'static,
    budget: Duration,
    entry_vendor: ReportVendor,
) -> Outcome {
    let Some(credential) = credential else {
        return Outcome::word("no-credential");
    };
    let bytes = match read_payload(payload, budget) {
        Ok(bytes) => bytes,
        Err(PayloadError::Timeout) => return Outcome::detail("timeout", "stdin"),
        Err(PayloadError::Reason(reason)) => return Outcome::detail("bad-payload", reason),
        Err(PayloadError::NoReader(err)) => {
            return Outcome::detail("bad-payload", format!("no-reader: {err}"));
        }
    };
    let mut report = match parse_payload(&bytes, entry_vendor) {
        Ok(report) => report,
        Err(reason) => return Outcome::detail("bad-payload", reason),
    };
    // A report that names a sub-agent is never accepted, and writing it
    // would replace whatever report the session's own agent left in the same
    // slot and has not had applied yet — while the supervisor is down, that
    // could be the only record of a conversation switch. So it is dropped
    // here, before it can take the slot; the supervisor still refuses one
    // that arrives some other way.
    if let Some(reason) = subagent_refusal(report.agent_id.as_ref()) {
        return Outcome::detail("bad-payload", reason).about(&report.conversation, &report.source);
    }
    // The ancestry is recorded HERE, while this process and its parents are
    // still running: it is the evidence the supervisor attributes the report
    // with, possibly long after they have all exited. A failure to read it
    // does not stop the report — see the module docs on `no-ancestry`.
    let missing_ancestry = match hook_report::record_own_ancestry() {
        Ok(recorded) => {
            report.ancestry = Some(recorded.links);
            report.ancestry_ended = recorded.ended;
            None
        }
        Err(error) => Some(error),
    };
    let outcome =
        match hook_report::write_report(&credential.state_dir, &credential.session_id, &report) {
            Ok(()) => match missing_ancestry {
                None => Outcome::word("written"),
                Some(error) => Outcome::detail("written", format!("no-ancestry: {error}")),
            },
            Err(error) => Outcome::detail("write-failed", error.to_string()),
        };
    outcome.about(&report.conversation, &report.source)
}

/// Why a report must not be written because of its sub-agent marker, or
/// `None` when it names no sub-agent. Mirrors the supervisor's own check: a
/// non-empty string marker is a sub-agent, an absent, `null` or empty one is
/// none, and any other shape is refused rather than read as absent.
fn subagent_refusal(agent_id: Option<&serde_json::Value>) -> Option<&'static str> {
    match agent_id {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(marker)) if marker.is_empty() => None,
        Some(serde_json::Value::String(_)) => Some("subagent-report"),
        Some(_) => Some("agent-id-not-a-string"),
    }
}

/// Why a payload never arrived intact.
enum PayloadError {
    /// The budget expired while the reader thread was still blocked.
    Timeout,
    /// A one-word reason for the log's `bad-payload` detail.
    Reason(&'static str),
    /// The reader thread could not be started at all — a process-level
    /// resource failure, reported as a payload failure because the
    /// observable effect is identical: there is no payload.
    NoReader(std::io::Error),
}

/// Read the payload under `budget` without ever blocking past it.
///
/// The reader thread is deliberately never joined: if it is stuck inside a
/// blocking read on a pipe the vendor keeps open, joining it would reintroduce
/// exactly the unbounded wait the budget exists to prevent. Abandoning it
/// is safe because the caller exits the process shortly afterwards.
fn read_payload(
    mut payload: impl Read + Send + 'static,
    budget: Duration,
) -> Result<Vec<u8>, PayloadError> {
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("farhelm-hook-payload".to_string())
        .spawn(move || {
            let mut buf = Vec::new();
            // One byte past the cap, so an exactly-at-cap payload is
            // accepted and anything larger is detectably oversized rather
            // than silently truncated into invalid JSON.
            let limit = MAX_PAYLOAD_BYTES as u64 + 1;
            let result = match payload.by_ref().take(limit).read_to_end(&mut buf) {
                Ok(_) if buf.len() > MAX_PAYLOAD_BYTES => Err("oversized"),
                Ok(_) => Ok(buf),
                Err(_) => Err("unreadable"),
            };
            let _ = tx.send(result);
        });
    if let Err(err) = spawned {
        return Err(PayloadError::NoReader(err));
    }
    match rx.recv_timeout(budget) {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(reason)) => Err(PayloadError::Reason(reason)),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(PayloadError::Timeout),
        // The thread dropped its sender without sending, which it only
        // does by panicking inside the read.
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(PayloadError::Reason("unreadable")),
    }
}

/// A report with no ancestry yet, in this build's file format.
fn new_report(
    vendor: ReportVendor,
    conversation: String,
    source: String,
    transcript_path: Option<serde_json::Value>,
    hook_event_name: Option<serde_json::Value>,
    agent_id: Option<serde_json::Value>,
) -> HookReport {
    HookReport {
        version: hook_report::FORMAT_VERSION,
        vendor,
        conversation,
        source,
        transcript_path,
        hook_event_name,
        agent_id,
        ancestry: None,
        ancestry_ended: None,
    }
}

/// Build a bounded report without interpreting vendor-specific evidence.
///
/// `entry_vendor` is the `--vendor` flag on this hook's own command line —
/// the vendor-specific entry point Farhelm installed — and it alone
/// decides the report's discriminator. It is never inferred from the
/// payload: the supervisor rejects a discriminator/kind mismatch before
/// any vendor I/O, so guessing here would only move the refusal later.
///
/// Only the supervisor knows the session's durable agent kind. Preserve the
/// exact transcript, event, and agent-identity fields for its checks;
/// malformed evidence must not disappear into a missing value or change
/// another vendor's parser. Unknown vendor fields remain ignored.
///
/// The payload's own `vendor` field, where the Pi/OMP assets send one, is
/// kept purely as a consistency check: present-and-mismatched rejects,
/// absent-or-agreeing passes. Agreement grants nothing — the entry point
/// is authoritative either way.
fn parse_payload(bytes: &[u8], entry_vendor: ReportVendor) -> Result<HookReport, &'static str> {
    use farhelm_supervisor::agent_kind::LocatorVendor;
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| "unparsable")?;
    if entry_vendor == ReportVendor::Grok {
        return parse_grok_payload(&value);
    }
    let session_id = match value.get("session_id") {
        None | Some(serde_json::Value::Null) => return Err("missing-session-id"),
        Some(serde_json::Value::String(id)) => id,
        Some(_) => return Err("session-id-not-a-string"),
    };
    if session_id.is_empty() {
        return Err("empty-session-id");
    }
    if session_id.len() > MAX_SESSION_ID_BYTES {
        return Err("oversized-session-id");
    }
    let source = match value.get("source") {
        Some(serde_json::Value::String(source)) => source.clone(),
        _ => String::new(),
    };
    // The entry point is authoritative; a payload `vendor` that disagrees
    // with it is a misrouted report, not a second opinion.
    let entry_name = match entry_vendor {
        ReportVendor::Claude => "claude",
        ReportVendor::Codex => "codex",
        ReportVendor::Goose => "goose",
        ReportVendor::Pi => "pi",
        ReportVendor::Omp => "omp",
        ReportVendor::Grok => unreachable!("Grok payloads use their dual-spelling parser"),
    };
    match value.get("vendor") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::String(vendor)) if vendor == entry_name => {}
        Some(serde_json::Value::String(_)) => return Err("vendor-mismatch"),
        Some(_) => return Err("vendor-not-a-string"),
    }
    // A Pi or OMP entry point encodes the durable locator under its own
    // vendor's spelling; those two locator vendors never accept each
    // other's tokens downstream, which starts here with a per-vendor
    // encode. Every other entry point forwards the id verbatim with its
    // raw evidence attached.
    if let Some(expected) = match entry_vendor {
        ReportVendor::Pi => Some(LocatorVendor::Pi),
        ReportVendor::Omp => Some(LocatorVendor::Omp),
        ReportVendor::Claude | ReportVendor::Codex | ReportVendor::Goose => None,
        ReportVendor::Grok => unreachable!("Grok payloads use their dual-spelling parser"),
    } {
        let session_file = match value.get("session_file") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(path)) => Some(path.clone()),
            Some(_) => return Err("session-file-not-a-string"),
        };
        let locator = farhelm_supervisor::agent_kind::SessionLocator {
            version: 1,
            session_id: session_id.clone(),
            session_file,
        };
        let encoded = farhelm_supervisor::agent_kind::encode_locator(expected, locator)
            .map_err(|_| "invalid-locator")?;
        return Ok(new_report(
            entry_vendor,
            encoded,
            source,
            None,
            None,
            value.get("agent_id").cloned(),
        ));
    }
    Ok(new_report(
        entry_vendor,
        session_id.clone(),
        source,
        value.get("transcript_path").cloned(),
        value.get("hook_event_name").cloned(),
        value.get("agent_id").cloned(),
    ))
}

/// Parse the three manually configured Grok callbacks into one bounded,
/// vendor-specific locator report.
///
/// Grok emits camelCase and snake_case copies of several fields. When both
/// are present, neither spelling is treated as a fallback: both must have
/// the expected type and name the same value. This matters most for the
/// selection timestamp, because accepting a half-malformed duplicate would
/// let a delayed event bypass the durable ordering fence.
fn parse_grok_payload(value: &serde_json::Value) -> Result<HookReport, &'static str> {
    match value.get("vendor") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::String(vendor)) if vendor == "grok" => {}
        Some(serde_json::Value::String(_)) => return Err("vendor-mismatch"),
        Some(_) => return Err("vendor-not-a-string"),
    }

    let aliased = |camel: &str, snake: &str| -> Result<Option<String>, &'static str> {
        let left = value.get(camel);
        let right = value.get(snake);
        if left.is_some() && right.is_some() {
            let Some(left) = left.and_then(serde_json::Value::as_str) else {
                return Err("aliased-field-not-a-string");
            };
            let Some(right) = right.and_then(serde_json::Value::as_str) else {
                return Err("aliased-field-not-a-string");
            };
            if left != right {
                return Err("conflicting-field-spellings");
            }
            return Ok(Some(left.to_string()));
        }
        match left.or(right) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(text)) => Ok(Some(text.clone())),
            Some(_) => Err("aliased-field-not-a-string"),
        }
    };

    let conversation = aliased("sessionId", "session_id")?.ok_or("missing-session-id")?;
    if conversation.is_empty() {
        return Err("empty-session-id");
    }
    if conversation.len() > MAX_SESSION_ID_BYTES {
        return Err("oversized-session-id");
    }

    let normalize_event = |event: &str| match event {
        "session_start" | "SessionStart" => Some("SessionStart"),
        "user_prompt_submit" | "UserPromptSubmit" => Some("UserPromptSubmit"),
        "stop" | "Stop" => Some("Stop"),
        _ => None,
    };
    let left_event = value.get("hookEventName");
    let right_event = value.get("hook_event_name");
    let event = if left_event.is_some() && right_event.is_some() {
        let Some(left) = left_event.and_then(serde_json::Value::as_str) else {
            return Err("hook-event-not-a-string");
        };
        let Some(right) = right_event.and_then(serde_json::Value::as_str) else {
            return Err("hook-event-not-a-string");
        };
        let left = normalize_event(left).ok_or("unsupported-hook-event")?;
        let right = normalize_event(right).ok_or("unsupported-hook-event")?;
        if left != right {
            return Err("conflicting-field-spellings");
        }
        left
    } else {
        match left_event.or(right_event) {
            None | Some(serde_json::Value::Null) => return Err("missing-hook-event"),
            Some(serde_json::Value::String(event)) => {
                normalize_event(event).ok_or("unsupported-hook-event")?
            }
            Some(_) => return Err("hook-event-not-a-string"),
        }
    };

    let source = match value.get("source") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(source)) => Some(source.clone()),
        Some(_) => return Err("source-not-a-string"),
    };
    if event == "SessionStart" && !matches!(source.as_deref(), Some("new" | "load")) {
        return Err("unsupported-session-source");
    }

    let timestamp = match value.get("timestamp") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(timestamp)) => Some(timestamp.as_str()),
        Some(_) => return Err("timestamp-not-a-string"),
    };
    if event == "SessionStart" && timestamp.is_none() {
        return Err("missing-timestamp");
    }

    let transcript_path = aliased("transcriptPath", "transcript_path")?;
    let agent_id = match value.get("subagentType") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(marker)) if marker.is_empty() => None,
        Some(serde_json::Value::String(marker)) => Some(serde_json::Value::String(marker.clone())),
        Some(_) => return Err("subagent-type-not-a-string"),
    };
    let selected_at = if event == "SessionStart" {
        timestamp
    } else {
        if let Some(timestamp) = timestamp {
            farhelm_supervisor::agent_kind::validate_grok_event_time(timestamp)
                .map_err(|_| "invalid-grok-evidence")?;
        }
        None
    };
    let conversation = farhelm_supervisor::agent_kind::encode_grok_report(
        conversation,
        transcript_path,
        selected_at,
    )
    .map_err(|_| "invalid-grok-evidence")?;

    Ok(new_report(
        ReportVendor::Grok,
        conversation,
        source.unwrap_or_default(),
        None,
        Some(serde_json::Value::String(event.to_string())),
        agent_id,
    ))
}

/// The one line a run leaves behind.
///
/// Assembled rather than formatted at each exit point so that the
/// one-line-per-run promise is structural: there is a single value
/// describing the run, and a single place that renders it.
struct Outcome {
    /// The terminal outcome word — see the module docs' table.
    word: &'static str,
    /// Outcome-specific detail, or empty.
    detail: String,
    /// `(conversation, source)`, present once the payload has parsed.
    about: Option<(String, String)>,
}

impl Outcome {
    /// An outcome with no detail, such as `written` or `no-credential`.
    fn word(word: &'static str) -> Self {
        Outcome {
            word,
            detail: String::new(),
            about: None,
        }
    }

    /// An outcome carrying free-form detail.
    fn detail(word: &'static str, detail: impl Into<String>) -> Self {
        Outcome {
            word,
            detail: detail.into(),
            about: None,
        }
    }

    /// Attach the reported identity, once the payload has yielded one.
    fn about(mut self, conversation: &str, source: &str) -> Self {
        self.about = Some((conversation.to_string(), source.to_string()));
        self
    }

    /// Render the log line, sanitizing every interpolated value.
    fn render(&self, seconds: u64) -> String {
        let about = self
            .about
            .as_ref()
            .map(|(conversation, source)| (conversation.as_str(), source.as_str()));
        hook_report::render_log_line(seconds, self.word, &self.detail, about)
    }
}

/// Append one line to the hook log, ignoring every failure; see
/// `hook_report::append_hook_log` for the rules (one write per line, no
/// locking, truncation past a size cap, private directory).
fn append_log(path: Option<&Path>, line: &str) {
    if let Some(path) = path {
        hook_report::append_hook_log(path, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::time::Instant;

    /// The Claude Code 2.1.241 `SessionStart` payload, verbatim from the
    /// hand-verified vendor audit (re-run by `real_agent_capture.rs`'s
    /// ignored hook tests).
    const CLAUDE_PAYLOAD: &str = r#"{"session_id":"6af192d4-0000-4000-8000-000000000000","transcript_path":"/home/u/.claude/projects/x/6af192d4.jsonl","cwd":"/home/u/src","hook_event_name":"SessionStart","source":"startup"}"#;

    /// The Codex CLI 0.149.1 `SessionStart` payload, verbatim from the
    /// hand-verified vendor audit (re-run by `real_agent_capture.rs`'s
    /// ignored hook tests). It carries two fields Claude's does not.
    const CODEX_PAYLOAD: &str = r#"{"session_id":"0198d3ac-0000-7000-8000-000000000000","transcript_path":"/home/u/.codex/sessions/x.jsonl","cwd":"/home/u/src","hook_event_name":"SessionStart","model":"gpt-5-codex","permission_mode":"default","source":"startup"}"#;

    /// A stdin budget short enough to keep the timeout test fast while
    /// staying far above the scheduling jitter of a loaded CI runner.
    const TEST_BUDGET: Duration = Duration::from_millis(300);
    /// A complete credential whose reports drop into `state_dir`, the way
    /// `main.rs` derives one from a session's environment.
    fn credential(state_dir: &Path) -> HookCredential {
        HookCredential {
            session_id: "sess-1".to_string(),
            state_dir: state_dir.to_path_buf(),
        }
    }

    /// Read the hook log, asserting it holds exactly one line.
    ///
    /// Every helper that checks an outcome goes through this, because
    /// "exactly one line per run" is itself part of the contract: a second
    /// line would break any reader that assumes a run is a line.
    fn single_line(path: &Path) -> String {
        let text = std::fs::read_to_string(path).expect("hook log should exist");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "expected exactly one log line, got {text:?}"
        );
        lines[0].to_string()
    }

    /// The parser must accept the real Claude payload untouched. This is
    /// the whole mechanism's entry point: if the verbatim vendor payload
    /// does not yield an id here, no resume ever gets the right
    /// conversation, and the failure is silent by design.
    #[farhelm_testtrace::test]
    fn parses_the_verbatim_claude_payload() {
        let HookReport {
            conversation: id,
            source,
            ..
        } = parse_payload(CLAUDE_PAYLOAD.as_bytes(), ReportVendor::Claude)
            .expect("claude payload parses");
        assert_eq!(id, "6af192d4-0000-4000-8000-000000000000");
        assert_eq!(source, "startup");
    }

    /// The same, for Codex — whose payload carries `model` and
    /// `permission_mode` on top of Claude's fields. Both vendors go
    /// through one parser, so this pins that the extra fields are simply
    /// ignored rather than being a second shape to maintain.
    #[farhelm_testtrace::test]
    fn parses_the_verbatim_codex_payload() {
        let HookReport {
            conversation: id,
            source,
            ..
        } = parse_payload(CODEX_PAYLOAD.as_bytes(), ReportVendor::Codex)
            .expect("codex payload parses");
        assert_eq!(id, "0198d3ac-0000-7000-8000-000000000000");
        assert_eq!(source, "startup");
    }

    /// A payload with fields no version of this code has ever seen must
    /// still parse. Vendors add hook fields without warning, and the
    /// failure mode of a strict parse would be a hook that silently stops
    /// reporting after a vendor upgrade — the exact bug this mechanism
    /// exists to avoid.
    #[farhelm_testtrace::test]
    fn ignores_unknown_payload_fields() {
        let payload = r#"{"session_id":"abc","source":"resume","future_field":{"nested":[1,2]},"another":null}"#;
        let HookReport {
            conversation: id,
            source,
            ..
        } = parse_payload(payload.as_bytes(), ReportVendor::Claude)
            .expect("unknown fields are ignored");
        assert_eq!(id, "abc");
        assert_eq!(source, "resume");
    }

    /// `source` is diagnostic-only and optional, so its absence must not
    /// cost the run its id. Codex's TUI, for one, reuses `startup` where
    /// Claude sends `clear`; nothing may key on the field's presence.
    #[farhelm_testtrace::test]
    fn missing_source_defaults_to_empty() {
        let HookReport {
            conversation: id,
            source,
            ..
        } = parse_payload(br#"{"session_id":"abc"}"#, ReportVendor::Claude)
            .expect("id alone is enough");
        assert_eq!(id, "abc");
        assert_eq!(source, "");
    }

    /// Every shape of unusable id is rejected with its own reason word.
    /// The reason is what a maintainer reads out of the hook log when a
    /// session mysteriously fails to resume, so each case earning a
    /// distinct word is the point, not an implementation detail.
    #[farhelm_testtrace::test]
    fn rejects_unusable_session_ids() {
        let oversized = format!(r#"{{"session_id":"{}"}}"#, "x".repeat(129));
        let cases: [(&[u8], &str); 6] = [
            (b"not json at all", "unparsable"),
            (
                br#"{"hook_event_name":"SessionStart"}"#,
                "missing-session-id",
            ),
            (br#"{"session_id":null}"#, "missing-session-id"),
            (br#"{"session_id":42}"#, "session-id-not-a-string"),
            (br#"{"session_id":""}"#, "empty-session-id"),
            (oversized.as_bytes(), "oversized-session-id"),
        ];
        for (payload, expected) in cases {
            let reason = parse_payload(payload, ReportVendor::Claude)
                .expect_err("payload should be rejected");
            assert_eq!(
                reason,
                expected,
                "payload {:?}",
                String::from_utf8_lossy(payload)
            );
        }
    }

    /// An id of exactly the cap is accepted: the bound is a sanity limit,
    /// not a format claim, and an off-by-one here would reject a
    /// legitimate vendor id for no reason.
    #[farhelm_testtrace::test]
    fn accepts_a_session_id_at_the_cap() {
        let payload = format!(r#"{{"session_id":"{}"}}"#, "x".repeat(MAX_SESSION_ID_BYTES));
        let HookReport {
            conversation: id, ..
        } = parse_payload(payload.as_bytes(), ReportVendor::Claude)
            .expect("an id at the cap is fine");
        assert_eq!(id.len(), MAX_SESSION_ID_BYTES);
    }

    /// The cap counts BYTES, not characters — pinned with a multibyte id
    /// because the ASCII test above cannot tell the two readings apart.
    ///
    /// Which one it is matters in both directions. Reading it as chars
    /// would let a four-byte-per-character id through at four times the
    /// intended size, and the point of the bound is to keep a bad payload
    /// from becoming a bad request. Reading it as bytes while documenting
    /// chars would reject ids a vendor is entitled to mint. `str::len` is
    /// bytes, so bytes is what the code does and what the constant's name
    /// says; this is the test that keeps the two agreeing.
    #[farhelm_testtrace::test]
    fn the_session_id_cap_counts_bytes_not_characters() {
        // Four bytes each, so 32 of them are 32 chars and exactly the cap.
        let at_cap = "😀".repeat(MAX_SESSION_ID_BYTES / 4);
        assert_eq!(at_cap.len(), MAX_SESSION_ID_BYTES, "fixture sanity");
        let payload = format!(r#"{{"session_id":"{at_cap}"}}"#);
        let HookReport {
            conversation: id, ..
        } = parse_payload(payload.as_bytes(), ReportVendor::Claude)
            .expect("128 bytes is at the cap");
        assert_eq!(id.len(), MAX_SESSION_ID_BYTES);

        // One character more is four bytes more, and therefore over.
        let over_cap = "😀".repeat(MAX_SESSION_ID_BYTES / 4 + 1);
        let payload = format!(r#"{{"session_id":"{over_cap}"}}"#);
        assert_eq!(
            parse_payload(payload.as_bytes(), ReportVendor::Claude)
                .expect_err("132 bytes is over the cap"),
            "oversized-session-id"
        );
    }

    /// The entry point's `--vendor` decides the discriminator and the
    /// locator encoding; the payload's own `vendor` is only a consistency
    /// check. Pinning them together keeps a new vendor from accidentally
    /// widening the plain-id path, from cross-reporting through another
    /// vendor's prefix, or from inferring the envelope from payload text
    /// the supervisor would then have to distrust.
    ///
    /// Why this test matters: the discriminator is what lets the
    /// supervisor reject a cross-kind report before any vendor I/O. If
    /// the hook inferred it from the payload, a credential-holding child
    /// could choose its own kind; sourcing it from the installed entry
    /// point keeps that choice with the injector.
    #[farhelm_testtrace::test]
    fn vendor_payloads_encode_locators_per_vendor_and_stay_closed() {
        let HookReport {
            conversation: id,
            source,
            vendor,
            ..
        } = parse_payload(
            br#"{"vendor":"omp","session_id":"omp-id-1",
            "session_file":"/tmp/s/conv.jsonl","source":"session_start"}"#,
            ReportVendor::Omp,
        )
        .expect("omp payload parses");
        assert!(
            id.starts_with("omp:"),
            "OMP reports under its own prefix: {id}"
        );
        assert_eq!(source, "session_start");
        assert_eq!(vendor, ReportVendor::Omp);

        let HookReport {
            conversation: id,
            vendor,
            ..
        } = parse_payload(br#"{"vendor":"pi","session_id":"pi-1"}"#, ReportVendor::Pi)
            .expect("pi payload parses");
        assert!(id.starts_with("pi:"), "Pi's spelling is unchanged: {id}");
        assert_eq!(vendor, ReportVendor::Pi);

        // The entry point is authoritative without a payload vendor: a Pi
        // entry encodes the locator even when the JSON names none.
        let HookReport {
            conversation: id,
            vendor,
            ..
        } = parse_payload(br#"{"session_id":"pi-2"}"#, ReportVendor::Pi)
            .expect("a vendorless pi payload still encodes");
        assert!(id.starts_with("pi:"), "entry decides the encoding: {id}");
        assert_eq!(vendor, ReportVendor::Pi);

        // Present-and-mismatched rejects; the payload never overrides the
        // entry point.
        assert_eq!(
            parse_payload(br#"{"vendor":"pi","session_id":"x"}"#, ReportVendor::Omp,)
                .expect_err("a payload vendor disagreeing with the entry is refused"),
            "vendor-mismatch"
        );
        assert_eq!(
            parse_payload(
                br#"{"vendor":"goose","session_id":"x"}"#,
                ReportVendor::Claude,
            )
            .expect_err("a foreign payload vendor is refused"),
            "vendor-mismatch"
        );
        assert_eq!(
            parse_payload(br#"{"vendor":7,"session_id":"x"}"#, ReportVendor::Claude)
                .expect_err("a non-string vendor is refused"),
            "vendor-not-a-string"
        );
        // A session_file that is not a string is refused rather than
        // stringified into a resume target.
        assert_eq!(
            parse_payload(
                br#"{"vendor":"omp","session_id":"x","session_file":42}"#,
                ReportVendor::Omp,
            )
            .expect_err("a non-string session file is refused"),
            "session-file-not-a-string"
        );
    }

    /// Grok emits both naming conventions in the same callback. Agreement
    /// must survive normalization into one report, including the raw child
    /// marker that the shared doorway uses to reject subagent callbacks.
    #[farhelm_testtrace::test]
    fn grok_accepts_agreeing_dual_spellings_and_normalizes_the_event() {
        let payload = br#"{
            "vendor":"grok",
            "sessionId":"grok-session-1",
            "session_id":"grok-session-1",
            "hookEventName":"session_start",
            "hook_event_name":"SessionStart",
            "transcriptPath":"/tmp/grok-session-1/updates.jsonl",
            "transcript_path":"/tmp/grok-session-1/updates.jsonl",
            "timestamp":"2026-09-22T12:00:00.123456789Z",
            "source":"new",
            "subagentType":"general-purpose"
        }"#;
        let HookReport {
            vendor,
            conversation,
            source,
            transcript_path,
            hook_event_name,
            agent_id,
            ..
        } = parse_payload(payload, ReportVendor::Grok).expect("Grok callback parses");
        assert_eq!(vendor, ReportVendor::Grok);
        assert_eq!(source, "new");
        assert_eq!(transcript_path, None, "evidence stays inside the locator");
        assert_eq!(hook_event_name, Some(serde_json::json!("SessionStart")));
        assert_eq!(agent_id, Some(serde_json::json!("general-purpose")));
        let encoded = conversation
            .strip_prefix("grok:")
            .expect("Grok reports use their reserved locator prefix");
        let locator: serde_json::Value =
            serde_json::from_str(encoded).expect("the locator is JSON");
        assert_eq!(locator["session_id"], "grok-session-1");
        assert_eq!(locator["session_file"], "/tmp/grok-session-1/updates.jsonl");
        assert!(
            locator["selected_at"].is_object(),
            "SessionStart carries its durable ordering key"
        );
    }

    /// A duplicate spelling is corroborating evidence, never a fallback.
    /// Wrong types, disagreement, and malformed selection metadata must be
    /// refused before the supervisor can perform vendor file or process I/O.
    #[farhelm_testtrace::test]
    fn grok_rejects_conflicting_or_malformed_callback_fields() {
        let cases = [
            (
                r#"{"sessionId":"a","session_id":"b","hookEventName":"SessionStart","timestamp":"2026-09-22T12:00:00Z","source":"new"}"#,
                "conflicting-field-spellings",
            ),
            (
                r#"{"sessionId":"a","hookEventName":"SessionStart","hook_event_name":"Stop","timestamp":"2026-09-22T12:00:00Z","source":"new"}"#,
                "conflicting-field-spellings",
            ),
            (
                r#"{"sessionId":"a","hookEventName":"SessionStart","transcriptPath":7,"timestamp":"2026-09-22T12:00:00Z","source":"new"}"#,
                "aliased-field-not-a-string",
            ),
            (
                r#"{"sessionId":"a","hookEventName":"SessionStart","timestamp":7,"source":"new"}"#,
                "timestamp-not-a-string",
            ),
            (
                r#"{"sessionId":"a","hookEventName":"SessionStart","timestamp":"not-a-time","source":"new"}"#,
                "invalid-grok-evidence",
            ),
            (
                r#"{"sessionId":"a","hookEventName":"SessionStart","timestamp":"2026-09-22T12:00:00Z","source":"resume"}"#,
                "unsupported-session-source",
            ),
            (
                r#"{"sessionId":"a","hookEventName":"Stop","subagentType":7}"#,
                "subagent-type-not-a-string",
            ),
        ];
        for (payload, expected) in cases {
            assert_eq!(
                parse_payload(payload.as_bytes(), ReportVendor::Grok)
                    .expect_err("the malformed Grok callback must be refused"),
                expected,
                "payload: {payload}"
            );
        }
    }

    /// Selection and enrichment carry different evidence: SessionStart must
    /// establish the ordering fence, while later lifecycle callbacks may be
    /// path-only and leave the stored timestamp to the supervisor.
    #[farhelm_testtrace::test]
    fn grok_requires_ordering_only_for_session_start() {
        assert_eq!(
            parse_payload(
                br#"{"sessionId":"a","hookEventName":"SessionStart","source":"new"}"#,
                ReportVendor::Grok,
            )
            .expect_err("SessionStart without a timestamp cannot select"),
            "missing-timestamp"
        );

        let HookReport {
            source,
            hook_event_name,
            conversation,
            ..
        } = parse_payload(
            br#"{"sessionId":"a","hookEventName":"user_prompt_submit","transcriptPath":"/tmp/a/updates.jsonl"}"#,
            ReportVendor::Grok,
        )
        .expect("enrichment needs neither source nor timestamp");
        assert_eq!(source, "");
        assert_eq!(hook_event_name, Some(serde_json::json!("UserPromptSubmit")));
        let locator: serde_json::Value = serde_json::from_str(
            conversation
                .strip_prefix("grok:")
                .expect("Grok reports use their locator prefix"),
        )
        .expect("the locator is JSON");
        assert_eq!(locator["selected_at"], serde_json::Value::Null);

        let HookReport { conversation, .. } = parse_payload(
            br#"{"sessionId":"a","hookEventName":"stop","timestamp":"2026-09-22T12:00:00.123456789+00:00","transcriptPath":"/tmp/a/updates.jsonl"}"#,
            ReportVendor::Grok,
        )
        .expect("enrichment timestamps are validated but not persisted");
        let locator: serde_json::Value = serde_json::from_str(
            conversation
                .strip_prefix("grok:")
                .expect("Grok reports use their locator prefix"),
        )
        .expect("the locator is JSON");
        assert_eq!(locator["selected_at"], serde_json::Value::Null);
        assert_eq!(
            parse_payload(
                br#"{"sessionId":"a","hookEventName":"stop","timestamp":"not-a-time"}"#,
                ReportVendor::Grok,
            )
            .expect_err("a malformed repeated timestamp is still refused"),
            "invalid-grok-evidence"
        );
    }

    /// Raw subagent evidence crosses into the report verbatim: the hook never
    /// interprets it, so a typed marker survives for the supervisor to
    /// reject before sanitation, and absence stays distinguishable from a
    /// malformed value.
    ///
    /// Why this test matters: `agent_id` is the signal that separates a
    /// foreground `SessionStart` from a delegated subagent event. If the
    /// hook dropped it (as it once did) or coerced a wrong-typed value
    /// to absent, the doorway could not tell the two apart.
    #[farhelm_testtrace::test]
    fn agent_identity_crosses_verbatim_for_the_doorway() {
        let HookReport { agent_id, .. } = parse_payload(
            br#"{"session_id":"abc","agent_id":"sub-1"}"#,
            ReportVendor::Claude,
        )
        .expect("an agent identity parses");
        assert_eq!(
            agent_id,
            Some(serde_json::Value::String("sub-1".to_string()))
        );

        let HookReport { agent_id, .. } =
            parse_payload(br#"{"session_id":"abc"}"#, ReportVendor::Claude)
                .expect("absence parses");
        assert_eq!(agent_id, None);

        let HookReport { agent_id, .. } = parse_payload(
            br#"{"session_id":"abc","agent_id":7}"#,
            ReportVendor::Claude,
        )
        .expect("a wrong-typed identity still parses here");
        assert_eq!(agent_id, Some(serde_json::Value::from(7)));
    }

    /// Without a credential the run must stop before writing a report, and
    /// say so. This is the "agent launched outside farhelm" case: it is
    /// expected, not an error, and the log line is the only way to tell it
    /// apart from a hook that never ran at all.
    #[farhelm_testtrace::test]
    fn no_credential_stops_before_writing_a_report() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        let started = Instant::now();
        run_with(
            None,
            Cursor::new(CLAUDE_PAYLOAD.to_string().into_bytes()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        assert!(
            started.elapsed() < TEST_BUDGET,
            "should not consume the budget"
        );
        let line = single_line(&log);
        assert!(line.ends_with(" no-credential"), "line was {line:?}");
    }

    /// A reader that fails the test if it is read from at all.
    ///
    /// Exists for the one claim a `Cursor` cannot make: that the
    /// no-credential path never TOUCHES stdin. The distinction is not
    /// academic — this hook is a child of the agent, holding the agent's
    /// own pipe, and draining a payload it will never send is work done on
    /// a descriptor that belongs to someone else.
    struct PanicOnRead;

    impl Read for PanicOnRead {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            panic!("stdin must not be read when there is no credential");
        }
    }

    /// Without a credential the payload is never read.
    ///
    /// [`run_inner`]'s phase order says the credential check comes before
    /// the stdin read, and the reason it can afford to is that a
    /// `SessionStart` payload is far smaller than a pipe buffer, so
    /// declining to drain it cannot block the agent. A future refactor
    /// that read the payload first — to "log what was reported" — would
    /// spend the budget on a read whose result is thrown away, and this is
    /// the test that would notice. The `no-credential` line proves it:
    /// reading would have produced `bad-payload unreadable` instead, since
    /// the panicking thread drops its sender.
    #[farhelm_testtrace::test]
    fn no_credential_never_touches_stdin() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        run_with(
            None,
            PanicOnRead,
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        let line = single_line(&log);
        assert!(line.ends_with(" no-credential"), "line was {line:?}");
    }

    /// Every payload rejection reaches the log with its own reason word,
    /// and none of them writes a report.
    ///
    /// [`parse_payload`]'s own test pins the reasons in isolation; this
    /// one pins that they survive the whole of [`run_with`] — the layer a
    /// human actually reads — and that the run STOPS there. Reporting an id
    /// we could not parse is the bug this guards against; the supervisor
    /// would have to refuse it, and the refusal would look like a
    /// supervisor problem.
    #[farhelm_testtrace::test]
    fn a_bad_payload_is_logged_without_writing_a_report() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cases: [(&[u8], &str); 4] = [
            (b"not json at all", "unparsable"),
            (
                br#"{"hook_event_name":"SessionStart"}"#,
                "missing-session-id",
            ),
            (br#"{"session_id":42}"#, "session-id-not-a-string"),
            (br#"{"session_id":""}"#, "empty-session-id"),
        ];
        for (index, (payload, reason)) in cases.into_iter().enumerate() {
            let log = dir.path().join("hook-log").join(format!("{index}.log"));
            run_with(
                Some(credential(dir.path())),
                Cursor::new(payload.to_vec()),
                TEST_BUDGET,
                Some(log.clone()),
                ReportVendor::Claude,
            );
            let line = single_line(&log);
            assert!(
                line.ends_with(&format!(" bad-payload {reason}")),
                "payload {:?} logged {line:?}",
                String::from_utf8_lossy(payload)
            );
        }
        assert!(
            !dir.path().join(hook_report::REPORTS_DIR).exists(),
            "no report was written for any rejected payload"
        );
    }

    /// A Goose report with no `AGENT_SESSION_ID` still leaves its one hook
    /// log line, naming the missing id.
    ///
    /// Why: the Goose adapter used to skip the reporter entirely when the
    /// variable was missing or not UTF-8, so the session silently never
    /// became resumable and the hook log, the one place a reporter failure
    /// is supposed to show, stayed empty. Spec: the adapter's payload for a
    /// missing id is logged as `bad-payload missing-session-id` without
    /// writing a report.
    #[farhelm_testtrace::test]
    fn a_goose_report_without_a_session_id_is_logged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("goose.log");
        let payload = serde_json::to_vec(&crate::goose_hook::report_payload(None)).unwrap();
        run_with(
            Some(credential(dir.path())),
            Cursor::new(payload),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Goose,
        );
        let line = single_line(&log);
        assert!(
            line.ends_with(" bad-payload missing-session-id"),
            "logged {line:?}"
        );
    }

    /// A `source` the vendor sent as something other than a string is
    /// treated exactly like an absent one, and renders as `-`.
    ///
    /// The field is diagnostic-only: nothing keys behavior on it, so
    /// failing a report over its TYPE would trade a working resume for a
    /// tidier log. Both shapes a vendor could plausibly produce by
    /// accident are pinned, and the conversation id still rides along —
    /// which is the half that matters.
    #[farhelm_testtrace::test]
    fn a_non_string_source_renders_as_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        for (index, payload) in [
            r#"{"session_id":"conv-1","source":null}"#,
            r#"{"session_id":"conv-1","source":7}"#,
        ]
        .into_iter()
        .enumerate()
        {
            let log = dir.path().join("hook-log").join(format!("{index}.log"));
            run_with(
                Some(credential(dir.path())),
                Cursor::new(payload.to_string().into_bytes()),
                TEST_BUDGET,
                Some(log.clone()),
                ReportVendor::Claude,
            );
            let line = single_line(&log);
            assert!(
                line.ends_with(" conv-1 -"),
                "payload {payload:?} logged {line:?}"
            );
        }
    }

    /// Two runs against one log leave two intact lines, in the order they
    /// ran.
    ///
    /// A session fires this hook once per conversation, so a log holding
    /// several lines is the normal case, not an edge one. What this pins
    /// is that appending is really appending: no truncation below the cap
    /// (which would silently discard the history a reader came for), and
    /// no partial line (each run writes its line in one `write_all`, which
    /// is what keeps concurrent hooks from interleaving mid-line). The two
    /// runs are given DIFFERENT outcomes so the assertion can tell which
    /// line landed first.
    #[farhelm_testtrace::test]
    fn consecutive_runs_append_whole_lines_in_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");

        run_with(
            None,
            Cursor::new(Vec::new()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        run_with(
            Some(credential(dir.path())),
            Cursor::new(b"not json at all".to_vec()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );

        let text = std::fs::read_to_string(&log).expect("hook log should exist");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "expected two lines, got {text:?}");
        assert!(lines[0].ends_with(" no-credential"), "first: {text:?}");
        assert!(
            lines[1].ends_with(" bad-payload unparsable"),
            "second: {text:?}"
        );
    }

    /// Spec: a run with a credential and a good payload writes the report
    /// into the session's drop directory, with the hook's own process
    /// ancestry starting at the hook itself, and logs `written` with the
    /// reported identity.
    ///
    /// Why: this is the whole delivery path now. The supervisor attributes
    /// the report from the recorded ancestry long after this process has
    /// exited, so the chain must be recorded at report time and must start
    /// at the reporter; and the log line is the only trace a human sees.
    #[farhelm_testtrace::test]
    fn a_good_payload_is_written_with_its_ancestry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        run_with(
            Some(credential(dir.path())),
            Cursor::new(CLAUDE_PAYLOAD.as_bytes().to_vec()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        let line = single_line(&log);
        assert!(
            line.ends_with(" written 6af192d4-0000-4000-8000-000000000000 startup"),
            "line was {line:?}"
        );
        let slot = hook_report::session_dir(dir.path(), "sess-1")
            .expect("valid id")
            .join(hook_report::Slot::Latest.file_name());
        let written: HookReport =
            serde_json::from_slice(&std::fs::read(&slot).expect("the report was written"))
                .expect("the report parses");
        assert_eq!(written.vendor, ReportVendor::Claude);
        assert_eq!(written.conversation, "6af192d4-0000-4000-8000-000000000000");
        let ancestry = written.ancestry.expect("the ancestry was recorded");
        assert_eq!(
            ancestry.first().map(|link| link.pid),
            Some(std::process::id()),
            "the recorded chain starts at the reporting process"
        );
    }

    /// Spec: a payload naming a sub-agent is logged as `bad-payload
    /// subagent-report` and writes nothing, and a marker of the wrong type
    /// is refused the same way rather than read as absent.
    ///
    /// Why: the supervisor refuses such a report anyway, but it would only
    /// get to refuse it after the report had replaced whatever the session's
    /// own agent left in the same slot. While the supervisor is down, that
    /// could be the only record of a conversation switch, lost to a report
    /// that was never going to count.
    #[farhelm_testtrace::test]
    fn a_subagent_report_is_not_written() {
        let dir = tempfile::tempdir().expect("tempdir");
        for (index, (payload, reason)) in [
            (
                r#"{"session_id":"conv-sub","agent_id":"sub-1"}"#,
                "subagent-report",
            ),
            (
                r#"{"session_id":"conv-sub","agent_id":7}"#,
                "agent-id-not-a-string",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let log = dir.path().join("hook-log").join(format!("{index}.log"));
            run_with(
                Some(credential(dir.path())),
                Cursor::new(payload.as_bytes().to_vec()),
                TEST_BUDGET,
                Some(log.clone()),
                ReportVendor::Claude,
            );
            let line = single_line(&log);
            assert!(
                line.contains(&format!(" bad-payload {reason} conv-sub ")),
                "line was {line:?}"
            );
        }
        assert!(
            !dir.path().join(hook_report::REPORTS_DIR).exists(),
            "no report was written for a sub-agent"
        );
    }

    /// Spec: a report that cannot be written is logged as `write-failed`
    /// with the error and the identity, and the run still returns normally.
    ///
    /// Why: a state directory that does not exist (no supervisor ever ran
    /// here) or cannot be written must not become a visible hook error; the
    /// log line is the only place the lost report shows.
    #[farhelm_testtrace::test]
    fn an_unwritable_report_is_logged_as_write_failed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        run_with(
            Some(credential(&dir.path().join("no-such-state"))),
            Cursor::new(CLAUDE_PAYLOAD.as_bytes().to_vec()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        let line = single_line(&log);
        assert!(line.contains(" write-failed "), "line was {line:?}");
        assert!(
            line.ends_with(" 6af192d4-0000-4000-8000-000000000000 startup"),
            "line was {line:?}"
        );
    }

    /// Spec: Grok's `SessionStart` and its later events land in separate
    /// slots, so a run of enrichments never replaces the selection.
    ///
    /// Why: Grok's enrichment is refused unless its selection is applied
    /// first; while the supervisor is down, every prompt fires another
    /// enrichment, and a single slot would lose the selection to them.
    #[farhelm_testtrace::test]
    fn grok_selection_survives_later_enrichments() {
        let dir = tempfile::tempdir().expect("tempdir");
        let selection = br#"{"sessionId":"019a0000-0000-7000-8000-000000000001","hookEventName":"SessionStart","source":"new","timestamp":"2026-10-01T00:00:00Z"}"#;
        let enrichment =
            br#"{"sessionId":"019a0000-0000-7000-8000-000000000001","hookEventName":"Stop"}"#;
        for payload in [&selection[..], &enrichment[..], &enrichment[..]] {
            run_with(
                Some(credential(dir.path())),
                Cursor::new(payload.to_vec()),
                TEST_BUDGET,
                None,
                ReportVendor::Grok,
            );
        }
        let session = hook_report::session_dir(dir.path(), "sess-1").expect("valid id");
        let mut names: Vec<String> = std::fs::read_dir(&session)
            .expect("drop dir")
            .map(|entry| entry.expect("entry").file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["enrichment.json", "selection.json"]);
    }

    /// A payload reader that never reaches EOF must cost the budget and no
    /// more. This is the vendor-holds-the-pipe case that motivates the
    /// detached reader thread: a blocking read cannot be cancelled, so the
    /// only proof the design works is that the call still returns.
    #[farhelm_testtrace::test]
    fn a_blocking_payload_reader_gives_up_at_the_budget() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        // A connected pair whose other end the test keeps alive: the read
        // half blocks forever, exactly like a vendor holding our stdin.
        let (read_half, _write_half) = std::os::unix::net::UnixStream::pair().expect("socketpair");
        let started = Instant::now();
        run_with(
            Some(credential(dir.path())),
            read_half,
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        assert!(
            started.elapsed() < TEST_BUDGET + Duration::from_millis(500),
            "run took {:?}",
            started.elapsed()
        );
        assert!(
            single_line(&log).contains(" timeout stdin"),
            "{:?}",
            single_line(&log)
        );
    }

    /// The log directory is created on demand and kept private.
    ///
    /// There is ONE `hook-log/` directory under the supervisor's state
    /// directory, shared by every session, holding one FILE per session —
    /// not a directory per session. Whichever hook runs first anywhere on
    /// that supervisor creates it; every later one finds it. Its mode is
    /// what this pins: the files inside carry conversation ids and vendor
    /// error text, and they live beside the launch specs, so 0700 is the
    /// same boundary the rest of the state directory already keeps.
    #[farhelm_testtrace::test]
    fn a_missing_log_directory_is_created_private() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        run_with(
            None,
            Cursor::new(Vec::new()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );

        let mode = std::fs::metadata(log.parent().expect("parent"))
            .expect("the directory was created")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "mode was {:o}", mode & 0o777);
        assert!(single_line(&log).contains(" no-credential"));
    }

    /// A log path that cannot possibly be written — here, one nested under
    /// a regular file — must not turn a working hook into a failing one.
    /// Logging is a diagnostic, and a diagnostic that can break the thing
    /// it observes is worse than none.
    #[farhelm_testtrace::test]
    fn an_unwritable_log_path_is_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let blocker = dir.path().join("not-a-directory");
        std::fs::write(&blocker, b"regular file").expect("write blocker");
        let log = blocker.join("hook-log").join("s.log");

        run_with(
            None,
            Cursor::new(Vec::new()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );

        assert!(!log.exists(), "nothing should have been created");
    }

    /// A log grown past its cap is truncated before the next append, so a
    /// long-lived session cannot fill the state directory. Truncation is
    /// the deliberate choice over rotation (see `hook_report::MAX_LOG_BYTES`); this
    /// test is what would catch a future "improvement" that silently
    /// removed the bound.
    #[farhelm_testtrace::test]
    fn an_oversized_log_is_truncated_before_appending() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log_dir = dir.path().join("hook-log");
        std::fs::create_dir_all(&log_dir).expect("create log dir");
        let log = log_dir.join("s.log");
        std::fs::write(
            &log,
            vec![b'x'; (hook_report::MAX_LOG_BYTES + 1024) as usize],
        )
        .expect("seed a big log");

        run_with(
            None,
            Cursor::new(Vec::new()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );

        let line = single_line(&log);
        let len = std::fs::metadata(&log).expect("metadata").len();
        assert_eq!(
            len,
            line.len() as u64 + 1,
            "the file should hold only the new line"
        );
        // Truncation, not rotation and not a seek: nothing the file held
        // before survives. The seeded bytes are a character no rendered
        // line ever contains, so their absence is the whole claim.
        let text = std::fs::read_to_string(&log).expect("read the truncated log");
        assert!(
            !text.contains('x'),
            "no seeded byte may survive truncation, got {text:?}"
        );
    }

    /// Nothing an agent puts in a payload may forge a second log line.
    /// The reporting process is the agent's own, so the id and source are
    /// attacker-controlled in the only threat model that matters here; a
    /// newline in either would let it write whatever it liked into the
    /// operator's diagnostic file.
    #[farhelm_testtrace::test]
    fn payload_values_cannot_forge_a_log_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        let payload = r#"{"session_id":"a\nb 1724470000 acked","source":"c\td"}"#;
        run_with(
            Some(credential(dir.path())),
            Cursor::new(payload.to_string().into_bytes()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        let line = single_line(&log);
        assert!(
            line.ends_with(" a_b_1724470000_acked c_d"),
            "line was {line:?}"
        );
    }

    /// The same forgery, spelled with characters `char::is_control` says
    /// nothing about.
    ///
    /// U+2028 is a line separator plenty of viewers break on, and the bidi
    /// overrides reorder what a human SEES without changing a byte — so a
    /// `write-failed` line could be made to read as a `written` one to
    /// the only audience this file has. Both are written into the payload
    /// as raw characters, exactly as a hostile agent would send them, and
    /// both must come back as `_`. This is a regression case: the
    /// original sanitizer checked `is_control` alone and let every one of
    /// them through.
    #[farhelm_testtrace::test]
    fn payload_values_cannot_smuggle_line_or_direction_controls() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        let payload = format!(
            r#"{{"session_id":"a{}b{}c","source":"d{}e"}}"#,
            '\u{2028}', '\u{202e}', '\u{2066}'
        );
        run_with(
            Some(credential(dir.path())),
            Cursor::new(payload.into_bytes()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        let line = single_line(&log);
        assert!(line.ends_with(" a_b_c d_e"), "line was {line:?}");
    }

    /// A payload larger than the cap is refused rather than truncated into
    /// something that might still parse. Truncated JSON would usually be
    /// unparsable, but "usually" is not a contract, and reporting a
    /// half-read id would be worse than reporting none.
    #[farhelm_testtrace::test]
    fn an_oversized_payload_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        let payload = format!(
            r#"{{"session_id":"abc","padding":"{}"}}"#,
            "p".repeat(MAX_PAYLOAD_BYTES)
        );
        run_with(
            Some(credential(dir.path())),
            Cursor::new(payload.into_bytes()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        assert!(single_line(&log).contains(" bad-payload oversized"));
    }

    /// [`announce`] writes the pointer and exactly one newline.
    ///
    /// "Exactly one line" is the contract the vendors' context injection
    /// is judged against: a second line is a second thing the model reads
    /// at the top of every session, and a missing newline runs the pointer
    /// into whatever the vendor appends after it.
    #[farhelm_testtrace::test]
    fn announce_writes_one_line_and_nothing_else() {
        let mut out = Vec::new();
        announce(&mut out);
        let text = String::from_utf8(out).expect("the pointer is ASCII");
        assert_eq!(text, format!("{POINTER_LINE}\n"));
        assert_eq!(text.lines().count(), 1);
    }

    /// [`announce`] survives a sink that always fails to write.
    ///
    /// [`announce_writes_one_line_and_nothing_else`] above writes to a
    /// `Vec<u8>`, which can never fail, so it never actually exercises the
    /// "ignore write failures" half of [`announce`]'s own contract — the
    /// real motivating case being a hook whose stdout is a closed pipe.
    /// This sink fails every call, so reaching the end of this test at all
    /// (rather than unwinding through `announce`'s `?`-free `let _ =`) is
    /// the assertion.
    #[farhelm_testtrace::test]
    fn announce_survives_a_sink_that_always_fails() {
        struct AlwaysErrors;
        impl std::io::Write for AlwaysErrors {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("simulated closed pipe"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::other("simulated closed pipe"))
            }
        }
        announce(&mut AlwaysErrors);
    }

    /// The pointer's SHAPE, which is what decides whether a vendor treats
    /// it as context or as a failed JSON parse.
    ///
    /// Both vendors route a `SessionStart` hook's stdout by shape: text
    /// starting with `{` and ending with `}` is parsed as JSON, and Codex
    /// fails the whole hook run when that parse does not succeed (see
    /// [`POINTER_LINE`] for the citations). So a well-meaning edit that
    /// wrapped the pointer in braces would turn a helpful line into a
    /// vendor-visible hook failure — silently, since nothing in farhelm
    /// would notice. The rest is budget: a pointer every session pays for
    /// has to stay small, and non-ASCII buys nothing when the reader is a
    /// tokenizer.
    #[farhelm_testtrace::test]
    fn the_pointer_line_is_plain_single_line_ascii() {
        assert!(
            !POINTER_LINE.starts_with('{'),
            "a pointer starting with a brace is read as JSON, not as context"
        );
        assert!(!POINTER_LINE.contains('\n'));
        assert!(POINTER_LINE.is_ascii());
        assert!(
            POINTER_LINE.len() < 160,
            "the pointer grew to {} bytes",
            POINTER_LINE.len()
        );
        // The two things the line has to convey: the trigger the user will
        // type, and the command that explains it.
        assert!(POINTER_LINE.contains("$farhelm"));
        assert!(POINTER_LINE.contains("farhelm agent instructions"));
    }

    /// Log lines are rendered from one value, so this pins the exact
    /// grammar a docs page and any future reader will quote. It is the
    /// only test that asserts the format character for character.
    #[farhelm_testtrace::test]
    fn renders_the_documented_line_shape() {
        assert_eq!(
            Outcome::word("no-credential").render(17),
            "17 no-credential"
        );
        assert_eq!(
            Outcome::detail("timeout", "stdin").render(17),
            "17 timeout stdin"
        );
        assert_eq!(
            Outcome::word("acked").about("conv-1", "startup").render(17),
            "17 acked conv-1 startup"
        );
        assert_eq!(
            Outcome::word("acked").about("conv-1", "").render(17),
            "17 acked conv-1 -"
        );
        assert_eq!(
            Outcome::detail("refused", "invalid_request bad id")
                .about("conv-1", "startup")
                .render(17),
            "17 refused invalid_request bad id conv-1 startup"
        );
        // A newline anywhere is what would forge a second line; spaces in
        // free-form detail survive, spaces in the identity fields do not.
        assert_eq!(
            Outcome::detail("refused", "line\none")
                .about("a\nb", "c d")
                .render(17),
            "17 refused line_one a_b c_d"
        );
    }
}
