//! Agent-reported conversation identity: the `SessionStart` hook path, end
//! to end against a real supervisor.
//!
//! The agent reports its own identity, including a conversation created by
//! `/clear` or `/new` inside an existing process. These tests exercise that
//! report through the real launch credential, socket, handler, and store.
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
//! session — see its docs for the ordering that matters. Every suite that
//! reports through the hook must bind an accept loop before creating its
//! session; "these tests serve" is a property of the hook, not of this file.

use crate::harness::*;
use farhelm_teststate::thread::FixtureThread;

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

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
pub(crate) async fn hook_harness() -> (Harness, CaptureFixtures, ServeTask) {
    let (h, fixtures) = fixture_harness_with_seams(|_| {}).await;
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
        "{} fake-agent --script {script} --record-home {}",
        shell_words::quote(&fixtures.bin().join(kind).to_string_lossy()),
        shell_words::quote(&fixtures.home().to_string_lossy())
    )
}

/// Create a claude-kind session running the hook-reporting fixture.
pub(crate) async fn hook_session(
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
///
/// The returned transcript includes initial replay and the fixture's readiness
/// witness. Its stream has already consumed those setup reads and the replay
/// marker, so callers retain startup evidence without waiting for that marker
/// again.
pub(crate) async fn attach_ready(h: &Harness, session: &SessionInfo) -> (u32, TermStream, Vec<u8>) {
    let (chan, mut seen, mut rx) = h
        .client
        .attach_live(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach");
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
pub(crate) async fn report(
    h: &Harness,
    chan: u32,
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    conversation: &str,
) {
    report_client(&h.client, chan, rx, seen, conversation).await;
}

/// Send an explicit identity report through an already connected client.
///
/// This variant keeps report-driven restart tests that construct supervisors
/// by hand on the same acceptance and silence checks as the shared harness.
pub(crate) async fn report_client(
    client: &SupervisorClient,
    chan: u32,
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    conversation: &str,
) {
    let from = seen.len();
    client
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
pub(crate) async fn wait_for_after_from(
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
            Ok(Some(TermEvent::Detached(detach))) => {
                while let Ok(TermEvent::Data(bytes)) = rx.try_recv() {
                    seen.extend_from_slice(&bytes);
                }
                ended = Some(detach.reason);
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
pub(crate) fn hook_log(h: &Harness, session_id: &str) -> String {
    hook_log_at(h.state.path(), session_id)
}

/// Read a hook log when a test owns a hand-built supervisor state directory.
pub(crate) fn hook_log_at(state: &std::path::Path, session_id: &str) -> String {
    let path = state.join("hook-log").join(format!("{session_id}.log"));
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

/// The value of every `--settings` flag in an argv marker line, in order.
///
/// Decode the fixture's shell-word serialization before looking for flags:
/// JSON values can contain spaces and their quoting is not part of argv.
/// What this exists for is COUNTING and IDENTITY together: a
/// count alone cannot tell "the user's own flag survived" from "ours
/// replaced theirs", and those are opposite outcomes.
fn settings_values(argv: &str) -> Vec<String> {
    let words = shell_words::split(argv).expect("fake agent printed shell-safe argv");
    words
        .iter()
        .enumerate()
        .filter(|(_, word)| **word == "--settings")
        .map(|(i, _)| words.get(i + 1).expect("--settings has a value").clone())
        .collect()
}

/// The injected `--settings` JSON, parsed out of an argv marker line.
///
/// The marker preserves real argv boundaries with shell-word quoting.
/// Decode those boundaries first, then parse the entire settings argument;
/// trailing malformed bytes must not be accepted as a valid hook document.
///
/// Asserting on the parsed value rather than on the flag's presence is what
/// makes an injection test mean something: a launch carrying `--settings`
/// with a value Claude cannot read as a hook block is indistinguishable
/// from no injection at all, and fails the same silent way.
fn injected_settings(argv: &str) -> serde_json::Value {
    let values = settings_values(argv);
    let value = values
        .first()
        .unwrap_or_else(|| panic!("no injected --settings in: {argv}"));
    serde_json::from_str(value)
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
/// column itself is where tests can verify report provenance.
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
async fn a_second_report_replaces_the_first() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let claude = fixtures.bin().join("claude").to_string_lossy().into_owned();
    let template = vec![
        claude,
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

/// A shelled-out child that inherited the session credential and fires its
/// own reporting hook cannot replace the foreground's conversation: the
/// supervisor refuses it, and the foreground's report stays the restart
/// target.
///
/// Why this test matters: this is the overwrite Claude's positional
/// admission rule exists to stop. A `claude` run from the foreground's Bash
/// tool inherits `FARHELM_SESSION_TOKEN` along with the rest of the
/// environment, and if it picks up a reporting hook (a user- or
/// project-level settings hook, say) its `SessionStart` names a different
/// conversation. Before the rule, credential and id shape were all
/// admission asked for, so the child's id silently became what a restart
/// would resume.
///
/// The fixture's nested shape is pinned rather than assumed: the child's
/// parent is a live `sh` (it printed so), which puts the child two links
/// below the pane. The refusal is read from the hook log, not inferred from
/// the unchanged identity alone, because the hook is silent either way and
/// "not stored" would also pass if the child had never reached the
/// supervisor at all.
#[farhelm_testtrace::test]
async fn a_shelled_out_child_cannot_replace_the_foreground_report() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;

    report(&h, chan, &mut rx, &mut seen, "conv-parent").await;

    let from = seen.len();
    h.client
        .send_input(chan, b"nested-report conv-child\r".to_vec())
        .await;
    wait_for_after_from(
        &mut rx,
        &mut seen,
        from,
        "NESTED-PARENT:",
        // The whole success marker, not a prefix: terminal output arrives
        // in arbitrary chunks, and a wait satisfied mid-marker would fail
        // the assertions below for a run that actually succeeded.
        "NESTED-REPORT-DONE:conv-child",
        30,
    )
    .await;
    let text = String::from_utf8_lossy(&seen[from..]).into_owned();
    // `ps -o comm=` prints the bare name on Linux and argv[0] with its
    // path on macOS (`/bin/sh`), so only the basename is compared.
    let parent = marker_value(&seen[from..], "NESTED-PARENT:");
    assert_eq!(
        parent.rsplit('/').next(),
        Some("sh"),
        "the child must have been run by a live shell below the pane, or the premise of the \
         refusal is gone (parent {parent:?}); transcript:\n{text}"
    );
    assert!(
        text.contains("HOOK-REPORTED:conv-child") && text.contains("HOOK-STDOUT-EMPTY"),
        "a refused hook still finishes silently; transcript:\n{text}"
    );

    let log = hook_log_lines(&h, &session.id);
    let child = log
        .iter()
        .find(|line| line.contains(" conv-child "))
        .unwrap_or_else(|| panic!("the child's hook must have left a log line: {log:?}"));
    assert!(
        child.contains(" refused conflict ") && child.contains("nested below"),
        "the supervisor must have refused the child for its ancestry: {child}"
    );
    assert!(
        log.iter()
            .any(|line| line.contains(" acked ") && line.contains(" conv-parent ")),
        "the foreground's own report must have been acked: {log:?}"
    );

    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation.as_deref(),
        Some("conv-parent"),
        "the refused child must not replace the foreground's identity; {}",
        hook_log(&h, &session.id)
    );
    assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
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
#[farhelm_testtrace::test]
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

/// A reported identity survives the supervisor that recorded it.
///
/// The whole reason capture is worth doing is the session that outlives its
/// supervisor — that is when a resume offer is the only way back into a
/// conversation. A report is held in memory as `CaptureState::Reported`,
/// and a successor rebuilds capture state from stored columns alone, so a
/// report that was never written down would simply be gone here.
///
/// The identity, offer, filled Resume argv, and stored report provenance must
/// all survive reconstruction. The row is also read directly because the
/// public snapshot deliberately omits where an identity originally came from.
///
/// The successor is deliberately built only after the predecessor has been
/// dropped and its accept loop stopped: an overlapping successor starts
/// read-only and reconciles nothing, so a test that skipped the drain would
/// exercise a path production never takes.
#[farhelm_testtrace::test]
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
        // sleep-ok: observe connection-reference drain before dropping the old supervisor.
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);

    let restarted = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams::default(),
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
    // it is not on the wire, so inspect the stored source directly.
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
        "a restart must preserve the stored report source"
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
/// An accepted report is part of the offer contract. A report that
/// moved the offer without also moving what the offer is VALIDATED against
/// would let a client's cached `FreshOnly` blow away a live conversation,
/// which is precisely what the exact-match rule exists to prevent.
///
/// The identity is re-read afterwards because a refusal must be total:
/// `begin_relaunch` is what would have cleared the columns, and a refusal
/// that had already run it would leave the session with no identity and no
/// relaunch either.
#[farhelm_testtrace::test]
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
/// than offering a fresh restart when no identity has been reported.
///
/// The assertion is on the surviving VALUE, not on a count of one: a merge
/// attempt that rewrote the user's settings in place would keep the count
/// at one while losing exactly what this test exists to protect.
#[farhelm_testtrace::test]
async fn hook_flags_are_not_injected_when_the_invocation_already_has_settings() {
    let (h, fixtures) = fixture_harness_with_seams(|_| {}).await;
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
#[farhelm_testtrace::test]
async fn generic_sessions_get_no_hook_flags() {
    let (h, fixtures) = fixture_harness_with_seams(|_| {}).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    // Launched under the binary's OWN name rather than through a
    // kind-named symlink, which is exactly what makes derivation call it
    // generic.
    let requested = [
        fixtures_bin().to_string(),
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
/// TUI print a warning line on every launch; a user who would rather run
/// without that hook needs a way to say so per kind, and that switch has to be
/// consulted where the flags are appended rather than anywhere they might
/// later be filtered.
///
/// Both halves run under ONE `Only(vec![Codex])` supervisor, and that is
/// what makes this a test of an allow-list rather than of an off switch: a
/// build that read the setting as "hooks are disabled" would pass the
/// claude half on its own. The codex session is the control — same
/// supervisor, same launch path, opposite outcome.
#[farhelm_testtrace::test]
async fn hooks_can_be_disabled_by_kind() {
    let (h, fixtures) = fixture_harness_with_seams(|seams| {
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
/// The child-only budget is intentionally 5 s so absent-socket cases can
/// exercise their 4 s reconnect cap without stretching the suite by half a
/// minute. Keep two seconds of margin here for spawning a debug binary and
/// building its runtime under a loaded runner; the margin, rather than the
/// hook budget, absorbs that scheduling cost.
const SILENCE_DEADLINE: Duration = Duration::from_secs(7);

/// Keep real-binary hook cases bounded without changing the test runner's
/// environment. The override is placed on each spawned hook command below.
const TEST_HOOK_BUDGET_MS: &str = "5000";

/// Owns a silent supervisor fixture and witnesses its connection and release edges.
struct SilentSupervisor {
    owner: FixtureThread,
    stop: std::sync::mpsc::Sender<()>,
    accepted: std::sync::mpsc::Receiver<()>,
    released: std::sync::mpsc::Receiver<()>,
}

/// Keep an accepted peer silent until cancellation or the safety deadline.
///
/// The listener must be nonblocking so cancellation also works before a connection arrives.
/// Keeping the accepted peer open makes the hook wait for a handshake; closing it would
/// turn the test into the connection-failure case covered by its sibling. The captured
/// trace remains owned through the holder thread's cleanup.
fn spawn_silent_supervisor(
    listener: std::os::unix::net::UnixListener,
    context: farhelm_testtrace::ThreadContext,
) -> SilentSupervisor {
    let (stop, stop_rx) = std::sync::mpsc::channel();
    let (accepted_tx, accepted) = std::sync::mpsc::channel();
    let (released_tx, released) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        context.enter(|| {
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            let mut held = None;
            while std::time::Instant::now() < deadline {
                // Cancellation can arrive before the hook dials. Poll it before
                // every accept attempt so cleanup does not wait for the deadline.
                match stop_rx.try_recv() {
                    Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        drop(listener);
                        let _ = released_tx.send(());
                        return;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = accepted_tx.send(());
                        held = Some(stream);
                        break;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        // sleep-ok: retry nonblocking accept while checking cancellation and the deadline.
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("accept failed: {error}"),
                }
            }
            let _ = stop_rx.recv_timeout(Duration::from_secs(30));
            drop(held);
            drop(listener);
            let _ = released_tx.send(());
        });
    });
    let cancellation = stop.clone();
    let owner = FixtureThread::new("hook-silent-supervisor", holder, move || {
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
    // The silence tests exercise the Claude entry point; the flag is
    // required since the envelope migration, and an invocation without it
    // fails closed at CLI parse rather than reporting untagged.
    cmd.args(["internal", "hook", "--vendor", "claude"])
        .env("FARHELM_TEST_HOOK_BUDGET_MS", TEST_HOOK_BUDGET_MS)
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
fn assert_silent(mut cmd: std::process::Command, payload: &[u8], hold_stdin: bool) -> Duration {
    use std::io::Write;
    let started = std::time::Instant::now();
    let mut child = ChildGuard(cmd.spawn().expect("spawn the hook binary"));
    // Kept in an `Option` so the write end can be released either here or
    // only after the wait: closing it is what gives the hook its EOF, and
    // `hold_stdin` is precisely the case where it must NOT get one.
    let mut stdin = Some(child.0.stdin.take().expect("piped stdin"));
    {
        let pipe = stdin.as_mut().expect("just installed");
        // A hook with nothing to do exits without reading its stdin, and on a
        // loaded box it can be gone before this write lands: the read end is
        // closed and the write fails with EPIPE (observable as an error rather
        // than a signal, since Rust ignores SIGPIPE). That is the silent early
        // exit the callers are asserting on, not a failed write, so it is
        // tolerated here and the exit status and captured output below decide
        // the test. Seen twice in six loaded full-binary runs on a 4-vCPU
        // sandbox on 2026-09-03; never on a developer machine, where the
        // write always wins.
        match pipe.write_all(payload).and_then(|()| pipe.flush()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
            Err(error) => panic!("write the payload: {error}"),
        }
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
                    "the hook did not finish within {SILENCE_DEADLINE:?}; it overran its test budget"
                );
                // sleep-ok: poll child exit while respecting the caller's chosen stdin lifetime.
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
    started.elapsed()
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
/// the dial was actually attempted and actually failed. The elapsed assertion
/// below also pins that the shipped binary used its production reconnect
/// window rather than a zero-cap test seam.
#[farhelm_testtrace::test]
fn a_hook_with_no_supervisor_is_silent_and_leaves_a_trace() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let socket = state.path().join("supervisor.sock");
    let payload =
        br#"{"session_id":"conv-missing","hook_event_name":"SessionStart","source":"startup"}"#;
    let started = std::time::Instant::now();
    assert_silent(hook_command(&socket, "sess-missing"), payload, false);
    assert!(
        started.elapsed() >= Duration::from_secs(3),
        "a missing supervisor must consume the production reconnect window: {:?}",
        started.elapsed()
    );

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
#[farhelm_testtrace::test]
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
    // The holder is a raw thread, so the wrapped test explicitly gives it
    // ownership of this test's trace while it keeps the peer alive.
    let context = farhelm_testtrace::current_thread_context().expect("test trace context");
    let SilentSupervisor {
        owner: holder,
        stop: stop_tx,
        accepted: dialled_rx,
        released,
    } = spawn_silent_supervisor(listener, context);

    let payload =
        br#"{"session_id":"conv-hung","hook_event_name":"SessionStart","source":"startup"}"#;
    let elapsed = assert_silent(hook_command(&socket, "sess-hung"), payload, false);
    assert!(
        elapsed >= Duration::from_millis(4500),
        "a connected silent supervisor must consume nearly the 5 s test budget: {elapsed:?}"
    );

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
    holder
        .finish(Duration::from_secs(1))
        .expect("the holding thread must not panic");
    released
        .recv_timeout(Duration::from_secs(1))
        .expect("the holding thread released its listener and peer");
}

/// Assertion unwind before accept must release the real-binary fixture listener.
#[farhelm_testtrace::test]
fn a_silent_supervisor_cancels_before_accept_on_unwind() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let socket = state.path().join("supervisor.sock");
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

/// Assertion unwind while a peer is held must close that peer and listener.
#[farhelm_testtrace::test]
fn a_silent_supervisor_cancels_while_holding_a_peer_on_unwind() {
    use std::io::Read as _;

    let state = farhelm_teststate::tempdir().expect("state dir");
    let socket = state.path().join("supervisor.sock");
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
#[farhelm_testtrace::test]
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
#[farhelm_testtrace::test]
fn a_hook_outside_a_farhelm_session_does_nothing_silently() {
    let mut cmd = std::process::Command::new(farhelm_bin());
    // A fully-formed vendor invocation: the flag has been required since
    // the envelope migration, and the silence contract below is about a
    // sessionless launch, not a vendorless one (which fails closed at
    // parse — see the next test).
    cmd.args(["internal", "hook", "--vendor", "claude"])
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

/// A hook invocation without `--vendor` never runs: clap refuses the argv
/// before anything reads stdin or dials, so no untagged report can reach
/// a supervisor. The exit code and stderr wording are deliberately clap's
/// rather than farhelm's — only a hand-typed argv can produce this shape,
/// since every vendor hook command embeds its flag — which is why this
/// test pins refusal and the flag's naming, not an exact status or text.
#[farhelm_testtrace::test]
fn a_hook_without_a_vendor_flag_fails_closed_at_parse() {
    use std::io::Read;
    let mut child = ChildGuard(
        std::process::Command::new(farhelm_bin())
            .args(["internal", "hook"])
            .env_remove(farhelm_supervisor::launch::SESSION_ID_ENV_VAR)
            .env_remove(farhelm_supervisor::launch::SESSION_TOKEN_ENV_VAR)
            .env_remove(farhelm_supervisor::launch::SUPERVISOR_SOCK_ENV_VAR)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn the hook binary"),
    );
    let started = std::time::Instant::now();
    let status = loop {
        match child.0.try_wait().expect("poll the hook child") {
            Some(status) => break status,
            None => {
                assert!(
                    started.elapsed() < SILENCE_DEADLINE,
                    "a parse refusal must not hang; the hook overran its test budget"
                );
                // sleep-ok: poll child exit while a parse refusal settles.
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    assert!(
        !status.success(),
        "a vendorless hook argv must be refused, not run untagged"
    );
    let mut err = Vec::new();
    child
        .0
        .stderr
        .take()
        .expect("piped stderr")
        .read_to_end(&mut err)
        .expect("read stderr");
    let err = String::from_utf8_lossy(&err);
    assert!(
        err.contains("--vendor"),
        "the refusal must name the missing flag; got {err:?}"
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
#[farhelm_testtrace::test]
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

/// Spawn `cmd`, feed it `payload`, close stdin, and collect what it said.
///
/// [`assert_silent`]'s sibling for the cases where output is the point
/// rather than the defect: it cannot be reused, because it asserts
/// emptiness. Stdin is closed immediately (no `hold_stdin` equivalent)
/// because these cases are about the pointer, and a hook that never gets
/// EOF spends its budget in the read — which is covered by its own test
/// above.
///
/// Polls with `try_wait` under [`SILENCE_DEADLINE`] and reads stdout/stderr
/// only once a status is in hand — the same two-step [`assert_silent`]
/// uses, and for the same reason this now shares [`ChildGuard`] with it:
/// wrapping the `Child` there is what makes a wedged hook (this test's own
/// failure mode) get killed and reaped on the way out instead of leaking
/// and holding the state directory's socket path open for the rest of the
/// suite, whether this function returns normally or panics.
fn run_hook(mut cmd: std::process::Command, payload: &[u8]) -> std::process::Output {
    use std::io::{Read, Write};
    let mut child = ChildGuard(cmd.spawn().expect("spawn the hook binary"));
    {
        // Dropped at the end of this block, which is what gives the hook
        // its EOF. Holding it open is a different test (see
        // `a_hook_whose_stdin_is_never_closed_still_finishes_in_budget`).
        let mut pipe = child.0.stdin.take().expect("piped stdin");
        pipe.write_all(payload).expect("write the payload");
    }
    let deadline = std::time::Instant::now() + SILENCE_DEADLINE;
    let status = loop {
        match child.0.try_wait().expect("poll the hook child") {
            Some(status) => break status,
            None => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "the hook did not finish within {SILENCE_DEADLINE:?}; ChildGuard will kill \
                     and reap it on the way out"
                );
                // sleep-ok: poll hook exit under the helper's deadline before inspecting its output.
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    // Only now, exactly as `assert_silent` does: reading before the child
    // exits could block, and the descriptors stay valid until `child`
    // (the `ChildGuard`) drops at the end of this function.
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .0
        .stdout
        .take()
        .expect("piped stdout")
        .read_to_end(&mut stdout)
        .expect("read stdout");
    child
        .0
        .stderr
        .take()
        .expect("piped stderr")
        .read_to_end(&mut stderr)
        .expect("read stderr");
    std::process::Output {
        status,
        stdout,
        stderr,
    }
}

// ---------------------------------------------------------------------
// `FARHELM_AGENT_INSTRUCTIONS`, through the REAL supervisor CLI
//
// Every test above builds a `SupervisorStartup` (or a `SupervisorSeams`)
// directly in Rust, which never touches `main.rs`'s own environment reads
// at all — those live one layer up, in the `Cmd::Supervisor::Run` arm that
// every constructor this file otherwise uses skips straight past. The two
// tests below spawn `farhelm supervisor run` as a real child process with
// the variable set on ITS environment (never this test process's own —
// see the section note near the top of this file), so a bug in the CLI's
// own `std::env::var` handling — including its `VarError::NotUnicode`
// fallback — cannot hide behind every other test constructing the parsed
// value by hand.
// ---------------------------------------------------------------------

/// Whether a real, out-of-process supervisor's injected Claude hook command
/// carries `--announce`, for a fresh session created against it over its
/// actual unix socket.
///
/// A `claude`-named symlink around the record fixture is what makes the
/// supervisor derive and hook the Claude integration exactly as it would
/// for the genuine CLI (the same trick [`fixture_invocation`] uses), and
/// dialling through [`farhelm_supervisor::service::connect`] plus
/// [`SupervisorClient::start`] — rather than any in-process duplex pipe —
/// is what makes this a client of the SPAWNED process rather than of a
/// `Supervisor` this test built itself.
async fn claude_hook_command_carries_announce(supervisor: &SupervisorProcess) -> bool {
    let bin = farhelm_teststate::tempdir().expect("bin dir");
    std::os::unix::fs::symlink(fixtures_bin(), bin.path().join("claude"))
        .expect("symlink claude onto the farhelm binary");
    let home = farhelm_teststate::tempdir().expect("record home");
    let work = farhelm_teststate::tempdir().expect("work dir");

    let stream = farhelm_supervisor::service::connect(supervisor.state.path())
        .await
        .expect("dial the real supervisor's socket");
    let (r, w) = tokio::io::split(stream);
    let client = SupervisorClient::start(r, w).await.expect("handshake");

    let invocation = format!(
        "{} fake-agent --script claude-record --record-home {}",
        shell_words::quote(&bin.path().join("claude").to_string_lossy()),
        shell_words::quote(&home.path().to_string_lossy())
    );
    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &invocation,
            None,
            WIDE_COLS,
            ROWS,
        )
        .await
        .expect("create a claude-kind session against the real supervisor");
    let (_chan, mut seen, mut rx) = client
        .attach_live(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach");
    wait_for(&mut rx, &mut seen, ARGV_MARKER, 20).await;
    let argv = argv_marker(&seen);
    injected_settings(&argv)["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .is_some_and(|command| command.contains("--announce"))
}

/// Spec: `FARHELM_AGENT_INSTRUCTIONS=off` on the spawned `farhelm
/// supervisor run` process suppresses `--announce` on a real launch's
/// injected hook command.
///
/// This is the merged review's D15 (A's F9/F10, B's F7): every other test
/// of this switch (`with_hook_argv_*` in `core.rs`, `agent_instructions.rs`'s
/// own unit tests) exercises the pure parser or the argv builder directly,
/// and none of them would notice a `main.rs` that read the wrong variable
/// name, read it twice, or forgot to pass the parsed value into
/// `SupervisorStartup` at all — only a real process, started the way an
/// operator actually starts one, can catch that class of bug.
#[farhelm_testtrace::test]
async fn farhelm_agent_instructions_off_suppresses_announce_through_the_real_cli() {
    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let supervisor = supervisor_process_with_env([
        (
            "FARHELM_AGENT_INSTRUCTIONS",
            std::ffi::OsString::from("off"),
        ),
        // Pinned rather than inherited: this test's assertion is entirely
        // about `--announce`, and `FARHELM_AGENT_HOOKS` gates whether the
        // hook is injected AT ALL. A suite run under `FARHELM_AGENT_HOOKS=none`
        // must not fail this test for that unrelated reason.
        ("FARHELM_AGENT_HOOKS", std::ffi::OsString::from("all")),
    ])
    .await;
    assert!(
        !claude_hook_command_carries_announce(&supervisor).await,
        "FARHELM_AGENT_INSTRUCTIONS=off must reach the real CLI's injected hook command"
    );
}

/// Spec: a non-UTF-8 `FARHELM_AGENT_INSTRUCTIONS` value on the real CLI
/// falls back to the default (`on`, `--announce` present) rather than
/// silently behaving like `off`.
///
/// This exercises the `VarError::NotUnicode` arm in `main.rs` that nothing
/// else in the suite reaches, because every other test sets the variable
/// (if at all) as an ordinary Rust `&str`. `OsString::from_vec` builds a
/// value `std::env::var` cannot parse as UTF-8 at all, standing in for
/// whatever produces one in a real shell profile (a stray byte from a
/// copy-paste, a locale mismatch) — the CLI's fallback direction matters
/// precisely because this switch's OFF position removes a feature, so a
/// value nobody can even read must not silently become "off".
#[farhelm_testtrace::test]
async fn farhelm_agent_instructions_non_utf8_falls_back_to_default_through_the_real_cli() {
    use std::os::unix::ffi::OsStringExt;

    let _slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let malformed = std::ffi::OsString::from_vec(vec![0xff, 0xfe]);
    let supervisor = supervisor_process_with_env([
        ("FARHELM_AGENT_INSTRUCTIONS", malformed),
        // See the sibling test above: pinned so a suite run under
        // `FARHELM_AGENT_HOOKS=none` cannot fail this assertion for a
        // reason that has nothing to do with the non-UTF-8 fallback under
        // test.
        ("FARHELM_AGENT_HOOKS", std::ffi::OsString::from("all")),
    ])
    .await;
    assert!(
        claude_hook_command_carries_announce(&supervisor).await,
        "a non-UTF-8 FARHELM_AGENT_INSTRUCTIONS must fall back to the default (on), not to off"
    );
}

/// An unhooked Claude launch offers and performs a fresh restart after input
/// creates an on-disk record. The replacement must start a different conversation.
/// This pins the fallback; removal of the scanner establishes that later input
/// cannot turn a nearby record into an identity.
#[farhelm_testtrace::test]
async fn an_unhooked_claude_session_restarts_fresh_after_input() {
    let (h, fixtures) = fixture_harness_with_seams(|seams| {
        seams.agent_hooks = farhelm_supervisor::agent_kind::AgentHooks::Only(vec![]);
    })
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;
    assert!(
        !argv_marker(&seen).contains("--settings"),
        "fixture must be unhooked"
    );
    assert_eq!(
        snapshot_of(&h, &session.id).await.kind,
        farhelm_proto::AgentKind::Claude
    );
    h.client.send_input(chan, b"first prompt\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;
    let first_record = marker_value(&seen, "RECORD-WRITTEN:");
    let before = snapshot_of(&h, &session.id).await;
    assert_eq!(before.captured_conversation, None);
    assert_eq!(before.restart_offer, farhelm_proto::RestartOffer::FreshOnly);
    let restarted = h
        .client
        .restart_session(&session.id, farhelm_proto::RestartMode::Fresh, true)
        .await
        .expect("fresh restart is the available fallback");
    assert_eq!(
        restarted.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;
    let from = seen.len();
    h.client
        .send_input(chan, b"replacement prompt\r".to_vec())
        .await;
    wait_for_after_from(
        &mut rx,
        &mut seen,
        from,
        "RECORD-WRITTEN:",
        "replacement prompt",
        20,
    )
    .await;
    assert_ne!(
        marker_value(&seen[from..], "RECORD-WRITTEN:"),
        first_record,
        "fresh restart must actually launch a new conversation"
    );
    let after = snapshot_of(&h, &session.id).await;
    assert_eq!(after.captured_conversation, None);
    assert_eq!(after.restart_offer, farhelm_proto::RestartOffer::FreshOnly);
}

/// Removing heuristic identification must not erase older stored identities.
/// Seed a historical row with no hook provenance, reconstruct the supervisor,
/// then observe the actual replacement process receiving that exact Resume id.
#[farhelm_testtrace::test]
async fn a_historical_identity_survives_restart_and_reaches_resume_argv() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (_chan, _rx, _seen) = attach_ready(&h, &session).await;
    let store = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open fixture store");
    let mut legacy = store.session(&session.id).await.unwrap().unwrap();
    legacy.id = uuid::Uuid::new_v4().to_string();
    legacy.tmux_name = format!("fh-{}", legacy.id);
    legacy.pane = String::new();
    legacy.outcome = LastOutcome::Interrupted;
    legacy.captured_conversation = Some("historical-conversation".to_string());
    legacy.conversation_source = None;
    legacy.capture_ownership_version = 0;
    legacy.resume_template = Some(vec![
        fixtures.bin().join("claude").to_string_lossy().into_owned(),
        "fake-agent".into(),
        "--script".into(),
        "hook-report".into(),
        "--record-home".into(),
        fixtures.home().to_string_lossy().into_owned(),
        "--resume".into(),
        "{conversation}".into(),
    ]);
    store
        .insert_session(legacy.clone(), None)
        .await
        .expect("seed historical identity");
    let seeded = store.session(&legacy.id).await.unwrap().unwrap();
    assert_eq!(seeded.conversation_source, None);
    assert_eq!(
        seeded.captured_conversation.as_deref(),
        Some("historical-conversation")
    );
    drop(store);

    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    serving.stop().await;
    let sup = crate::create_idempotency::handoff_to_new_supervisor(state.path(), sup, client).await;
    let after = sup.session_snapshot(&legacy.id).await.unwrap().unwrap();
    assert_eq!(after.restart_offer, farhelm_proto::RestartOffer::Resume);
    assert_eq!(
        after.captured_conversation.as_deref(),
        Some("historical-conversation")
    );
    let client = connect_client(&sup).await;
    client
        .restart_session(&legacy.id, farhelm_proto::RestartMode::Resume, false)
        .await
        .expect("resume historical conversation");
    let (_chan, mut seen, mut rx) = client
        .attach_live(&legacy.id, WIDE_COLS, ROWS)
        .await
        .unwrap();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    assert!(
        argv_marker(&seen).contains("--resume historical-conversation"),
        "the actual child must receive the historical id: {}",
        argv_marker(&seen)
    );
}
