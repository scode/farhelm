//! `farhelm internal hook`: the agent's own `SessionStart` hook, reporting
//! the conversation id the vendor just minted.
//!
//! Both supported vendors fire a `SessionStart` hook whose JSON payload
//! carries the exact `session_id` a later resume needs. Farhelm injects
//! itself as that hook, so this module runs as a short-lived child of the
//! agent process, inside the agent's own terminal, with the session
//! credential already in its environment. It reads the payload from stdin
//! and sends one `ControlMsg::ReportConversation` over the supervisor
//! socket. Everything else about it is a consequence of *where* it runs.
//!
//! ## The contract
//!
//! These rules are not stylistic; each one exists because the alternative
//! is visible to the human using the agent.
//!
//! 1. **Silence on stdout and stderr, except for one deliberate line.**
//!    The identity work itself writes nothing to either descriptor: both
//!    vendors feed a `SessionStart` hook's stdout into the model's context
//!    as text, and both surface stderr when the hook fails, so a
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
//!    the agent outside farhelm with our flags somehow present); the run
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
//! working resume for a log nicety. There is
//! deliberately no separate "reported" line ahead of the outcome: one line
//! per run is the whole promise, so the outcome word is always the
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

/// Largest stdin payload accepted. A `SessionStart` payload is a few
/// hundred bytes of JSON; the cap exists so a vendor (or anything else
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
/// Both supported vendors treat plain-text stdout from a `SessionStart`
/// hook as context for the model, and both were checked rather than
/// assumed:
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
/// One mechanism serves both vendors.
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
pub const POINTER_LINE: &str = "farhelm: when the user writes \"$farhelm ...\", run `farhelm agent instructions` and follow \
     its output.";

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
) {
    // Two nested catches, for two different failures. The inner one turns
    // a panic in the work into the `panic` outcome, so the log still gets
    // its one line; the outer one guarantees that even a panic while
    // logging cannot escape into the caller, which must reach `exit(0)`.
    let outcome =
        match std::panic::catch_unwind(AssertUnwindSafe(|| run_inner(credential, payload, budget)))
        {
            Ok(outcome) => outcome,
            Err(_) => Outcome::word("panic"),
        };
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
        append_log(hook_log.as_deref(), &outcome.render(unix_seconds()));
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
    let (conversation, source) = match parse_payload(&bytes) {
        Ok(parsed) => parsed,
        Err(reason) => return Outcome::detail("bad-payload", reason),
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
                .about(&conversation, &source);
        }
    };
    runtime
        .block_on(report(credential, &conversation, &source, deadline))
        .about(&conversation, &source)
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

/// Extract `(session_id, source)` from a vendor `SessionStart` payload.
///
/// Parsed as a free-form [`serde_json::Value`] rather than a struct on
/// purpose: both vendors already send fields this side has no use for
/// (`transcript_path`, `cwd`, `model`, `permission_mode`, …) and both are
/// free to add more. A payload gaining a field must never turn into a
/// failed report, so unknown fields are ignored and only `session_id` is
/// required. `source` is optional and defaults to empty, because it is
/// diagnostic-only — nothing keys behavior on it.
///
/// The returned id is untrusted and merely length-bounded here; the
/// supervisor decides whether it is plausible.
fn parse_payload(bytes: &[u8]) -> Result<(String, String), &'static str> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| "unparsable")?;
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
    Ok((session_id.clone(), source))
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
async fn report(
    credential: HookCredential,
    conversation: &str,
    source: &str,
    deadline: Instant,
) -> Outcome {
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

    let request = ControlMsg::ReportConversation {
        req_id: REQUEST_ID,
        conversation: conversation.to_string(),
        source: source.to_string(),
    };
    match tokio::time::timeout(remaining(deadline), writer.write_control(&request)).await {
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
/// Control characters always become `_` — that is what stops an
/// agent-supplied conversation id from embedding a newline and forging an
/// extra line, which is the only forgery this format is exposed to.
/// [`is_line_or_direction_control`] covers the characters that do the same
/// damage without being control characters at all.
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
            if c.is_control()
                || is_line_or_direction_control(c)
                || (single_token && c.is_whitespace())
            {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// Characters that break a line-oriented log without being
/// `char::is_control`.
///
/// Two families, one motivation — the operator reading this file must see
/// what was actually written:
///
/// - U+2028 and U+2029, the Unicode line and paragraph separators. Rust
///   classifies them as `Zl`/`Zp` rather than `Cc`, so `is_control` says
///   no, but plenty of viewers and log processors break a line on them.
///   That is the same forgery a raw newline would be, reached through a
///   character the obvious check misses.
/// - The bidirectional formatting characters: the marks U+200E/U+200F, the
///   embeddings and overrides U+202A–U+202E, and the isolates
///   U+2066–U+2069. These reorder the VISIBLE text without changing the
///   bytes, so an agent-supplied id could make a `refused` line render as
///   an `acked` one to the human reading it — a lie told to the only
///   audience this file has.
fn is_line_or_direction_control(c: char) -> bool {
    matches!(
        c,
        '\u{2028}'
            | '\u{2029}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

/// Wall-clock seconds for the log's timestamp, or 0 if the clock is before
/// the epoch. A nonsense timestamp is still better than losing the line.
fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
    #[test]
    fn parses_the_verbatim_claude_payload() {
        let (id, source) = parse_payload(CLAUDE_PAYLOAD.as_bytes()).expect("claude payload parses");
        assert_eq!(id, "6af192d4-0000-4000-8000-000000000000");
        assert_eq!(source, "startup");
    }

    /// The same, for Codex — whose payload carries `model` and
    /// `permission_mode` on top of Claude's fields. Both vendors go
    /// through one parser, so this pins that the extra fields are simply
    /// ignored rather than being a second shape to maintain.
    #[test]
    fn parses_the_verbatim_codex_payload() {
        let (id, source) = parse_payload(CODEX_PAYLOAD.as_bytes()).expect("codex payload parses");
        assert_eq!(id, "0198d3ac-0000-7000-8000-000000000000");
        assert_eq!(source, "startup");
    }

    /// A payload with fields no version of this code has ever seen must
    /// still parse. Vendors add hook fields without warning, and the
    /// failure mode of a strict parse would be a hook that silently stops
    /// reporting after a vendor upgrade — the exact bug this mechanism
    /// exists to avoid.
    #[test]
    fn ignores_unknown_payload_fields() {
        let payload = r#"{"session_id":"abc","source":"resume","future_field":{"nested":[1,2]},"another":null}"#;
        let (id, source) = parse_payload(payload.as_bytes()).expect("unknown fields are ignored");
        assert_eq!(id, "abc");
        assert_eq!(source, "resume");
    }

    /// `source` is diagnostic-only and optional, so its absence must not
    /// cost the run its id. Codex's TUI, for one, reuses `startup` where
    /// Claude sends `clear`; nothing may key on the field's presence.
    #[test]
    fn missing_source_defaults_to_empty() {
        let (id, source) = parse_payload(br#"{"session_id":"abc"}"#).expect("id alone is enough");
        assert_eq!(id, "abc");
        assert_eq!(source, "");
    }

    /// Every shape of unusable id is rejected with its own reason word.
    /// The reason is what a maintainer reads out of the hook log when a
    /// session mysteriously fails to resume, so each case earning a
    /// distinct word is the point, not an implementation detail.
    #[test]
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
            let reason = parse_payload(payload).expect_err("payload should be rejected");
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
    #[test]
    fn accepts_a_session_id_at_the_cap() {
        let payload = format!(r#"{{"session_id":"{}"}}"#, "x".repeat(MAX_SESSION_ID_BYTES));
        let (id, _) = parse_payload(payload.as_bytes()).expect("an id at the cap is fine");
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
    #[test]
    fn the_session_id_cap_counts_bytes_not_characters() {
        // Four bytes each, so 32 of them are 32 chars and exactly the cap.
        let at_cap = "😀".repeat(MAX_SESSION_ID_BYTES / 4);
        assert_eq!(at_cap.len(), MAX_SESSION_ID_BYTES, "fixture sanity");
        let payload = format!(r#"{{"session_id":"{at_cap}"}}"#);
        let (id, _) = parse_payload(payload.as_bytes()).expect("128 bytes is at the cap");
        assert_eq!(id.len(), MAX_SESSION_ID_BYTES);

        // One character more is four bytes more, and therefore over.
        let over_cap = "😀".repeat(MAX_SESSION_ID_BYTES / 4 + 1);
        let payload = format!(r#"{{"session_id":"{over_cap}"}}"#);
        assert_eq!(
            parse_payload(payload.as_bytes()).expect_err("132 bytes is over the cap"),
            "oversized-session-id"
        );
    }

    /// Without a credential the run must stop before any socket work, and
    /// say so. This is the "agent launched outside farhelm" case: it is
    /// expected, not an error, and the log line is the only way to tell it
    /// apart from a hook that never ran at all.
    #[test]
    fn no_credential_stops_before_any_socket_work() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        let started = Instant::now();
        run_with(
            None,
            Cursor::new(CLAUDE_PAYLOAD.to_string().into_bytes()),
            TEST_BUDGET,
            Some(log.clone()),
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
    #[test]
    fn no_credential_never_touches_stdin() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        run_with(None, PanicOnRead, TEST_BUDGET, Some(log.clone()));
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
    #[test]
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
    #[test]
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
    #[test]
    fn consecutive_runs_append_whole_lines_in_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");

        run_with(
            None,
            Cursor::new(Vec::new()),
            TEST_BUDGET,
            Some(log.clone()),
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
    #[test]
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
    #[test]
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
    #[test]
    fn an_unanswering_supervisor_gives_up_at_the_budget() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        let socket = dir.path().join("supervisor.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let server = std::thread::spawn(move || {
            // Accept and hold the connection open without ever speaking,
            // then wait to be told the test is done. Both waits are
            // bounded (see `SERVER_DEADLINE`): this server's whole purpose
            // is to answer nothing, so an unbounded wait here would be a
            // thread designed to hang.
            let deadline = Instant::now() + SERVER_DEADLINE;
            let mut accepted = None;
            while Instant::now() < deadline {
                match listener.accept() {
                    Ok(connection) => {
                        accepted = Some(connection);
                        break;
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(SERVER_POLL);
                    }
                    Err(err) => panic!("accept failed: {err}"),
                }
            }
            let _ = stop_rx.recv_timeout(SERVER_DEADLINE);
            drop(accepted);
        });

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
        server.join().expect("server thread");
    }

    /// The whole point, end to end: a supervisor that completes the
    /// handshake and acknowledges gets the reported id and source, and the
    /// hook logs `acked`. Everything else in this module is a failure
    /// path; this is the one that has to work.
    #[test]
    fn a_completed_round_trip_reports_and_acks() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        let socket = dir.path().join("supervisor.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let (seen_tx, seen_rx) = mpsc::channel::<(String, String, Option<SessionAuth>)>();

        // The supervisor's side runs on its own thread with its own
        // current-thread runtime: `run_with` builds and owns the client
        // runtime, so the test cannot host both on this thread.
        let server = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("server runtime");
            runtime.block_on(async move {
                // Every milestone carries its own deadline. This server is
                // driven by the code under test, so a regression there —
                // a hook that never connects, never sends, or sends
                // something unreadable — would otherwise park this thread
                // forever and take the `join` below with it.
                let listener =
                    tokio::net::UnixListener::from_std(listener).expect("adopt listener");
                let (stream, _) = tokio::time::timeout(SERVER_DEADLINE, listener.accept())
                    .await
                    .expect("the hook must connect within the deadline")
                    .expect("accept");
                let (read, write) = tokio::io::split(stream);
                let mut reader = FrameReader::new(read);
                let mut writer = FrameWriter::new(write);
                let hello = tokio::time::timeout(
                    SERVER_DEADLINE,
                    farhelm_proto::io::handshake(&mut reader, &mut writer, "supervisor"),
                )
                .await
                .expect("the handshake must complete within the deadline")
                .expect("handshake");
                let auth = match hello {
                    ControlMsg::Hello { auth, .. } => auth,
                    other => panic!("expected a hello, got {other:?}"),
                };
                let frame = tokio::time::timeout(SERVER_DEADLINE, reader.read_frame())
                    .await
                    .expect("the report must arrive within the deadline")
                    .expect("read the report")
                    .expect("a frame, not EOF");
                match parse_control(&frame).expect("decode the report") {
                    ControlMsg::ReportConversation {
                        req_id,
                        conversation,
                        source,
                    } => {
                        let _ = seen_tx.send((conversation, source, auth));
                        tokio::time::timeout(
                            SERVER_DEADLINE,
                            writer.write_control(&ControlMsg::ConversationReported { req_id }),
                        )
                        .await
                        .expect("the acknowledgement must be written within the deadline")
                        .expect("acknowledge");
                    }
                    other => panic!("expected a report, got {other:?}"),
                }
            });
        });

        run_with(
            Some(HookCredential {
                session_id: "sess-1".to_string(),
                token: "tok-1".to_string(),
                socket,
            }),
            Cursor::new(CLAUDE_PAYLOAD.to_string().into_bytes()),
            Duration::from_secs(5),
            Some(log.clone()),
        );
        server.join().expect("server thread");

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

    /// The log directory is created on demand and kept private.
    ///
    /// There is ONE `hook-log/` directory under the supervisor's state
    /// directory, shared by every session, holding one FILE per session —
    /// not a directory per session. Whichever hook runs first anywhere on
    /// that supervisor creates it; every later one finds it. Its mode is
    /// what this pins: the files inside carry conversation ids and vendor
    /// error text, and they live beside the launch specs, so 0700 is the
    /// same boundary the rest of the state directory already keeps.
    #[test]
    fn a_missing_log_directory_is_created_private() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("hook-log").join("s.log");
        run_with(
            None,
            Cursor::new(Vec::new()),
            TEST_BUDGET,
            Some(log.clone()),
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
    #[test]
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
        );

        assert!(!log.exists(), "nothing should have been created");
    }

    /// A log grown past its cap is truncated before the next append, so a
    /// long-lived session cannot fill the state directory. Truncation is
    /// the deliberate choice over rotation (see `MAX_LOG_BYTES`); this
    /// test is what would catch a future "improvement" that silently
    /// removed the bound.
    #[test]
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
    #[test]
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
    #[test]
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
        );
        let line = single_line(&log);
        assert!(line.ends_with(" a_b_c d_e"), "line was {line:?}");
    }

    /// A payload larger than the cap is refused rather than truncated into
    /// something that might still parse. Truncated JSON would usually be
    /// unparsable, but "usually" is not a contract, and reporting a
    /// half-read id would be worse than reporting none.
    #[test]
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
        );
        assert!(single_line(&log).contains(" bad-payload oversized"));
    }

    /// [`announce`] writes the pointer and exactly one newline.
    ///
    /// "Exactly one line" is the contract the vendors' context injection
    /// is judged against: a second line is a second thing the model reads
    /// at the top of every session, and a missing newline runs the pointer
    /// into whatever the vendor appends after it.
    #[test]
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
    #[test]
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
    #[test]
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
    #[test]
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
