//! `farhelm internal hook`: a bounded vendor callback that reports the
//! conversation identity the agent is currently using.
//!
//! Most adapters report a session-start event. Grok uses three manually
//! configured callbacks (`SessionStart`, `UserPromptSubmit`, and `Stop`) so
//! the first event can select a UUID and later events can supply its exact
//! record path. In every case this module runs as a short-lived child below
//! the agent process, inside the agent's terminal, with the session credential
//! already in its environment. It reads one JSON payload from stdin and sends
//! one `ControlMsg::ReportConversation` over the supervisor socket. Everything
//! else about it is a consequence of *where* it runs.
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
//! 3. **One budget for the whole run, stdin included.** Overrunning the
//!    vendor's own hook timeout is exactly the failure that shows up in
//!    the agent's UI, so the budget covers reading the payload as well as
//!    the socket round trip. See [`run_with`] for why that forces a
//!    detached reader thread rather than any async-stdin design.
//! 4. **No credential, no IDENTITY work.** Without the three injected
//!    environment values there is no supervisor to talk to (someone ran
//!    the agent outside Farhelm with its callback still configured); the run
//!    logs `no-credential` and stops without touching a socket. This rule
//!    is scoped to identity capture only: [`announce`] needs no credential
//!    at all and still prints [`POINTER_LINE`] whenever `--announce` was
//!    passed, credential or not — the pointer is a fact about the launch,
//!    not about whether the supervisor is reachable.
//! 5. **Nothing about the payload is trusted.** Unknown fields are
//!    ignored, and the reported id is an opaque string this side merely
//!    length-checks — the supervisor owns plausibility (see
//!    `ControlMsg::ReportConversation`'s "Trust boundary").
//!
//! ## The hook log
//!
//! Because nothing may be printed, the per-session log file is the only
//! place a failure is ever visible. Every run appends **exactly one line**
//! and then stops; a run never writes two lines, so counting lines counts
//! runs. The file lives at `<state_dir>/hook-log/<session-id>.log`, where
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
//! There is deliberately no separate "reported" line ahead of the outcome:
//! one line per run is the whole promise, so the outcome word is always the
//! *terminal* outcome, and the identity rides along in the detail.
//!
//! Outcome words, and the detail each carries:
//!
//! | Outcome | Detail |
//! | --- | --- |
//! | `acked` | — (the supervisor replied `ConversationReported`) |
//! | `refused` | `<error kind> <message>`, or `unexpected <message type>` |
//! | `no-credential` | — |
//! | `bad-payload` | a one-word reason (`unparsable`, `missing-session-id`, …), or `no-reader: <io error>` |
//! | `connect-failed` | `<phase>: <error>` |
//! | `timeout` | the phase the budget expired in |
//! | `panic` | — |
//!
//! Phases are `stdin`, `connect`, `handshake`, `send`, `reply`.
//!
//! Two details do not name a phase in that vocabulary, because the failure
//! is on this side rather than on the wire: `bad-payload no-reader: <io
//! error>` is a reader thread that could not be started at all, and
//! `connect-failed runtime: <error>` is a tokio runtime that could not be
//! built. Both are process-level resource failures, filed under the
//! outcome whose observable effect they share.
//!
//! ```text
//! 1724470000 acked conv-1 startup
//! 1724470000 refused invalid_request implausible conversation id conv-1 startup
//! 1724470000 timeout stdin
//! 1724470000 connect-failed connect: No such file or directory (os error 2) conv-1 -
//! ```
//!
//! Every value interpolated into a line is sanitized and length-capped
//! before it is written: control characters and the Unicode
//! direction-and-line controls always become `_`, and the two
//! trailing identity fields additionally lose their spaces so they stay
//! single positional tokens. Free-form detail keeps its spaces, since a
//! supervisor's refusal message is a sentence. The conversation id and
//! source come from the agent — the same process that could otherwise
//! embed a newline and forge a log line — and a log nobody can trust to be
//! one-line-per-run is worse than no log.
//!
//! Every failure of logging itself — a missing directory that cannot be
//! created, an unwritable path, a full disk — is ignored. The log exists
//! to explain a broken run, never to become one.

use farhelm_proto::io::{FrameReader, FrameWriter, handshake_with_session_auth, parse_control};
use farhelm_proto::{ControlMsg, SessionAuth};
use std::io::Read;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Largest stdin payload accepted. A hook payload is normally a few hundred
/// bytes of JSON; the cap exists so a vendor (or anything else
/// holding our stdin) cannot make the hook allocate without bound while
/// the budget runs down.
const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

/// Longest `session_id` this side will forward. Purely a sanity bound —
/// the supervisor makes the real plausibility judgement — but forwarding
/// a megabyte "id" would only convert a bad payload into a bad request.
const MAX_SESSION_ID_BYTES: usize = 128;

/// Size past which the hook log is truncated before the next append.
///
/// Truncation rather than rotation is deliberate: this file is a
/// last-resort diagnostic for a session someone is actively looking at,
/// and losing an old line is cheaper than owning a rotation scheme (and
/// its own failure modes) for a file nothing else reads.
const MAX_LOG_BYTES: u64 = 64 * 1024;

/// The correlation id every hook request uses. One request per process,
/// so there is nothing to correlate against.
const REQUEST_ID: u64 = 1;

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

/// The injected session credential, already extracted from the
/// environment by the caller.
///
/// A struct rather than three arguments read from the environment inside
/// [`run_with`] because this repo's tests never mutate the process
/// environment — and, more sharply, because a test process running inside
/// a real farhelm session already carries those variables and would
/// otherwise pick up a live supervisor. The environment read stays in the
/// `main.rs` arm; everything testable takes the credential as a value.
pub struct HookCredential {
    /// The farhelm session this hook is reporting for — the identity the
    /// supervisor authenticates, not the vendor's conversation id.
    pub session_id: String,
    /// The unguessable bearer minted for that session.
    pub token: String,
    /// The supervisor's unix socket path.
    pub socket: PathBuf,
}

/// Run one hook report to completion, or to the end of `budget`, whichever
/// comes first, and record the outcome in `hook_log`.
///
/// Never returns an error and never panics out: the caller's only job
/// after this returns is to exit 0. `payload` is taken by value because it
/// is moved onto a reader thread that outlives this call.
///
/// ## Why the payload is read on a detached thread
///
/// `budget` covers reading stdin, and two otherwise-obvious designs cannot
/// honour that:
///
/// - A blocking `std::io::Read` cannot be interrupted by a tokio timeout
///   at all. Wrapping the read in `tokio::time::timeout` bounds nothing;
///   the runtime simply never gets the thread back.
/// - `tokio::io::stdin()` is a `spawn_blocking` read, and dropping the
///   runtime waits for blocking tasks to finish. The timeout would fire,
///   and then the runtime's `Drop` would block for as long as the vendor
///   holds the pipe open — moving the overrun from the read to the
///   teardown without removing it.
///
/// So the read happens on a plain `std::thread` that is spawned and never
/// joined, handing bytes back over a channel; the main thread waits with
/// `recv_timeout`. A stuck reader thread is simply abandoned. The socket
/// round trip then runs under `tokio::time::timeout` on a current-thread
/// runtime built and dropped inside this function — with no
/// `spawn_blocking` and no spawned tasks anywhere in it, so that drop
/// cannot block either.
///
/// The remaining hazard is a destructor outliving the budget after this
/// returns, which is why the caller exits the process rather than
/// returning up through `main`.
pub fn run_with(
    credential: Option<HookCredential>,
    payload: impl Read + Send + 'static,
    budget: Duration,
    hook_log: Option<PathBuf>,
    entry_vendor: farhelm_proto::ReportVendor,
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
/// stdin is even read, because there is nowhere to send a payload and the
/// vendor's `SessionStart` payload is far smaller than a pipe buffer, so
/// declining to drain it cannot block the agent.
fn run_inner(
    credential: Option<HookCredential>,
    payload: impl Read + Send + 'static,
    budget: Duration,
    entry_vendor: farhelm_proto::ReportVendor,
) -> Outcome {
    let deadline = Instant::now() + budget;
    let Some(credential) = credential else {
        return Outcome::word("no-credential");
    };

    // `remaining`, not `budget`: every phase from here on spends against
    // the one deadline, so the stdin read cannot quietly get a fresh
    // allowance of its own.
    let bytes = match read_payload(payload, remaining(deadline)) {
        Ok(bytes) => bytes,
        Err(PayloadError::Timeout) => return Outcome::detail("timeout", "stdin"),
        Err(PayloadError::Reason(reason)) => return Outcome::detail("bad-payload", reason),
        Err(PayloadError::NoReader(err)) => {
            return Outcome::detail("bad-payload", format!("no-reader: {err}"));
        }
    };
    let request = match parse_payload(&bytes, entry_vendor) {
        Ok(parsed) => parsed,
        Err(reason) => return Outcome::detail("bad-payload", reason),
    };
    let ControlMsg::ReportConversation {
        conversation,
        source,
        ..
    } = &request
    else {
        unreachable!("the payload parser constructs only conversation reports");
    };

    // Every failure from here on has an id to name, so the outcome carries
    // the identity pair even when the report never landed.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            return Outcome::detail("connect-failed", format!("runtime: {err}"))
                .about(conversation, source);
        }
    };
    runtime
        .block_on(report(credential, &request, deadline))
        .about(conversation, source)
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
fn parse_payload(
    bytes: &[u8],
    entry_vendor: farhelm_proto::ReportVendor,
) -> Result<ControlMsg, &'static str> {
    use farhelm_proto::ReportVendor;
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
        return Ok(ControlMsg::ReportConversation {
            req_id: REQUEST_ID,
            vendor: entry_vendor,
            conversation: encoded,
            source,
            transcript_path: None,
            hook_event_name: None,
            agent_id: value.get("agent_id").cloned(),
        });
    }
    Ok(ControlMsg::ReportConversation {
        req_id: REQUEST_ID,
        vendor: entry_vendor,
        conversation: session_id.clone(),
        source,
        transcript_path: value.get("transcript_path").cloned(),
        hook_event_name: value.get("hook_event_name").cloned(),
        agent_id: value.get("agent_id").cloned(),
    })
}

/// Parse the three manually configured Grok callbacks into one bounded,
/// vendor-specific locator report.
///
/// Grok emits camelCase and snake_case copies of several fields. When both
/// are present, neither spelling is treated as a fallback: both must have
/// the expected type and name the same value. This matters most for the
/// selection timestamp, because accepting a half-malformed duplicate would
/// let a delayed event bypass the durable ordering fence.
fn parse_grok_payload(value: &serde_json::Value) -> Result<ControlMsg, &'static str> {
    use farhelm_proto::ReportVendor;

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

    Ok(ControlMsg::ReportConversation {
        req_id: REQUEST_ID,
        vendor: ReportVendor::Grok,
        conversation,
        source: source.unwrap_or_default(),
        transcript_path: None,
        hook_event_name: Some(serde_json::Value::String(event.to_string())),
        agent_id,
    })
}

/// One authenticated round trip: connect, handshake, report, read the reply.
///
/// Each step is bounded by whatever is left of the shared deadline rather
/// than by a per-step timeout, so a slow connect cannot buy the handshake
/// extra time. The reply is read even though the hook does nothing with a
/// successful one: reading it is what distinguishes "the supervisor
/// accepted this" from "the supervisor refused it" in the log, and the
/// handshake's own contract requires callers to keep reading anyway (an
/// `Unauthorized` error arrives uncorrelated, after the hellos cross).
///
/// Takes the credential BY VALUE: this is the only round trip the process
/// will make, so the id and token can be moved into the handshake rather
/// than cloned out of a borrow, and the bearer token then exists in
/// exactly one place on its way to the socket.
async fn report(credential: HookCredential, request: &ControlMsg, deadline: Instant) -> Outcome {
    let connect = tokio::net::UnixStream::connect(&credential.socket);
    let stream = match tokio::time::timeout(remaining(deadline), connect).await {
        Err(_) => return Outcome::detail("timeout", "connect"),
        Ok(Err(err)) => return Outcome::io("connect", &err),
        Ok(Ok(stream)) => stream,
    };
    let (read, write) = tokio::io::split(stream);
    let mut reader = FrameReader::new(read);
    let mut writer = FrameWriter::new(write);

    let auth = SessionAuth {
        session_id: credential.session_id,
        token: credential.token,
    };
    let handshake = handshake_with_session_auth(&mut reader, &mut writer, auth);
    match tokio::time::timeout(remaining(deadline), handshake).await {
        Err(_) => return Outcome::detail("timeout", "handshake"),
        Ok(Err(err)) => return Outcome::io("handshake", &err),
        Ok(Ok(_peer_hello)) => {}
    }

    match tokio::time::timeout(remaining(deadline), writer.write_control(request)).await {
        Err(_) => return Outcome::detail("timeout", "send"),
        Ok(Err(err)) => return Outcome::io("send", &err),
        Ok(Ok(())) => {}
    }

    let frame = match tokio::time::timeout(remaining(deadline), reader.read_frame()).await {
        Err(_) => return Outcome::detail("timeout", "reply"),
        Ok(Err(err)) => return Outcome::io("reply", &err),
        Ok(Ok(None)) => {
            return Outcome::detail("connect-failed", "reply: closed before answering");
        }
        Ok(Ok(Some(frame))) => frame,
    };
    match parse_control(&frame) {
        Err(err) => Outcome::io("reply", &err),
        // The reply's `req_id` is not checked: this connection carried
        // exactly one request, so there is nothing a mismatched id could
        // disambiguate, and refusing on it would only convert a
        // successful report into a confusing log line.
        Ok(ControlMsg::ConversationReported { .. }) => Outcome::word("acked"),
        Ok(ControlMsg::Error { kind, message, .. }) => {
            Outcome::detail("refused", format!("{} {message}", error_kind_word(kind)))
        }
        Ok(other) => Outcome::detail("refused", format!("unexpected {}", control_tag(&other))),
    }
}

/// Time left before the shared deadline, saturating at zero.
///
/// A zero duration handed to `tokio::time::timeout` still polls the future
/// once, so an already-expired budget reports the phase it expired in
/// rather than skipping straight past it.
fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

/// The wire spelling of an [`farhelm_proto::ErrorKind`], for the log.
///
/// Routed through serde rather than a hand-written match so the log always
/// says exactly what the protocol says, and so a new kind cannot silently
/// become a stale word here.
fn error_kind_word(kind: farhelm_proto::ErrorKind) -> String {
    match serde_json::to_value(kind) {
        Ok(serde_json::Value::String(word)) => word,
        _ => "unknown".to_string(),
    }
}

/// The `type` tag of an unexpected reply, for the log.
///
/// `ControlMsg` is an internally-tagged enum, so its own serialization is
/// the authoritative name; `Debug` would drag the whole payload — possibly
/// including a session listing — into a log line.
fn control_tag(message: &ControlMsg) -> String {
    match serde_json::to_value(message) {
        Ok(serde_json::Value::Object(map)) => match map.get("type") {
            Some(serde_json::Value::String(tag)) => tag.clone(),
            _ => "unknown".to_string(),
        },
        _ => "unknown".to_string(),
    }
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
    /// An outcome with no detail, such as `acked` or `no-credential`.
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

    /// A socket-side I/O failure, named by the phase it happened in.
    ///
    /// Every I/O failure on the socket shares the `connect-failed` word,
    /// including ones in the later `send` and `reply` phases: the log's
    /// outcome vocabulary is fixed, and the phase prefix is what tells a
    /// reader which step actually broke.
    fn io(phase: &'static str, err: &std::io::Error) -> Self {
        Outcome::detail("connect-failed", format!("{phase}: {err}"))
    }

    /// Attach the reported identity, once the payload has yielded one.
    fn about(mut self, conversation: &str, source: &str) -> Self {
        self.about = Some((conversation.to_string(), source.to_string()));
        self
    }

    /// Render the log line, sanitizing every interpolated value.
    fn render(&self, seconds: u64) -> String {
        let mut line = format!("{seconds} {}", self.word);
        if !self.detail.is_empty() {
            line.push(' ');
            line.push_str(&sanitize(&self.detail, 512, false));
        }
        if let Some((conversation, source)) = &self.about {
            line.push(' ');
            line.push_str(&sanitize(conversation, MAX_SESSION_ID_BYTES, true));
            line.push(' ');
            // A missing `source` becomes `-` rather than an empty field so
            // the line keeps a fixed shape: a trailing space is invisible
            // in a log and turns "no source" into "unparsable line".
            if source.is_empty() {
                line.push('-');
            } else {
                line.push_str(&sanitize(source, 64, true));
            }
        }
        line
    }
}

/// Make a value safe to interpolate into one log line.
///
/// Every character [`farhelm_proto::text::is_presentation_unsafe`] names
/// becomes `_`. Control characters are the obvious case: they stop an
/// agent-supplied conversation id from embedding a newline and forging an
/// extra line. The rest do the same damage without being controls (line
/// separators many viewers break on, bidi controls that make a `refused` line
/// render as an `acked` one) or let two different ids read identically
/// (zero-width characters), which in an audit log is the same lie.
/// `single_token` additionally collapses spaces, and is used for the
/// trailing identity fields: those are positional, so a space inside one
/// would silently shift the other. Free-form detail keeps its spaces,
/// because a supervisor's refusal message is a sentence and mangling it
/// would defeat the log's only purpose.
///
/// The length cap is counted in chars and applied before replacement, so a
/// hostile value cannot outgrow its field.
fn sanitize(value: &str, max_chars: usize, single_token: bool) -> String {
    value
        .chars()
        .take(max_chars)
        .map(|c| {
            if farhelm_proto::text::is_presentation_unsafe(c) || (single_token && c.is_whitespace())
            {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// Append one line to the hook log, ignoring every failure.
///
/// Creates the parent directory 0700 if missing — it holds one file per
/// session under the supervisor's state directory, and nothing outside
/// that state directory has any business reading them. Truncates the file
/// first when it has grown past [`MAX_LOG_BYTES`].
///
/// ## One line, one write
///
/// The line and its trailing newline go out in a SINGLE `write_all`, never
/// as a `writeln!` that may reach the descriptor in pieces. There is no
/// locking here and deliberately so: several agents can be launched in one
/// session, and two hooks appending at once would, with a split write,
/// interleave halfway through a line and destroy the one property this
/// format promises. With one write per line the worst case is whole lines
/// out of order, which costs a reader nothing. This relies on
/// `O_APPEND` + a single small write, which is atomic on the local
/// filesystems this file lives on; it is a diagnostic, not a ledger, and
/// that is the right amount of guarantee to buy for it.
///
/// ## Why no symlink hardening
///
/// The path is inside the supervisor's own 0700 state directory. Any
/// process able to plant a symlink or a FIFO there already holds the
/// session credential sitting beside it and already has the user's own
/// file access, so `O_NOFOLLOW` would defend a boundary that was crossed
/// before this function ran.
///
/// Nothing here reports failure, by design: this function exists to
/// explain a broken run, and a hook that fails because its own diagnostics
/// failed would be the worst outcome of all.
fn append_log(path: Option<&Path>, line: &str) {
    use std::io::Write as _;
    use std::os::unix::fs::DirBuilderExt as _;

    let Some(path) = path else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent);
    }
    if std::fs::metadata(path).is_ok_and(|meta| meta.len() > MAX_LOG_BYTES) {
        // Truncate rather than rotate; see MAX_LOG_BYTES.
        let _ = std::fs::File::create(path);
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = file.write_all(format!("{line}\n").as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::ReportVendor;
    use farhelm_teststate::thread::FixtureThread;
    use std::io::Cursor;

    /// The Claude Code 2.1.241 `SessionStart` payload, verbatim from the
    /// hand-verified vendor audit (re-run by `real_agent_capture.rs`'s
    /// ignored hook tests).
    const CLAUDE_PAYLOAD: &str = r#"{"session_id":"6af192d4-0000-4000-8000-000000000000","transcript_path":"/home/u/.claude/projects/x/6af192d4.jsonl","cwd":"/home/u/src","hook_event_name":"SessionStart","source":"startup"}"#;

    /// The Codex CLI 0.149.1 `SessionStart` payload, verbatim from the
    /// hand-verified vendor audit (re-run by `real_agent_capture.rs`'s
    /// ignored hook tests). It carries two fields Claude's does not.
    const CODEX_PAYLOAD: &str = r#"{"session_id":"0198d3ac-0000-7000-8000-000000000000","transcript_path":"/home/u/.codex/sessions/x.jsonl","cwd":"/home/u/src","hook_event_name":"SessionStart","model":"gpt-5-codex","permission_mode":"default","source":"startup"}"#;

    /// A budget short enough to keep the timeout tests fast while staying
    /// far above the scheduling jitter of a loaded CI runner.
    const TEST_BUDGET: Duration = Duration::from_millis(300);

    /// How long a test's fake supervisor will wait for any single
    /// milestone before giving up.
    ///
    /// Every blocking wait on a server thread is bounded by this, and the
    /// reason is CI rather than speed: an unbounded `accept` or `recv` in a
    /// test server turns a regression in the code under test into a job
    /// that hangs until the runner's global timeout kills it, with no
    /// output naming the culprit. Bounded, the same regression fails the
    /// assertion that follows. It is set generously — orders of magnitude
    /// above what these round trips need on a loaded runner — because its
    /// only job is to be finite.
    const SERVER_DEADLINE: Duration = Duration::from_secs(10);

    /// Poll interval for the one server that has to accept without
    /// blocking. Short enough not to distort a budget test, long enough
    /// not to spin a core.
    const SERVER_POLL: Duration = Duration::from_millis(5);

    /// Owns a silent supervisor fixture and witnesses for its connection and release edges.
    ///
    /// The witnesses let focused tests prove both cancellation before a dial and
    /// cancellation while a peer is held, without synchronizing on sleeps.
    struct SilentSupervisor {
        owner: FixtureThread,
        stop: mpsc::Sender<()>,
        accepted: mpsc::Receiver<()>,
        released: mpsc::Receiver<()>,
    }

    /// Hold an accepted peer open without speaking until cancellation or the safety deadline.
    ///
    /// Keeping the peer alive preserves the timeout premise; closing it early would test
    /// connection failure instead. The listener must be nonblocking so cancellation can
    /// interrupt the accept loop. The owner retains the test trace through thread exit.
    fn spawn_silent_supervisor(
        listener: std::os::unix::net::UnixListener,
        context: farhelm_testtrace::ThreadContext,
    ) -> SilentSupervisor {
        let (stop, stop_rx) = mpsc::channel::<()>();
        let (accepted_tx, accepted) = mpsc::channel();
        let (released_tx, released) = mpsc::channel();
        let server = std::thread::spawn(move || {
            context.enter(|| {
                let deadline = Instant::now() + SERVER_DEADLINE;
                let mut held = None;
                while Instant::now() < deadline {
                    // Drop cleanup can happen before the hook ever dials. Poll
                    // cancellation before every accept attempt so that case
                    // does not fall through to another wait.
                    match stop_rx.try_recv() {
                        Ok(()) | Err(mpsc::TryRecvError::Disconnected) => {
                            drop(listener);
                            let _ = released_tx.send(());
                            return;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                    match listener.accept() {
                        Ok(connection) => {
                            let _ = accepted_tx.send(());
                            held = Some(connection);
                            break;
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            // sleep-ok: the nonblocking accept loop must also observe owner cancellation before a client connects.
                            std::thread::sleep(SERVER_POLL);
                        }
                        Err(err) => panic!("accept failed: {err}"),
                    }
                }
                let _ = stop_rx.recv_timeout(SERVER_DEADLINE);
                drop(held);
                drop(listener);
                let _ = released_tx.send(());
            });
        });
        let cancellation = stop.clone();
        let owner = FixtureThread::new("hook-silent-supervisor", server, move || {
            let _ = cancellation.send(());
        })
        .expect("start fixture join observer");
        SilentSupervisor {
            owner,
            stop,
            accepted,
            released,
        }
    }

    /// Owns an async round-trip fixture and reports whether its transaction succeeded.
    struct RoundTripSupervisor {
        owner: FixtureThread,
        outcome: mpsc::Receiver<Result<(), &'static str>>,
    }

    /// Run the complete supervisor exchange under one deadline and report cancellation separately.
    ///
    /// The nonblocking listener belongs to a separate runtime because the synchronous hook
    /// entry point creates its own runtime. The captured context covers this server runtime's
    /// full lifetime, including teardown, rather than only its protocol future.
    fn spawn_round_trip_supervisor(
        listener: std::os::unix::net::UnixListener,
        context: farhelm_testtrace::ThreadContext,
        seen_tx: mpsc::Sender<(String, String, Option<SessionAuth>)>,
    ) -> RoundTripSupervisor {
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
        let (outcome_tx, outcome) = mpsc::channel();
        let server = std::thread::spawn(move || {
            context
                .with_runtime(
                    farhelm_testtrace::RuntimeConfig {
                        flavor: farhelm_testtrace::RuntimeFlavor::CurrentThread,
                        worker_threads: None,
                        start_paused: false,
                    },
                    |runtime| {
                        runtime.block_on(async move {
                            let listener = tokio::net::UnixListener::from_std(listener)
                                .expect("adopt listener");
                            // One aggregate bound covers the whole exchange. The stage
                            // bounds retain useful diagnostics, but cannot multiply the
                            // time this fixture keeps the test alive.
                            let transaction = async {
                                let (stream, _) =
                                    tokio::time::timeout(SERVER_DEADLINE, listener.accept())
                                        .await
                                        .expect("the hook must connect within the deadline")
                                        .expect("accept");
                                let (read, write) = tokio::io::split(stream);
                                let mut reader = FrameReader::new(read);
                                let mut writer = FrameWriter::new(write);
                                let hello = tokio::time::timeout(
                                    SERVER_DEADLINE,
                                    farhelm_proto::io::handshake(
                                        &mut reader,
                                        &mut writer,
                                        "supervisor",
                                    ),
                                )
                                .await
                                .expect("the handshake must complete within the deadline")
                                .expect("handshake");
                                let auth = match hello {
                                    ControlMsg::Hello { auth, .. } => auth,
                                    other => panic!("expected a hello, got {other:?}"),
                                };
                                let frame =
                                    tokio::time::timeout(SERVER_DEADLINE, reader.read_frame())
                                        .await
                                        .expect("the report must arrive within the deadline")
                                        .expect("read the report")
                                        .expect("a frame, not EOF");
                                match parse_control(&frame).expect("decode the report") {
                                    ControlMsg::ReportConversation {
                                        req_id,
                                        conversation,
                                        source,
                                        ..
                                    } => {
                                        let _ = seen_tx.send((conversation, source, auth));
                                        tokio::time::timeout(
                                            SERVER_DEADLINE,
                                            writer.write_control(
                                                &ControlMsg::ConversationReported { req_id },
                                            ),
                                        )
                                        .await
                                        .expect(
                                            "the acknowledgement must be written within the deadline",
                                        )
                                        .expect("acknowledge");
                                    }
                                    other => panic!("expected a report, got {other:?}"),
                                }
                                Ok::<(), &'static str>(())
                            };
                            let outcome = match tokio::time::timeout(SERVER_DEADLINE, async {
                                tokio::select! {
                                    _ = cancel_rx => Err("fixture cancelled"),
                                    result = transaction => result,
                                }
                            })
                            .await
                            {
                                Ok(outcome) => outcome,
                                Err(_) => Err("aggregate timeout"),
                            };
                            let _ = outcome_tx.send(outcome);
                        });
                    },
                )
                .expect("server runtime");
        });
        let owner = FixtureThread::new("hook-round-trip-supervisor", server, move || {
            let _ = cancel_tx.send(());
        })
        .expect("start fixture join observer");
        RoundTripSupervisor { owner, outcome }
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
        let ControlMsg::ReportConversation {
            conversation: id,
            source,
            ..
        } = parse_payload(CLAUDE_PAYLOAD.as_bytes(), ReportVendor::Claude)
            .expect("claude payload parses")
        else {
            panic!("expected a conversation report");
        };
        assert_eq!(id, "6af192d4-0000-4000-8000-000000000000");
        assert_eq!(source, "startup");
    }

    /// The same, for Codex — whose payload carries `model` and
    /// `permission_mode` on top of Claude's fields. Both vendors go
    /// through one parser, so this pins that the extra fields are simply
    /// ignored rather than being a second shape to maintain.
    #[farhelm_testtrace::test]
    fn parses_the_verbatim_codex_payload() {
        let ControlMsg::ReportConversation {
            conversation: id,
            source,
            ..
        } = parse_payload(CODEX_PAYLOAD.as_bytes(), ReportVendor::Codex)
            .expect("codex payload parses")
        else {
            panic!("expected a conversation report");
        };
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
        let ControlMsg::ReportConversation {
            conversation: id,
            source,
            ..
        } = parse_payload(payload.as_bytes(), ReportVendor::Claude)
            .expect("unknown fields are ignored")
        else {
            panic!("expected a conversation report");
        };
        assert_eq!(id, "abc");
        assert_eq!(source, "resume");
    }

    /// `source` is diagnostic-only and optional, so its absence must not
    /// cost the run its id. Codex's TUI, for one, reuses `startup` where
    /// Claude sends `clear`; nothing may key on the field's presence.
    #[farhelm_testtrace::test]
    fn missing_source_defaults_to_empty() {
        let ControlMsg::ReportConversation {
            conversation: id,
            source,
            ..
        } = parse_payload(br#"{"session_id":"abc"}"#, ReportVendor::Claude)
            .expect("id alone is enough")
        else {
            panic!("expected a conversation report");
        };
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
        let ControlMsg::ReportConversation {
            conversation: id, ..
        } = parse_payload(payload.as_bytes(), ReportVendor::Claude)
            .expect("an id at the cap is fine")
        else {
            panic!("expected a conversation report");
        };
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
        let ControlMsg::ReportConversation {
            conversation: id, ..
        } = parse_payload(payload.as_bytes(), ReportVendor::Claude)
            .expect("128 bytes is at the cap")
        else {
            panic!("expected a conversation report");
        };
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
        let ControlMsg::ReportConversation {
            conversation: id,
            source,
            vendor,
            ..
        } = parse_payload(
            br#"{"vendor":"omp","session_id":"omp-id-1",
            "session_file":"/tmp/s/conv.jsonl","source":"session_start"}"#,
            ReportVendor::Omp,
        )
        .expect("omp payload parses")
        else {
            panic!("expected a conversation report");
        };
        assert!(
            id.starts_with("omp:"),
            "OMP reports under its own prefix: {id}"
        );
        assert_eq!(source, "session_start");
        assert_eq!(vendor, ReportVendor::Omp);

        let ControlMsg::ReportConversation {
            conversation: id,
            vendor,
            ..
        } = parse_payload(br#"{"vendor":"pi","session_id":"pi-1"}"#, ReportVendor::Pi)
            .expect("pi payload parses")
        else {
            panic!("expected a conversation report");
        };
        assert!(id.starts_with("pi:"), "Pi's spelling is unchanged: {id}");
        assert_eq!(vendor, ReportVendor::Pi);

        // The entry point is authoritative without a payload vendor: a Pi
        // entry encodes the locator even when the JSON names none.
        let ControlMsg::ReportConversation {
            conversation: id,
            vendor,
            ..
        } = parse_payload(br#"{"session_id":"pi-2"}"#, ReportVendor::Pi)
            .expect("a vendorless pi payload still encodes")
        else {
            panic!("expected a conversation report");
        };
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
        let ControlMsg::ReportConversation {
            vendor,
            conversation,
            source,
            transcript_path,
            hook_event_name,
            agent_id,
            ..
        } = parse_payload(payload, ReportVendor::Grok).expect("Grok callback parses")
        else {
            panic!("expected a conversation report");
        };
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

        let ControlMsg::ReportConversation {
            source,
            hook_event_name,
            conversation,
            ..
        } = parse_payload(
            br#"{"sessionId":"a","hookEventName":"user_prompt_submit","transcriptPath":"/tmp/a/updates.jsonl"}"#,
            ReportVendor::Grok,
        )
        .expect("enrichment needs neither source nor timestamp")
        else {
            panic!("expected a conversation report");
        };
        assert_eq!(source, "");
        assert_eq!(hook_event_name, Some(serde_json::json!("UserPromptSubmit")));
        let locator: serde_json::Value = serde_json::from_str(
            conversation
                .strip_prefix("grok:")
                .expect("Grok reports use their locator prefix"),
        )
        .expect("the locator is JSON");
        assert_eq!(locator["selected_at"], serde_json::Value::Null);

        let ControlMsg::ReportConversation { conversation, .. } = parse_payload(
            br#"{"sessionId":"a","hookEventName":"stop","timestamp":"2026-09-22T12:00:00.123456789+00:00","transcriptPath":"/tmp/a/updates.jsonl"}"#,
            ReportVendor::Grok,
        )
        .expect("enrichment timestamps are validated but not persisted")
        else {
            panic!("expected a conversation report");
        };
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

    /// Raw subagent evidence crosses the socket verbatim: the hook never
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
        let ControlMsg::ReportConversation { agent_id, .. } = parse_payload(
            br#"{"session_id":"abc","agent_id":"sub-1"}"#,
            ReportVendor::Claude,
        )
        .expect("an agent identity parses") else {
            panic!("expected a conversation report");
        };
        assert_eq!(
            agent_id,
            Some(serde_json::Value::String("sub-1".to_string()))
        );

        let ControlMsg::ReportConversation { agent_id, .. } =
            parse_payload(br#"{"session_id":"abc"}"#, ReportVendor::Claude)
                .expect("absence parses")
        else {
            panic!("expected a conversation report");
        };
        assert_eq!(agent_id, None);

        let ControlMsg::ReportConversation { agent_id, .. } = parse_payload(
            br#"{"session_id":"abc","agent_id":7}"#,
            ReportVendor::Claude,
        )
        .expect("a wrong-typed identity still parses here") else {
            panic!("expected a conversation report");
        };
        assert_eq!(agent_id, Some(serde_json::Value::from(7)));
    }

    /// Without a credential the run must stop before any socket work, and
    /// say so. This is the "agent launched outside farhelm" case: it is
    /// expected, not an error, and the log line is the only way to tell it
    /// apart from a hook that never ran at all.
    #[farhelm_testtrace::test]
    fn no_credential_stops_before_any_socket_work() {
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
    /// and none of them dials the supervisor.
    ///
    /// [`parse_payload`]'s own test pins the reasons in isolation; this
    /// one pins that they survive the whole of [`run_with`] — the layer a
    /// human actually reads — and that the run STOPS there. The second
    /// claim rides on the socket path being absent: a dial would have
    /// replaced the ending with `connect-failed connect: No such file or
    /// directory`, so a line ending in the reason word is proof the socket
    /// was never touched. Reporting an id we could not parse is the bug
    /// this guards against; the supervisor would have to refuse it, and
    /// the refusal would look like a supervisor problem.
    #[farhelm_testtrace::test]
    fn a_bad_payload_is_logged_without_dialing_the_supervisor() {
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
                Some(HookCredential {
                    session_id: "sess-1".to_string(),
                    token: "tok".to_string(),
                    socket: dir.path().join("absent.sock"),
                }),
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
                Some(HookCredential {
                    session_id: "sess-1".to_string(),
                    token: "tok".to_string(),
                    socket: dir.path().join("absent.sock"),
                }),
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
            Some(HookCredential {
                session_id: "sess-1".to_string(),
                token: "tok".to_string(),
                socket: dir.path().join("absent.sock"),
            }),
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

    /// A socket path that does not exist must produce a `connect-failed`
    /// line and an ordinary return — never a panic and never a message on
    /// a descriptor the agent can see. A stale or removed socket is a real
    /// situation (a supervisor restart mid-session), so it has to be the
    /// boring path.
    #[farhelm_testtrace::test]
    fn missing_socket_reports_connect_failed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        run_with(
            Some(HookCredential {
                session_id: "sess-1".to_string(),
                token: "tok".to_string(),
                socket: dir.path().join("absent.sock"),
            }),
            Cursor::new(CLAUDE_PAYLOAD.to_string().into_bytes()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        let line = single_line(&log);
        let detail = line.splitn(3, ' ').nth(2).expect("outcome word and detail");
        assert!(
            line.contains(" connect-failed connect: "),
            "line was {line:?}"
        );
        // The identity pair still rides along on a failed report: knowing
        // which conversation went unreported is the point of the log.
        assert!(
            detail.ends_with(" 6af192d4-0000-4000-8000-000000000000 startup"),
            "line was {line:?}"
        );
    }

    /// A payload reader that never reaches EOF must cost the budget and no
    /// more. This is the vendor-holds-the-pipe case that motivates the
    /// detached reader thread: a blocking read cannot be cancelled, so the
    /// only proof the design works is that the call still returns.
    #[farhelm_testtrace::test]
    fn a_blocking_payload_reader_gives_up_at_the_budget() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        // A socketpair whose write half the test keeps alive: the read
        // half blocks forever, exactly like a vendor holding our stdin.
        let (read_half, _write_half) = std::os::unix::net::UnixStream::pair().expect("socketpair");
        let started = Instant::now();
        run_with(
            Some(HookCredential {
                session_id: "sess-1".to_string(),
                token: "tok".to_string(),
                socket: dir.path().join("absent.sock"),
            }),
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

    /// A supervisor that accepts the connection and then says nothing must
    /// also cost only the budget. A wedged supervisor is the failure most
    /// likely to push the hook past the vendor's own timeout, which is the
    /// one failure the user would actually see.
    #[farhelm_testtrace::test]
    fn an_unanswering_supervisor_gives_up_at_the_budget() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        let socket = dir.path().join("supervisor.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        // This server leaves the wrapped test thread, so retain the capture
        // while its bounded hold and cleanup run on the raw thread.
        let context = farhelm_testtrace::current_thread_context().expect("test trace context");
        let SilentSupervisor {
            owner: server,
            stop: stop_tx,
            accepted: _,
            released,
        } = spawn_silent_supervisor(listener, context);

        let started = Instant::now();
        run_with(
            Some(HookCredential {
                session_id: "sess-1".to_string(),
                token: "tok".to_string(),
                socket,
            }),
            Cursor::new(CLAUDE_PAYLOAD.to_string().into_bytes()),
            TEST_BUDGET,
            Some(log.clone()),
            ReportVendor::Claude,
        );
        assert!(
            started.elapsed() < TEST_BUDGET + Duration::from_millis(500),
            "run took {:?}",
            started.elapsed()
        );
        let line = single_line(&log);
        // Which phase the budget dies in depends on how far the handshake
        // got before the silence; the contract is only that it is a
        // timeout and that it names a phase.
        assert!(
            line.contains(" timeout connect") || line.contains(" timeout handshake"),
            "line was {line:?}"
        );

        let _ = stop_tx.send(());
        server
            .finish(Duration::from_secs(1))
            .expect("silent supervisor fixture");
        released
            .recv_timeout(Duration::from_secs(1))
            .expect("silent supervisor released its listener and peer");
    }

    /// Cancellation before accept must release the listener during assertion unwind.
    #[farhelm_testtrace::test]
    fn a_silent_supervisor_cancels_before_accept_on_unwind() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("supervisor.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let context = farhelm_testtrace::current_thread_context().expect("test trace context");
        let SilentSupervisor {
            owner,
            stop,
            accepted,
            released,
        } = spawn_silent_supervisor(listener, context);
        drop(stop);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _owner = owner;
            panic!("exercise cancellation before accept");
        }));
        assert!(unwind.is_err());
        assert!(accepted.try_recv().is_err());
        released
            .recv_timeout(Duration::from_secs(1))
            .expect("cancellation released the pre-accept listener");
        // Unlinking would permit rebinding even while the original listener remained alive.
        assert!(
            std::os::unix::net::UnixStream::connect(&socket).is_err(),
            "the original listener must refuse new connections"
        );
    }

    /// Cancellation while a peer is held must close that peer during unwind.
    #[farhelm_testtrace::test]
    fn a_silent_supervisor_cancels_while_holding_a_peer_on_unwind() {
        use std::io::Read as _;

        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("supervisor.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let context = farhelm_testtrace::current_thread_context().expect("test trace context");
        let SilentSupervisor {
            owner,
            stop,
            accepted,
            released,
        } = spawn_silent_supervisor(listener, context);
        let mut peer = std::os::unix::net::UnixStream::connect(&socket).expect("connect peer");
        accepted
            .recv_timeout(Duration::from_secs(1))
            .expect("fixture accepted the peer");
        peer.set_read_timeout(Some(Duration::from_secs(1)))
            .expect("bound peer read");
        drop(stop);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _owner = owner;
            panic!("exercise cancellation while holding a peer");
        }));
        assert!(unwind.is_err());
        released
            .recv_timeout(Duration::from_secs(1))
            .expect("cancellation released the held peer");
        let mut byte = [0; 1];
        assert_eq!(peer.read(&mut byte).expect("peer EOF"), 0);
        // Unlinking would permit rebinding even while the original listener remained alive.
        assert!(
            std::os::unix::net::UnixStream::connect(&socket).is_err(),
            "the original listener must refuse new connections"
        );
    }

    /// The whole point, end to end: a supervisor that completes the
    /// handshake and acknowledges gets the reported id and source, and the
    /// hook logs `acked`. Everything else in this module is a failure
    /// path; this is the one that has to work.
    #[farhelm_testtrace::test]
    fn a_completed_round_trip_reports_and_acks() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        let socket = dir.path().join("supervisor.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let (seen_tx, seen_rx) = mpsc::channel::<(String, String, Option<SessionAuth>)>();

        let context = farhelm_testtrace::current_thread_context().expect("test trace context");
        let RoundTripSupervisor {
            owner: server,
            outcome,
        } = spawn_round_trip_supervisor(listener, context, seen_tx);

        run_with(
            Some(HookCredential {
                session_id: "sess-1".to_string(),
                token: "tok-1".to_string(),
                socket,
            }),
            Cursor::new(CLAUDE_PAYLOAD.to_string().into_bytes()),
            Duration::from_secs(5),
            Some(log.clone()),
            ReportVendor::Claude,
        );
        server
            .finish(SERVER_DEADLINE)
            .expect("round-trip supervisor fixture");
        assert_eq!(
            outcome
                .recv_timeout(SERVER_DEADLINE)
                .expect("round-trip outcome"),
            Ok(())
        );

        let (conversation, source, auth) = seen_rx
            .recv_timeout(SERVER_DEADLINE)
            .expect("the server saw a report");
        assert_eq!(conversation, "6af192d4-0000-4000-8000-000000000000");
        assert_eq!(source, "startup");
        let auth = auth.expect("the hook authenticates as its session");
        assert_eq!(auth.session_id, "sess-1");
        assert_eq!(auth.token, "tok-1");

        let line = single_line(&log);
        assert!(
            line.ends_with(" acked 6af192d4-0000-4000-8000-000000000000 startup"),
            "line was {line:?}"
        );
    }

    /// Cancellation before accept must produce an explicit unsuccessful transaction result.
    #[farhelm_testtrace::test]
    fn a_round_trip_fixture_reports_cancellation_before_accept() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("supervisor.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let (seen_tx, seen_rx) = mpsc::channel::<(String, String, Option<SessionAuth>)>();
        let context = farhelm_testtrace::current_thread_context().expect("test trace context");
        let RoundTripSupervisor { owner, outcome } =
            spawn_round_trip_supervisor(listener, context, seen_tx);
        drop(owner);
        assert_eq!(
            outcome
                .recv_timeout(Duration::from_secs(1))
                .expect("round-trip cancellation outcome"),
            Err("fixture cancelled")
        );
        assert!(seen_rx.try_recv().is_err());
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
    /// the deliberate choice over rotation (see `MAX_LOG_BYTES`); this
    /// test is what would catch a future "improvement" that silently
    /// removed the bound.
    #[farhelm_testtrace::test]
    fn an_oversized_log_is_truncated_before_appending() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log_dir = dir.path().join("hook-log");
        std::fs::create_dir_all(&log_dir).expect("create log dir");
        let log = log_dir.join("s.log");
        std::fs::write(&log, vec![b'x'; (MAX_LOG_BYTES + 1024) as usize]).expect("seed a big log");

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
            Some(HookCredential {
                session_id: "sess-1".to_string(),
                token: "tok".to_string(),
                socket: dir.path().join("absent.sock"),
            }),
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
    /// `connect-failed` line could be made to read as an `acked` one to
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
            Some(HookCredential {
                session_id: "sess-1".to_string(),
                token: "tok".to_string(),
                socket: dir.path().join("absent.sock"),
            }),
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
            Some(HookCredential {
                session_id: "sess-1".to_string(),
                token: "tok".to_string(),
                socket: dir.path().join("absent.sock"),
            }),
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
