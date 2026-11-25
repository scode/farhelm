//! A launch prompt is a first turn on the fresh or named conversation. Restart restores the conversation
//! without giving that prompt another turn.

use super::*;
use farhelm_helm::CreateExtras;
use farhelm_proto::SessionLaunch;

/// The first line starts with an option-shaped token and holds the word the second line needs. The expected
/// answer never occurs as a contiguous string in the launch arguments or the TUI's input echo.
struct PromptProbe {
    word: String,
    initial: String,
    reply: String,
}

impl PromptProbe {
    /// An unpredictable answer keeps prior conversations and generic acknowledgments out of the oracle.
    fn new() -> Self {
        let random = uuid::Uuid::new_v4().simple().to_string();
        let word = format!("FH{}", random[..8].to_ascii_uppercase());
        let initial = format!(
            "-- Remember the test word {word}\nReply with FIRST, a hyphen, and the test word from the previous line. If there is no previous line, reply SINGLELINE. No tools."
        );
        let reply = format!("FIRST-{word}");
        Self {
            word,
            initial,
            reply,
        }
    }
}

/// Use the ordinary launch compiler and test workspace for both starts. A CLI that preserves a conversation
/// goes through the same named-start compiler Farhelm uses for a new session.
async fn execute(harness: LaunchHarness) -> DriftResult {
    let profile = profile(harness).expect("an initial-prompt case has a drift profile");
    let selection = LaunchSelection {
        custom_agent: None,
        harness,
        model: profile.model.map(str::to_owned),
        effort: profile.effort.map(Into::into),
        permissions: None,
        permission_flags: None,
        wrapper_prefix: None,
        container: None,
        workspace_trust: profile.trust_at_launch.then_some(true),
    };
    let probe = PromptProbe::new();
    with_drift_supervisor(&profile, selection, None, async |env, launch| {
        let selector = env
            .with_session(
                launch.clone(),
                CreateExtras {
                    prompt: Some(probe.initial.clone()),
                    ..CreateExtras::default()
                },
                async |id| check_fresh(&env, &profile, id, &probe).await,
            )
            .await?;
        if harness.supports_initial_prompt(true) {
            let selector = selector.ok_or_else(|| {
                DriftError::CouldNotRun(
                    "fresh prompt passed, but this CLI did not expose a conversation selector for the test workspace"
                        .into(),
                )
            })?;
            check_named(&env, &profile, launch, selector, &probe).await?;
            println!("named conversation launch prompt confirmed");
        }
        Ok(())
    })
    .await
}

/// Observe the launched turn without typing at the composer. Tracked agents must also report an accepted
/// conversation before Restart can exercise its saved command.
async fn check_fresh(
    env: &DriftEnv<'_>,
    profile: &Harness,
    id: &str,
    probe: &PromptProbe,
) -> Result<Option<String>, DriftError> {
    let mut terminal = Terminal::attach(env.sup, env.client, id, false).await?;
    terminal
        .ready(profile, TrustPolicy::AutoAnswer, false)
        .await?;
    // The model can infer the answer if its input turns the newline into a space. Check the literal value
    // at the child boundary as well as the automatic reply. Flag values may be attached with `=`.
    let argv = live_child_argv(&terminal, "fresh launch")?;
    if !argv.split(|&byte| byte == 0).any(|arg| {
        arg == probe.initial.as_bytes()
            || arg
                .strip_suffix(probe.initial.as_bytes())
                .is_some_and(|prefix| prefix.ends_with(b"="))
    }) {
        return Err(anyhow::anyhow!(
            "the fresh process did not receive the literal prompt as one argument"
        )
        .into());
    }
    let snapshot = await_prompt_reply(env, profile, id, &mut terminal, &probe.reply, None).await?;
    terminal
        .ready(profile, TrustPolicy::AutoAnswer, true)
        .await?;
    println!("fresh interactive launch prompt confirmed");
    let Some(snapshot) = snapshot else {
        return untracked_selector(env, profile).await;
    };
    drop(terminal);
    let resume = snapshot
        .resume_argv
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("a reported launch has no resume command"))?;
    let template = snapshot
        .resume_template
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("a reported launch has no resume template"))?;
    let selector = conversation_selector(template, resume)
        .ok_or_else(|| anyhow::anyhow!("the saved resume command has no conversation selector"))?;
    check_restart(env, profile, id, &snapshot, selector, probe).await?;
    println!("Restart returned to the composer without the initial prompt");
    Ok(env
        .harness
        .supports_initial_prompt(true)
        .then(|| selector.to_owned()))
}

/// Both the automatic answer and the accepted report matter for tracked starts. An untracked agent is judged
/// on the same reply oracle without a report, using its own visible screen and post-replay output.
async fn await_prompt_reply(
    env: &DriftEnv<'_>,
    profile: &Harness,
    id: &str,
    terminal: &mut Terminal<'_>,
    reply: &str,
    original_argv: Option<&[String]>,
) -> Result<Option<SessionSnapshot>, DriftError> {
    let expected = TurnExpectation {
        reply,
        acked_before: 0,
        original_argv,
        hook_witness: profile.hook_witness.unwrap_or(HookWitness::Fresh),
    };
    if profile.hook_witness.is_some() {
        let hook_log = env.state.join("hook-log").join(format!("{id}.log"));
        return wait_for_turn(terminal, env.sup, id, &hook_log, &expected)
            .await
            .map(Some);
    }
    let deadline = tokio::time::Instant::now() + TURN_LIMIT;
    loop {
        let screen = terminal.host.screen_tail(SCREEN_BYTES);
        terminal
            .check(&screen.to_lowercase(), original_argv.is_some())
            .await?;
        if expected.saw_reply(&screen) || expected.saw_reply(&terminal.output.tail(SCREEN_BYTES)) {
            return Ok(None);
        }
        if tokio::time::Instant::now() >= deadline {
            let start = if original_argv.is_some() {
                "named"
            } else {
                "fresh"
            };
            return Err(anyhow::anyhow!(
                "no automatic reply to the {start} launch prompt\n{}",
                terminal.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// Read the attached host's pinned child. Verify the kernel start identity again after reading `/proc` so
/// an exited process and reused PID cannot satisfy a launch assertion. Keep raw argv out of diagnostics.
fn live_child_argv(terminal: &Terminal<'_>, phase: &str) -> Result<Vec<u8>, DriftError> {
    let Some(child) = terminal.host.child_root() else {
        return Err(anyhow::anyhow!(
            "{phase} has no live child with a verified process identity\n{}",
            terminal.diagnostics()
        )
        .into());
    };
    let argv =
        std::fs::read(format!("/proc/{}/cmdline", child.pid)).map_err(anyhow::Error::from)?;
    if argv.is_empty() || terminal.host.child_root() != Some(child) {
        return Err(
            anyhow::anyhow!("the {phase} child changed before its argv could be checked").into(),
        );
    }
    Ok(argv)
}

/// Restart must use the saved selector in a new process. Some vendors redraw the previous conversation
/// at startup, so one restored reply is allowed. A second visible reply or the test word in the Restart
/// command is evidence of an unwanted repeated prompt.
async fn check_restart(
    env: &DriftEnv<'_>,
    profile: &Harness,
    id: &str,
    before: &SessionSnapshot,
    selector: &str,
    probe: &PromptProbe,
) -> DriftResult {
    let resume = before
        .resume_argv
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("the original session has no resume argv"))?;
    if resume.iter().any(|arg| arg.contains(&probe.word)) {
        return Err(anyhow::anyhow!("the saved resume argv contains the initial prompt").into());
    }
    env.client.stop_session(id).await?;
    env.client.restart_session(id, false).await?;
    let after = env
        .sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Restart lost the session"))?;
    if after.generation <= before.generation || after.resume_argv.as_deref() != Some(resume) {
        return Err(anyhow::anyhow!(
            "Restart did not preserve the resume command in a new generation"
        )
        .into());
    }
    let mut terminal = Terminal::attach(env.sup, env.client, id, true).await?;
    terminal
        .ready(profile, TrustPolicy::AutoAnswer, true)
        .await?;
    let argv = live_child_argv(&terminal, "Restart")?;
    if !argv
        .split(|&byte| byte == 0)
        .any(|arg| arg == selector.as_bytes())
    {
        return Err(anyhow::anyhow!(
            "the Restart process did not receive the saved conversation selector"
        )
        .into());
    }
    if String::from_utf8_lossy(&argv).contains(&probe.word) {
        return Err(
            anyhow::anyhow!("the restarted process received the one-time prompt again").into(),
        );
    }
    let expected = TurnExpectation {
        reply: &probe.reply,
        acked_before: 0,
        original_argv: None,
        hook_witness: HookWitness::Fresh,
    };
    let screen = terminal.host.screen_tail(SCREEN_BYTES);
    if screen
        .lines()
        .filter(|line| expected.saw_reply(line))
        .take(2)
        .count()
        > 1
    {
        return Err(anyhow::anyhow!(
            "Restart displayed the initial reply twice\n{}",
            terminal.diagnostics()
        )
        .into());
    }
    Ok(())
}

/// The named launch requests a distinct answer and supplies no code word. The first reply or an echo of these
/// arguments can never satisfy the recall check.
async fn check_named(
    env: &DriftEnv<'_>,
    profile: &Harness,
    launch: SessionLaunch,
    selector: String,
    probe: &PromptProbe,
) -> DriftResult {
    let named_start =
        farhelm_helm::compile_named_start(&launch, selector).map_err(anyhow::Error::msg)?;
    let named_argv = named_start.argv.clone();
    let recall = format!("RECALL-{}", probe.word);
    env.with_session(
        launch,
        CreateExtras {
            named_start: Some(named_start),
            prompt: Some(RECALL_PROMPT.to_owned()),
            ..CreateExtras::default()
        },
        async |id| {
            let mut terminal = Terminal::attach(env.sup, env.client, id, true).await?;
            terminal
                .ready(profile, TrustPolicy::AutoAnswer, true)
                .await?;
            await_prompt_reply(env, profile, id, &mut terminal, &recall, Some(&named_argv)).await?;
            terminal.ready(profile, TrustPolicy::AutoAnswer, true).await
        },
    )
    .await
}

/// OpenCode must list one conversation for the workspace after the fresh turn. A missing or ambiguous match
/// fails the case. Cursor exposes a resume UI, so the named prompt check has no selector to use there.
async fn untracked_selector(
    env: &DriftEnv<'_>,
    profile: &Harness,
) -> Result<Option<String>, DriftError> {
    use LaunchHarness::*;
    match env.harness {
        OpenCode => opencode_workspace_session(env.work, profile.binary)
            .await
            .map(Some),
        Cursor | Claude | Codex | Antigravity | Goose | Grok | Muse | Pi | Omp | Bash | Zsh
        | Custom => Ok(None),
    }
}

drift_cases! {
    execute;
    drift_claude => Claude,
    drift_codex => Codex,
    drift_antigravity => Antigravity,
    drift_grok => Grok,
    drift_muse => Muse,
    drift_omp => Omp,
    drift_cursor => Cursor,
    drift_opencode => OpenCode,
}

/// Changes to the shipped launch capability must account for the drift case that exercises it.
#[farhelm_testtrace::test]
fn initial_prompt_covers_every_builtin_harness_that_accepts_it() {
    let expected: Vec<_> = LaunchHarness::BUILT_INS
        .iter()
        .copied()
        .filter(|harness| harness.supports_initial_prompt(false))
        .collect();
    assert_eq!(HARNESSES.len(), expected.len());
    for harness in expected {
        assert_eq!(HARNESSES.iter().filter(|&&case| case == harness).count(), 1);
    }
}
