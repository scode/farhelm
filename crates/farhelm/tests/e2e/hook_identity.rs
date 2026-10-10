//! Agent-reported conversation identity: the `SessionStart` hook path, end
//! to end against a real supervisor.
//!
//! The agent reports its own identity, including a conversation created by
//! `/clear` or `/new` inside an existing process. These tests exercise that
//! report through the real launch environment, the report file the hook
//! drops, the supervisor's reconciliation pass, and the store.
//!
//! ## What is real here, and the one thing that is not
//!
//! Everything downstream of the vendor is genuine: the `farhelm internal
//! hook` binary runs as a real child of the supervised process, finds the
//! supervisor's state directory from the environment the launch injected,
//! records its own process ancestry, and drops a real report file, which
//! the supervisor attributes and applies on a real reconciliation pass.
//! Only the TRIGGER is faked — `Script::HookReport` fires the hook when a
//! test types `report <id>` instead of when a vendor decides a conversation
//! started. The `#[ignore]`d tests in `real_agent_capture` are what keep
//! that last step honest across vendor versions.
//!
//! ## When a report is applied
//!
//! The hook only drops its report; the supervisor applies it on watch events,
//! with the two-second reconciliation ticker as its backstop. Most tests do
//! not depend on asynchronous pickup: [`report`] runs a pass explicitly once the hook
//! has exited, so "the report was accepted or refused" holds when it
//! returns. [`hook_harness`] still starts the real accept loop, so these
//! sessions run under a supervisor that is serving the way production's
//! does, ticker included.

use crate::harness::*;

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// A capture harness whose supervisor is genuinely serving: listening on
/// its unix socket and running its ticker, as production's does.
///
/// The accept loop is started before any session exists, and this returns
/// only once it is bound. See [`ServeTask::spawn`] for both orderings.
///
/// Returns the [`ServeTask`] the caller must keep alive for as long as it
/// expects hooks to work.
pub(crate) async fn hook_harness() -> (Harness, CaptureFixtures, ServeTask) {
    let (h, fixtures) = fixture_harness_with_seams(|_| {}).await;
    let task = ServeTask::spawn(&h.sup, h.state.path()).await;
    (h, fixtures, task)
}

/// Start a long-ticker supervisor and wait for the watch's initial empty scan.
/// A report written after this boundary cannot be picked up by startup catchup;
/// only a later filesystem event may drive its drain before the first tick.
async fn watched_hook_harness() -> (Harness, CaptureFixtures, ServeTask) {
    let scanned = Arc::new(tokio::sync::Notify::new());
    let (h, fixtures) = fixture_harness_with_seams(|seams| {
        seams.ticker_interval = Duration::from_secs(600);
        seams.faults.report_drain_listed = Some(Arc::new({
            let scanned = Arc::clone(&scanned);
            move || {
                scanned.notify_one();
                Box::pin(async {})
            }
        }));
    })
    .await;
    let serving = ServeTask::spawn(&h.sup, h.state.path()).await;
    tokio::time::timeout(Duration::from_secs(5), scanned.notified())
        .await
        .expect("initial empty watch scan");
    (h, fixtures, serving)
}

/// Wait for an accepted identity without a reconciliation call. Listings and
/// snapshots only read state, so this observes what the watcher actually applied.
/// Timeout diagnostics retain the live supervisor and its session's hook log.
async fn wait_for_watched_identity(h: &Harness, id: &str, conversation: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = snapshot_of(h, id).await;
        if snapshot.captured_conversation.as_deref() == Some(conversation) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "watch never accepted {conversation}; status={:?}; {}",
            snapshot.restart_offer,
            hook_log(h, id)
        );
        // sleep-ok: polling accepted identity without driving reconciliation.
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// The first report creates a new session directory after watcher startup.
/// A ten-minute ticker cannot explain acceptance within this five-second budget;
/// the real hook, ancestry admission and durable identity must all succeed.
#[farhelm_testtrace::test]
async fn watch_applies_first_report_before_a_long_ticker_interval() {
    let started = tokio::time::Instant::now();
    let (h, fixtures, serving) = watched_hook_harness().await;
    let work = farhelm_teststate::tempdir().unwrap();
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;
    let report_dir =
        farhelm_supervisor::hook_report::session_dir(h.state.path(), &session.id).unwrap();
    assert!(
        !report_dir.exists(),
        "first report must create a fresh watched directory"
    );
    assert!(
        snapshot_of(&h, &session.id)
            .await
            .captured_conversation
            .is_none()
    );
    write_report_client(&h.client, chan, &mut rx, &mut seen, "conv-watched-first").await;
    wait_for_watched_identity(&h, &session.id, "conv-watched-first").await;
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "no ten-minute tick may explain pickup"
    );
    wait_for_hook_log_words(h.state.path(), &session.id, "acked", 1).await;
    assert!(
        !report_dir.join("latest.json").exists(),
        "accepted report is settled"
    );
    drop(rx);
    serving.stop().await;
}

/// Several first reports in newly created directories may arrive as one backend
/// burst. Every session must gain its own identity without an explicit drain or
/// a tick; the assertion is correctness, not how many drains the backend chose.
#[farhelm_testtrace::test]
async fn watch_applies_a_burst_of_reports_from_several_sessions() {
    let started = tokio::time::Instant::now();
    let (h, fixtures, serving) = watched_hook_harness().await;
    let work = farhelm_teststate::tempdir().unwrap();
    let mut peers = Vec::new();
    for n in 0..3 {
        let session = hook_session(&h, &fixtures, work.path()).await;
        let (chan, rx, seen) = attach_ready(&h, &session).await;
        assert!(
            snapshot_of(&h, &session.id)
                .await
                .captured_conversation
                .is_none()
        );
        peers.push((session, chan, rx, seen, format!("conv-watched-burst-{n}")));
    }
    for (_, chan, _, _, conversation) in &peers {
        h.client
            .send_input(*chan, format!("report {conversation}\r").into_bytes())
            .await;
    }
    for (session, _, rx, seen, conversation) in &mut peers {
        wait_for_after_from(
            rx,
            seen,
            0,
            &format!("HOOK-REPORTED:{conversation}"),
            "HOOK-STDOUT-EMPTY",
            30,
        )
        .await;
        wait_for_watched_identity(&h, &session.id, conversation).await;
        wait_for_hook_log_words(h.state.path(), &session.id, "acked", 1).await;
    }
    assert!(
        started.elapsed() < Duration::from_secs(600),
        "no tick may explain burst pickup"
    );
    drop(peers);
    serving.stop().await;
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
/// the process looks like a real `claude` (or `codex`) on the user's PATH.
fn fixture_invocation(fixtures: &CaptureFixtures, kind: &str, script: &str) -> String {
    format!(
        "{} fake-agent --script {script} --record-home {}",
        shell_words::quote(&fixtures.bin().join(kind).to_string_lossy()),
        shell_words::quote(&fixtures.home().to_string_lossy())
    )
}

/// A command launch running `script` through the `kind` symlink and
/// declaring that agent type, with `{farhelm_args}` last and the resume
/// command the agent type's own resume selector would give (Claude's
/// `--resume`, Codex's `resume` subcommand): the declared-type launch a
/// user would write for this program.
fn hook_launch(
    fixtures: &CaptureFixtures,
    kind: farhelm_proto::LaunchHarness,
    script: &str,
) -> farhelm_proto::SessionLaunch {
    let (name, selector) = match kind {
        farhelm_proto::LaunchHarness::Codex => ("codex", "resume"),
        farhelm_proto::LaunchHarness::Claude => ("claude", "--resume"),
        other => panic!("the hook fixtures stand in for Claude and Codex, not {other:?}"),
    };
    let invocation = fixture_invocation(fixtures, name, script);
    declared_command(
        &format!("{invocation} {{farhelm_args}}"),
        kind,
        Some(&format!(
            "{invocation} {selector} {{conversation}} {{farhelm_args}}"
        )),
    )
}

/// Create a Claude command launch running the hook-reporting fixture.
pub(crate) async fn hook_session(
    h: &Harness,
    fixtures: &CaptureFixtures,
    cwd: &std::path::Path,
) -> SessionInfo {
    h.client
        .create_session_with_extras(
            &cwd.to_string_lossy(),
            hook_launch(
                fixtures,
                farhelm_proto::LaunchHarness::Claude,
                "hook-report",
            ),
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras::default(),
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

/// Type one `report <conversation>` at the fixture, wait until the hook
/// child it spawned has exited, then run the supervisor's reconciliation
/// pass so the report it dropped has been judged.
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
/// The hook only drops its report; the supervisor applies it on its next
/// reconciliation pass, which the ticker would run within two seconds. The
/// explicit pass here makes "the report has been accepted or refused" true
/// when this returns, without a timed wait. Whether it was accepted is the
/// caller's assertion on the stored identity — which is why every failure
/// message below quotes the hook's own log.
pub(crate) async fn report(
    h: &Harness,
    chan: u32,
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    conversation: &str,
) {
    report_client(&h.sup, &h.client, chan, rx, seen, conversation).await;
}

/// Send an explicit identity report through an already connected client and
/// let `sup` apply it.
///
/// This variant keeps report-driven restart tests that construct supervisors
/// by hand on the same acceptance and silence checks as the shared harness.
pub(crate) async fn report_client(
    sup: &Supervisor,
    client: &SupervisorClient,
    chan: u32,
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    conversation: &str,
) {
    write_report_client(client, chan, rx, seen, conversation).await;
    sup.reconcile_for_test().await;
}

/// Run the real hook to completion without causing a supervisor drain.
/// Watch pickup tests use this boundary so a timer or explicit reconciliation
/// cannot explain their result. The transcript still proves silence and success.
async fn write_report_client(
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
/// directory they pointed the hook's environment at. Asserting the line COUNT
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

/// The conversation a restart would resume, read from a snapshot's filled
/// resume command.
///
/// A declared Claude command's resume command ends `--resume
/// {conversation} {farhelm_args}`: the snapshot fills the conversation and
/// leaves `{farhelm_args}` for the spawn to expand, so the id sits in the
/// second-to-last slot. Asserting that the last slot is still the
/// placeholder is what proves the id landed in the conversation's own slot
/// rather than merely somewhere in the command.
fn resumed_conversation(argv: &Option<Vec<String>>) -> &str {
    let argv = argv
        .as_deref()
        .expect("a resumable session has a filled resume command");
    match argv {
        [.., conversation, last]
            if last == farhelm_proto::session_launch::FARHELM_ARGS_PLACEHOLDER =>
        {
            conversation
        }
        _ => panic!(
            "the resume command must end with the conversation and {{farhelm_args}}: {argv:?}"
        ),
    }
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
        resumed_conversation(&snapshot.resume_argv),
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
            declared_command(
                &format!(
                    "{} {{farhelm_args}}",
                    fixture_invocation(&fixtures, "claude", "hook-report")
                ),
                farhelm_proto::LaunchHarness::Claude,
                Some(&format!(
                    "{} {{farhelm_args}}",
                    shell_words::join(&template)
                )),
            ),
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras::default(),
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
    assert_eq!(resumed_conversation(&snapshot.resume_argv), "conv-2");

    h.client
        .restart_session(&session.id, true)
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

    h.sup.reconcile_for_test().await;
    let log = hook_log_lines(&h, &session.id);
    let child = log
        .iter()
        .find(|line| line.contains(" conv-child ") && !line.contains(" written "))
        .unwrap_or_else(|| panic!("the supervisor must have judged the child's report: {log:?}"));
    assert!(
        child.contains(" refused conflict ") && child.contains("nested below"),
        "the supervisor must have refused the child for its ancestry: {child}"
    );
    assert!(
        log.iter()
            .any(|line| line.contains(" written ") && line.contains(" conv-parent ")),
        "the foreground's own report must have been written: {log:?}"
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
/// The hook log is the second, independent witness: the hook writes one
/// line per run, so two `written` lines for one id mean two hook processes
/// really did write a report, and the supervisor's two `acked` lines mean
/// both were judged and accepted.
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
    let word = |line: &String| line.split_whitespace().nth(1).map(str::to_string);
    let runs: Vec<_> = log
        .iter()
        .filter(|line| word(line).as_deref() == Some("written"))
        .collect();
    assert_eq!(
        runs.len(),
        2,
        "one hook line per run is the log's contract, and two runs happened: {log:?}"
    );
    let verdicts: Vec<_> = log
        .iter()
        .filter(|line| word(line).as_deref() != Some("written"))
        .collect();
    assert!(
        verdicts.len() == 2
            && verdicts
                .iter()
                .all(|line| word(line).as_deref() == Some("acked")),
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
    assert_eq!(resumed_conversation(&after.resume_argv), "conv-durable");
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

    // The fixture shell-quotes its real argv, preserving word boundaries.
    // Use the same encoding so paths containing spaces or quotes cannot
    // make an unchanged launch look like hook injection.
    assert_eq!(
        argv_marker(&seen),
        shell_words::join(&requested),
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
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            hook_launch(
                &fixtures,
                farhelm_proto::LaunchHarness::Codex,
                "hook-report",
            ),
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras::default(),
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
/// The child-only stdin budget is 5 s, which only the held-stdin case
/// spends. Keep two seconds of margin here for spawning a debug binary
/// under a loaded runner; the margin, rather than the hook budget, absorbs
/// that scheduling cost.
const SILENCE_DEADLINE: Duration = Duration::from_secs(7);

/// Keep real-binary hook cases bounded without changing the test runner's
/// environment. The override is placed on each spawned hook command below.
const TEST_HOOK_BUDGET_MS: &str = "5000";

/// Kills and reaps the hook child on the way out, however the test leaves.
///
/// The whole point of the tests below is that the hook might NOT exit —
/// hung on a stdin nobody closes — and a failing deadline assertion unwinds
/// past any explicit cleanup. A leaked hook child would then hold its own
/// pipes open for as long as the test binary runs, with nothing left to
/// reap it. Killing an already-exited child is a harmless error,
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

/// A hook with no supervisor running says nothing, exits 0 at once, and
/// leaves its report waiting on disk with a `written` line in its log.
///
/// This is the case the file drop exists for: on the Mac the supervisor is
/// not running whenever the desktop app is closed, and the agents keep
/// running and keep firing hooks. Nothing about the hook may depend on a
/// supervisor being there — no wait, no retry, no lost report — and the
/// report must be in its slot for the supervisor that starts next.
///
/// The payload is deliberately WELL-FORMED: a bad one is rejected before
/// any report is written, so garbage here would pass without exercising
/// the write at all. The elapsed bound pins that nothing waits on an absent
/// supervisor any more; it is far below the old reconnect window and far
/// above process start-up.
#[farhelm_testtrace::test]
fn a_hook_with_no_supervisor_is_silent_and_leaves_its_report() {
    let state = farhelm_teststate::tempdir().expect("state dir");
    let socket = state.path().join("supervisor.sock");
    let payload =
        br#"{"session_id":"conv-missing","hook_event_name":"SessionStart","source":"startup"}"#;
    let elapsed = assert_silent(hook_command(&socket, "sess-missing"), payload, false);
    assert!(
        elapsed < Duration::from_secs(3),
        "a hook must not wait for a supervisor that is not there: {elapsed:?}"
    );

    let outcome = sole_hook_log_outcome(state.path(), "sess-missing");
    assert_eq!(outcome, "written conv-missing startup");
    let slot = farhelm_supervisor::hook_report::session_dir(state.path(), "sess-missing")
        .expect("a valid session id")
        .join(farhelm_supervisor::hook_report::Slot::Latest.file_name());
    let report: farhelm_supervisor::hook_report::HookReport =
        serde_json::from_slice(&std::fs::read(&slot).expect("the report waits in its slot"))
            .expect("the waiting report parses");
    assert_eq!(report.conversation, "conv-missing");
    assert!(
        report.ancestry.is_some_and(|links| !links.is_empty()),
        "the report carries the hook's recorded ancestry"
    );
}

/// A vendor that never closes the hook's stdin cannot hold it past its
/// budget either.
///
/// The budget deliberately covers READING the payload, and this is the
/// reason: a blocking read cannot be interrupted by any timeout, so an
/// implementation that bounded anything but the read itself would hang here
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
/// No supervisor is running, which is the point: the pointer is not
/// conditional on a report ever being applied, because a session whose
/// supervisor is down is exactly a session whose agent may need to ask
/// farhelm what is going on. The log line is read to prove the identity
/// half really ran (and wrote its report) rather than being skipped.
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
    assert_eq!(
        outcome, "written conv-a startup",
        "the identity half must still have run and written its report"
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
/// for the rest of the suite, whether this function returns normally or
/// panics.
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
/// The session is a command launch declaring Claude, which is what makes
/// the supervisor hook the Claude integration exactly as it would for the
/// genuine CLI; the `claude`-named symlink keeps the process looking like
/// the real one (the same trick [`fixture_invocation`] uses). And
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
    // A declared Claude command: an undeclared one gets no hooks at all,
    // so it could not show whether `--announce` was suppressed.
    let session = client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            declared_command(
                &format!("{invocation} {{farhelm_args}}"),
                farhelm_proto::LaunchHarness::Claude,
                None,
            ),
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras::default(),
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

/// An unhooked Claude launch cannot be restarted, even after input creates
/// an on-disk record, and a restart request launches nothing.
///
/// Why: SPEC.md identifies a conversation only from the harness's own
/// report, so a record on disk is not an identity, and Restart always
/// resumes: a session with no captured conversation has Restart
/// unavailable ("rather than starting fresh or running some other
/// command"). This used to pin a fresh restart as the fallback; that
/// fallback is the bug the spec now names. Specified: the offer is
/// `NotCaptured` before and after the refused request, the refusal is a
/// `Conflict`, and the agent keeps running in the same pane.
#[farhelm_testtrace::test]
async fn an_unhooked_claude_session_cannot_be_restarted_after_input() {
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
    let before = snapshot_of(&h, &session.id).await;
    assert_eq!(before.captured_conversation, None);
    assert_eq!(
        before.restart_offer,
        farhelm_proto::RestartOffer::NotCaptured
    );
    let sock = h.state.path().join("tmux.sock");
    let pane = pane_id_of(&sock, &format!("fh-{}", session.id)).await;

    let err = h
        .client
        .restart_session(&session.id, true)
        .await
        .expect_err("a session with no captured conversation cannot be restarted");
    let err = err
        .downcast_ref::<SupervisorError>()
        .expect("the refusal carries its classification");
    assert_eq!(err.kind, ErrorKind::Conflict, "{}", err.message);

    let after = snapshot_of(&h, &session.id).await;
    assert_eq!(after.captured_conversation, None);
    assert_eq!(
        after.restart_offer,
        farhelm_proto::RestartOffer::NotCaptured
    );
    assert_eq!(
        after.generation, before.generation,
        "a refused restart must not open a relaunch generation"
    );
    assert_eq!(
        pane_id_of(&sock, &format!("fh-{}", session.id)).await,
        pane,
        "the running agent is left where it was"
    );
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
    // A session from before launch kinds: its stored command, kind and
    // resume command as that release kept them (SPEC.md, the upgrade).
    legacy.launch = farhelm_proto::SessionLaunch::Legacy {
        // Without `{farhelm_args}`: no release before launch kinds wrote
        // one, which is why a legacy session gets the old injection.
        invocation: shell_words::join(
            legacy
                .launch
                .start_argv()
                .expect("the hook session's command splits")
                .into_iter()
                .filter(|word| word != farhelm_proto::session_launch::FARHELM_ARGS_PLACEHOLDER),
        ),
        agent_kind: farhelm_proto::AgentKind::Claude,
        resume_template: Some(vec![
            fixtures.bin().join("claude").to_string_lossy().into_owned(),
            "fake-agent".into(),
            "--script".into(),
            "hook-report".into(),
            "--record-home".into(),
            fixtures.home().to_string_lossy().into_owned(),
            "--resume".into(),
            "{conversation}".into(),
        ]),
    };
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
        .restart_session(&legacy.id, false)
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

// ---------------------------------------------------------------------
// Reports made while no supervisor is running
// ---------------------------------------------------------------------
//
// On the Mac the supervisor runs inside the desktop app, so closing the app
// stops it while sessions keep running. Hooks keep firing in that window, and
// their reports must wait on disk for the supervisor that starts next rather
// than being lost. These tests stop the supervisor for real, drive the
// still-running agent through tmux directly (there is no supervisor to attach
// through), and start a successor on the same state directory.
// ---------------------------------------------------------------------

/// Stop `h`'s supervisor for real and wait until nothing holds it, returning
/// what the test needs to keep driving the session and to start a successor.
///
/// Mirrors [`a_report_survives_a_supervisor_restart`]'s teardown: the accept
/// loop stops, the client drops, and the supervisor is dropped only once no
/// connection task still holds a reference, so the successor really is the
/// only supervisor on the state directory.
async fn stop_supervisor(
    h: Harness,
    serving: ServeTask,
) -> (
    farhelm_teststate::TestDir,
    TmuxServerGuard,
    tokio::sync::SemaphorePermit<'static>,
) {
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
    (state, _tmux, _slot)
}

/// Type `line` into the session's agent through tmux itself, the way a
/// person types into a session whose supervisor is not running.
async fn type_without_supervisor(state: &std::path::Path, session_id: &str, line: &str) {
    let sent = tmux_query(
        &state.join("tmux.sock"),
        &[
            "send-keys",
            "-t",
            &format!("fh-{session_id}"),
            line,
            "Enter",
        ],
    )
    .await;
    assert!(
        sent.status.success(),
        "typing into the agent through tmux failed: {}",
        String::from_utf8_lossy(&sent.stderr)
    );
}

/// Wait until the session's hook log holds `count` lines whose outcome word
/// is `word`. Hook runs fired through tmux leave no other witness this test
/// can wait on: there is no supervisor to attach a terminal through.
async fn wait_for_hook_log_words(
    state: &std::path::Path,
    session_id: &str,
    word: &str,
    count: usize,
) {
    let path = state.join("hook-log").join(format!("{session_id}.log"));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let seen = text
            .lines()
            .filter(|line| line.split_whitespace().nth(1) == Some(word))
            .count();
        if seen >= count {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the hook log never reached {count} {word:?} lines:\n{text}"
        );
        // sleep-ok: polling interval for a file another process appends to.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// A supervisor on `state` with production defaults, constructed after its
/// predecessor is gone.
async fn successor(state: &std::path::Path) -> Arc<Supervisor> {
    let restarted = Supervisor::new_with_seams(
        state,
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams::default(),
    )
    .await
    .expect("successor supervisor");
    assert!(
        restarted.owns_state_dir(),
        "the predecessor must be gone, or this proves nothing"
    );
    restarted
}

/// Spec: a conversation report a hook makes while no supervisor is running
/// waits on disk, and the next supervisor to start applies it — through the
/// same attribution as ever — so Restart resumes the conversation the agent
/// switched to while the supervisor was down.
///
/// Why: this is the Mac case the report files exist for. With the desktop
/// app closed the supervisor is not running, yet the agent keeps running and
/// can start a new conversation (`/clear`). Losing that report would make the
/// next Restart resume the discarded conversation.
#[farhelm_testtrace::test]
async fn a_report_made_while_no_supervisor_runs_is_applied_when_one_starts() {
    let (h, fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;
    report(&h, chan, &mut rx, &mut seen, "conv-before").await;
    assert_eq!(
        snapshot_of(&h, &session.id)
            .await
            .captured_conversation
            .as_deref(),
        Some("conv-before"),
        "fixture premise: the first report landed; {}",
        hook_log(&h, &session.id)
    );
    drop(rx);

    let (state, _tmux, _slot) = stop_supervisor(h, serving).await;
    type_without_supervisor(state.path(), &session.id, "report conv-offline").await;
    wait_for_hook_log_words(state.path(), &session.id, "written", 2).await;
    let waiting = farhelm_supervisor::hook_report::session_dir(state.path(), &session.id)
        .expect("a session id names a drop directory")
        .join(farhelm_supervisor::hook_report::Slot::Latest.file_name());
    assert!(
        waiting.exists(),
        "the report waits on disk while no supervisor runs"
    );

    let restarted = successor(state.path()).await;
    let after = restarted
        .session_snapshot(&session.id)
        .await
        .expect("snapshot")
        .expect("present");
    assert_eq!(
        after.captured_conversation.as_deref(),
        Some("conv-offline"),
        "the successor's first pass applied the waiting report; {}",
        hook_log_at(state.path(), &session.id)
    );
    assert_eq!(after.restart_offer, farhelm_proto::RestartOffer::Resume);
    assert_eq!(resumed_conversation(&after.resume_argv), "conv-offline");
    assert!(
        !waiting.exists(),
        "the applied report is settled and removed"
    );
    assert!(
        hook_log_at(state.path(), &session.id).contains(" acked conv-offline "),
        "the supervisor's verdict sits beside the hook's line"
    );
}

/// An executable named `grok` that runs the fixture binary: Grok attribution
/// recognizes its native runtime by the kernel's image basename, so a
/// symlink (which the kernel resolves) would not do; a hard link or copy
/// does.
fn native_grok_image(dir: &std::path::Path) -> std::path::PathBuf {
    let image = dir.join("grok");
    if std::fs::hard_link(fixtures_bin(), &image).is_err() {
        std::fs::copy(fixtures_bin(), &image).expect("create the native Grok image");
    }
    image
}

/// Spec: when Grok selects a conversation and then enriches it twice while
/// no supervisor is running, the next supervisor applies the selection
/// before the latest enrichment, and the session becomes resumable to that
/// conversation through its exact record.
///
/// Why: Grok is the one vendor whose reports depend on order — an
/// enrichment is refused unless its selection came first — and it reports
/// on every prompt, so a supervisor outage sees many enrichments after one
/// selection. A drop directory that kept only the latest report would lose
/// the selection to them; the separate selection slot is what prevents it.
#[farhelm_testtrace::test]
async fn grok_selection_survives_enrichments_made_while_no_supervisor_runs() {
    let (h, _fixtures, serving) = hook_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let records = farhelm_teststate::tempdir().expect("private Grok records");
    let images = farhelm_teststate::tempdir().expect("native Grok image directory");
    let grok = native_grok_image(images.path());
    let invocation = format!(
        "{} fake-agent --script grok-conversation --record-home {} --hook-binary {} --no-leader",
        shell_words::quote(&grok.to_string_lossy()),
        shell_words::quote(&records.path().to_string_lossy()),
        shell_words::quote(farhelm_bin()),
    );
    let session = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            declared_command(
                &format!("{invocation} {{farhelm_args}}"),
                farhelm_proto::LaunchHarness::Grok,
                Some(&format!(
                    "{invocation} --resume {{conversation}} {{farhelm_args}}"
                )),
            ),
            None,
            WIDE_COLS,
            ROWS,
            farhelm_helm::CreateExtras::default(),
        )
        .await
        .expect("create a Grok session");
    let (_chan, mut seen, mut rx) = h
        .client
        .attach_live(&session.id, WIDE_COLS, ROWS)
        .await
        .expect("attach");
    wait_for(&mut rx, &mut seen, "GROK-CONVERSATION READY", 20).await;
    drop(rx);

    let (state, _tmux, _slot) = stop_supervisor(h, serving).await;
    let uuid = "019a0000-0000-7000-8000-00000000c0de";
    type_without_supervisor(state.path(), &session.id, &format!("select {uuid}")).await;
    wait_for_hook_log_words(state.path(), &session.id, "written", 1).await;
    for runs in [2, 3] {
        type_without_supervisor(state.path(), &session.id, &format!("enrich {uuid}")).await;
        wait_for_hook_log_words(state.path(), &session.id, "written", runs).await;
    }
    let drop_dir = farhelm_supervisor::hook_report::session_dir(state.path(), &session.id)
        .expect("a session id names a drop directory");
    let mut waiting: Vec<String> = std::fs::read_dir(&drop_dir)
        .expect("the drop directory exists")
        .map(|entry| entry.expect("entry").file_name().into_string().unwrap())
        .collect();
    waiting.sort();
    assert_eq!(
        waiting,
        ["enrichment.json", "selection.json"],
        "two enrichments did not displace the selection"
    );

    let restarted = successor(state.path()).await;
    let after = restarted
        .session_snapshot(&session.id)
        .await
        .expect("snapshot")
        .expect("present");
    assert_eq!(
        after.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "the selection and its enrichment were both applied; {}",
        hook_log_at(state.path(), &session.id)
    );
    assert!(
        after
            .resume_argv
            .as_ref()
            .is_some_and(|argv| argv.iter().any(|arg| arg == uuid)),
        "Resume targets the selected conversation: {:?}",
        after.resume_argv
    );
    let acked = hook_log_at(state.path(), &session.id)
        .lines()
        .filter(|line| line.split_whitespace().nth(1) == Some("acked"))
        .count();
    assert_eq!(
        acked, 2,
        "the selection and the latest enrichment were each accepted"
    );
    assert!(
        std::fs::read_dir(&drop_dir)
            .expect("drop dir")
            .next()
            .is_none(),
        "both reports are settled and removed"
    );
}

/// Spec: a report made under an earlier launch of a session is discarded
/// when the supervisor reads it after the session has been restarted: it is
/// refused for not reaching the current launch's terminal, removed, and
/// changes nothing about the conversation the restarted launch reported.
///
/// Why: report files outlive the launch that wrote them (a report can wait
/// on disk while the supervisor is down, or race a restart). Applying an old
/// launch's report to the new one would point Resume at a conversation the
/// running agent is not in. The anchor at the session's current pane
/// process is what tells the two apart, and this pins it end to end.
#[farhelm_testtrace::test]
async fn a_report_from_an_earlier_launch_is_discarded() {
    // No accept loop and so no ticker: the test copies the first launch's
    // report aside before any pass may take it, and drives every pass
    // itself with `reconcile_for_test`.
    let (h, fixtures) = fixture_harness_with_seams(|_| {}).await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = hook_session(&h, &fixtures, work.path()).await;
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;
    let slot = farhelm_supervisor::hook_report::session_dir(h.state.path(), &session.id)
        .expect("a session id names a drop directory")
        .join(farhelm_supervisor::hook_report::Slot::Latest.file_name());

    // The first launch's report, kept aside before the supervisor takes it.
    let from = seen.len();
    h.client
        .send_input(chan, b"report conv-old\r".to_vec())
        .await;
    wait_for_after_from(
        &mut rx,
        &mut seen,
        from,
        "HOOK-REPORTED:conv-old",
        "HOOK-STDOUT-EMPTY",
        30,
    )
    .await;
    let stale = std::fs::read(&slot).expect("the first launch's report is waiting");
    h.sup.reconcile_for_test().await;
    drop(rx);

    h.client
        .restart_session(&session.id, true)
        .await
        .expect("restart the session");
    let (chan, mut rx, mut seen) = attach_ready(&h, &session).await;
    report(&h, chan, &mut rx, &mut seen, "conv-new").await;
    assert_eq!(
        snapshot_of(&h, &session.id)
            .await
            .captured_conversation
            .as_deref(),
        Some("conv-new"),
        "fixture premise: the restarted launch's report landed; {}",
        hook_log(&h, &session.id)
    );

    // Put it back the way a hook writes: a private name, then a rename.
    let temp = slot.with_file_name(".tmp-latest-test");
    std::fs::write(&temp, &stale).expect("stage the old launch's report");
    std::fs::rename(&temp, &slot).expect("put the old launch's report back");
    h.sup.reconcile_for_test().await;
    assert_eq!(
        snapshot_of(&h, &session.id)
            .await
            .captured_conversation
            .as_deref(),
        Some("conv-new"),
        "the old launch's report must not replace the current one; {}",
        hook_log(&h, &session.id)
    );
    assert!(
        !slot.exists(),
        "the old launch's report is settled and removed"
    );
    let log = hook_log(&h, &session.id);
    assert!(
        log.lines().any(|line| line.contains(" refused conflict ")
            && line.contains("cannot be attributed")
            && line.contains(" conv-old ")),
        "the refusal names the launch check: {log}"
    );
}
