//! Agent-reported conversation identity: the `SessionStart` hook path, end
//! to end against a real supervisor.
//!
//! The record scan these tests sit next to
//! (`conversation_identity_capture`) infers identity by watching the agent
//! from outside, and it cannot see a conversation that starts mid-process:
//! Claude's `/clear` and Codex's `/new` mint a new id with no new session,
//! no new first input, and nothing to correlate against. The hook is the
//! answer — the agent tells us its id from inside its own process — and
//! these tests are where that claim is checked against the real machinery
//! rather than against a mock.
//!
//! ## What is real here, and the one thing that is not
//!
//! Everything downstream of the vendor is genuine: the `farhelm internal
//! hook` binary runs as a real child of the supervised process, dials the
//! supervisor's real unix socket, authenticates with the real credential
//! the launch injected into the agent's environment, and the supervisor
//! handles a real `ControlMsg::ReportConversation`. Only the TRIGGER is
//! faked — `Script::HookReport` fires the hook when a test types
//! `report <id>` instead of when a vendor decides a conversation started.
//! The `#[ignore]`d tests in `real_agent_capture` are what keep that last
//! step honest across vendor versions.
//!
//! ## Why these tests must `serve()`
//!
//! The suite's ordinary harness talks to the supervisor over an in-process
//! duplex pipe and never binds a socket. The hook cannot: it is a separate
//! process that only knows `FARHELM_SUPERVISOR_SOCK`. So [`hook_harness`]
//! spawns the real accept loop and waits for the bind before creating any
//! session — see its docs for the ordering that matters. The other two
//! places a hook has to reach a supervisor do the same thing for the same
//! reason (`real_agent_capture`'s `serving_supervisor` and
//! `restart_with_resume`'s hook-reported resume test), so "these tests
//! serve" is a property of the hook, not of this file.

use crate::boot_id_durable_outcome::listed;
use crate::conversation_identity_capture::{
    CaptureFixtures, assert_windows_disjoint, assert_windows_overlap, capture_harness,
    capture_harness_with_seams, marker_value, settle_past_horizon, snapshot_of,
    test_capture_bounds, wait_for_first_input, wait_until_window_disjoint_from,
};
use crate::harness::*;

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// Pane width every session here is created and attached at.
///
/// Far wider than the suite's usual 80 because these tests read the
/// fixture's `FAKE-AGENT ARGV:` marker, and that line carries two absolute
/// tempdir paths plus the injected `--settings` JSON — several hundred
/// characters. A pane narrower than the line would wrap it, and a replayed
/// wrap comes back as a real newline (`capture-pane` is run without `-J`),
/// so the assertions would read a truncated argv and fail for a reason
/// that has nothing to do with injection. [`argv_marker`] asserts the line
/// fit, so outgrowing this constant fails loudly instead of subtly.
const WIDE_COLS: u16 = 500;

/// Pane height; nothing here depends on it.
const ROWS: u16 = 24;

/// The marker the record-writing fixtures echo their own argv under.
const ARGV_MARKER: &str = "FAKE-AGENT ARGV:";

/// A capture harness whose supervisor is genuinely LISTENING on its unix
/// socket, so a hook child process can dial it.
///
/// The accept loop is started before any session exists, and this returns
/// only once it is bound — a session created against an unbound socket
/// would launch an agent whose hook has nowhere to report, and the failure
/// would look like a lost report rather than a race in the harness. See
/// [`ServeTask::spawn`] for both orderings.
///
/// Returns the [`ServeTask`] the caller must keep alive for as long as it
/// expects hooks to work.
async fn hook_harness() -> (Harness, CaptureFixtures, ServeTask) {
    let (h, fixtures) = capture_harness().await;
    let task = ServeTask::spawn(&h.sup, h.state.path()).await;
    (h, fixtures, task)
}

/// The supervisor's accept loop, stopped on drop and never silent about a
/// failure.
///
/// Drop-based because an assertion failure never reaches an explicit
/// teardown, and a leaked accept loop keeps an `Arc<Supervisor>` alive —
/// which matters beyond tidiness for the restart tests, where the whole
/// point is that the predecessor is genuinely gone before the successor is
/// constructed. Those tests call [`ServeTask::stop`] instead, because
/// `abort()` alone only REQUESTS cancellation: the task may still hold its
/// `Arc` when `drop` returns, and the drain loop would then spin forever.
///
/// The handle is held directly rather than in an `Option`: [`Self::stop`]
/// awaits it through `&mut` instead of moving it out, so there is no
/// "already taken" state for a reader to reason about.
pub(crate) struct ServeTask(tokio::task::JoinHandle<anyhow::Result<()>>);

impl ServeTask {
    /// Spawn `sup`'s accept loop and return once it is genuinely
    /// listening on `state`'s socket.
    ///
    /// Two orderings are load-bearing:
    ///
    /// 1. The caller must not have created any session yet. `serve()`
    ///    reloads the session map wholesale and replaces it, which is only
    ///    safe while no connection holds an attachment against an entry.
    /// 2. Readiness is raced against the TASK, not merely polled. A
    ///    `serve()` that fails to bind returns immediately, and a plain
    ///    poll would then spend its whole 20 s budget before reporting a
    ///    socket that was never going to appear — with the actual error
    ///    discarded. Racing turns that into the bind error itself.
    pub(crate) async fn spawn(sup: &Arc<Supervisor>, state: &std::path::Path) -> ServeTask {
        let serving = Arc::clone(sup);
        let mut task = tokio::spawn(async move { serving.serve().await });
        tokio::select! {
            finished = &mut task => panic!(
                "the supervisor's accept loop ended before it was ready to accept: {finished:?}"
            ),
            () = wait_for_supervisor_ready(state) => {}
        }
        ServeTask(task)
    }

    /// Stop accepting and wait until the task has actually released its
    /// supervisor reference, failing the test if it ended in any way other
    /// than the cancellation this asked for.
    pub(crate) async fn stop(mut self) {
        self.0.abort();
        // Awaited through `&mut` so `self` is still whole for `Drop`,
        // whose second `abort()` on a finished task is a no-op.
        let outcome = (&mut self.0).await;
        match outcome {
            // The expected end: the abort above landed.
            Err(joined) if joined.is_cancelled() => {}
            Err(joined) => panic!("the supervisor's accept loop panicked: {joined:?}"),
            // `serve()` returning at all means it stopped accepting on its
            // own — a bind or accept failure the tests would otherwise see
            // only as hooks mysteriously failing to connect.
            Ok(result) => result.expect("the supervisor's accept loop failed"),
        }
    }
}

impl Drop for ServeTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// An invocation string running `script` through the kind-named symlink, so
/// the supervisor derives the integration (and therefore hooks it) exactly
/// as it would for a real `claude` on the user's PATH.
fn fixture_invocation(fixtures: &CaptureFixtures, kind: &str, script: &str) -> String {
    format!(
        "{} internal fake-agent --script {script} --record-home {}",
        shell_words::quote(&fixtures.bin().join(kind).to_string_lossy()),
        shell_words::quote(&fixtures.home().to_string_lossy())
    )
}

/// Create a claude-kind session running the hook-reporting fixture.
async fn hook_session(
    h: &Harness,
    fixtures: &CaptureFixtures,
    cwd: &std::path::Path,
) -> SessionInfo {
    h.client
        .create_session(
            &cwd.to_string_lossy(),
            &fixture_invocation(fixtures, "claude", "hook-report"),
            None,
            WIDE_COLS,
            ROWS,
        )
        .await
        .expect("create a hook-reporting session")
}

/// Attach to a session and wait until its fixture is listening.
async fn attach_ready(h: &Harness, session: &SessionInfo) -> (u32, TermStream, Vec<u8>) {
    let (chan, mut rx) = h
        .client
        .attach(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    (chan, rx, seen)
}

/// Type one `report <conversation>` at the fixture and wait until the hook
/// child it spawned has exited.
///
/// Asserts the silence contract on every single report rather than only in
/// the test that is nominally about it: a hook that starts printing is a
/// user-visible defect in EVERY test's scenario, and the marginal cost of
/// checking here is one substring scan.
///
/// Every wait is anchored at the transcript length observed BEFORE the
/// input went out, and that is what makes a second report observable at
/// all. Two reports can legitimately name the same conversation (the
/// repeated-report test below, and a real vendor firing `SessionStart`
/// twice for an unchanged id), so a scan over the whole transcript would be
/// satisfied by the FIRST run's marker pair and return before the second
/// hook process had started — leaving the caller to assert against a
/// supervisor that had not yet been told anything.
///
/// Note what this does NOT prove: the hook exits 0 and silently whether the
/// supervisor accepted the report or refused it, by design (see
/// `crate::hook`'s contract). Only the caller's own assertion on the stored
/// identity proves the report LANDED — which is why every failure message
/// below quotes the hook's own log.
async fn report(
    h: &Harness,
    chan: u32,
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    conversation: &str,
) {
    let from = seen.len();
    h.client
        .send_input(chan, format!("report {conversation}\r").into_bytes())
        .await;
    wait_for_after_from(
        rx,
        seen,
        from,
        &format!("HOOK-REPORTED:{conversation}"),
        "HOOK-STDOUT-EMPTY",
        30,
    )
    .await;
    let text = String::from_utf8_lossy(&seen[from..]);
    assert!(
        !text.contains("HOOK-STDOUT-DIRTY"),
        "the hook wrote to a descriptor the vendor surfaces; transcript:\n{text}"
    );
    assert!(
        !text.contains("HOOK-EXIT:"),
        "the hook exited non-zero, which Claude shows the user as a hook error; \
         transcript:\n{text}"
    );
}

/// [`wait_for_after`], restricted to the transcript received from `from`
/// onwards.
///
/// The harness's own version scans everything accumulated so far, which is
/// right for a marker that can only appear once and wrong for the hook
/// markers: they repeat, by design, once per `report` line typed. `from` is
/// the caller's record of how much transcript existed before it sent the
/// input that should produce the next pair, so anchoring there is what
/// distinguishes "this run said so" from "a previous run did".
///
/// A `Detached` event ends the stream but is not itself a failure, for the
/// same reason [`wait_for`] tolerates it: the last output and the
/// pane-death notice race, so the needles are re-checked after the stream
/// ends and only then reported missing.
async fn wait_for_after_from(
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    from: usize,
    first: &str,
    then: &str,
    secs: u64,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut ended: Option<String> = None;
    loop {
        // Lossy, and sliced at a byte offset that may land mid-character:
        // this is a raw terminal stream, so it is not guaranteed to be
        // valid UTF-8 anywhere, and the markers under test are ASCII.
        let text = String::from_utf8_lossy(&seen[from.min(seen.len())..]).into_owned();
        if let Some(idx) = text.find(first)
            && text[idx + first.len()..].contains(then)
        {
            return;
        }
        if let Some(reason) = ended {
            panic!(
                "stream ended ({reason}) without {then:?} after {first:?}; transcript since the \
                 triggering input:\n{text}"
            );
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            // Presentation-only metadata, carrying no bytes for a text
            // scan to consider (see `wait_for`'s twin arm).
            Ok(Some(TermEvent::ReplayComplete)) => {}
            Ok(Some(TermEvent::Detached(reason))) => {
                while let Ok(TermEvent::Data(bytes)) = rx.try_recv() {
                    seen.extend_from_slice(&bytes);
                }
                ended = Some(reason);
            }
            Ok(None) => ended = Some("closed".to_string()),
            Err(_) => panic!(
                "timed out waiting for {then:?} after {first:?}; transcript since the \
                 triggering input:\n{text}"
            ),
        }
    }
}

/// This session's hook-log file contents, or a note saying it is absent.
///
/// Quoted into the failure message of every assertion about a report that
/// should have landed. The hook is silent by contract, so when a report
/// does not arrive this file is the ONLY evidence of why — whether it never
/// ran, could not connect, or was refused and by whom.
fn hook_log(h: &Harness, session_id: &str) -> String {
    let path = h
        .state
        .path()
        .join("hook-log")
        .join(format!("{session_id}.log"));
    match std::fs::read_to_string(&path) {
        Ok(text) => format!("hook log ({}):\n{text}", path.display()),
        Err(e) => format!("no hook log at {}: {e}", path.display()),
    }
}

/// One line per hook run, in the order the runs happened.
///
/// The log's whole contract is one line per run (`crate::hook`'s module
/// docs), which is what makes counting lines a way to count RUNS — the
/// only evidence a test has that a second `report` really started a second
/// hook process rather than being answered by the first one's markers.
fn hook_log_lines(h: &Harness, session_id: &str) -> Vec<String> {
    let path = h
        .state
        .path()
        .join("hook-log")
        .join(format!("{session_id}.log"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the hook must leave a trace at {}: {e}", path.display()));
    text.lines().map(str::to_string).collect()
}

/// The single line a hook log must hold after exactly one run, minus its
/// leading timestamp.
///
/// Used by the silence tests, which have no `Harness` — only the state
/// directory they pointed the hook's socket into. Asserting the line COUNT
/// here rather than in each caller is deliberate: every one of those tests
/// runs the binary exactly once, so a second line would mean the log had
/// stopped being one-line-per-run and every other assertion about it would
/// quietly start meaning something else.
fn sole_hook_log_outcome(state: &std::path::Path, session_id: &str) -> String {
    let path = state.join("hook-log").join(format!("{session_id}.log"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no hook log at {}: {e}", path.display()));
    assert_eq!(
        text.lines().count(),
        1,
        "exactly one line per run is what makes this file readable at all: {text}"
    );
    let line = text.lines().next().expect("just counted one line");
    line.split_once(' ')
        .expect("every log line is `<unix-seconds> <outcome> ...`")
        .1
        .to_string()
}

/// The argv the fixture echoed on its most recent launch.
///
/// The LAST occurrence, because a reattach after a relaunch replays the
/// previous run's markers too and the first one would answer for the wrong
/// generation.
///
/// The width assertion is not paranoia: a wrapped line comes back from
/// replay as a genuine newline, so this would silently return a prefix of
/// the argv and every "the injected flag is present" assertion would fail
/// with the flag simply missing. Failing on the width instead names the fix
/// (raise [`WIDE_COLS`]).
///
/// The MARKER's own width counts toward that bound. The fixture prints it
/// at column zero and the argv follows on the same row, so a bound that
/// measured only the argv would pass a line that had already wrapped by
/// exactly the marker's length — returning a suffix-truncated argv while
/// claiming it fit.
fn argv_marker(transcript: &[u8]) -> String {
    let text = String::from_utf8_lossy(transcript);
    let start = text
        .rfind(ARGV_MARKER)
        .unwrap_or_else(|| panic!("no {ARGV_MARKER} in transcript:\n{text}"))
        + ARGV_MARKER.len();
    let line = text[start..]
        .lines()
        .next()
        .expect("a marker is followed by at least a line ending")
        .trim_end_matches('\r')
        .to_string();
    assert!(
        ARGV_MARKER.chars().count() + line.chars().count() < WIDE_COLS as usize,
        "the argv line filled the pane and may have wrapped; raise WIDE_COLS: {line}"
    );
    line
}

/// The value of every `--settings` flag in an argv marker line, in order.
///
/// Splitting on whitespace is safe for this specific purpose because the
/// injected JSON is `serde_json`'s compact form and the values these tests
/// pass are single tokens; [`injected_settings`] is what handles the
/// general case. What this exists for is COUNTING and IDENTITY together: a
/// count alone cannot tell "the user's own flag survived" from "ours
/// replaced theirs", and those are opposite outcomes.
fn settings_values(argv: &str) -> Vec<String> {
    let words: Vec<&str> = argv.split_whitespace().collect();
    words
        .iter()
        .enumerate()
        .filter(|(_, word)| **word == "--settings")
        .map(|(i, _)| words.get(i + 1).copied().unwrap_or("<nothing>").to_string())
        .collect()
}

/// The injected `--settings` JSON, parsed out of an argv marker line.
///
/// Parsed with a streaming deserializer rather than by taking the rest of
/// the line: the marker joins the process's real argv with single spaces,
/// so everything after the JSON is another argv element, and the JSON's own
/// closing brace is the only reliable terminator. (A `--settings` value is
/// also not guaranteed space-free — the command inside it is a
/// shell-quoted path.)
///
/// Asserting on the parsed value rather than on the flag's presence is what
/// makes an injection test mean something: a launch carrying `--settings`
/// with a value Claude cannot read as a hook block is indistinguishable
/// from no injection at all, and fails the same silent way.
fn injected_settings(argv: &str) -> serde_json::Value {
    const FLAG: &str = "--settings ";
    let start = argv
        .find(FLAG)
        .unwrap_or_else(|| panic!("no injected --settings in: {argv}"))
        + FLAG.len();
    serde_json::Deserializer::from_str(&argv[start..])
        .into_iter::<serde_json::Value>()
        .next()
        .unwrap_or_else(|| panic!("--settings carried no value at all in: {argv}"))
        .unwrap_or_else(|e| panic!("the injected --settings value must be JSON ({e}): {argv}"))
}

/// Assert `settings` is a Claude settings document whose first
/// `SessionStart` hook runs farhelm's own hook command.
///
/// The command is checked for its `internal hook` subcommand rather than
/// by an exact string: the executable path is the test binary's, quoting is
/// the supervisor's business (`ClaudeIntegration::hook_argv` shell-quotes
/// it because Claude runs the command through a shell), and the tail may
/// carry `--announce` depending on the supervisor's instructions setting.
/// What matters here is that a launch which claims to be hooked really
/// would run the hook.
fn assert_declares_session_start_hook(settings: &serde_json::Value) {
    let hooks = &settings["hooks"]["SessionStart"];
    assert!(
        hooks.is_array() && !hooks.as_array().expect("just checked").is_empty(),
        "the injected settings must declare a SessionStart hook: {settings}"
    );
    let command = hooks[0]["hooks"][0]["command"]
        .as_str()
        .unwrap_or_else(|| panic!("the declared hook must carry a command string: {settings}"));
    assert!(
        command.contains("internal hook"),
        "the declared hook must run farhelm's own hook command: {command}"
    );
}

/// The stored row, read through a second connection to the live
/// supervisor's own database — the same bytes a restart would reload.
///
/// Needed because `conversation_source` is deliberately not on the wire:
/// the UI has no use for which writer set the identity (plan §2.7), so the
/// only place a test can observe the scan-versus-report distinction is the
/// column itself.
async fn stored_row(h: &Harness, session_id: &str) -> StoredSession {
    let store = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open the store directly");
    store
        .session(session_id)
        .await
        .expect("read the session row")
        .expect("the session exists")
}

/// The instant stored INSIDE a Claude record, as unix seconds.
///
/// This is the number capture windows are compared against — the record's
/// own header, written by the agent — so a test whose premise is "this
/// record lands in that window" has to read it rather than time the write
/// from outside. The two can differ: the fixture stamps the record before
/// it prints its marker, and a busy machine can put a second between them.
fn record_timestamp(home: &std::path::Path, cwd: &std::path::Path, conversation: &str) -> i64 {
    let canonical = std::fs::canonicalize(cwd).expect("canonicalize the working directory");
    let path = home
        .join(".claude")
        .join("projects")
        .join(farhelm_supervisor::agent_kind::munge_cwd(
            &canonical.to_string_lossy(),
        ))
        .join(format!("{conversation}.jsonl"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no record for {conversation} at {}: {e}", path.display()));
    let line = text
        .lines()
        .next()
        .unwrap_or_else(|| panic!("the record at {} is empty", path.display()));
    let value: serde_json::Value = serde_json::from_str(line)
        .unwrap_or_else(|e| panic!("the fixture writes JSONL ({e}): {line}"));
    let stamp = value["timestamp"]
        .as_str()
        .unwrap_or_else(|| panic!("a record carries an RFC 3339 timestamp: {line}"));
    farhelm_supervisor::agent_kind::parse_rfc3339(stamp)
        .unwrap_or_else(|| panic!("the supervisor's own parser must read {stamp:?}"))
}

// ---------------------------------------------------------------------
// The report as an identity
// ---------------------------------------------------------------------

/// The whole feature in one test: an agent that reports its conversation
/// id gets a working resume offer, with no record on disk and no scan
/// involved at all.
///
/// Specifies that a single `SessionStart` report is enough to move a
/// session from "no identity" to `RestartOffer::Resume` with the reported
/// id substituted into its resume argv — and that the hook process that
/// did it said nothing on stdout or stderr and exited 0, which is the
/// non-negotiable half of the contract (Claude feeds a `SessionStart`
/// hook's stdout to the model and shows its stderr to the user).
///
/// This session never writes a record, so nothing here could have come
/// from the scan: the identity is the agent's own answer or it is nothing.
#[tokio::test]
async fn a_reported_identity_is_offered_for_resume() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;

    report(&h, chan, &mut rx, &mut seen, "conv-1").await;

    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation.as_deref(),
        Some("conv-1"),
        "the reported identity must be durable before the hook is answered; {}",
        hook_log(&h, &session.id)
    );
    assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
    assert_eq!(
        snapshot.resume_argv.as_deref().unwrap().last().unwrap(),
        "conv-1",
        "the offer is only real if the id reaches the argv a restart would run"
    );
    assert!(
        !snapshot.capture_ambiguous,
        "a session that answered for itself is not ambiguous about anything"
    );
    serving.stop().await;
}

/// A second report REPLACES the first, and the replacement is what a
/// restart actually runs.
///
/// This is the case the whole feature exists for: `/clear` inside a running
/// Claude mints a new conversation in the same process, and the old
/// identity is precisely the one that must never be resumed again. Every
/// other capture state in this codebase is write-once for good reason, so
/// "a report may overwrite a report" is a deliberate exception worth
/// pinning.
///
/// The relaunch half proves two things a snapshot assertion cannot:
/// `--resume conv-2` shows the REPLACEMENT id was substituted into the
/// template, and the injected `--settings` element shows the restart path
/// hooks its launches too — so the resumed process can report again. A
/// resume that arrived unhooked would work exactly once and then go blind
/// at the next `/clear`.
///
/// The resume template here is this test's own rather than
/// `restart_with_resume::fixture_resume_template`: that one wraps the
/// fixture in `sh -c` and moves the substituted id into an environment
/// variable, which is what makes the id invisible in the argv marker. This
/// test needs it visible, and the fixture's `extra` catch-all is what makes
/// a bare `--resume <id>` acceptable to clap.
#[tokio::test]
async fn a_second_report_replaces_the_first() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let claude = fixtures.bin().join("claude").to_string_lossy().into_owned();
    let template = vec![
        claude,
        "internal".to_string(),
        "fake-agent".to_string(),
        "--script".to_string(),
        "hook-report".to_string(),
        "--record-home".to_string(),
        fixtures.home().to_string_lossy().into_owned(),
        "--resume".to_string(),
        farhelm_supervisor::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
    ];
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &fixture_invocation(&fixtures, "claude", "hook-report"),
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras {
                resume_template: Some(template),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create a hook-reporting session with an echoing resume template");

    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;
    report(&h, chan, &mut rx, &mut seen, "conv-1").await;
    report(&h, chan, &mut rx, &mut seen, "conv-2").await;

    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation.as_deref(),
        Some("conv-2"),
        "the newer report is the one a resume must land in; {}",
        hook_log(&h, &session.id)
    );
    assert_eq!(
        snapshot.resume_argv.as_deref().unwrap().last().unwrap(),
        "conv-2"
    );

    h.client
        .restart_session(&session.id, farhelm_proto::RestartMode::Resume, true)
        .await
        .expect("resume the running session");
    let (_chan, mut rx, mut seen) = attach_ready(&h, &session).await;
    // Anchored on `--resume `, not on the argv marker: a reattach replays
    // the terminal's history, so the PREVIOUS generation's `FAKE-AGENT
    // ARGV:` line is already in this buffer before the relaunched fixture
    // has printed a byte. Waiting for the marker would therefore return
    // instantly and read the wrong launch. `--resume ` appears only in the
    // resume template, so it is the one token that cannot be satisfied by
    // the replay of the create-time invocation.
    wait_for(&mut rx, &mut seen, "--resume ", 30).await;
    let argv = argv_marker(&seen);
    assert!(
        argv.contains("--resume conv-2"),
        "the resume must run the REPLACEMENT identity: {argv}"
    );
    // Parsed, not merely present: a `--settings` the vendor cannot read as
    // a hook block leaves the resumed process exactly as blind as no
    // injection would, and looks identical from a substring scan.
    assert_declares_session_start_hook(&injected_settings(&argv));
    serving.stop().await;
}

/// The SAME conversation reported twice is two hook runs, and the second
/// one is genuinely observed as the second.
///
/// This is a test about the test harness as much as the product, and it
/// earns its place because the failure it guards is invisible: every
/// assertion in this file is made after a [`report`] call returns, so a
/// wait that could be satisfied by a PREVIOUS run's markers would quietly
/// let those assertions run against a supervisor that had not been told
/// anything yet. A repeated id is the shape that exposes it — the marker
/// text is identical, so only position separates the two runs — and it is
/// not a contrived one: a vendor is free to fire `SessionStart` again for
/// an unchanged conversation, and the report has to remain a no-op rather
/// than a confusion.
///
/// The hook log is the second, independent witness: its contract is one
/// line per run, so two `acked` lines for one id mean two hook processes
/// really did dial the supervisor and be answered.
#[tokio::test]
async fn a_repeated_report_of_one_id_is_two_hook_runs() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;

    report(&h, chan, &mut rx, &mut seen, "conv-1").await;
    let after_first = seen.len();
    report(&h, chan, &mut rx, &mut seen, "conv-1").await;
    assert!(
        String::from_utf8_lossy(&seen[after_first..]).contains("HOOK-REPORTED:conv-1"),
        "the second report must be observed in transcript written after the first one \
         finished; transcript:\n{}",
        String::from_utf8_lossy(&seen)
    );

    let log = hook_log_lines(&h, &session.id);
    assert_eq!(
        log.len(),
        2,
        "one line per run is the log's whole contract, and two runs happened: {log:?}"
    );
    assert!(
        log.iter()
            .all(|line| line.split_whitespace().nth(1) == Some("acked")),
        "reporting an identity a session already holds is a no-op, not a refusal: {log:?}"
    );

    assert_eq!(
        snapshot_of(&h, &session.id)
            .await
            .captured_conversation
            .as_deref(),
        Some("conv-1"),
        "and the identity is unchanged by having been said twice; {}",
        hook_log(&h, &session.id)
    );
    serving.stop().await;
}

/// A record on disk cannot overwrite what the agent said about itself.
///
/// The scan is evidence ABOUT which conversation is ours; a report IS the
/// answer. So when one session produces both — a real record the scan can
/// see, and a later report naming something else — the report has to win,
/// permanently, and the scan's write has to become a no-op rather than a
/// race. The `/clear` case makes this the normal state of affairs, not a
/// corner: the record on disk is the pre-clear conversation and resuming it
/// would drop the user into the wrong history.
///
/// `conversation_source` is asserted directly because it is the only thing
/// that distinguishes "the report won" from "the scan happened to agree":
/// it is also what a supervisor restart reloads the state from, so a right
/// answer stored under the wrong provenance would come back wrong.
#[tokio::test]
async fn a_scan_cannot_override_a_report() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;

    h.client.send_input(chan, b"first prompt\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;
    let scanned = marker_value(&seen, "RECORD-WRITTEN:");
    report(&h, chan, &mut rx, &mut seen, "conv-other").await;
    assert_ne!(
        scanned, "conv-other",
        "the premise is that the two writers disagree"
    );

    settle_past_horizon(&h).await;

    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation.as_deref(),
        Some("conv-other"),
        "the scan's own record was on disk and in window, and still lost; {}",
        hook_log(&h, &session.id)
    );
    assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
    assert_eq!(
        stored_row(&h, &session.id).await.conversation_source,
        Some("hook".to_string()),
        "provenance is what a reload reads the capture state back from"
    );
    serving.stop().await;
}

/// A report that arrives BEFORE the scan's evidence does is not clobbered
/// when that evidence finally lands — in either of the two ways the scan
/// writes.
///
/// The orderings matter because they are the real ones. Claude fires its
/// hook at process START, so the report reliably beats the first record to
/// the supervisor; the scan then finds a record and would, without the
/// fence, overwrite an identity that is strictly better than what it
/// deduced. The second half covers the other write: ambiguity. Two sessions
/// in one directory poison each other's windows, and a pass that computed
/// "ambiguous" for a session and persisted it after a report had landed
/// would erase the report on disk while memory still advertised a resume —
/// a divergence that only shows up after a supervisor restart.
///
/// Both halves run in their own working directory: the second half's whole
/// point is a shared directory, and the first half's session must not be
/// dragged into it.
#[tokio::test]
async fn a_report_before_the_scan_lands_is_not_clobbered() {
    let (h, fixtures, serving) = hook_harness().await;

    // --- The plain scan write, reported before any record exists. ---
    let solo_work = farhelm_teststate::tempdir().expect("workdir");
    let solo = hook_session(&h, &fixtures, solo_work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &solo).await;
    report(&h, chan, &mut rx, &mut seen, "conv-early").await;
    h.client.send_input(chan, b"first prompt\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;

    settle_past_horizon(&h).await;
    assert_eq!(
        snapshot_of(&h, &solo.id)
            .await
            .captured_conversation
            .as_deref(),
        Some("conv-early"),
        "a record appearing after a report is later evidence, not better evidence; {}",
        hook_log(&h, &solo.id)
    );

    // --- The ambiguity write, reported before the horizon closes. ---
    let shared_work = farhelm_teststate::tempdir().expect("workdir");
    let reporter = hook_session(&h, &fixtures, shared_work.path()).await;
    let rival = hook_session(&h, &fixtures, shared_work.path()).await;
    let (chan_r, mut rx_r, mut seen_r) = attach_ready(&h, &reporter).await;
    let (chan_v, mut rx_v, mut seen_v) = attach_ready(&h, &rival).await;
    // Both first inputs go out together. The premise is asserted below
    // either way, but the correlator truncates to whole seconds and the
    // test window is short: sending sequentially puts a round trip
    // between the two anchors, which on a loaded machine is enough to
    // straddle a second boundary and turn the premise assertion into a
    // failure about nothing.
    tokio::join!(
        h.client.send_input(chan_r, b"first prompt\r".to_vec()),
        h.client.send_input(chan_v, b"first prompt\r".to_vec()),
    );
    wait_for(&mut rx_r, &mut seen_r, "RECORD-WRITTEN:", 20).await;
    wait_for(&mut rx_v, &mut seen_v, "RECORD-WRITTEN:", 20).await;
    let at_reporter = wait_for_first_input(&h, &reporter.id, 20).await;
    let at_rival = wait_for_first_input(&h, &rival.id, 20).await;
    assert_windows_overlap(at_reporter, at_rival);

    // Reported while both windows are still open, so a later pass has
    // every opportunity to compute ambiguity for this session and persist
    // it over the report.
    report(&h, chan_r, &mut rx_r, &mut seen_r, "conv-reported").await;
    settle_past_horizon(&h).await;

    let reported = snapshot_of(&h, &reporter.id).await;
    assert_eq!(
        reported.captured_conversation.as_deref(),
        Some("conv-reported"),
        "an ambiguity pass must not erase an answer the agent gave directly; {}",
        hook_log(&h, &reporter.id)
    );
    assert!(
        !reported.capture_ambiguous,
        "the row must not carry both an identity and the flag that says there is none"
    );
    assert_eq!(reported.restart_offer, farhelm_proto::RestartOffer::Resume);
    assert_eq!(
        stored_row(&h, &reporter.id).await.conversation_source,
        Some("hook".to_string())
    );

    let untouched = snapshot_of(&h, &rival.id).await;
    assert_eq!(
        untouched.captured_conversation, None,
        "the rival never reported anything, so its own ambiguity stands"
    );
    assert!(untouched.capture_ambiguous);
    assert_eq!(
        listed(&h.client, &rival.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );
    serving.stop().await;
}

/// A record another session has been TOLD is its own drops out of this
/// session's candidate list — which is the one thing a report buys a rival.
///
/// The scenario is the realistic one and not a contrivance: session A takes
/// its first input, later runs `/clear` (the fixture's `fork`, which mints a
/// new conversation record with a CURRENT timestamp), and reports the new
/// id. That fresh record lands squarely inside a much later session B's
/// capture window even though A's own window closed long ago. Without the
/// exclusion B sees two in-window candidates and bails; with it, B is left
/// with exactly the one record still unspoken for.
///
/// Both variants matter, and they fail differently:
///
/// - B has its own record: the exclusion turns a needless bail into an
///   honest capture. Losing it costs a resume offer.
/// - B has no record yet: the exclusion is what stops B from claiming A's
///   post-`/clear` conversation as its own. Losing it costs CORRECTNESS —
///   a session resuming somebody else's history, the exact failure the
///   whole capture design exists to exclude.
///
/// ## Why the windows are disjoint rather than overlapping
///
/// The candidate exclusion can only be observed with disjoint windows.
/// Sessions holding `Reported` deliberately stay in the pass's `occupied`
/// grouping (plan §2.5), and that grouping's overlap bail runs BEFORE any
/// scan — so an overlapping rival is declared ambiguous without its
/// candidate list ever being built. Disjoint windows plus a record minted
/// late is the only shape in which the filter decides anything.
#[tokio::test]
async fn a_reported_id_is_excluded_from_a_rivals_candidates() {
    let (h, fixtures, serving) = hook_harness().await;

    for rival_writes_a_record in [true, false] {
        // A private directory per variant: the two rivals would otherwise
        // poison each other's windows and both would bail for a reason
        // this test is not about.
        let work = farhelm_teststate::tempdir().expect("workdir");
        let reporter = hook_session(&h, &fixtures, work.path()).await;
        let (chan_a, mut rx_a, mut seen_a) = attach_ready(&h, &reporter).await;
        h.client
            .send_input(chan_a, b"first prompt\r".to_vec())
            .await;
        wait_for(&mut rx_a, &mut seen_a, "RECORD-WRITTEN:", 20).await;
        let at_reporter = wait_for_first_input(&h, &reporter.id, 20).await;

        wait_until_window_disjoint_from(at_reporter).await;

        // The rival: either the record-writing fixture or a claude-kind
        // session that takes input and writes nothing at all. `basic` is
        // what gives the second variant a session with a real capture
        // window and no record of its own — the state a session is in
        // between its first keystroke and its agent's first write.
        let rival = if rival_writes_a_record {
            hook_session(&h, &fixtures, work.path()).await
        } else {
            h.client
                .create_session(
                    &work.path().to_string_lossy(),
                    &fixture_invocation(&fixtures, "claude", "basic"),
                    None,
                    WIDE_COLS,
                    ROWS,
                )
                .await
                .expect("create a recordless claude-kind rival")
        };
        let (chan_b, mut rx_b, mut seen_b) = attach_ready(&h, &rival).await;
        h.client
            .send_input(chan_b, b"first prompt\r".to_vec())
            .await;
        let rival_conversation = if rival_writes_a_record {
            wait_for(&mut rx_b, &mut seen_b, "RECORD-WRITTEN:", 20).await;
            Some(marker_value(&seen_b, "RECORD-WRITTEN:"))
        } else {
            wait_for(&mut rx_b, &mut seen_b, "echo:", 20).await;
            None
        };
        let at_rival = wait_for_first_input(&h, &rival.id, 20).await;
        assert_windows_disjoint(at_reporter, at_rival);

        // A's `/clear`: a brand-new conversation record, minted now — which
        // is to say inside B's window and nowhere near A's. The premise is
        // asserted from the record's OWN stored timestamp, because that is
        // the value the correlator compares against a window; wall-clock
        // readings taken around the marker only bound when the fixture
        // said it was done, which is a different number on a loaded
        // machine and a different number again if the fixture ever stamps
        // its records any other way.
        h.client.send_input(chan_a, b"fork\r".to_vec()).await;
        wait_for(&mut rx_a, &mut seen_a, "RECORD-FORKED:", 20).await;
        let cleared = marker_value(&seen_a, "RECORD-FORKED:");
        let minted_at = record_timestamp(fixtures.home(), work.path(), &cleared);
        let rival_window = CaptureWindow::around(at_rival, test_capture_bounds());
        assert!(
            rival_window.contains(minted_at),
            "this test's premise is that the post-clear record lands in the rival's window \
             {rival_window:?}, but it is stamped {minted_at}"
        );
        report(&h, chan_a, &mut rx_a, &mut seen_a, &cleared).await;

        settle_past_horizon(&h).await;

        assert_eq!(
            snapshot_of(&h, &reporter.id).await.captured_conversation,
            Some(cleared.clone()),
            "the reporter holds the conversation it reported; {}",
            hook_log(&h, &reporter.id)
        );
        let rival_snapshot = snapshot_of(&h, &rival.id).await;
        assert_ne!(
            rival_snapshot.captured_conversation.as_deref(),
            Some(cleared.as_str()),
            "a rival may never claim a conversation another session was told is its own"
        );
        match rival_conversation {
            Some(own) => assert_eq!(
                rival_snapshot.captured_conversation.as_deref(),
                Some(own.as_str()),
                "with the spoken-for record filtered out the rival's own is the lone \
                 candidate, so it must capture rather than bail"
            ),
            None => {
                assert_eq!(
                    rival_snapshot.captured_conversation, None,
                    "a rival with no record of its own must stay uncaptured"
                );
                assert_eq!(
                    listed(&h.client, &rival.id).await.restart_offer,
                    farhelm_proto::RestartOffer::FreshOnly
                );
            }
        }
    }
    serving.stop().await;
}

/// A report clears an ambiguity that has already been declared and made
/// durable — the one place this design deliberately weakens an existing
/// guarantee.
///
/// Ambiguity is otherwise permanent for a launch, and rightly so: it means
/// the scan cannot tell two sessions' records apart, and no amount of
/// further scanning makes that better (see
/// `two_near_simultaneous_sessions_in_one_directory_stay_uncaptured`, whose
/// setup this reuses). A report is not more scan evidence, though — it is
/// the agent's own answer — so it must dominate. The guarantee existed
/// because scan evidence could not be trusted, and this is precisely the
/// input that is not scan evidence.
///
/// The rival is asserted to stay uncaptured in the same breath: one
/// session answering for itself says nothing about which record belongs to
/// the other, and a report that resolved the WHOLE group would be exactly
/// the guess this design refuses to make.
#[tokio::test]
async fn a_report_clears_ambiguity() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let first = hook_session(&h, &fixtures, work.path()).await;
    let second = hook_session(&h, &fixtures, work.path()).await;
    let (chan_1, mut rx_1, mut seen_1) = attach_ready(&h, &first).await;
    let (chan_2, mut rx_2, mut seen_2) = attach_ready(&h, &second).await;
    // Together, for the reason the sibling overlap test gives: a round
    // trip between the two anchors can straddle a second boundary, and
    // the premise here is that they land in the same window.
    tokio::join!(
        h.client.send_input(chan_1, b"first prompt\r".to_vec()),
        h.client.send_input(chan_2, b"first prompt\r".to_vec()),
    );
    wait_for(&mut rx_1, &mut seen_1, "RECORD-WRITTEN:", 20).await;
    wait_for(&mut rx_2, &mut seen_2, "RECORD-WRITTEN:", 20).await;
    let at_first = wait_for_first_input(&h, &first.id, 20).await;
    let at_second = wait_for_first_input(&h, &second.id, 20).await;
    assert_windows_overlap(at_first, at_second);

    // The ambiguity is DURABLE before the report, not merely pending: this
    // test is about overriding a decision that has already been written
    // down, which is the harder direction.
    settle_past_horizon(&h).await;
    for session in [&first, &second] {
        let snapshot = snapshot_of(&h, &session.id).await;
        assert!(
            snapshot.capture_ambiguous,
            "the premise is that both sessions bailed before either reported"
        );
        assert_eq!(snapshot.captured_conversation, None);
    }

    report(&h, chan_1, &mut rx_1, &mut seen_1, "conv-cleared").await;

    let cleared = snapshot_of(&h, &first.id).await;
    assert_eq!(
        cleared.captured_conversation.as_deref(),
        Some("conv-cleared"),
        "a report dominates a durable ambiguity; {}",
        hook_log(&h, &first.id)
    );
    assert!(
        !cleared.capture_ambiguous,
        "the flag must be cleared with the claim, or a reload contradicts the offer"
    );
    assert_eq!(cleared.restart_offer, farhelm_proto::RestartOffer::Resume);

    let still_ambiguous = snapshot_of(&h, &second.id).await;
    assert_eq!(
        still_ambiguous.captured_conversation, None,
        "one session's answer is not evidence about the other's"
    );
    assert!(still_ambiguous.capture_ambiguous);
    serving.stop().await;
}

/// A reported identity survives the supervisor that recorded it.
///
/// The whole reason capture is worth doing is the session that outlives its
/// supervisor — that is when a resume offer is the only way back into a
/// conversation. A report is held in memory as `CaptureState::Reported`,
/// and a successor rebuilds capture state from stored columns alone, so a
/// report that was never written down would simply be gone here.
///
/// What this can and cannot see is worth being exact about. The identity,
/// the offer, the filled resume argv and the stored provenance are all
/// observable, and all are asserted. The in-memory `CaptureState` the
/// successor rebuilt is NOT: nothing on the wire or in the snapshot
/// distinguishes a reloaded `Reported` from a reloaded scan claim, and
/// adding a seam to expose it would be instrumenting production for a test.
/// So this pins that the stored facts survive intact, not that the
/// successor classified them correctly — the classification is covered
/// where it is decided, in farhelm-supervisor's own reload tests.
///
/// The successor is deliberately built only after the predecessor has been
/// dropped and its accept loop stopped: an overlapping successor starts
/// read-only and reconciles nothing, so a test that skipped the drain would
/// exercise a path production never takes.
#[tokio::test]
async fn a_report_survives_a_supervisor_restart() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;
    report(&h, chan, &mut rx, &mut seen, "conv-durable").await;
    assert_eq!(
        snapshot_of(&h, &session.id)
            .await
            .captured_conversation
            .as_deref(),
        Some("conv-durable"),
        "{}",
        hook_log(&h, &session.id)
    );

    // `_tmux` LAST in the destructuring: these become ordinary locals that
    // drop in reverse declaration order, so listing the guard before
    // `state` would delete the state tempdir — and the socket the guard
    // kills through — before the guard ran, leaking the tmux server.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    serving.stop().await;
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);

    let restarted = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_home: Some(fixtures.home().to_path_buf()),
            capture_window: test_capture_bounds(),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("restarted supervisor");
    assert!(
        restarted.owns_state_dir(),
        "the predecessor must be gone, or this proves nothing"
    );

    let after = restarted
        .session_snapshot(&session.id)
        .await
        .expect("snapshot")
        .expect("present");
    assert_eq!(
        after.captured_conversation.as_deref(),
        Some("conv-durable"),
        "the successor reloaded the reported identity from the store alone"
    );
    assert_eq!(after.restart_offer, farhelm_proto::RestartOffer::Resume);
    assert_eq!(
        after.resume_argv.as_deref().unwrap().last().unwrap(),
        "conv-durable"
    );
    // The provenance the successor reloads FROM, read back after it did:
    // it is not on the wire, so the column is the only place a reload that
    // quietly downgraded a report to a scan claim could be seen at all.
    let store = SessionStore::open(&state.path().join("supervisor.db"), false)
        .await
        .expect("open the store directly");
    assert_eq!(
        store
            .session(&session.id)
            .await
            .expect("read the session row")
            .expect("the session exists")
            .conversation_source,
        Some("hook".to_string()),
        "a restart must not launder a report into a scan claim"
    );
}

/// A `Fresh` restart is REFUSED while a report stands, and the report
/// survives the refusal untouched.
///
/// Written to the behaviour the product actually has rather than to the
/// plan's wording ("report, restart `Fresh`, assert `FreshOnly`"): SPEC.md
/// has no fresh-restart variant, so `relaunch_argv` requires the mode to
/// match the CURRENT offer exactly and a `Fresh` request against a `Resume`
/// offer is a `Conflict`. A reported identity therefore cannot be forgotten
/// through the restart API at all — the only mode a reporting session can
/// be restarted in is `Resume`, which keeps it by design. The reset half of
/// the plan's intent (a non-`Resume` relaunch clearing
/// `conversation_source`) is pinned where it is reachable: the store's own
/// `begin_relaunch_clears_conversation_source_only_when_resetting_capture`.
///
/// What is worth pinning here is that a report joins the offer contract on
/// exactly the same terms a scan capture does — the sibling test for the
/// scan path is `restart_with_resume`'s stale-offer refusal. A report that
/// moved the offer without also moving what the offer is VALIDATED against
/// would let a client's cached `FreshOnly` blow away a live conversation,
/// which is precisely what the exact-match rule exists to prevent.
///
/// The identity is re-read afterwards because a refusal must be total:
/// `begin_relaunch` is what would have cleared the columns, and a refusal
/// that had already run it would leave the session with no identity and no
/// relaunch either.
#[tokio::test]
async fn a_fresh_restart_is_refused_while_a_report_stands() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;
    // What a client that listed at create time would have cached: no
    // identity yet, so nothing to resume.
    assert_eq!(
        session.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );
    report(&h, chan, &mut rx, &mut seen, "conv-standing").await;

    let err = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect_err("a fresh restart is not a legal answer to a session that can resume");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("a stale-offer refusal carries its classification");
    assert_eq!(err.kind, ErrorKind::Conflict);
    assert!(
        err.message.contains("resum"),
        "the refusal must name the CURRENT offer so the client can re-present it: {}",
        err.message
    );

    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation.as_deref(),
        Some("conv-standing"),
        "a refused restart must not have opened a relaunch generation"
    );
    assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
    assert_eq!(
        stored_row(&h, &session.id).await.conversation_source,
        Some("hook".to_string())
    );
    serving.stop().await;
}

// ---------------------------------------------------------------------
// Injection: when the flags are appended, and when they are not
// ---------------------------------------------------------------------

/// A user invocation that already passes `--settings` gets no injection.
///
/// Claude Code applies only the LAST `--settings` flag, so appending ours
/// after the user's would silently discard theirs — turning an identity
/// improvement into lost configuration, which is a strictly worse trade
/// than falling back to the record scan.
///
/// The assertion is on the surviving VALUE, not on a count of one: a merge
/// attempt that rewrote the user's settings in place would keep the count
/// at one while losing exactly what this test exists to protect.
#[tokio::test]
async fn hook_flags_are_not_injected_when_the_invocation_already_has_settings() {
    let (h, fixtures) = capture_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    // A value that is valid JSON, uniquely the user's, and one shell word:
    // an empty object configures nothing, so the launch behaves exactly as
    // an unhooked one while staying trivially identifiable in the argv.
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &format!(
                "{} --settings {{}}",
                fixture_invocation(&fixtures, "claude", "hook-report")
            ),
            None,
            WIDE_COLS,
            ROWS,
        )
        .await
        .expect("create a session whose invocation carries its own --settings");
    // The fixture prints its argv BEFORE the ready marker, so waiting for
    // readiness has already waited for the line.
    let (_chan, _rx, seen) = attach_ready(&h, &session).await;

    let argv = argv_marker(&seen);
    assert_eq!(
        settings_values(&argv),
        ["{}"],
        "the user's own --settings must survive unchanged and alone, or theirs is silently \
         dropped: {argv}"
    );
}

/// A generic session gets no hook flags at all.
///
/// Generic means the supervisor recognises no agent: no record location, no
/// resume template, and therefore no hook either — there is nothing to
/// report an identity TO. Injecting anyway would append vendor-specific
/// flags to an arbitrary command the user asked to run, which at best fails
/// to start and at worst does something else entirely.
///
/// The whole argv is compared against the request rather than checked for
/// the absence of `--settings`: the flags that would be wrong here are not
/// only Claude's, and a tail of Codex's shape (or any future kind's) would
/// pass an absence check while being exactly the defect. "Launched exactly
/// as asked" is the claim, so exactly-as-asked is what is asserted.
#[tokio::test]
async fn generic_sessions_get_no_hook_flags() {
    let (h, fixtures) = capture_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    // Launched under the binary's OWN name rather than through a
    // kind-named symlink, which is exactly what makes derivation call it
    // generic.
    let requested = [
        farhelm_bin().to_string(),
        "internal".to_string(),
        "fake-agent".to_string(),
        "--script".to_string(),
        "hook-report".to_string(),
        "--record-home".to_string(),
        fixtures.home().to_string_lossy().into_owned(),
    ];
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &requested
                .iter()
                .map(|word| shell_words::quote(word).into_owned())
                .collect::<Vec<_>>()
                .join(" "),
            None,
            WIDE_COLS,
            ROWS,
        )
        .await
        .expect("create a generic session");
    let (_chan, _rx, seen) = attach_ready(&h, &session).await;

    // Both sides are the same argv joined the same way — the marker joins
    // the process's REAL argv with single spaces — so this compares every
    // element and the count. What it deliberately cannot see is a word
    // boundary the shell moved without changing the characters, which is
    // not a failure mode injection has.
    assert_eq!(
        argv_marker(&seen),
        requested.join(" "),
        "a generic session has no integration and must be launched exactly as asked"
    );
    assert_eq!(
        snapshot_of(&h, &session.id).await.kind,
        farhelm_proto::AgentKind::Generic,
        "if this session were derived as Claude the assertion above would prove nothing"
    );
}

/// The per-kind opt-out is honoured at the point of injection.
///
/// Codex's hook needs `--dangerously-bypass-hook-trust`, which makes its
/// TUI print a warning line on every launch; a user who would rather have
/// the scan back needs a way to say so per kind, and that switch has to be
/// consulted where the flags are appended rather than anywhere they might
/// later be filtered.
///
/// Both halves run under ONE `Only(vec![Codex])` supervisor, and that is
/// what makes this a test of an allow-list rather than of an off switch: a
/// build that read the setting as "hooks are disabled" would pass the
/// claude half on its own. The codex session is the control — same
/// supervisor, same launch path, opposite outcome.
#[tokio::test]
async fn hooks_can_be_disabled_by_kind() {
    let (h, fixtures) = capture_harness_with_seams(|seams| {
        seams.agent_hooks =
            farhelm_supervisor::agent_kind::AgentHooks::Only(vec![farhelm_proto::AgentKind::Codex]);
    })
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");

    let excluded = hook_session(&h, &fixtures, work.path()).await;
    let (_chan, _rx, seen) = attach_ready(&h, &excluded).await;
    let argv = argv_marker(&seen);
    assert!(
        !argv.contains("--settings"),
        "claude is not in the allow-list, so its launch must be left alone: {argv}"
    );
    assert_eq!(
        snapshot_of(&h, &excluded.id).await.kind,
        farhelm_proto::AgentKind::Claude,
        "this must be the kind that was excluded, not an accidentally generic session"
    );

    // The allowed kind, through the `codex` symlink. The script it runs is
    // still the claude-shaped one — nothing here reads a record — because
    // what is under test is which flags the LAUNCH appended.
    let allowed = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &fixture_invocation(&fixtures, "codex", "hook-report"),
            None,
            WIDE_COLS,
            ROWS,
        )
        .await
        .expect("create a codex-kind session");
    let (_chan, _rx, seen) = attach_ready(&h, &allowed).await;
    let argv = argv_marker(&seen);
    assert!(
        argv.contains("--dangerously-bypass-hook-trust"),
        "codex IS in the allow-list, so its launch must carry the hook flags: {argv}"
    );
    assert_eq!(
        snapshot_of(&h, &allowed.id).await.kind,
        farhelm_proto::AgentKind::Codex,
        "and it must have derived as codex, or the assertion above proves nothing"
    );
}

// ---------------------------------------------------------------------
// The silence contract, asserted against the real binary
// ---------------------------------------------------------------------
//
// These run the built `farhelm internal hook` as a CHILD process, which is
// the only place the contract can be checked at all: the rules are about a
// process's stdout, stderr and exit status, and `src/` unit tests have
// neither a built binary nor descriptors of their own to inspect.
//
// The three environment values are set ON THE COMMAND, never on the test
// process. That is not merely this repo's rule about tests and the
// environment — a suite run from inside a real farhelm session already
// carries all three, so a test that exported its own would either dial a
// live supervisor or corrupt every sibling test's view of one.
// ---------------------------------------------------------------------

/// Longest any hook run may take before the test calls it hung.
///
/// Comfortably past the binary's own 2 s internal budget and comfortably
/// under the 5 s timeout the injected hook configuration hands the vendor:
/// a run that lands between those two numbers has already failed the thing
/// the budget exists for, which is never letting the VENDOR be the one to
/// time us out.
const SILENCE_DEADLINE: Duration = Duration::from_secs(4);

/// Kills and reaps the hook child on the way out, however the test leaves.
///
/// The whole point of the tests below is that the hook might NOT exit —
/// hung on a stdin nobody closes, or on a supervisor that never answers —
/// and a failing deadline assertion unwinds past any explicit cleanup. A
/// leaked hook child would then hold the state directory's socket path (and
/// its own pipes) open for as long as the test binary runs, with nothing
/// left to reap it. Killing an already-exited child is a harmless error,
/// which is why this makes no attempt to track whether the wait already
/// happened.
struct ChildGuard(std::process::Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Build a `farhelm internal hook` command carrying a session credential.
///
/// `.env` on the command, not `std::env::set_var`: see the section note
/// above for why that distinction is load-bearing here specifically.
fn hook_command(socket: &std::path::Path, session_id: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(farhelm_bin());
    cmd.args(["internal", "hook"])
        .env(farhelm_supervisor::launch::SESSION_ID_ENV_VAR, session_id)
        .env(
            farhelm_supervisor::launch::SESSION_TOKEN_ENV_VAR,
            "not-a-real-token",
        )
        .env(farhelm_supervisor::launch::SUPERVISOR_SOCK_ENV_VAR, socket)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    cmd
}

/// Spawn `cmd`, write `payload` to its stdin, and assert it finished
/// silently and successfully inside [`SILENCE_DEADLINE`].
///
/// `hold_stdin` keeps the write end OPEN for the whole wait, standing in
/// for a vendor that hands the hook a pipe and forgets about it. That is
/// not a hypothetical: the budget has to cover reading stdin precisely
/// because a blocking read cannot be cancelled, and this is the only way to
/// exercise that from outside.
///
/// Polled with `try_wait` rather than `wait` because the deadline is the
/// assertion: a blocking wait on a hung child would hang the test instead
/// of failing it.
fn assert_silent(mut cmd: std::process::Command, payload: &[u8], hold_stdin: bool) {
    use std::io::Write;
    let started = std::time::Instant::now();
    let mut child = ChildGuard(cmd.spawn().expect("spawn the hook binary"));
    // Kept in an `Option` so the write end can be released either here or
    // only after the wait: closing it is what gives the hook its EOF, and
    // `hold_stdin` is precisely the case where it must NOT get one.
    let mut stdin = Some(child.0.stdin.take().expect("piped stdin"));
    {
        let pipe = stdin.as_mut().expect("just installed");
        pipe.write_all(payload).expect("write the payload");
        pipe.flush().expect("flush the payload");
    }
    if !hold_stdin {
        stdin = None;
    }

    let status = loop {
        match child.0.try_wait().expect("poll the hook child") {
            Some(status) => break status,
            None => {
                assert!(
                    started.elapsed() < SILENCE_DEADLINE,
                    "the hook did not finish within {SILENCE_DEADLINE:?}; a run this long is \
                     one the vendor times out and shows the user"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    // Only now: reading the pipes before the child exits could block, and
    // the descriptors are still open until `child` drops.
    drop(stdin);
    let mut out = Vec::new();
    let mut err = Vec::new();
    std::io::Read::read_to_end(&mut child.0.stdout.take().expect("piped stdout"), &mut out)
        .expect("read stdout");
    std::io::Read::read_to_end(&mut child.0.stderr.take().expect("piped stderr"), &mut err)
        .expect("read stderr");

    assert_eq!(
        status.code(),
        Some(0),
        "the hook must always exit 0; a non-zero status is what makes the vendor show the \
         user a hook error"
    );
    assert!(
        out.is_empty(),
        "a SessionStart hook's stdout is injected into the model's context as text, so it \
         must be empty; got {:?}",
        String::from_utf8_lossy(&out)
    );
    assert!(
        err.is_empty(),
        "stderr is the agent's own terminal; got {:?}",
        String::from_utf8_lossy(&err)
    );
}

/// A hook whose supervisor socket does not exist says nothing, exits 0,
/// and leaves its explanation in the per-session log.
///
/// This is the ordinary failure: a supervisor that died, or a launch whose
/// state directory has moved. The hook has no descriptor it is allowed to
/// complain on, so silence is the only correct behaviour — and the log file
/// is the only place the failure is ever visible, which is exactly why it
/// is read rather than assumed.
///
/// The payload is deliberately WELL-FORMED. The hook rejects a bad payload
/// before it ever dials, so garbage here would produce a silent, successful
/// run that never touched a socket — passing this test without exercising
/// the failure it is named for. `connect-failed` in the log is what says
/// the dial was actually attempted and actually failed.
#[test]
fn a_hook_with_no_supervisor_is_silent_and_leaves_a_trace() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let socket = state.path().join("supervisor.sock");
    let payload =
        br#"{"session_id":"conv-missing","hook_event_name":"SessionStart","source":"startup"}"#;
    assert_silent(hook_command(&socket, "sess-missing"), payload, false);

    let outcome = sole_hook_log_outcome(state.path(), "sess-missing");
    assert!(
        outcome.starts_with("connect-failed "),
        "a socket that is not there must be logged as a failed dial: {outcome}"
    );
}

/// A supervisor that accepts the connection and then never answers cannot
/// hold the hook past its budget.
///
/// The nastiest reachable case, and the one a naive implementation gets
/// wrong: the dial succeeds, so nothing errors, and a hook that simply
/// waited for a reply would sit there until the vendor's own timeout fired
/// and reported a hook failure to the user. A wedged or overloaded
/// supervisor must degrade to "no identity this launch", never to a visible
/// error in someone's agent.
///
/// The payload here is deliberately WELL-FORMED: the hook rejects a bad
/// payload before it ever dials, so garbage would make this test pass
/// without a socket ever being touched.
///
/// Three things together are what make the scenario real rather than
/// merely quiet. The fixture ACCEPTS a connection, so the hook's dial
/// succeeds and nothing errors. It then holds that connection without
/// speaking, so the hook is waiting on a peer rather than on a closed
/// socket. And the log outcome is required to be a `timeout` — of the dial
/// or of the handshake — because those are the phases a wedged supervisor
/// can strand a hook in; any other outcome means this test stopped
/// reproducing the case it is named for.
#[test]
fn a_hook_talking_to_a_silent_supervisor_still_finishes_in_budget() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let socket = state.path().join("supervisor.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind a fake supervisor");
    // Non-blocking so the accept loop below is bounded: a thread parked
    // forever in `accept` could not be joined, and this test's own failure
    // path (the hook never dialled) is exactly when that would happen.
    listener
        .set_nonblocking(true)
        .expect("a bounded accept loop needs a non-blocking listener");
    let (dialled_tx, dialled_rx) = std::sync::mpsc::channel();
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut held = None;
        while std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = dialled_tx.send(());
                    held = Some(stream);
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return,
            }
        }
        // Held, not dropped: the peer must see an established connection
        // that simply never speaks. Dropping it would close the socket and
        // turn this into the ordinary connect-failure case, which the
        // sibling test above already covers.
        let _ = stop_rx.recv_timeout(Duration::from_secs(30));
        drop(held);
    });

    let payload =
        br#"{"session_id":"conv-hung","hook_event_name":"SessionStart","source":"startup"}"#;
    assert_silent(hook_command(&socket, "sess-hung"), payload, false);

    dialled_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the hook must have dialled the socket, or nothing here was silent AT it");
    let outcome = sole_hook_log_outcome(state.path(), "sess-hung");
    assert!(
        outcome.starts_with("timeout "),
        "a supervisor that accepts and then says nothing must strand the hook in a phase it \
         times out of: {outcome}"
    );

    let _ = stop_tx.send(());
    holder.join().expect("the holding thread must not panic");
}

/// A vendor that never closes the hook's stdin cannot hold it past its
/// budget either.
///
/// The budget deliberately covers READING the payload, and this is the
/// reason: a blocking read cannot be interrupted by any timeout, so an
/// implementation that bounded only the socket round trip would hang here
/// forever while every diagnostic said it had a timeout. The test holds the
/// write end open for the whole wait, which is the only way to reproduce
/// that from outside the process.
///
/// The logged phase is asserted, not just the exit: the payload here is a
/// fragment, so a hook that gave up for any OTHER reason would also finish
/// silently and in budget. `timeout stdin` is what says the budget stopped
/// a read that was still blocked, which is the whole claim.
#[test]
fn a_hook_whose_stdin_is_never_closed_still_finishes_in_budget() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let socket = state.path().join("supervisor.sock");
    assert_silent(hook_command(&socket, "sess-open"), b"partial", true);

    assert_eq!(
        sole_hook_log_outcome(state.path(), "sess-open"),
        "timeout stdin",
        "the budget must have expired in the READ, not anywhere later"
    );
}

/// Run outside farhelm entirely, the hook does nothing at all — silently.
///
/// The reachable shape is a user who copied a hooked invocation out of
/// their profile, or a `--settings` file that outlived the launch that
/// wrote it. There is no supervisor to report to and no session to report
/// about, so the only acceptable behaviour is to leave no trace on any
/// descriptor and get out of the agent's way.
#[test]
fn a_hook_outside_a_farhelm_session_does_nothing_silently() {
    let mut cmd = std::process::Command::new(farhelm_bin());
    cmd.args(["internal", "hook"])
        // Removed on the COMMAND, because the test process may itself be
        // running inside a farhelm session and would otherwise pass a live
        // credential down to the child.
        .env_remove(farhelm_supervisor::launch::SESSION_ID_ENV_VAR)
        .env_remove(farhelm_supervisor::launch::SESSION_TOKEN_ENV_VAR)
        .env_remove(farhelm_supervisor::launch::SUPERVISOR_SOCK_ENV_VAR)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    assert_silent(
        cmd,
        br#"{"session_id":"conv-x","hook_event_name":"SessionStart"}"#,
        false,
    );
}

/// The exact bytes an announcing hook must produce, newline included.
///
/// Spelled out here rather than imported, and the duplication is the
/// point twice over. `farhelm` is a binary crate with no library target,
/// so `hook::POINTER_LINE` is not reachable from a test process at all —
/// but even if it were, importing it would turn this assertion into "the
/// binary printed whatever the binary says", which proves nothing about
/// the contract. What a vendor splices into a model's context is a
/// sequence of bytes, and this is the test that reads them from outside
/// the process that wrote them. A change to the line must be made in both
/// places, on purpose.
const EXPECTED_POINTER: &str = "farhelm: when the user writes \"$farhelm ...\", run `farhelm agent instructions` and \
     follow its output.\n";

/// With `--announce`, the hook prints exactly the pointer line on stdout,
/// nothing on stderr, and still exits 0 inside the budget.
///
/// The pointer is the only thing farhelm deliberately makes visible from
/// inside a session, and every property asserted here is one the vendors
/// key on. Both Claude Code and Codex feed a `SessionStart` hook's
/// plain-text stdout into the model's context, so a stray second line is
/// text the model reads at the top of every session; both surface stderr
/// on failure, so a byte there is the user's problem; and both bound the
/// hook with a timeout of their own, so a run that slows down to say
/// something is a run they report as broken.
///
/// The supervisor socket deliberately does not exist. That makes the
/// identity half FAIL — which is the point: the pointer is not conditional
/// on the report landing, because a session whose supervisor is wedged is
/// exactly a session whose agent may need to ask farhelm what is going on.
/// The log line is read to prove the run really did take the failing path
/// rather than skipping the socket entirely.
#[test]
fn an_announcing_hook_prints_exactly_the_pointer_line() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let socket = state.path().join("supervisor.sock");
    let mut cmd = hook_command(&socket, "sess-announce");
    cmd.arg("--announce");

    let started = std::time::Instant::now();
    let output = run_hook(cmd, br#"{"session_id":"conv-a","source":"startup"}"#);
    assert!(
        started.elapsed() < SILENCE_DEADLINE,
        "an announcing hook took {:?}; the pointer must not cost the budget",
        started.elapsed()
    );

    assert_eq!(output.status.code(), Some(0), "the hook must always exit 0");
    assert!(
        output.stderr.is_empty(),
        "stderr is the agent's own terminal; got {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("the pointer is ASCII");
    assert_eq!(
        stdout, EXPECTED_POINTER,
        "stdout must be the pointer and nothing else"
    );

    let outcome = sole_hook_log_outcome(state.path(), "sess-announce");
    assert!(
        outcome.starts_with("connect-failed "),
        "the identity half must still have run and failed on the absent socket: {outcome}"
    );
}

/// Without `--announce` the same run says nothing at all.
///
/// The negative half of the pair, and it is not redundant with the silence
/// tests above: those predate the flag and would keep passing if
/// `--announce` were wired to default-on, which is precisely the mistake
/// that would put a line into every session of every user who turned
/// instructions off. Same fixture as the announcing case, one argument
/// apart, so the only thing that can explain a difference is the flag.
#[test]
fn a_hook_without_announce_prints_nothing() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let socket = state.path().join("supervisor.sock");
    let output = run_hook(
        hook_command(&socket, "sess-silent"),
        br#"{"session_id":"conv-b","source":"startup"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stdout.is_empty(),
        "an unannounced hook must print nothing: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stderr.is_empty());
}

/// Spawn `cmd`, feed it `payload`, close stdin, and collect what it said.
///
/// [`assert_silent`]'s sibling for the cases where output is the point
/// rather than the defect: it cannot be reused, because it asserts
/// emptiness. Stdin is closed immediately (no `hold_stdin` equivalent)
/// because these cases are about the pointer, and a hook that never gets
/// EOF spends its budget in the read — which is covered by its own test
/// above.
///
/// Polls with `try_wait` under [`SILENCE_DEADLINE`], killing and reaping
/// on the way out, for the reason that helper does: a blocking wait on a
/// wedged child hangs the run instead of failing it, and a leaked hook
/// child would hold the state directory open for the rest of the suite.
fn run_hook(mut cmd: std::process::Command, payload: &[u8]) -> std::process::Output {
    use std::io::Write;
    let mut child = cmd.spawn().expect("spawn the hook binary");
    {
        // Dropped at the end of this block, which is what gives the hook
        // its EOF. Holding it open is a different test (see
        // `a_hook_whose_stdin_is_never_closed_still_finishes_in_budget`).
        let mut pipe = child.stdin.take().expect("piped stdin");
        pipe.write_all(payload).expect("write the payload");
    }
    let deadline = std::time::Instant::now() + SILENCE_DEADLINE;
    loop {
        if child.try_wait().expect("poll the hook child").is_some() {
            return child.wait_with_output().expect("collect hook output");
        }
        if std::time::Instant::now() >= deadline {
            child.kill().expect("kill the wedged hook");
            let output = child.wait_with_output().expect("collect the killed hook");
            panic!(
                "the hook did not finish within {SILENCE_DEADLINE:?}; stderr was {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
