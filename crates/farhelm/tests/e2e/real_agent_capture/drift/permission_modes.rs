//! Check the CLI choices the launcher exposes beyond the base category. The checks send no tool requests. A Custom
//! launch for a tracked agent completes one text turn so the test can inspect the command Restart will use.
//!
//! Where the supervisor reads a harness's permission footer, the check also reads the live footer itself: a YOLO launch
//! must show the bypass footer, read and latched into the session listing, and a Guarded launch of a harness whose
//! reader recognizes a non-bypass footer must show that footer, read as definitely not bypass rather than unreadable. A
//! vendor rewording or moving its footer then fails here instead of silently leaving every session's mode unknown.

use super::*;
use farhelm_proto::LaunchPermission;
use farhelm_supervisor::agent_kind::{
    StatusLineYoloSupport, status_line_yolo_for_test, status_line_yolo_support_for_test,
};

/// The literal words establish that the intended mode reached the CLI. Custom words also have to survive Restart.
struct Mode {
    permission: LaunchPermission,
    words: &'static [&'static str],
}

/// Use the documented permission spellings, including modes that need no particular model or provider. A harness
/// whose footer reader recognizes a non-bypass footer also gets a Guarded launch, with the launcher's own Guarded words
/// ([`LaunchHarness::guarded_flags`]), so its non-bypass footer is checked against the reader too.
#[warn(clippy::wildcard_enum_match_arm)]
fn modes(harness: LaunchHarness) -> Vec<Mode> {
    use LaunchPermission::{Approve, Chat, Custom, Guarded, SmartApprove, Yolo};
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
    if reads_non_yolo_footer(harness) && harness.offers_permission(Guarded) {
        modes.push(choice(Guarded, harness.guarded_flags()));
    }
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
        if let Some(delimited) = delimited_custom(harness) {
            modes.push(choice(Custom, delimited.words));
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
            terminal
                .ready(profile, TrustPolicy::AutoAnswer, false)
                .await?;
            if let Some(expected) = footer_reading(harness, mode.permission) {
                wait_for_permission_footer(&mut terminal, id, harness, expected).await?;
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

/// The footer reading a launch in `permission` must show, if the supervisor reads `harness`'s footer for that mode:
/// `Some(true)` (bypass) for a YOLO launch of any harness with a reader, `Some(false)` (a recognized non-bypass footer)
/// for a Guarded launch of a harness whose reader recognizes one, and `None`, nothing to check, otherwise. Which
/// harnesses have which readers comes from the supervisor ([`status_line_yolo_support_for_test`]), so a reader added
/// there is checked here without a second list to update.
#[warn(clippy::wildcard_enum_match_arm)]
fn footer_reading(harness: LaunchHarness, permission: LaunchPermission) -> Option<bool> {
    match permission {
        LaunchPermission::Yolo => reads_yolo_footer(harness).then_some(true),
        LaunchPermission::Guarded => reads_non_yolo_footer(harness).then_some(false),
        LaunchPermission::Approve
        | LaunchPermission::SmartApprove
        | LaunchPermission::Chat
        | LaunchPermission::Custom => None,
    }
}

/// Whether the supervisor reads `harness`'s bypass (YOLO) footer. Matched exhaustively so a new kind of support has to
/// be placed here deliberately rather than falling on either side of a comparison.
#[warn(clippy::wildcard_enum_match_arm)]
fn reads_yolo_footer(harness: LaunchHarness) -> bool {
    match status_line_yolo_support_for_test(harness) {
        StatusLineYoloSupport::YoloOnly | StatusLineYoloSupport::YoloAndNonYolo => true,
        StatusLineYoloSupport::None => false,
    }
}

/// Whether the supervisor recognizes a non-bypass footer for `harness`, so a Guarded launch's footer can be checked
/// to read as definitely not YOLO. Exhaustive for the same reason as [`reads_yolo_footer`].
#[warn(clippy::wildcard_enum_match_arm)]
fn reads_non_yolo_footer(harness: LaunchHarness) -> bool {
    match status_line_yolo_support_for_test(harness) {
        StatusLineYoloSupport::YoloAndNonYolo => true,
        StatusLineYoloSupport::YoloOnly | StatusLineYoloSupport::None => false,
    }
}

/// A Custom launch ending in a bare `--`, for a CLI that reads `--` as the end of its options: anything Restart adds
/// after it would be taken as an operand, so the conversation option has to stay before it.
struct DelimitedCustom {
    /// The Custom words to launch, ending in `--`.
    words: &'static [&'static str],
    /// The option Restart uses to select the conversation, which must come before the `--`.
    selector: &'static str,
}

/// The harnesses whose CLI ends its options at `--` get a second Custom launch that ends in one.
#[warn(clippy::wildcard_enum_match_arm)]
fn delimited_custom(harness: LaunchHarness) -> Option<DelimitedCustom> {
    use LaunchHarness::*;
    match harness {
        Antigravity => Some(DelimitedCustom {
            words: &["--mode", "auto", "--settings", "{}", "--"],
            selector: "--conversation",
        }),
        Claude | Codex | Muse | Grok | Omp | Goose | Cursor | Pi | OpenCode | Bash | Zsh
        | Custom => None,
    }
}

/// Check what Farhelm reads from the live screen's permission footer. The permission choice in the session row alone
/// proves nothing.
///
/// Polls the live screen until the footer reads `expected` the way the supervisor's sampler reads it
/// ([`status_line_yolo_for_test`]): `Some(true)` for a bypass footer, `Some(false)` for a recognized non-bypass one, so
/// an unreadable footer (`None`, the reader no longer recognizing the vendor's wording) fails rather than passing as
/// "not YOLO". For a bypass footer it also waits for the session listing to report `screen_yolo`, which the sampler
/// latches on its next pass; a bypass reading seen once counts, since the sampler latches it too, and a footer that a
/// dialog covers by the time the listing catches up must not fail a launch Farhelm handled correctly. For a non-bypass
/// footer the listing is read once, as a sanity check that nothing latched bypass: the listing's `false` is also what
/// an unreadable footer leaves, so it is no evidence on its own, and the sampler's never latching `false` is already
/// pinned by the supervisor's own tests, so there is no reason to watch the listing for a while. Gives up after 15
/// seconds, naming the last reading, whether a bypass reading was ever seen, and the listing's value.
async fn wait_for_permission_footer(
    terminal: &mut Terminal<'_>,
    id: &str,
    harness: LaunchHarness,
    expected: bool,
) -> DriftResult {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let mut seen_bypass = false;
    loop {
        let screen = terminal.host.screen_tail(SCREEN_BYTES);
        terminal.check(&screen.to_lowercase(), false).await?;
        let reading = status_line_yolo_for_test(harness, &screen);
        seen_bypass |= reading == Some(true);
        let listing = terminal.client.list_sessions().await?;
        let session = listing
            .sessions
            .iter()
            .find(|session| session.id == id)
            .ok_or_else(|| anyhow::anyhow!("the permission test session vanished"))?;
        if expected && seen_bypass && session.screen_yolo {
            return Ok(());
        }
        if !expected && reading == Some(false) {
            if session.screen_yolo {
                return Err(anyhow::anyhow!(
                    "the footer reads not YOLO, but the session listing reports YOLO\n{}",
                    terminal.diagnostics()
                )
                .into());
            }
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            let wanted = if expected { "YOLO" } else { "non-YOLO" };
            return Err(anyhow::anyhow!(
                "Farhelm did not recognize the {wanted} footer: last reading {reading:?}, bypass seen {seen_bypass}, \
                 listing screen_yolo {}\n{}",
                session.screen_yolo,
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

/// For a CLI whose `--` ends its option list (Antigravity's does), independent checks for the Custom option words and
/// the delimiter leave space for the conversation selector between them while rejecting a selector that would be
/// parsed as an operand.
fn require_resume_words(
    harness: LaunchHarness,
    argv: &[String],
    words: &[&str],
) -> anyhow::Result<()> {
    let Some(delimited) = delimited_custom(harness) else {
        return require_words(argv, words);
    };
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
        before_marker.iter().any(|word| word == delimited.selector),
        "{harness:?} puts the Restart conversation option {} after Custom's -- marker",
        delimited.selector
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

/// Every footer the supervisor reads must be checked against a live launch, or a vendor's rewording would pass the
/// drift run unnoticed: a harness with a reader needs a YOLO launch (the bypass footer), and a harness whose reader
/// recognizes a non-bypass footer also needs a Guarded launch, with the launcher's Guarded words, where it offers
/// Guarded. Specified: every harness the supervisor reads a footer for has a case in this category, whose modes hold a
/// YOLO launch, and a Guarded launch with exactly `guarded_flags()` when the reader recognizes a non-bypass footer and
/// the harness offers Guarded, and no Guarded launch otherwise.
#[farhelm_testtrace::test]
fn permission_modes_check_every_footer_the_supervisor_reads() {
    let mut checked = Vec::new();
    for harness in LaunchHarness::ALL.iter().copied() {
        if !reads_yolo_footer(harness) {
            continue;
        }
        checked.push(harness);
        let modes = modes(harness);
        assert!(
            HARNESSES.contains(&harness),
            "no permission case for {harness:?}, whose footer the supervisor reads"
        );
        assert!(
            modes
                .iter()
                .any(|mode| mode.permission == LaunchPermission::Yolo),
            "{harness:?}'s bypass footer is never checked"
        );
        let guarded = modes
            .iter()
            .find(|mode| mode.permission == LaunchPermission::Guarded);
        if reads_non_yolo_footer(harness) && harness.offers_permission(LaunchPermission::Guarded) {
            let guarded = guarded
                .unwrap_or_else(|| panic!("{harness:?}'s non-bypass footer is never checked"));
            assert_eq!(guarded.words, harness.guarded_flags(), "{harness:?}");
        } else {
            assert!(guarded.is_none(), "{harness:?} has no non-bypass reader");
        }
    }
    // The premise that the loop reached the readers at all.
    assert!(!checked.is_empty(), "the supervisor reads no footer");
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
