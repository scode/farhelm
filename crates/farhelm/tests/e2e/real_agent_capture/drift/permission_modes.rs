//! Check the CLI choices the launcher exposes beyond the base category. The checks send no tool requests. A Custom
//! launch for a tracked agent completes one text turn so the test can inspect the command Restart will use.

use super::*;
use farhelm_proto::LaunchPermission;

/// The literal words establish that the intended mode reached the CLI. Custom words also have to survive Restart.
struct Mode {
    permission: LaunchPermission,
    words: &'static [&'static str],
}

/// Use the documented permission spellings, including modes that need no particular model or provider.
#[warn(clippy::wildcard_enum_match_arm)]
fn modes(harness: LaunchHarness) -> Vec<Mode> {
    use LaunchPermission::{Approve, Chat, Custom, SmartApprove, Yolo};
    let choice = |permission, words: &'static [&'static str]| Mode { permission, words };
    let mut modes = match harness {
        LaunchHarness::Claude | LaunchHarness::Antigravity => {
            vec![choice(Yolo, &["--dangerously-skip-permissions"])]
        }
        LaunchHarness::Codex | LaunchHarness::Muse => vec![choice(Yolo, &["--yolo"])],
        LaunchHarness::Grok => vec![choice(Yolo, &["--always-approve"])],
        LaunchHarness::Omp => vec![
            choice(Yolo, &["--approval-mode", "yolo"]),
            choice(Approve, &["--approval-mode", "always-ask"]),
        ],
        LaunchHarness::Goose => vec![
            choice(Approve, &["GOOSE_MODE=approve"]),
            choice(SmartApprove, &["GOOSE_MODE=smart_approve"]),
            choice(Chat, &["GOOSE_MODE=chat"]),
        ],
        LaunchHarness::Cursor => vec![choice(Yolo, &["--force"])],
        LaunchHarness::Pi
        | LaunchHarness::OpenCode
        | LaunchHarness::Bash
        | LaunchHarness::Zsh
        | LaunchHarness::Custom => return Vec::new(),
    };
    if harness.offers_permission(Custom) {
        let custom = match harness {
            LaunchHarness::Antigravity => &["--mode", "auto", "--settings", "{}"][..],
            // Distinct words let the case detect a silent fallback to the launcher's default.
            LaunchHarness::Cursor => &["-f"][..],
            LaunchHarness::Claude => &["--permission-mode", "manual"][..],
            LaunchHarness::Codex => &["-a", "on-request", "-s", "read-only"][..],
            LaunchHarness::Grok => &["--permission-mode", "default"][..],
            LaunchHarness::Muse => &["--approval-mode", "untrusted"][..],
            LaunchHarness::Omp => &["--approval-mode", "always-ask"][..],
            LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::OpenCode
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => {
                unreachable!("a permission drift case must have explicit Custom words")
            }
        };
        modes.push(choice(Custom, custom));
        if harness == LaunchHarness::Antigravity {
            modes.push(choice(
                Custom,
                &["--mode", "auto", "--settings", "{}", "--"],
            ));
        }
    }
    modes
}

/// A failure in one mode still leaves evidence for the other modes of the same installed harness.
async fn execute(harness: LaunchHarness) -> DriftResult {
    let profile = profile(harness).expect("a permission drift case has a profile");
    let mut failed = Vec::new();
    let mut unavailable = Vec::new();
    for mode in modes(harness) {
        let label = format!("{:?} ({})", mode.permission, mode.words.join(" "));
        match check_mode(&profile, harness, &mode).await {
            Ok(()) => println!("permission mode confirmed: {label}"),
            Err(DriftError::CouldNotRun(reason)) => unavailable.push(format!("{label}: {reason}")),
            Err(DriftError::Failed(error)) => {
                eprintln!("permission mode {label}: {error:#}");
                failed.push(format!("{label}: {error}"));
            }
        }
    }
    if !failed.is_empty() {
        failed.extend(unavailable);
        return Err(anyhow::anyhow!("{}", failed.join("\n")).into());
    }
    if !unavailable.is_empty() {
        return Err(DriftError::CouldNotRun(unavailable.join("\n")));
    }
    Ok(())
}

/// Give every mode its own session so a previous CLI's screen or capture cannot count as new evidence.
async fn check_mode(profile: &Harness, harness: LaunchHarness, mode: &Mode) -> DriftResult {
    let custom = mode.permission == LaunchPermission::Custom;
    let selection = LaunchSelection {
        custom_agent: None,
        harness,
        model: profile.model.map(str::to_owned),
        effort: profile.effort.map(Into::into),
        permissions: Some(mode.permission),
        permission_flags: custom
            .then(|| mode.words.iter().map(|word| (*word).to_owned()).collect()),
        wrapper_prefix: None,
        container: None,
        workspace_trust: profile.trust_at_launch.then_some(true),
    };
    with_drift_supervisor(profile, selection, None, async |env, launch| {
        require_words(
            &launch.start_argv().map_err(anyhow::Error::msg)?,
            mode.words,
        )?;
        env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
            let mut terminal = Terminal::attach(env.sup, env.client, id, false).await?;
            let observe_yolo = mode.permission == LaunchPermission::Yolo && reads_yolo(harness);
            terminal
                .ready(profile, TrustPolicy::AutoAnswer, false)
                .await?;
            if observe_yolo {
                wait_for_yolo(&mut terminal, id).await?;
            }
            if custom && let Some(witness) = profile.hook_witness {
                check_custom_restart(&env, profile, mode, id, &mut terminal, witness).await?;
            }
            Ok(())
        })
        .await
    })
    .await
}

/// These integrations classify a rendered permission footer into the supervisor's shared session listing.
fn reads_yolo(harness: LaunchHarness) -> bool {
    matches!(
        harness,
        LaunchHarness::Claude | LaunchHarness::Grok | LaunchHarness::Muse | LaunchHarness::Cursor
    )
}

/// Check what Farhelm read from the live screen. The permission choice in the session row alone proves nothing.
async fn wait_for_yolo(terminal: &mut Terminal<'_>, id: &str) -> DriftResult {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        terminal
            .check(
                &terminal.host.screen_tail(SCREEN_BYTES).to_lowercase(),
                false,
            )
            .await?;
        let listing = terminal.client.list_sessions().await?;
        let session = listing
            .sessions
            .iter()
            .find(|session| session.id == id)
            .ok_or_else(|| anyhow::anyhow!("the permission test session vanished"))?;
        if session.screen_yolo {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "Farhelm did not recognize the YOLO footer\n{}",
                terminal.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// A word match preserves argument boundaries and order. Substring matching would accept a different flag or value.
fn require_words(argv: &[String], words: &[&str]) -> anyhow::Result<()> {
    anyhow::ensure!(
        !words.is_empty()
            && argv
                .windows(words.len())
                .any(|part| { part.iter().map(String::as_str).eq(words.iter().copied()) }),
        "expected literal permission words {words:?} in {argv:?}"
    );
    Ok(())
}

/// Antigravity's `--` ends its option list. Independent checks for the Custom option words and the delimiter leave
/// space for the conversation selector between them while rejecting a selector that would be parsed as an operand.
fn require_resume_words(
    harness: LaunchHarness,
    argv: &[String],
    words: &[&str],
) -> anyhow::Result<()> {
    if harness != LaunchHarness::Antigravity {
        return require_words(argv, words);
    }
    let marker = argv.iter().position(|word| word == "--");
    let options = if let Some(custom_options) = words.strip_suffix(&["--"]) {
        anyhow::ensure!(marker.is_some(), "Restart dropped Custom's -- marker");
        custom_options
    } else {
        words
    };
    let before_marker = &argv[..marker.unwrap_or(argv.len())];
    require_words(before_marker, options)?;
    anyhow::ensure!(
        before_marker.iter().any(|word| word == "--conversation"),
        "Antigravity puts the Restart conversation option after Custom's -- marker"
    );
    Ok(())
}

/// Complete one text turn to capture a resumable conversation, then verify the retained command and the new composer.
async fn check_custom_restart(
    env: &DriftEnv<'_>,
    profile: &Harness,
    mode: &Mode,
    id: &str,
    first: &mut Terminal<'_>,
    hook_witness: HookWitness,
) -> DriftResult {
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let reply = format!("FH{}", &nonce[..8]);
    let prompt = format!("Reply with exactly the single word {reply}. No tools.");
    let expected = TurnExpectation {
        reply: &reply,
        acked_before: 0,
        original_argv: None,
        hook_witness,
    };
    first.type_text(&prompt, false).await?;
    if expected.saw_reply(&first.host.screen_tail(SCREEN_BYTES)) {
        return Err(anyhow::anyhow!(
            "the composer echoed a line that could be mistaken for the reply"
        )
        .into());
    }
    env.client.send_keys(first.channel, b"\r".to_vec()).await;
    let captured = wait_for_turn(
        first,
        env.sup,
        id,
        &env.state.join("hook-log").join(format!("{id}.log")),
        &expected,
    )
    .await?;
    let argv = captured
        .resume_argv
        .as_deref()
        .expect("accepted capture has resume argv");
    require_resume_words(env.harness, argv, mode.words)?;
    env.client.stop_session(id).await?;
    env.client.restart_session(id, false).await?;
    let resumed = env
        .sup
        .session_snapshot(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Restart lost the permission test session"))?;
    if resumed.generation <= captured.generation || resumed.resume_argv != captured.resume_argv {
        return Err(anyhow::anyhow!(
            "Restart did not keep the Custom resume command in a new generation"
        )
        .into());
    }
    let mut second = Terminal::attach(env.sup, env.client, id, true).await?;
    second.ready(profile, TrustPolicy::AutoAnswer, true).await
}

drift_cases! {
    execute;
    drift_claude => Claude,
    drift_codex => Codex,
    drift_antigravity => Antigravity,
    drift_grok => Grok,
    drift_muse => Muse,
    drift_omp => Omp,
    drift_goose => Goose,
    drift_cursor => Cursor,
}

/// New permission choices must get a deliberate drift case. The base category already starts YOLO-only agents.
#[farhelm_testtrace::test]
fn permission_modes_cover_agents_with_permission_choices() {
    let expected: Vec<_> = LaunchHarness::BUILT_INS
        .iter()
        .copied()
        .filter(|harness| !harness.is_shell() && !harness.offers_only_yolo())
        .collect();
    assert_eq!(HARNESSES.len(), expected.len());
    for harness in expected {
        assert!(
            HARNESSES.contains(&harness),
            "missing permission coverage for {harness:?}"
        );
        let modes = modes(harness);
        assert!(!modes.is_empty(), "empty permission case for {harness:?}");
        for mode in &modes {
            assert!(
                harness.offers_permission(mode.permission),
                "invalid case for {harness:?}"
            );
            assert!(!mode.words.is_empty(), "missing words for {harness:?}");
        }
        assert_eq!(
            modes
                .iter()
                .any(|mode| mode.permission == LaunchPermission::Custom),
            harness.offers_permission(LaunchPermission::Custom)
        );
    }
}

/// Antigravity may insert the selector between the retained settings and the boundary. Moving either option after
/// the boundary would leave it in the saved text but take away its effect when the CLI starts.
#[farhelm_testtrace::test]
fn antigravity_resume_options_must_all_precede_the_delimiter() {
    let check = |words: &[&str]| {
        let argv = words
            .iter()
            .map(|word| (*word).to_owned())
            .collect::<Vec<_>>();
        require_resume_words(
            LaunchHarness::Antigravity,
            &argv,
            &["--settings", "{}", "--"],
        )
    };
    assert!(
        check(&[
            "agy",
            "--settings",
            "{}",
            "--conversation",
            "captured",
            "--"
        ])
        .is_ok()
    );
    assert!(
        check(&[
            "agy",
            "--settings",
            "{}",
            "--",
            "--conversation",
            "captured"
        ])
        .is_err()
    );
    assert!(
        check(&[
            "agy",
            "--conversation",
            "captured",
            "--",
            "--settings",
            "{}"
        ])
        .is_err()
    );
}
