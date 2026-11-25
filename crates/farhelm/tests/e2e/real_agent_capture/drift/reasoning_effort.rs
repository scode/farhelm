//! Check that an installed CLI still accepts Farhelm's reasoning-effort choices and shows the requested value.
//! One selected non-low launch per harness also has to return a live reply.

use super::*;
use farhelm_proto::launcher::LaunchCatalogModel;

/// The startup check needs time for trust and the composer without spending most of a case's recorder budget.
const EFFORT_START_LIMIT: Duration = Duration::from_secs(50);

/// The active effort label may appear after an initial warning or composer redraw.
const EFFORT_DISPLAY_LIMIT: Duration = Duration::from_secs(12);

/// Select boundary efforts only when a current model accepts them. For a harness with no unusual boundary,
/// exercise its High setting so the category still covers a choice other than the base run's Low.
fn boundary_choices(
    catalog: &[LaunchCatalogModel],
    harness: LaunchHarness,
) -> Vec<(&str, LaunchEffort)> {
    use LaunchEffort::*;
    let pair = |effort| {
        catalog
            .iter()
            .filter(|model| {
                model.harness == harness && model.latest && model.efforts.contains(&effort)
            })
            .min_by_key(|model| {
                // Prefer inexpensive current families for the real turn, while letting the catalog own model IDs.
                !matches!(
                    model.family.as_str(),
                    "flash" | "glm-flash" | "hydrogen" | "luna"
                )
            })
            .map(|model| (model.id.as_str(), effort))
    };
    let mut choices: Vec<_> = [Off, Minimal, Xhigh, Max]
        .into_iter()
        .filter_map(pair)
        .collect();
    if choices.is_empty() {
        choices.extend(pair(High));
    }
    choices
}

/// Run each boundary in a new session. An unavailable login or provider prevents verifying the selection on
/// this host. A working CLI that shows another value still fails the case.
async fn execute(harness: LaunchHarness) -> DriftResult {
    use LaunchHarness::*;
    let profile = profile(harness).expect("an effort case must have a drift profile");
    let needs_openrouter = match harness {
        Goose | Pi | Omp => true,
        Claude | Codex | Antigravity | Muse | Cursor | OpenCode | Grok | Bash | Zsh | Custom => {
            false
        }
    };
    if needs_openrouter && std::env::var_os("OPENROUTER_API_KEY").is_none_or(|key| key.is_empty()) {
        return Err(DriftError::CouldNotRun(
            "OPENROUTER_API_KEY is missing".into(),
        ));
    }

    let catalog = farhelm_helm::launcher_catalog();
    let choices = boundary_choices(&catalog, harness);
    let turn_effort = choices
        .iter()
        .map(|&(_, effort)| effort)
        .find(|&effort| effort != LaunchEffort::Off)
        .ok_or_else(|| anyhow::anyhow!("no enabled boundary effort for {harness:?}"))?;
    let mut failures = Vec::new();
    for (model, effort) in choices {
        let label = format!("{model} {}", effort.as_cli_arg());
        match check_launch(&profile, harness, model, effort, effort == turn_effort).await {
            Ok(()) if harness == Goose => {
                println!("reasoning effort launch accepted: {label}; Goose exposes no effort value")
            }
            Ok(()) => println!("reasoning effort confirmed: {label}"),
            Err(DriftError::CouldNotRun(reason)) => {
                if failures.is_empty() {
                    return Err(DriftError::CouldNotRun(format!("{label}: {reason}")));
                }
                failures.push(format!("{label}: unavailable: {reason}"));
                break;
            }
            Err(DriftError::Failed(error)) => {
                eprintln!("reasoning effort {label}: {error:#}");
                failures.push(format!("{label}: {error}"));
            }
        }
    }
    if failures.is_empty() {
        return Ok(());
    }
    Err(anyhow::anyhow!("{}", failures.join("\n")).into())
}

/// Use Farhelm's compiled launch and read the active effort from that session alone.
async fn check_launch(
    profile: &Harness,
    harness: LaunchHarness,
    model: &str,
    effort: LaunchEffort,
    play_turn: bool,
) -> DriftResult {
    let selection = LaunchSelection {
        custom_agent: None,
        harness,
        model: Some(model.to_owned()),
        effort: Some(effort.into()),
        permissions: None,
        permission_flags: None,
        // Claude installed an update between two launches on the first run. Keep the CLI version under test
        // stable without changing the user's settings. See https://code.claude.com/docs/en/env-vars.
        wrapper_prefix: (harness == LaunchHarness::Claude)
            .then(|| vec!["env".into(), "DISABLE_AUTOUPDATER=1".into()]),
        container: None,
        workspace_trust: profile.trust_at_launch.then_some(true),
    };
    with_drift_supervisor(profile, selection, None, async |env, launch| {
        if harness == LaunchHarness::Goose {
            let argv = launch.start_argv().map_err(anyhow::Error::msg)?;
            let assignment = format!("GOOSE_THINKING_EFFORT={}", effort.as_cli_arg());
            if !argv.starts_with(&["env".to_owned(), assignment]) {
                return Err(anyhow::anyhow!(
                    "Goose launch omitted the requested effort environment assignment"
                )
                .into());
            }
        }
        env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
            // Muse can open a warning beside the footer. Keep the full model and effort label visible.
            let cols = if harness == LaunchHarness::Muse {
                180
            } else {
                COLS
            };
            let mut terminal =
                Terminal::attach_with_width(env.sup, env.client, id, false, cols).await?;
            tokio::time::timeout(
                EFFORT_START_LIMIT,
                terminal.ready(profile, TrustPolicy::AutoAnswer, false),
            )
            .await
            .map_err(|_| anyhow::anyhow!("composer did not open\n{}", terminal.diagnostics()))??;

            let deadline = tokio::time::Instant::now() + EFFORT_DISPLAY_LIMIT;
            let (observed, goose_ready) = loop {
                let screen = terminal.host.screen_tail(SCREEN_BYTES).to_ascii_lowercase();
                terminal.check(&screen, false).await?;
                let shown = displayed_effort(harness, model, &screen);
                // Goose prints its banner before starting the line editor. Wait for the editor's own prompt.
                let goose_ready = harness == LaunchHarness::Goose
                    && screen.lines().any(|line| line.trim() == ">");
                if goose_ready || shown == Some(effort) || tokio::time::Instant::now() >= deadline {
                    break (shown, goose_ready);
                }
                pace().await;
            };
            if harness == LaunchHarness::Goose && !goose_ready {
                return Err(anyhow::anyhow!(
                    "Goose did not open its composer\n{}",
                    terminal.diagnostics()
                )
                .into());
            }
            let observed = if play_turn {
                let witness = profile
                    .hook_witness
                    .expect("every effort case reports conversations");
                live_reply(&mut terminal, &env, id, witness).await?;
                println!(
                    "live turn completed with launch request for {}",
                    effort.as_cli_arg()
                );
                let screen = terminal.host.screen_tail(SCREEN_BYTES).to_ascii_lowercase();
                if harness == LaunchHarness::Claude && screen.contains("switched to opus") {
                    println!("Claude reported a vendor model fallback during the turn");
                }
                // A visible change during the turn takes precedence over the earlier startup reading.
                displayed_effort(harness, model, &screen).or(observed)
            } else {
                observed
            };
            if !goose_ready && observed != Some(effort) {
                return Err(anyhow::anyhow!(
                    "active effort was {:?}, expected {}\n{}",
                    observed.map(LaunchEffort::as_cli_arg),
                    effort.as_cli_arg(),
                    terminal.diagnostics()
                )
                .into());
            }
            Ok(())
        })
        .await
    })
    .await
}

/// A nonce in the reply distinguishes the model's answer from startup text and the echoed user prompt.
async fn live_reply(
    terminal: &mut Terminal<'_>,
    env: &DriftEnv<'_>,
    id: &str,
    hook_witness: HookWitness,
) -> DriftResult {
    let label = uuid::Uuid::new_v4().simple().to_string()[..8].to_ascii_uppercase();
    let reply = format!("EFFORT-{label}");
    // Keep the prompt below the terminal width so the echo cannot wrap its embedded label onto its own line.
    // The reply witness only accepts the label at the start of a line.
    let prompt = format!("Reply with {reply} on its own line. No tools.");
    let expected = TurnExpectation {
        reply: &reply,
        acked_before: 0,
        original_argv: None,
        hook_witness,
    };
    if expected.saw_reply(&terminal.host.screen_tail(SCREEN_BYTES)) {
        return Err(anyhow::anyhow!("reply witness was already present before the prompt").into());
    }
    terminal.type_text(&prompt, false).await?;
    terminal
        .client
        .send_keys(terminal.channel, b"\r".to_vec())
        .await;
    let hook_log = env.state.join("hook-log").join(format!("{id}.log"));
    wait_for_turn(terminal, env.sup, id, &hook_log, &expected).await?;
    Ok(())
}

/// Inspect the active status field. A requested value appearing elsewhere on screen, such as in a warning,
/// effort selector or routed model's readout, cannot establish that the CLI chose it.
fn displayed_effort(harness: LaunchHarness, model: &str, screen: &str) -> Option<LaunchEffort> {
    use LaunchHarness::*;
    // A current footer comes below a startup readout which may still be visible in the same screen.
    screen.lines().rev().map(str::trim).find_map(|line| {
        let raw = match harness {
            Claude => {
                if let Some((status, gauge)) = line.split_once(" [")
                    && gauge.starts_with(['░', '█'])
                {
                    status.split_once(" effort:")?.1
                } else {
                    let (heading, _) = line.split_once(" effort · ")?;
                    if !heading.starts_with(['▝', '▜', '█']) {
                        return None;
                    }
                    heading.rsplit_once(" with ")?.1
                }
            }
            Codex => {
                if let Some((status, _)) = line.split_once(" · context ") {
                    status.split(" · ").next()?.split_whitespace().last()?
                } else {
                    line.trim_start_matches(['│', '║', ' '])
                        .strip_prefix("model:")?
                        .split_once("/model to change")?
                        .0
                        .split_whitespace()
                        .last()?
                }
            }
            Antigravity => line.strip_prefix("? for shortcuts")?.split_once(" · ")?.1,
            Muse => line
                .strip_prefix(model)?
                .strip_prefix(" · ")?
                .split(" · ")
                .next()?
                .split("  ")
                .next()?,
            Pi => {
                let (usage, selection) = line.split_once("  ")?;
                if !usage.contains("%/") && !usage.contains("?/") {
                    return None;
                }
                selection
                    .split(" → ")
                    .next()?
                    .split_once(" • ")?
                    .1
                    .trim_start_matches("thinking ")
            }
            Omp => line
                .strip_prefix('')?
                .split('')
                .nth(1)?
                .split(" · ")
                .nth(1)?
                // OMP's ANSI renderer can place a theme glyph before the level word.
                .split_whitespace()
                .last()?,
            Goose | Cursor | OpenCode | Grok | Bash | Zsh | Custom => return None,
        };
        LaunchEffort::ALL
            .into_iter()
            .find(|level| level.as_cli_arg() == raw.trim())
    })
}

drift_cases! {
    execute;
    drift_antigravity => Antigravity,
    drift_claude => Claude,
    drift_codex => Codex,
    drift_goose => Goose,
    drift_muse => Muse,
    drift_omp => Omp,
    drift_pi => Pi,
}

/// Keep registration and boundary selection tied to the latest catalog, including model-specific effort limits.
#[farhelm_testtrace::test]
fn effort_cases_cover_every_builtin_with_catalog_efforts() {
    let catalog = farhelm_helm::launcher_catalog();
    for &harness in LaunchHarness::BUILT_INS {
        assert_eq!(
            HARNESSES.iter().filter(|&&case| case == harness).count(),
            usize::from(harness.offers_effort())
        );
        let expected: Vec<_> = [
            LaunchEffort::Off,
            LaunchEffort::Minimal,
            LaunchEffort::Xhigh,
            LaunchEffort::Max,
        ]
        .into_iter()
        .filter(|effort| {
            catalog.iter().any(|model| {
                model.harness == harness && model.latest && model.efforts.contains(effort)
            })
        })
        .collect();
        let choices = boundary_choices(&catalog, harness);
        if harness.offers_effort() && expected.is_empty() {
            assert_eq!(
                choices.iter().map(|choice| choice.1).collect::<Vec<_>>(),
                [LaunchEffort::High]
            );
        } else {
            assert_eq!(
                choices.iter().map(|choice| choice.1).collect::<Vec<_>>(),
                expected
            );
        }
        for (id, effort) in choices {
            assert!(catalog.iter().any(|model| model.id == id
                && model.harness == harness
                && model.latest
                && model.efforts.contains(&effort)));
        }
    }
}

/// Keep similarly named levels and routed levels from being mistaken for the active selection.
#[farhelm_testtrace::test]
fn effort_witness_comes_from_the_active_status_field() {
    use LaunchHarness::{Antigravity, Claude, Codex, Muse, Omp, Pi};
    assert_eq!(
        displayed_effort(
            Antigravity,
            "gemini-4-hydrogen",
            "? for shortcuts     gemini 4 hydrogen · xhigh"
        ),
        Some(LaunchEffort::Xhigh)
    );
    assert_eq!(
        displayed_effort(Antigravity, "gemini-4-hydrogen", "Choose xhigh to continue"),
        None
    );
    assert_eq!(
        displayed_effort(
            Pi,
            "z-ai/glm-5.3-flash",
            "0%/262k  (openrouter) z-ai/glm-5.3-flash • thinking off → routed • high"
        ),
        Some(LaunchEffort::Off)
    );
    assert_eq!(
        displayed_effort(
            Claude,
            "claude-fable-5",
            "▝▜██████▀  fable 5 with max effort · api usage billing"
        ),
        Some(LaunchEffort::Max)
    );
    assert_eq!(
        displayed_effort(
            Claude,
            "claude-fable-5",
            "warning: unsupported effort: xhigh"
        ),
        None
    );
    assert_eq!(
        displayed_effort(
            Codex,
            "gpt-6-luna",
            "│ model: gpt-6-luna high /model to change │\ngpt-6-luna low · context 0% used"
        ),
        Some(LaunchEffort::Low)
    );
    assert_eq!(
        displayed_effort(
            Muse,
            "muse-spark-1.3-contributor",
            "muse-spark-1.3-contributor · xhigh · workspace · auto-review"
        ),
        Some(LaunchEffort::Xhigh)
    );
    assert_eq!(
        displayed_effort(
            Omp,
            "z-ai/glm-5.3-flash",
            " 󰵗  󰪡 glm-5.3-flash (openrouter) · ◉ xhigh  workspace "
        ),
        Some(LaunchEffort::Xhigh)
    );
}
