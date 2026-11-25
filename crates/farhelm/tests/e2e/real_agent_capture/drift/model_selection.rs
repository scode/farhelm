//! Check the model a CLI actually selects when Farhelm launches it. The catalog supplies the inputs, while the
//! agent's own active-model display supplies the evidence.

use super::*;

/// Leave time for all the models in a category, even when a startup cannot confirm its selection.
const MODEL_START_LIMIT: Duration = Duration::from_secs(40);

/// Muse's sidebar can otherwise shorten the model in its footer to an ambiguous prefix.
const MUSE_MODEL_COLS: u16 = 180;

/// The named OpenRouter choices need credentials for that provider. The vendor-default launch is independent of
/// this credential and still runs first.
fn uses_openrouter(harness: LaunchHarness) -> bool {
    use LaunchHarness::*;
    match harness {
        Goose | Pi | Omp => true,
        Claude | Codex | Antigravity | Muse | Cursor | OpenCode | Grok | Bash | Zsh | Custom => {
            false
        }
    }
}

/// Keep a case tied to the shipped catalog as the meaning of "latest" changes with a release.
async fn execute(harness: LaunchHarness) -> DriftResult {
    let profile = profile(harness).expect("a model-selection case has a drift profile");
    let models: Vec<_> = farhelm_helm::launcher_catalog()
        .into_iter()
        .filter(|model| model.harness == harness && model.latest)
        .collect();
    if models.is_empty() {
        return Err(anyhow::anyhow!("the launcher has no current models for {harness:?}").into());
    }

    let mut failures = Vec::new();
    let mut unavailable = Vec::new();
    let requires_openrouter = uses_openrouter(harness);
    let check_default = requires_openrouter.then_some(None);
    let has_openrouter = std::env::var_os("OPENROUTER_API_KEY").is_some_and(|key| !key.is_empty());
    let named = models
        .iter()
        .filter(|_| !requires_openrouter || has_openrouter)
        .map(|model| Some(model.id.as_str()));
    if requires_openrouter && !has_openrouter {
        unavailable.push("OpenRouter models: OPENROUTER_API_KEY is missing".to_owned());
    }
    for model in check_default.into_iter().chain(named) {
        let label = model.unwrap_or("launch without model flags");
        match check_launch(&profile, harness, model).await {
            Ok(()) => println!("model selection confirmed: {label}"),
            Err(DriftError::CouldNotRun(reason)) => {
                unavailable.push(format!("{label}: {reason}"));
            }
            Err(DriftError::Failed(error)) => {
                eprintln!("model selection {label}: {error:#}");
                failures.push(format!("{label}: {error}"));
            }
        }
    }
    if !failures.is_empty() {
        failures.extend(unavailable);
        return Err(anyhow::anyhow!("{}", failures.join("\n")).into());
    }
    if !unavailable.is_empty() {
        return Err(DriftError::CouldNotRun(unavailable.join("\n")));
    }
    Ok(())
}

/// Start each model in a fresh session with Farhelm's real flag compiler, so one launch's selected model cannot
/// supply the next one's evidence.
async fn check_launch(
    profile: &Harness,
    harness: LaunchHarness,
    model: Option<&str>,
) -> DriftResult {
    let selection = LaunchSelection {
        custom_agent: None,
        harness,
        model: model.map(str::to_owned),
        effort: None,
        permissions: None,
        permission_flags: None,
        wrapper_prefix: None,
        container: None,
        workspace_trust: profile.trust_at_launch.then_some(true),
    };
    with_drift_supervisor(profile, selection, None, async |env, launch| {
        if model.is_none() {
            let argv = launch.start_argv().map_err(anyhow::Error::msg)?;
            if argv.iter().any(|arg| {
                matches!(arg.as_str(), "--provider" | "--model" | "-m")
                    || arg.starts_with("--provider=")
                    || arg.starts_with("--model=")
            }) {
                return Err(anyhow::anyhow!(
                    "the default launch explicitly selected a provider or model"
                )
                .into());
            }
        }
        env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
            let cols = match harness {
                LaunchHarness::Muse => MUSE_MODEL_COLS,
                _ => COLS,
            };
            let mut terminal =
                Terminal::attach_with_width(env.sup, env.client, id, false, cols).await?;
            let observed = tokio::time::timeout(MODEL_START_LIMIT, async {
                terminal
                    .ready(profile, TrustPolicy::AutoAnswer, false)
                    .await?;
                loop {
                    let screen = terminal.host.screen_tail(SCREEN_BYTES).to_ascii_lowercase();
                    terminal.check(&screen, false).await?;
                    if harness == LaunchHarness::Codex
                        && model.is_some_and(|id| codex_retirement_prompt(&screen, id))
                    {
                        return Err(DriftError::CouldNotRun(
                            "Codex asks whether to switch models before drawing the composer"
                                .to_owned(),
                        ));
                    }
                    if !trust_prompt(&screen) && screen_proves_model(harness, &screen, model) {
                        return Ok(());
                    }
                    pace().await;
                }
            })
            .await;
            observed.unwrap_or_else(|_| {
                Err(
                    anyhow::anyhow!("active model was not confirmed\n{}", terminal.diagnostics())
                        .into(),
                )
            })
        })
        .await
    })
    .await
}

/// The retirement notice names two models before Codex chooses one. The test leaves that choice to the vendor user.
fn codex_retirement_prompt(screen: &str, expected: &str) -> bool {
    screen.lines().map(str::trim).any(|line| {
        line.split_once(" retires on ")
            .is_some_and(|(model, _)| display_matches(model, expected))
    }) && screen.contains("try new model")
        && screen.contains("use existing model")
        && screen.contains("enter/esc confirm")
}

/// Read the agent's own active model field. Restrict matching to that field where the screen exposes it separately
/// from effort or provider, so a warning or a similar model name cannot vouch for the selected model.
fn screen_proves_model(harness: LaunchHarness, screen: &str, expected: Option<&str>) -> bool {
    use LaunchHarness::*;
    screen.lines().map(trim_frame).any(|line| match harness {
        Claude => expected.is_some_and(|id| {
            let model = id.trim_start_matches("claude-");
            (line.contains("effort:")
                || line.contains(" · api usage billing")
                || line.contains(" · claude "))
                && (has_model_token(line, model) || has_model_token(line, &model.replace('-', " ")))
        }),
        Codex => expected.is_some_and(|id| {
            (line.starts_with("model:") || line.contains(" · context "))
                && has_model_token(line, id)
        }),
        Antigravity => expected.is_some_and(|id| {
            line.strip_prefix("? for shortcuts")
                .and_then(|footer| footer.split_once(" · "))
                .is_some_and(|(model, _)| display_matches(model.trim(), id))
        }),
        Muse => expected.is_some_and(|id| {
            line.split_once(" · ")
                .is_some_and(|(model, _)| model == id && model.starts_with("muse-spark-"))
        }),
        Cursor => expected.is_some_and(|id| {
            line.strip_prefix("model:")
                .or_else(|| line.strip_prefix("cursor agent ·"))
                .is_some_and(|model| display_matches(model.trim(), id))
        }),
        Goose => line
            .split_once("● new session · ")
            .and_then(|(_, selection)| selection.split_once(' '))
            .is_some_and(|(provider, model)| {
                expected.map_or(!model.is_empty(), |id| {
                    provider == "openrouter" && model == id
                })
            }),
        Pi => pi_footer_selects(line, expected),
        Omp => omp_badge_selects(line, expected),
        OpenCode => expected.is_some_and(|id| {
            let Some(id) = id.strip_prefix("opencode/") else {
                return false;
            };
            line.strip_prefix("build ")
                .or_else(|| line.strip_prefix("plan "))
                .and_then(|composer| composer.split_once(" · "))
                .and_then(|(_, selection)| selection.split(" · ").next())
                .and_then(|selection| {
                    selection
                        .strip_suffix(" opencode")
                        .or_else(|| selection.strip_suffix(" opencode zen"))
                })
                .is_some_and(|model| display_matches(model, id))
        }),
        Grok | Bash | Zsh | Custom => false,
    })
}

/// Pi prints the selected model first on the right side of the context-usage row. A virtual model can also show a
/// routed model after an arrow. Only the selection at the start of the field counts.
fn pi_footer_selects(line: &str, expected: Option<&str>) -> bool {
    let Some((usage, selection)) = line.split_once("  ") else {
        return false;
    };
    if !usage.contains("%/") && !usage.contains("?/") {
        return false;
    }
    let selection = selection.trim_start();
    let provider = selection
        .strip_prefix('(')
        .and_then(|label| label.split_once(") "));
    let model = provider.map_or(selection, |(_, model)| model);
    let Some(model) = model.split_whitespace().next() else {
        return false;
    };
    match expected {
        None => model != "no-model",
        Some(id) => model == id && provider.is_some_and(|(name, _)| name == "openrouter"),
    }
}

/// OMP puts the selected model and its provider in one segment of the editor's status bar.
fn omp_badge_selects(line: &str, expected: Option<&str>) -> bool {
    if !line.starts_with('') {
        return false;
    }
    line.split('')
        .nth(1)
        .and_then(|badge| badge.trim().split_once(' '))
        .and_then(|(_, selection)| selection.strip_suffix(')')?.rsplit_once(" ("))
        .is_some_and(|(model, provider)| {
            expected.map_or(!model.is_empty(), |id| {
                provider == "openrouter" && display_matches(model, id)
            })
        })
}

/// Several CLIs display an ID with spaces in some or all of the places where the catalog has dashes.
fn display_matches(label: &str, id: &str) -> bool {
    label.replace(' ', "-") == id
}

/// Framed status rows can start with a border before their meaningful columns.
fn trim_frame(line: &str) -> &str {
    line.trim_matches([' ', '│', '┃', '║'])
}

/// Require a whole model ID. In particular, a longer ID such as `glm-5.3-flash` cannot vouch for `glm-5.3`.
fn has_model_token(line: &str, model: &str) -> bool {
    let id_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '/');
    line.match_indices(model).any(|(at, _)| {
        !line[..at].chars().next_back().is_some_and(id_char)
            && !line[at + model.len()..].chars().next().is_some_and(id_char)
    })
}

drift_cases! {
    execute;
    drift_antigravity => Antigravity,
    drift_claude => Claude,
    drift_codex => Codex,
    drift_cursor => Cursor,
    drift_goose => Goose,
    drift_muse => Muse,
    drift_omp => Omp,
    drift_opencode => OpenCode,
    drift_pi => Pi,
}

/// Adding a model choice must require deciding how its CLI discloses the selected model.
#[farhelm_testtrace::test]
fn model_selection_covers_every_built_in_harness_that_offers_models() {
    let expected: Vec<_> = LaunchHarness::BUILT_INS
        .iter()
        .copied()
        .filter(|harness| harness.offers_model())
        .collect();
    assert_eq!(HARNESSES.len(), expected.len());
    for harness in expected {
        assert_eq!(HARNESSES.iter().filter(|&&case| case == harness).count(), 1);
    }
}

/// A provider mismatch or a model appearing only in a warning does not establish selection.
#[farhelm_testtrace::test]
fn the_selected_model_needs_active_model_evidence() {
    use LaunchHarness::{Antigravity, Codex, Goose, Muse, Omp, OpenCode, Pi};
    assert!(!screen_proves_model(
        Codex,
        "warning about gpt-6.2-sol",
        Some("gpt-6.2-sol")
    ));
    assert!(screen_proves_model(
        Codex,
        "│ model: gpt-6.2-sol │",
        Some("gpt-6.2-sol")
    ));
    let flash = "__( o)>  ● new session · openrouter z-ai/glm-5.3-flash";
    assert!(screen_proves_model(
        Goose,
        flash,
        Some("z-ai/glm-5.3-flash")
    ));
    assert!(!screen_proves_model(Goose, flash, Some("z-ai/glm-5.3")));
    assert!(!screen_proves_model(
        Goose,
        "__( o)>  ● new session · other z-ai/glm-5.3-flash",
        Some("z-ai/glm-5.3-flash")
    ));
    assert!(screen_proves_model(Goose, flash, None));
    assert!(screen_proves_model(
        Antigravity,
        "? for shortcuts           gemini 4 hydrogen · medium",
        Some("gemini-4-hydrogen")
    ));
    let opencode_flash = "┃  build auto · glm-5.3-flash opencode · high";
    assert!(screen_proves_model(
        OpenCode,
        opencode_flash,
        Some("opencode/glm-5.3-flash")
    ));
    assert!(!screen_proves_model(
        OpenCode,
        opencode_flash,
        Some("opencode/glm-5.3")
    ));
    assert!(!screen_proves_model(
        OpenCode,
        "┃  build auto · glm-5.3-flash other · high",
        Some("opencode/glm-5.3-flash")
    ));
    assert!(screen_proves_model(
        Muse,
        "muse-spark-1.3-contributor · auto-review",
        Some("muse-spark-1.3-contributor")
    ));
    assert!(!screen_proves_model(
        Muse,
        "muse-spark-1.3-contr… · auto-review",
        Some("muse-spark-1.3-contributor")
    ));
    assert!(screen_proves_model(
        Omp,
        " 󰵗  󰪡 qwen4 max preview (alibaba token plan)   workspace ",
        None
    ));
    assert!(!screen_proves_model(Pi, "0.8%/262k", None));
    assert!(!screen_proves_model(Pi, "0.0%/0 (auto)  no-model", None));
    assert!(screen_proves_model(
        Pi,
        "0.8%/262k  (openrouter) z-ai/glm-5.3-flash • low",
        Some("z-ai/glm-5.3-flash")
    ));
    assert!(!screen_proves_model(
        Pi,
        "0.8%/262k  (openrouter) other-model → z-ai/glm-5.3-flash",
        Some("z-ai/glm-5.3-flash")
    ));
    let retirement = "gpt-5.6 terra retires on december 23, 2026.\n\
        › 1. try new model\n2. use existing model\nenter/esc confirm";
    assert!(codex_retirement_prompt(retirement, "gpt-5.6-terra"));
    assert!(!codex_retirement_prompt(retirement, "gpt-6.2-sol"));
}
