//! The original drift checks cover one ordinary launch per harness, each completing a real turn. The agents that
//! report a conversation also have to resume it, recall a code word and accept a fresh hook report for the same
//! identity; Cursor and OpenCode, which report none, stop after their one turn.

use super::*;
use farhelm_proto::LaunchPermission;

/// Drive Start and, for agents that report a conversation, Stop and Restart through the same supervisor.
async fn execute(kind: LaunchHarness) -> DriftResult {
    let profile = profile(kind).expect("a drift case must name a supported agent");
    let selection = LaunchSelection {
        custom_agent: None,
        harness: kind,
        model: profile.model.map(str::to_owned),
        effort: profile.effort.map(Into::into),
        // Exercise the two untracked agents' actual permission flags. YOLO also keeps their one turn from stopping
        // at a tool approval.
        permissions: profile
            .hook_witness
            .is_none()
            .then_some(LaunchPermission::Yolo),
        permission_flags: None,
        wrapper_prefix: None,
        container: None,
        // Codex's modal and Muse's startup redraw can lose input. Trust only the test workspace for this launch.
        workspace_trust: profile.trust_at_launch.then_some(true),
    };
    with_drift_supervisor(&profile, selection, None, async |env, launch| {
        env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
            check_base_session(&env, &profile, id).await
        })
        .await
    })
    .await
}

/// Every base case answers a workspace trust question in its empty directory, if one appears, and completes a real
/// turn under Farhelm's terminal host. An agent that reports a conversation must then prove recall and fresh hook
/// evidence on the new generation; one that reports none (Cursor, OpenCode) ends after its turn's reply is on screen.
async fn check_base_session(env: &DriftEnv<'_>, profile: &Harness, id: &str) -> DriftResult {
    let (sup, client, state) = (env.sup, env.client, env.state);
    let trust = TrustPolicy::AutoAnswer;
    let mut first = Terminal::attach(sup, client, id, false).await?;
    first.ready(profile, trust, false).await?;
    let Some(hook_witness) = profile.hook_witness else {
        return untracked_turn(&mut first).await;
    };
    let random = uuid::Uuid::new_v4().simple().to_string();
    let code_word = format!("FH{}", random[..8].to_ascii_uppercase());
    // Both turns can be answered from the conversation. A tool approval would strand the terminal on a dialog
    // that has nothing to do with whether the agent remembered the word.
    let prompt = format!(
        "Remember test label {code_word}. Reply with O and K joined into one word, and nothing else. No tools."
    );
    send_prompt(&mut first, &prompt, "OK", false).await?;
    let hook_log = state.join("hook-log").join(format!("{id}.log"));
    let captured = wait_for_turn(
        &mut first,
        sup,
        id,
        &hook_log,
        &TurnExpectation {
            reply: "OK",
            acked_before: 0,
            original_argv: None,
            hook_witness,
        },
    )
    .await?;

    // Listing observes the stopped row but does not apply reports. Drain them before taking the baseline
    // so Restart cannot acknowledge a pending Turn-1 callback and count it as a new-generation witness.
    client.stop_session(id).await?;
    client.list_sessions().await?;
    sup.reconcile_for_test().await;
    let hook_baseline = std::fs::read_to_string(&hook_log).map_err(anyhow::Error::from)?;
    let acked_before = acknowledged_reports(&hook_baseline).count();
    if acked_before == 0 {
        return Err(anyhow::anyhow!("the accepted Turn-1 hook disappeared before Restart").into());
    }
    client.restart_session(id, false).await?;
    let resumed = sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Restart lost the session"))?;
    if resumed.generation <= captured.generation {
        return Err(anyhow::anyhow!("Restart did not start a new generation").into());
    }
    let mut second = Terminal::attach(sup, client, id, true).await?;
    second.ready(profile, trust, true).await?;
    let expected = TurnExpectation {
        reply: &format!("RECALL-{code_word}"),
        acked_before,
        original_argv: Some(
            captured
                .resume_argv
                .as_deref()
                .expect("accepted Turn 1 has resume argv"),
        ),
        hook_witness: match hook_witness {
            HookWitness::CommittedIdentity { .. } => HookWitness::CommittedIdentity {
                resumed_generation: Some(resumed.generation),
            },
            HookWitness::Fresh | HookWitness::MatchingIdentity => hook_witness,
        },
    };
    send_prompt(&mut second, RECALL_PROMPT, expected.reply, true).await?;
    wait_for_turn(&mut second, sup, id, &hook_log, &expected).await?;
    Ok(())
}

/// One short no-tools turn for an agent with no conversation report, proving that an interactive turn works under
/// Farhelm's terminal host and not only that the CLI starts. The answer is split in the prompt ("FH" and a random
/// nonce, joined only in the reply) so a wrapped prompt echo can never read as the answer, and the prompt is short:
/// `Terminal::type_text` waits for the whole prompt on one screen line, and OpenCode's composer wraps well before the
/// terminal's width. A login screen before the reply is still COULD NOT RUN, since no turn has succeeded yet.
async fn untracked_turn(terminal: &mut Terminal<'_>) -> DriftResult {
    let random = uuid::Uuid::new_v4().simple().to_string();
    let nonce = &random[..8];
    let reply = format!("FH{nonce}");
    let prompt = format!("Reply FH then {nonce}, no space. No tools.");
    send_prompt(terminal, &prompt, &reply, false).await?;
    wait_for_reply(terminal, &reply, false).await
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

/// The base category is the default, so every built-in agent must have a case there.
#[farhelm_testtrace::test]
fn base_covers_all_builtin_agent_harnesses() {
    let mut kinds = HARNESSES.to_vec();
    kinds.extend([LaunchHarness::Bash, LaunchHarness::Zsh]);
    assert!(LaunchHarness::is_ordering_of_built_ins(&kinds));
}
