//! A named start must recall a conversation seeded by the test. Only harnesses that report a named start may
//! offer a later plain Restart. Goose's named Restart must also clear the capture from its previous launch.

use super::*;
use farhelm_helm::CreateExtras;
use farhelm_proto::SessionLaunch;
use std::path::PathBuf;
use std::process::Stdio;

/// The vendor's ID-discovery command is part of the fixture, with its own deadline inside the whole case.
const LOOKUP_LIMIT: Duration = Duration::from_secs(30);

/// OpenCode's composer wraps well before the terminal width. Keep the input short enough to verify its echo
/// before submitting. The echo lacks the random label needed to match a recall reply.
const NAMED_RECALL_PROMPT: &str = "Reply RECALL- then my label. No spaces or tools.";

/// Use the same launch choices as the base category so the named selector is the change under test.
async fn execute(harness: LaunchHarness) -> DriftResult {
    let profile = profile(harness).expect("a named-conversation case has a drift profile");
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
    let code_word = new_code_word();
    with_drift_supervisor(&profile, selection, None, async |env, launch| {
        if harness.captures_named_conversation() {
            check_capturing_harness(&env, &profile, launch, &code_word).await
        } else {
            use LaunchHarness::*;
            match harness {
                Goose => check_goose(&env, &profile, launch, &code_word).await,
                Cursor | OpenCode => {
                    check_without_capture(&env, &profile, launch, &code_word).await
                }
                Claude | Codex | Antigravity | Muse | Pi | Omp | Grok | Bash | Zsh | Custom => {
                    Err(anyhow::anyhow!("no named-start check for {harness:?}").into())
                }
            }
        }
    })
    .await
}

/// Read the resumable target out of the supervisor's argv. Stored capture locators may also contain provenance.
fn selector(snapshot: &SessionSnapshot) -> Result<&str, DriftError> {
    snapshot
        .resume_template
        .as_deref()
        .zip(snapshot.resume_argv.as_deref())
        .and_then(|(template, argv)| conversation_selector(template, argv))
        .ok_or_else(|| anyhow::anyhow!("the accepted capture has no conversation selector").into())
}

/// Observe a stopped session's report before using its selector in a new launch.
async fn seed_capture(
    env: &DriftEnv<'_>,
    profile: &Harness,
    id: &str,
    code_word: &str,
    hook_witness: HookWitness,
) -> Result<SessionSnapshot, DriftError> {
    let mut terminal = Terminal::attach(env.sup, env.client, id, false).await?;
    terminal
        .ready(profile, TrustPolicy::AutoAnswer, false)
        .await?;
    send_prompt(&mut terminal, &seed_prompt(code_word), code_word, false).await?;
    let captured = wait_for_turn(
        &mut terminal,
        env.sup,
        id,
        &env.state.join("hook-log").join(format!("{id}.log")),
        &TurnExpectation {
            reply: code_word,
            acked_before: 0,
            original_argv: None,
            hook_witness,
        },
    )
    .await?;
    env.client.stop_session(id).await?;
    Ok(captured)
}

/// Pi and OMP get both path forms. Make the relative fixture before the first recall can append to the seed file.
async fn check_capturing_harness(
    env: &DriftEnv<'_>,
    profile: &Harness,
    launch: SessionLaunch,
    code_word: &str,
) -> DriftResult {
    let hook_witness = profile
        .hook_witness
        .expect("a capturing harness has a hook witness");
    let captured = env
        .with_session(launch.clone(), CreateExtras::default(), async |id| {
            seed_capture(env, profile, id, code_word, hook_witness).await
        })
        .await?;
    let target = selector(&captured)?;
    if env.harness.named_conversation_is_file() {
        let absolute = env
            .work
            .join(target)
            .canonicalize()
            .map_err(anyhow::Error::from)?;
        let relative = "named-drift-seed.jsonl";
        std::fs::copy(&absolute, env.work.join(relative)).map_err(anyhow::Error::from)?;
        check_named_capture(
            env,
            profile,
            launch.clone(),
            &absolute.to_string_lossy(),
            code_word,
            hook_witness,
        )
        .await?;
        check_named_capture(env, profile, launch, relative, code_word, hook_witness).await
    } else {
        check_named_capture(env, profile, launch, target, code_word, hook_witness).await
    }
}

/// Require both a fresh reply and an accepted hook that makes the new session resume the selected ID or file.
async fn check_named_capture(
    env: &DriftEnv<'_>,
    profile: &Harness,
    launch: SessionLaunch,
    target: &str,
    code_word: &str,
    hook_witness: HookWitness,
) -> DriftResult {
    env.with_session(launch.clone(), named_extras(&launch, target)?, async |id| {
        let mut terminal = Terminal::attach(env.sup, env.client, id, true).await?;
        terminal
            .ready(profile, TrustPolicy::AutoAnswer, true)
            .await?;
        let reply = format!("RECALL-{code_word}");
        send_prompt(&mut terminal, NAMED_RECALL_PROMPT, &reply, true).await?;
        let captured = wait_for_turn(
            &mut terminal,
            env.sup,
            id,
            &env.state.join("hook-log").join(format!("{id}.log")),
            &TurnExpectation {
                reply: &reply,
                acked_before: 0,
                original_argv: None,
                hook_witness,
            },
        )
        .await
        .map_err(|error| match error {
            // This launch has a fresh hook log and a completed seed turn. A refusal during recall is a failure.
            DriftError::CouldNotRun(reason) => anyhow::anyhow!(
                "the named start could not recall after a successful seed: {reason}\n{}",
                terminal.diagnostics()
            )
            .into(),
            error => error,
        })?;
        let reported = selector(&captured)?;
        let matches = if env.harness.named_conversation_is_file() {
            env.work
                .join(reported)
                .canonicalize()
                .map_err(anyhow::Error::from)?
                == env
                    .work
                    .join(target)
                    .canonicalize()
                    .map_err(anyhow::Error::from)?
        } else {
            reported == target
        };
        if !matches {
            return Err(anyhow::anyhow!("the new capture names a different conversation").into());
        }
        Ok(())
    })
    .await
}

/// Restart a captured Goose row into a separate test conversation. Its label proves that named Restart used the
/// requested target, while the row's old capture makes the clearing assertion meaningful.
async fn check_goose(
    env: &DriftEnv<'_>,
    profile: &Harness,
    launch: SessionLaunch,
    code_word: &str,
) -> DriftResult {
    let previous_word = new_code_word();
    let target = env
        .with_session(launch.clone(), CreateExtras::default(), async |id| {
            seed_capture(env, profile, id, code_word, HookWitness::Fresh).await
        })
        .await?;
    let target = selector(&target)?;
    env.with_session(launch.clone(), CreateExtras::default(), async |id| {
        let seed = seed_capture(env, profile, id, &previous_word, HookWitness::Fresh).await?;
        let stopped = snapshot(env, id).await?;
        ensure_seed_still_captured(&seed, &stopped)?;
        if selector(&stopped)? == target {
            return Err(anyhow::anyhow!(
                "Goose used the same selector for both seed conversations"
            )
            .into());
        }
        let named = farhelm_helm::compile_named_start(&launch, target.to_owned())
            .map_err(anyhow::Error::msg)?;
        // The named selector is part of Restart with an explicit launch, even when the choices stay the same.
        env.client
            .restart_session_named(id, false, Some(launch), None, Some(named))
            .await?;
        let restarted = snapshot(env, id).await?;
        if restarted.generation <= stopped.generation {
            return Err(anyhow::anyhow!("Goose did not start a new generation").into());
        }
        // The saved reporter may already have run. Retain the immediate observation, then check recall
        // so a fast report cannot conceal an independent failure to select the named destination.
        let cleared = ensure_uncaptured(&restarted)
            .map_err(|error| error.context("immediately after named Restart"));
        recall_without_capture(env, profile, id, code_word).await?;
        cleared.map_err(Into::into)
    })
    .await
}

/// Stop must leave the ordinary launch's old capture in place for the named Restart to clear.
fn ensure_seed_still_captured(
    seed: &SessionSnapshot,
    stopped: &SessionSnapshot,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        stopped.restart_offer == RestartOffer::Resume
            && stopped.captured_conversation == seed.captured_conversation,
        "the seed capture was gone before named Restart"
    );
    Ok(())
}

/// Cursor can allocate an ID before starting the seed. OpenCode creates an ID on the first turn, then the list
/// can identify it using the test's fresh working directory.
async fn check_without_capture(
    env: &DriftEnv<'_>,
    profile: &Harness,
    launch: SessionLaunch,
    code_word: &str,
) -> DriftResult {
    use LaunchHarness::*;
    let cursor_id = match env.harness {
        Cursor => {
            let output = vendor_lookup(env.work, profile.binary, &["create-chat"], false).await?;
            let id = output.trim();
            uuid::Uuid::parse_str(id)
                .map_err(|_| anyhow::anyhow!("Cursor create-chat did not return a chat UUID"))?;
            Some(id.to_owned())
        }
        // OpenCode's ID is found after the seed turn, from its session list.
        OpenCode => None,
        Claude | Codex | Antigravity | Muse | Goose | Pi | Omp | Grok | Bash | Zsh | Custom => {
            return Err(anyhow::anyhow!(
                "no way to find an uncaptured conversation for {:?}",
                env.harness
            )
            .into());
        }
    };
    let extras = cursor_id
        .as_deref()
        .map(|id| named_extras(&launch, id))
        .transpose()?
        .unwrap_or_default();
    env.with_session(launch.clone(), extras, async |id| {
        let mut terminal = Terminal::attach(env.sup, env.client, id, false).await?;
        terminal
            .ready(profile, TrustPolicy::AutoAnswer, false)
            .await?;
        send_prompt(&mut terminal, &seed_prompt(code_word), code_word, false).await?;
        wait_for_reply(&mut terminal, code_word, false).await?;
        env.client.stop_session(id).await?;
        Ok(())
    })
    .await?;

    let target = match cursor_id {
        Some(id) => id,
        None => {
            let output = vendor_lookup(
                env.work,
                profile.binary,
                &[
                    "session",
                    "list",
                    "--pure",
                    "--format",
                    "json",
                    "--max-count",
                    "32",
                ],
                true,
            )
            .await?;
            opencode_session(&output, env.work)?
        }
    };
    env.with_session(
        launch.clone(),
        named_extras(&launch, &target)?,
        async |id| recall_without_capture(env, profile, id, code_word).await,
    )
    .await
}

/// Read only the two list fields used to identify the fixture. A recent session in another directory belongs to
/// someone else, and multiple matches in our directory cannot prove which conversation received the seed.
fn opencode_session(output: &str, work: &Path) -> anyhow::Result<String> {
    #[derive(serde::Deserialize)]
    struct Session {
        id: String,
        directory: PathBuf,
    }

    let sessions: Vec<Session> = serde_json::from_str(output)?;
    let canonical = work.canonicalize()?;
    let mut matching = sessions
        .into_iter()
        .filter(|session| session.directory == canonical);
    let owned = matching
        .next()
        .ok_or_else(|| anyhow::anyhow!("OpenCode listed no session for the seed workspace"))?;
    anyhow::ensure!(
        matching.next().is_none(),
        "OpenCode listed more than one session for the seed workspace"
    );
    Ok(owned.id)
}

/// The lookup runs in the same workspace as the interactive launch. Keep output in private test evidence and
/// allow missing login to explain an early refusal, before the test has seen a successful turn.
async fn vendor_lookup(
    work: &Path,
    binary: &str,
    args: &[&str],
    turned: bool,
) -> Result<String, DriftError> {
    let mut command = tokio::process::Command::new(binary);
    command
        .args(args)
        .current_dir(work)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(LOOKUP_LIMIT, command.output())
        .await
        .map_err(|_| anyhow::anyhow!("{binary} {args:?} timed out"))?
        .map_err(anyhow::Error::from)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(
            refusal(&stderr.to_lowercase(), &stdout, turned).unwrap_or_else(|| {
                anyhow::anyhow!("{binary} {args:?} exited {}\n{stderr}", output.status).into()
            }),
        );
    }
    String::from_utf8(output.stdout).map_err(|error| anyhow::Error::from(error).into())
}

/// A typed selector is passed through the ordinary launcher request field, separate from the stored launch.
fn named_extras(launch: &SessionLaunch, target: &str) -> anyhow::Result<CreateExtras> {
    Ok(CreateExtras {
        named_start: Some(
            farhelm_helm::compile_named_start(launch, target.to_owned())
                .map_err(anyhow::Error::msg)?,
        ),
        ..Default::default()
    })
}

/// An accepted answer proves named recall for a non-reporting launch. The chooser's selector alone must never
/// become a durable capture or make plain Restart available.
async fn recall_without_capture(
    env: &DriftEnv<'_>,
    profile: &Harness,
    id: &str,
    code_word: &str,
) -> DriftResult {
    let mut terminal = Terminal::attach(env.sup, env.client, id, true).await?;
    terminal
        .ready(profile, TrustPolicy::AutoAnswer, true)
        .await?;
    let reply = format!("RECALL-{code_word}");
    send_prompt(&mut terminal, NAMED_RECALL_PROMPT, &reply, true).await?;
    wait_for_reply(&mut terminal, &reply, true).await?;
    env.sup.reconcile_for_test().await;
    ensure_uncaptured(&snapshot(env, id).await?)
        .map_err(|error| error.context("after the named launch recalled the seed"))?;
    Ok(())
}

/// Sample the tested row while the fixture still owns it.
async fn snapshot(env: &DriftEnv<'_>, id: &str) -> Result<SessionSnapshot, DriftError> {
    env.sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("the named-start session vanished").into())
}

/// Named starts without reporting leave no capture to offer on the next plain Restart.
fn ensure_uncaptured(snapshot: &SessionSnapshot) -> anyhow::Result<()> {
    anyhow::ensure!(
        snapshot.captured_conversation.is_none() && snapshot.restart_offer != RestartOffer::Resume,
        "named start: captured conversation = {}, Restart offer = {:?}",
        snapshot.captured_conversation.is_some(),
        snapshot.restart_offer
    );
    Ok(())
}

/// The seed and recall prompts can both be answered from conversation text without invoking a tool.
fn seed_prompt(code_word: &str) -> String {
    format!("Remember {code_word}. Reply with that label. No tools.")
}

/// An unpredictable label distinguishes recall from a reply the model could produce in a fresh conversation.
fn new_code_word() -> String {
    let random = uuid::Uuid::new_v4().simple().to_string();
    format!("FH{}", random[..8].to_ascii_uppercase())
}

/// Refuse pre-existing answer text from replay before sending a prompt that can produce the witness.
async fn send_prompt(
    terminal: &mut Terminal<'_>,
    prompt: &str,
    reply: &str,
    turned: bool,
) -> DriftResult {
    if reply_on_screen(&terminal.host.screen_tail(SCREEN_BYTES), reply) {
        return Err(anyhow::anyhow!("the reply was already visible before the prompt").into());
    }
    terminal.type_text(prompt, turned).await?;
    terminal
        .client
        .send_keys(terminal.channel, b"\r".to_vec())
        .await;
    Ok(())
}

/// Wait for the rendered reply in a harness that has no hook report to wait for on this launch.
async fn wait_for_reply(terminal: &mut Terminal<'_>, reply: &str, turned: bool) -> DriftResult {
    let waited = tokio::time::timeout(TURN_LIMIT, async {
        loop {
            let text = terminal.host.screen_tail(SCREEN_BYTES);
            terminal.check(&text.to_lowercase(), turned).await?;
            if reply_on_screen(&text, reply) {
                return Ok(());
            }
            pace().await;
        }
    })
    .await;
    waited.unwrap_or_else(|_| {
        Err(anyhow::anyhow!(
            "the named-start drift prompt got no matching reply\n{}",
            terminal.diagnostics()
        )
        .into())
    })
}

drift_cases! {
    execute;
    drift_claude => Claude,
    drift_codex => Codex,
    drift_antigravity => Antigravity,
    drift_goose => Goose,
    drift_grok => Grok,
    drift_muse => Muse,
    drift_pi => Pi,
    drift_omp => Omp,
    drift_cursor => Cursor,
    drift_opencode => OpenCode,
}

/// An agent gaining the named-conversation option also needs an explicit live case, even without capture.
#[farhelm_testtrace::test]
fn named_conversation_covers_every_supporting_harness() {
    let offered: Vec<_> = LaunchHarness::BUILT_INS
        .iter()
        .copied()
        .filter(|kind| kind.offers_named_conversation())
        .collect();
    assert_eq!(HARNESSES.len(), offered.len());
    for harness in offered {
        assert!(HARNESSES.contains(&harness), "{harness:?}");
    }
}

/// Never use the last session on the machine as a fallback. The lookup has to find exactly one owned row,
/// even when an unrelated session was listed first.
#[farhelm_testtrace::test]
fn opencode_only_resumes_a_session_in_the_test_workspace() {
    let scratch = farhelm_teststate::tempdir().expect("owned workspace");
    let work = scratch.path().canonicalize().expect("workspace exists");
    let other = serde_json::json!({"id": "ses_other", "directory": work.join("another")});
    let seed = serde_json::json!({"id": "ses_seed", "directory": work});
    let single = serde_json::json!([other, seed]).to_string();
    assert_eq!(opencode_session(&single, &work).unwrap(), "ses_seed");
    let missing = serde_json::json!([other]).to_string();
    assert!(opencode_session(&missing, &work).is_err());
    let ambiguous =
        serde_json::json!([other, seed, {"id": "ses_second", "directory": work}]).to_string();
    assert!(opencode_session(&ambiguous, &work).is_err());
}
