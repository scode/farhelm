//! Test when Farhelm can reopen a conversation, using the installed CLI and the launcher's real reporter.

use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;

use farhelm_proto::{AgentKind, RestartReadiness, SessionStatus};
use farhelm_supervisor::agent_kind::{LocatorVendor, parse_locator};

use super::*;

/// The short idle observation catches a report produced at startup when the vendor owes one only after a prompt.
const BEFORE_PROMPT: Duration = Duration::from_secs(3);

/// Look for early Resume while the composer waits for input. A pending locator can be stored before the agent
/// has persisted any conversation to reopen. With hooks off, even a pending report is unexpected.
async fn early_capture(
    env: &DriftEnv<'_>,
    term: &mut Terminal<'_>,
    id: &str,
    turned: bool,
    hooks_off: bool,
) -> Result<Option<String>, DriftError> {
    let deadline = tokio::time::Instant::now() + BEFORE_PROMPT;
    loop {
        term.check(&term.host.screen_tail(SCREEN_BYTES).to_lowercase(), turned)
            .await?;
        let snapshot = env.sup.session_snapshot(id).await?.ok_or_else(|| {
            anyhow::anyhow!("the session vanished while observing the capture boundary")
        })?;
        if snapshot.restart_offer == RestartOffer::Resume
            || (hooks_off && snapshot.captured_conversation.is_some())
        {
            return Ok(Some(term.diagnostics()));
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(None);
        }
        pace().await;
    }
}

/// A pre-prompt capture needs both a resumable row and acknowledgment of that identity before any input is sent.
/// `previous` is an identity that does not count, for a session that keeps an earlier capture across a Restart: the
/// new agent must report a conversation of its own, under another runtime id.
async fn startup_capture(
    env: &DriftEnv<'_>,
    term: &mut Terminal<'_>,
    id: &str,
    log: &Path,
    previous: Option<&str>,
) -> Result<String, DriftError> {
    let deadline = tokio::time::Instant::now() + START_LIMIT;
    loop {
        term.check(&term.host.screen_tail(SCREEN_BYTES).to_lowercase(), false)
            .await?;
        let snapshot =
            env.sup.session_snapshot(id).await?.ok_or_else(|| {
                anyhow::anyhow!("the session vanished before its startup capture")
            })?;
        let log = std::fs::read_to_string(log).unwrap_or_default();
        let last = (
            snapshot.captured_conversation.clone(),
            snapshot.restart_offer,
        );
        if snapshot.restart_offer == RestartOffer::Resume
            && let Some(identity) = snapshot.captured_conversation
            && previous.is_none_or(|previous| {
                reported_runtime_identity(previous) != reported_runtime_identity(&identity)
            })
            && acknowledged_reports(&log).any(|candidate| {
                candidate.identity().is_some_and(|id| {
                    hook_log_names_identity(id, &reported_runtime_identity(&identity))
                })
            })
        {
            return Ok(identity);
        }
        if tokio::time::Instant::now() >= deadline {
            // Say which part was missing: the capture (still the earlier one, when `previous` is set), the Resume
            // offer, or an acknowledgment of the captured id in the hook log.
            let tail: Vec<_> = log.lines().rev().take(8).collect();
            return Err(anyhow::anyhow!(
                "no accepted resumable conversation arrived before input (last capture {:?}, offer {:?}, \
                 earlier capture {previous:?}, recent hook log {tail:?})\n{}",
                last.0,
                last.1,
                term.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// Keep the witness split in the input so a wrapped prompt echo cannot resemble the answer after Enter.
/// Capture the session log at Enter so a callback from readiness cannot count toward this turn.
async fn submit(term: &mut Terminal<'_>, log: &Path) -> Result<(String, String), DriftError> {
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let nonce = &nonce[..8];
    term.type_text(
        &format!("Join FH and {nonce} without spaces. Reply with only that word. No tools."),
        false,
    )
    .await?;
    let baseline = std::fs::read_to_string(log)
        .or_else(|error| match error.kind() {
            std::io::ErrorKind::NotFound => Ok(String::new()),
            _ => Err(error),
        })
        .map_err(anyhow::Error::from)?;
    term.client.send_keys(term.channel, b"\r".to_vec()).await;
    Ok((format!("FH{nonce}"), baseline))
}

/// The checks this category makes for some harnesses only, beyond the steps every harness shares. (The hook's
/// protocol reply, which must never reach the screen, is shared with other categories: `hook_protocol_reply`.)
#[derive(Clone, Copy)]
struct HarnessChecks {
    /// The reported conversation file starts with OMP's title slot, whose width and parsing are checked.
    title_slot_prelude: bool,
    /// Words the agent's process must still carry when hooks are off.
    hooks_off_child_words: &'static [&'static str],
    /// The hooks-off session goes on to Restart onto a conversation captured with hooks on, which Restart must keep,
    /// with the reporter still off and no hook report written.
    hooks_off_resume: bool,
    /// The agent reports its conversation as it starts but saves nothing until the first prompt, so the case presses
    /// Restart at the idle composer, before any prompt, and requires the new agent to come up fresh (SPEC.md,
    /// Restart). It spends no turn: the first turn then runs on the restarted agent.
    restart_before_prompt: bool,
}

/// Each harness states its extra checks, so a newly added one has to decide them rather than skip them by default.
#[warn(clippy::wildcard_enum_match_arm)]
fn checks(harness: LaunchHarness) -> HarnessChecks {
    use LaunchHarness::*;
    let none = HarnessChecks {
        title_slot_prelude: false,
        hooks_off_child_words: &[],
        hooks_off_resume: false,
        restart_before_prompt: false,
    };
    match harness {
        Omp => HarnessChecks {
            title_slot_prelude: true,
            ..none
        },
        // Turning hooks off must not drop `--no-daemon`, which keeps Codex's commands in the session's tree.
        Codex => HarnessChecks {
            hooks_off_child_words: &["--no-daemon"],
            ..none
        },
        // Goose's hooks are turned off again at resume time, so Restart is part of what the hooks-off case checks.
        Goose => HarnessChecks {
            hooks_off_resume: true,
            ..none
        },
        // Claude reports its id at start but writes the transcript only once a prompt is submitted (observed with
        // Claude Code 2.1.332 on 2026-11-26).
        Claude => HarnessChecks {
            restart_before_prompt: true,
            ..none
        },
        Antigravity | Muse | Grok | Pi | Cursor | OpenCode | Bash | Zsh | Custom => none,
    }
}

/// Grok's reports name the conversation by the resume selector rather than a runtime ID that survives enrichment.
#[warn(clippy::wildcard_enum_match_arm)]
fn identity_is_resume_selector(kind: AgentKind) -> bool {
    use AgentKind::*;
    match kind {
        Grok => true,
        Claude | Codex | Goose | Pi | Omp | Antigravity | Muse | Custom | Generic => false,
    }
}

/// The text-only turn uses the same reply boundary as the base drift case.
fn expectation(reply: &str, hook_witness: HookWitness) -> TurnExpectation<'_> {
    TurnExpectation {
        reply,
        acked_before: 0,
        original_argv: None,
        hook_witness,
    }
}

/// Read only the bounded prefix of the reported file. Verify width and parser acceptance on the same bytes even
/// if the vendor has rewritten its title since the report was admitted.
fn check_omp_title(conversation: &str) -> anyhow::Result<()> {
    let locator = parse_locator(LocatorVendor::Omp, conversation)?;
    let file = locator
        .session_file
        .ok_or_else(|| anyhow::anyhow!("OMP capture omitted its session file"))?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(file)?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "OMP reported a non-regular session file"
    );
    let mut bytes = Vec::new();
    file.take(64 * 1024).read_to_end(&mut bytes)?;
    let prefix = String::from_utf8_lossy(&bytes);
    let parser = farhelm_supervisor::agent_kind::integration_for(AgentKind::Omp)
        .expect("OMP has a record parser");
    anyhow::ensure!(
        parser
            .parse_record(&prefix)?
            .is_some_and(|header| header.conversation == locator.session_id),
        "the OMP parser did not accept the captured file's identity"
    );
    let first = prefix.split_inclusive('\n').next().unwrap_or_default();
    let record: serde_json::Value = serde_json::from_str(first)?;
    if record.get("type").and_then(serde_json::Value::as_str) == Some("title") {
        anyhow::ensure!(
            first.len() == 256 && first.ends_with('\n'),
            "OMP title prelude was not a complete 256-byte line"
        );
        println!("the OMP resume parser accepted the live 256-byte title prelude");
    } else {
        println!("OMP wrote its session header without a title prelude");
    }
    Ok(())
}

/// Callback evidence pins the intended phase even when the terminal and supervisor deliver updates out of order.
/// Antigravity's Stop can admit a conversation whose transcript was unreadable when SessionStart ran.
#[warn(clippy::wildcard_enum_match_arm)]
fn turn_reports(harness: LaunchHarness) -> &'static [&'static str] {
    match harness {
        LaunchHarness::Grok => &["event=UserPromptSubmit", "event=Stop"],
        LaunchHarness::Antigravity => &["event=SessionStart", "event=Stop"],
        LaunchHarness::Claude => &["event=Stop"],
        LaunchHarness::Codex | LaunchHarness::Muse => &["startup", "event=Stop"],
        LaunchHarness::Pi | LaunchHarness::Omp => &["agent_end"],
        LaunchHarness::Goose
        | LaunchHarness::Cursor
        | LaunchHarness::OpenCode
        | LaunchHarness::Bash
        | LaunchHarness::Zsh
        | LaunchHarness::Custom => &[],
    }
}

/// The locator can acquire a transcript path after the first callback. Compare the runtime ID that survives
/// that enrichment and the hook log's length cap.
fn turn_identity(snapshot: &SessionSnapshot, conversation: &str) -> anyhow::Result<String> {
    if identity_is_resume_selector(snapshot.kind) {
        return snapshot
            .resume_template
            .as_deref()
            .zip(snapshot.resume_argv.as_deref())
            .and_then(|(template, argv)| conversation_selector(template, argv))
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("the Grok Resume command omitted its conversation"));
    }
    for (prefix, vendor) in [("omp:", LocatorVendor::Omp), ("pi:", LocatorVendor::Pi)] {
        if conversation.starts_with(prefix) {
            return Ok(parse_locator(vendor, conversation)?.session_id);
        }
    }
    Ok(reported_runtime_identity(conversation))
}

/// The event suffix is separated from sanitized vendor fields by a tab. Plain source names occupy their own field.
fn names_report(line: &str, identity: &str, expected: &str) -> bool {
    parse_log_line(line).is_some_and(|entry| {
        entry.word == "written"
            && entry.about.is_some_and(|(reported, source)| {
                hook_log_names_identity(reported, identity)
                    && (entry.suffix == Some(expected)
                        || entry.suffix.is_none()
                            && source == expected
                            && !expected.starts_with("event="))
            })
    })
}

/// A prompt-timed capture must precede the first Stop for that same conversation.
/// A late duplicate of the start report cannot repair a Stop that arrived first.
fn prompt_report_precedes_stop(fresh: &str, identity: &str, prompt_event: &str) -> bool {
    let mut lines = fresh.lines();
    if let Some(first) = lines.find(|line| {
        names_report(line, identity, prompt_event) || names_report(line, identity, "event=Stop")
    }) {
        names_report(first, identity, prompt_event)
            && lines.any(|line| names_report(line, identity, "event=Stop"))
    } else {
        false
    }
}

/// The two Grok callbacks can replace the same durable slot. The hook's written event records each actual payload
/// independently, while the required resumable capture separately proves supervisor admission.
async fn check_turn_reports(
    harness: LaunchHarness,
    term: &mut Terminal<'_>,
    log: &Path,
    baseline: &str,
    snapshot: &SessionSnapshot,
) -> DriftResult {
    let expected = turn_reports(harness);
    if expected.is_empty() {
        return Ok(());
    }
    let conversation = snapshot
        .captured_conversation
        .as_deref()
        .expect("an accepted turn has a conversation");
    let identity = turn_identity(snapshot, conversation)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        term.check(&term.host.screen_tail(SCREEN_BYTES).to_lowercase(), true)
            .await?;
        let text = std::fs::read_to_string(log).map_err(anyhow::Error::from)?;
        let fresh = text
            .strip_prefix(baseline)
            .ok_or_else(|| anyhow::anyhow!("the hook log changed under the pre-submit baseline"))?;
        let missing: Vec<_> = expected
            .iter()
            .filter(|event| {
                !fresh
                    .lines()
                    .any(|line| names_report(line, &identity, event))
            })
            .collect();
        if missing.is_empty() {
            if matches!(
                harness.agent_kind().restart_readiness(),
                Some(RestartReadiness::FirstPrompt | RestartReadiness::FirstPromptWithHooks(_))
            ) && !prompt_report_precedes_stop(fresh, &identity, expected[0])
            {
                return Err(anyhow::anyhow!(
                    "the first matching {} report must precede Stop\n{}\nrecent hook log: {:?}",
                    expected[0],
                    term.diagnostics(),
                    fresh.lines().rev().take(8).collect::<Vec<_>>()
                )
                .into());
            }
            println!(
                "observed matching post-submit reports: {}",
                expected.join(", ")
            );
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "no post-submit report for {missing:?}\n{}\nrecent hook log: {:?}",
                term.diagnostics(),
                fresh.lines().rev().take(8).collect::<Vec<_>>()
            )
            .into());
        }
        pace().await;
    }
}

/// Restart at the idle composer before any prompt and require a fresh agent. The capture must name a transcript that
/// does not exist yet (the premise that makes this the case under test); the Restart must start a new generation whose
/// agent reaches its composer without printing Claude's failed-resume error, and that agent must report its own
/// conversation before input as the first one did, under a new id (the session keeps the old capture across the
/// Restart, so the old report must not count). Returns the new generation's terminal.
async fn restart_before_prompt<'a>(
    env: &DriftEnv<'a>,
    profile: &Harness,
    id: &str,
    log: &Path,
) -> Result<Terminal<'a>, DriftError> {
    let before = env
        .sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the session vanished before the pre-prompt Restart"))?;
    let stored = before
        .captured_conversation
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no capture before the pre-prompt Restart"))?;
    let transcript = parse_locator(LocatorVendor::Claude, stored)
        .map_err(|error| {
            anyhow::anyhow!("the capture carries no reported transcript ({error}): {stored}")
        })?
        .session_file
        .ok_or_else(|| anyhow::anyhow!("the capture's locator names no transcript: {stored}"))?;
    if Path::new(&transcript).exists() {
        return Err(anyhow::anyhow!(
            "the premise failed: Claude saved {transcript} before any prompt, so this Restart would resume"
        )
        .into());
    }
    env.client.restart_session(id, true).await?;
    let after = env
        .sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the pre-prompt Restart lost the session"))?;
    if after.generation <= before.generation {
        return Err(
            anyhow::anyhow!("the pre-prompt Restart did not start a new generation").into(),
        );
    }
    // No turn has run yet, so a sign-in screen here is a missing prerequisite, not a failure.
    let mut term = Terminal::attach(env.sup, env.client, id, false).await?;
    // A Restart that resumed instead would show Claude's failed-resume error, and Claude exits on it before its
    // composer appears, so the error is named where readiness fails.
    if let Err(error) = term.ready(profile, TrustPolicy::AutoAnswer, false).await {
        if term
            .diagnostics()
            .to_lowercase()
            .contains("no conversation found")
        {
            return Err(anyhow::anyhow!(
                "the restarted Claude tried to resume a conversation it never saved\n{}",
                term.diagnostics()
            )
            .into());
        }
        return Err(error);
    }
    // The restarted process must be the start command. 2.1.332 exits on `--resume` of an unknown id, which `ready`
    // catches, but a later Claude that started a new conversation there instead would pass every other check here.
    check_child_lacks_words(&term, &["--resume", &reported_runtime_identity(stored)])?;
    startup_capture(env, &mut term, id, log, Some(stored)).await?;
    Ok(term)
}

/// Exercise the expected pre-input boundary, then the first real turn under the active reporter.
async fn enabled(
    harness: LaunchHarness,
    profile: &Harness,
    capture: &mut Option<String>,
) -> DriftResult {
    let readiness = harness
        .agent_kind()
        .restart_readiness()
        .expect("the category contains tracked agents");
    let witness = profile
        .hook_witness
        .expect("the harness reports a conversation");
    with_drift_supervisor(
        profile,
        ordinary_selection(harness, profile),
        None,
        async |env, launch| {
            env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
                let mut term = Terminal::attach(env.sup, env.client, id, false).await?;
                term.ready(profile, TrustPolicy::AutoAnswer, false).await?;
                hook_invisible(harness, &term)?;
                let log = env.state.join("hook-log").join(format!("{id}.log"));
                let premature = match readiness {
                    RestartReadiness::AtStart | RestartReadiness::ShortlyAfterStart => {
                        startup_capture(&env, &mut term, id, &log, None).await?;
                        println!("captured before the first prompt");
                        if checks(harness).restart_before_prompt {
                            term = restart_before_prompt(&env, profile, id, &log).await?;
                            println!("restarted before the first prompt; the new agent started fresh");
                        }
                        None
                    }
                    RestartReadiness::FirstPrompt
                    | RestartReadiness::FirstPromptWithHooks(_)
                    | RestartReadiness::FirstReply => {
                        // The pre-prompt Restart needs a capture before the prompt; a harness that stops reporting
                        // at start must not silently skip it.
                        if checks(harness).restart_before_prompt {
                            return Err(anyhow::anyhow!(
                                "the pre-prompt Restart check is set, but this harness reports only after input"
                            )
                            .into());
                        }
                        let early = early_capture(&env, &mut term, id, false, false).await?;
                        if let Some(screen) = &early {
                            let reports = std::fs::read_to_string(&log).unwrap_or_default();
                            println!("Resume was available before the first prompt\n{screen}\nrecent hook log: {:?}", reports.lines().rev().take(8).collect::<Vec<_>>());
                        } else {
                            println!("Resume stayed unavailable at the idle composer before the first prompt");
                        }
                        early
                    }
                };
                let (reply, baseline) = submit(&mut term, &log).await?;
                let snapshot =
                    wait_for_turn(&mut term, env.sup, id, &log, &expectation(&reply, witness))
                        .await?;
                check_turn_reports(harness, &mut term, &log, &baseline, &snapshot).await?;
                let conversation = snapshot.captured_conversation.expect("an accepted turn has a conversation");
                if checks(harness).title_slot_prelude {
                    check_omp_title(&conversation)?;
                }
                *capture = Some(conversation);
                hook_invisible(harness, &term)?;
                if premature.is_some() {
                    return Err(anyhow::anyhow!("Resume was available before the first prompt").into());
                }
                Ok(())
            })
            .await
        },
    )
    .await
}

/// Wait for the reply and for the supervisor's terminal samples to settle. A visible answer alone can precede a
/// turn-end callback while the agent is still streaming. The quiet window that follows also checks for a late report.
async fn wait_for_disabled_reply(
    env: &DriftEnv<'_>,
    term: &mut Terminal<'_>,
    id: &str,
    reply: &str,
    witness: HookWitness,
) -> DriftResult {
    let deadline = tokio::time::Instant::now() + TURN_LIMIT;
    let expected = expectation(reply, witness);
    let mut saw_reply = false;
    loop {
        let screen = term.host.screen_tail(SCREEN_BYTES);
        saw_reply |= expected.saw_reply(&screen);
        term.check(&screen.to_lowercase(), saw_reply).await?;
        if saw_reply {
            let current = env
                .client
                .list_sessions()
                .await?
                .sessions
                .into_iter()
                .find(|session| session.id == id)
                .ok_or_else(|| anyhow::anyhow!("the hooks-off session vanished after its reply"))?;
            if matches!(current.status, SessionStatus::Idle | SessionStatus::Waiting) {
                return Ok(());
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "hooks-off launch did not complete its text turn\n{}",
                term.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// The owned CLI child's command line holds none of `words` as a whole argument. Like [`check_child_words`], the
/// child is revalidated around the read and the command line stays out of the error.
fn check_child_lacks_words(term: &Terminal<'_>, words: &[&str]) -> anyhow::Result<()> {
    let bytes = read_child_proc(term, "cmdline")?;
    for word in words {
        anyhow::ensure!(
            !bytes
                .split(|byte| *byte == 0)
                .any(|entry| entry == word.as_bytes()),
            "the restarted CLI child was given {word:?}: it resumed rather than starting fresh"
        );
    }
    Ok(())
}

/// No callback should write a report in this private supervisor. A missing log is the normal case; I/O failures
/// other than absence are evidence failures.
fn require_no_report(log: &Path) -> anyhow::Result<()> {
    match std::fs::read_to_string(log) {
        Ok(text) => anyhow::ensure!(text.is_empty(), "a callback ran with hooks off"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

/// Reuse a genuine Goose conversation to reach Restart after proving the disabled session remains uncaptured.
/// The test seam supplies the saved identity. The actual restart and its reporter environment are production behavior.
async fn check_goose_resume(
    env: &DriftEnv<'_>,
    profile: &Harness,
    id: &str,
    capture: &str,
    log: &Path,
) -> DriftResult {
    env.client.stop_session(id).await?;
    env.sup.record_conversation_for_test(id, capture).await;
    let stopped = env
        .sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the Goose session vanished before Restart"))?;
    if stopped.restart_offer != RestartOffer::Resume
        || stopped.captured_conversation.as_deref() != Some(capture)
    {
        return Err(anyhow::anyhow!(
            "the stopped Goose session did not retain the supplied conversation"
        )
        .into());
    }
    env.client.restart_session(id, false).await?;
    let mut resumed = Terminal::attach(env.sup, env.client, id, true).await?;
    // The captured conversation came from the other disposable workspace. Keep the current directory when Goose
    // offers to move back to the previous launch's already-closed workspace.
    let deadline = tokio::time::Instant::now() + START_LIMIT;
    loop {
        let screen = resumed.host.screen_tail(SCREEN_BYTES);
        let text = screen.to_lowercase();
        resumed.check(&text, true).await?;
        if text.contains("original working directory of this session") && text.contains("● yes") {
            resumed
                .client
                .send_keys(resumed.channel, b"\x1b[C".to_vec())
                .await;
        } else if text.contains("original working directory of this session")
            && text.contains("● no")
        {
            resumed
                .client
                .send_keys(resumed.channel, b"\r".to_vec())
                .await;
        } else if profile.composer.iter().any(|marker| text.contains(marker)) {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "the Goose Restart composer did not become ready\n{}",
                resumed.diagnostics()
            )
            .into());
        }
        pace().await;
    }
    let after = env
        .sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the Goose session vanished after Restart"))?;
    if after.generation <= stopped.generation
        || after.captured_conversation != stopped.captured_conversation
    {
        return Err(anyhow::anyhow!(
            "Goose Restart did not retain the conversation in a new generation"
        )
        .into());
    }
    check_child_words(&resumed, "cmdline", &["--resume", "--session-id", capture])?;
    check_child_words(&resumed, "environ", &["FARHELM_GOOSE_REPORTER_ENABLED=0"])?;
    hook_invisible(LaunchHarness::Goose, &resumed)?;
    require_no_report(log)?;
    println!("Goose Restart kept its reporter off");
    Ok(())
}

/// Exercise the opt-out through the supervisor even for agents that keep their reporter in vendor configuration.
async fn disabled(harness: LaunchHarness, profile: &Harness, capture: Option<&str>) -> DriftResult {
    let witness = profile
        .hook_witness
        .expect("the harness reports a conversation");
    with_drift_supervisor_hooks(
        profile,
        ordinary_selection(harness, profile),
        None,
        AgentHooks::None,
        async |env, launch| {
            env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
                let mut term = Terminal::attach(env.sup, env.client, id, false).await?;
                term.ready(profile, TrustPolicy::AutoAnswer, false).await?;
                if let Some(screen) = early_capture(&env, &mut term, id, false, true).await? {
                    return Err(
                        anyhow::anyhow!("hooks-off startup reported a capture\n{screen}").into(),
                    );
                }
                let hooks_off_words = checks(harness).hooks_off_child_words;
                if !hooks_off_words.is_empty() {
                    check_child_words(&term, "cmdline", hooks_off_words)?;
                }
                let log = env.state.join("hook-log").join(format!("{id}.log"));
                let (reply, _) = submit(&mut term, &log).await?;
                wait_for_disabled_reply(&env, &mut term, id, &reply, witness).await?;
                if let Some(screen) = early_capture(&env, &mut term, id, true, true).await? {
                    return Err(
                        anyhow::anyhow!("hooks-off turn reported a capture\n{screen}").into(),
                    );
                }
                require_no_report(&log)?;
                hook_invisible(harness, &term)?;
                println!("no callback or capture on hooks-off startup or turn");
                if checks(harness).hooks_off_resume
                    && let Some(conversation) = capture
                {
                    check_goose_resume(&env, profile, id, conversation, &log).await?;
                }
                Ok(())
            })
            .await
        },
    )
    .await
}

/// A failed timing observation should still give the independent hooks-off path a chance to produce evidence.
async fn execute(harness: LaunchHarness) -> DriftResult {
    let profile = profile(harness).expect("a capture-timing case has a drift profile");
    let mut capture = None;
    let active = enabled(harness, &profile, &mut capture).await;
    if matches!(active, Err(DriftError::CouldNotRun(_))) {
        return active;
    }
    let disabled = disabled(harness, &profile, capture.as_deref()).await;
    match (active, disabled) {
        (Ok(()), result) | (result, Ok(())) => result,
        (Err(active), Err(disabled)) => {
            Err(anyhow::anyhow!("hooks on: {active:?}\nhooks off: {disabled:?}").into())
        }
    }
}

drift_cases! {
    execute;
    drift_claude => Claude,
    drift_goose => Goose,
    drift_codex => Codex,
    drift_muse => Muse,
    drift_grok => Grok,
    drift_antigravity => Antigravity,
    drift_pi => Pi,
    drift_omp => Omp,
}

/// Adding a timing promise to the launcher also needs a live case that exercises it.
#[farhelm_testtrace::test]
fn capture_timing_covers_every_tracked_builtin_agent() {
    let expected: Vec<_> = LaunchHarness::BUILT_INS
        .iter()
        .copied()
        .filter(|harness| harness.agent_kind().restart_readiness().is_some())
        .collect();
    assert_eq!(HARNESSES.len(), expected.len());
    for harness in expected {
        assert!(
            HARNESSES.contains(&harness),
            "missing capture case for {harness:?}"
        );
    }
}

/// The pre-prompt Restart needs a capture before any input, so its flag is only meaningful for a harness that reports
/// at or shortly after start; set elsewhere, the live case fails at run time, which this catches without a vendor CLI.
/// Specified: every built-in harness with the flag set reports at or shortly after start.
#[farhelm_testtrace::test]
fn the_pre_prompt_restart_is_set_only_where_capture_comes_before_input() {
    for harness in LaunchHarness::BUILT_INS.iter().copied() {
        if !checks(harness).restart_before_prompt {
            continue;
        }
        assert!(
            matches!(
                harness.agent_kind().restart_readiness(),
                Some(RestartReadiness::AtStart | RestartReadiness::ShortlyAfterStart)
            ),
            "{harness:?} sets the pre-prompt Restart but does not report before input"
        );
    }
}

/// A source can resemble a callback name. Only a real written event for the captured conversation proves delivery.
#[farhelm_testtrace::test]
fn turn_evidence_distinguishes_a_written_event_from_the_payload_source() {
    for line in [
        "17 written conv-1 event=Stop",
        "17 written conv-2 -\tevent=Stop",
        "17 acked conv-1 -\tevent=Stop",
    ] {
        assert!(!names_report(line, "conv-1", "event=Stop"), "{line}");
    }
    assert!(names_report(
        "17 written conv-1 -\tevent=Stop",
        "conv-1",
        "event=Stop"
    ));
    assert!(names_report(
        "17 written conv-1 startup",
        "conv-1",
        "startup"
    ));
    assert!(!prompt_report_precedes_stop(
        "17 written conv-1 startup\tevent=Stop\n18 written conv-1 -\tevent=Stop",
        "conv-1",
        "startup",
    ));
}

/// A report from a different conversation or from after the first matching Stop cannot witness prompt capture.
#[farhelm_testtrace::test]
fn prompt_timing_uses_the_first_matching_stop() {
    for (harness, prompt) in [
        (LaunchHarness::Codex, "17 written conv-1 startup"),
        (LaunchHarness::Muse, "17 written conv-1 startup"),
        (
            LaunchHarness::Grok,
            "17 written conv-1 -\tevent=UserPromptSubmit",
        ),
        (
            LaunchHarness::Antigravity,
            "17 written conv-1 -\tevent=SessionStart",
        ),
    ] {
        let event = turn_reports(harness)[0];
        let stop = "18 written conv-1 -\tevent=Stop";
        let unrelated = "16 written conv-2 -\tevent=Stop";
        assert!(prompt_report_precedes_stop(
            &format!("{unrelated}\n{prompt}\n{stop}"),
            "conv-1",
            event
        ));
        assert!(!prompt_report_precedes_stop(
            &format!("{stop}\n{prompt}\n{stop}"),
            "conv-1",
            event
        ));
        assert!(!prompt_report_precedes_stop(prompt, "conv-1", event));
    }
}
