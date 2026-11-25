//! Exercise workspace trust with the shipped launch choices in fresh directories.
//! The test never answers a trust question. A trusted launch must reach the composer on its own.

use super::*;
use anyhow::{Context as _, ensure};

const PI_EXTENSION_WITNESS: &str = "farhelm-drift-pi-extension-loaded";
const PI_TRUST_QUESTION: &str = "trust project folder?";

/// Run the explicit trust choice through the real compiler. Codex and Pi also have an explicit untrusted launch
/// to exercise in a second fresh workspace.
async fn execute(harness: LaunchHarness) -> DriftResult {
    let profile = profile(harness).expect("a trust case has a drift profile");
    let trusted = check_launch(harness, &profile, true).await;
    if trust_evidence(harness).has_untrusted_launch()
        && !matches!(trusted, Err(DriftError::CouldNotRun(_)))
    {
        let untrusted = check_launch(harness, &profile, false).await;
        if trusted.is_err()
            && let Err(ref error) = untrusted
        {
            eprintln!("the explicit untrusted launch also failed: {error:?}");
        }
        return trusted.and(untrusted);
    }
    trusted
}

/// Each launch owns its composer, the local resource under test and any isolated vendor configuration.
async fn check_launch(harness: LaunchHarness, profile: &Harness, trusted: bool) -> DriftResult {
    let selection = LaunchSelection {
        custom_agent: None,
        harness,
        model: profile.model.map(str::to_owned),
        effort: None,
        permissions: None,
        permission_flags: None,
        wrapper_prefix: None,
        container: None,
        workspace_trust: Some(trusted),
    };
    let evidence = trust_evidence(harness);
    let private = (evidence == TrustEvidence::ClaudeConfig).then_some(
        private_config::PrivateConfig::prepare_claude_trust
            as fn(&Path) -> anyhow::Result<private_config::PrivateConfig>,
    );
    with_drift_supervisor(profile, selection, private, async |env, launch| {
        if evidence == TrustEvidence::PiExtension {
            write_pi_extension(env.work)?;
        }
        env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
            let mut terminal = Terminal::attach(env.sup, env.client, id, false).await?;
            if evidence == TrustEvidence::CodexArgument && !trusted {
                codex_stayed_untrusted(&mut terminal).await?;
                check_codex_trust_argument(&terminal, env.work, false)?;
                return Ok(());
            }
            terminal
                .ready(
                    profile,
                    TrustPolicy::FailOnPrompt {
                        composer: composer_cues(harness),
                        extra_prompt: extra_trust_prompt(harness),
                    },
                    false,
                )
                .await?;
            check_startup_through_composer(&mut terminal, harness).await?;
            match evidence {
                TrustEvidence::CodexArgument => {
                    check_codex_trust_argument(&terminal, env.work, true)?;
                }
                TrustEvidence::PiExtension => {
                    check_pi_extension(&mut terminal, env.work, trusted).await?;
                }
                TrustEvidence::ClaudeConfig => {
                    env.private
                        .expect("Claude must have a private config")
                        .check_claude_trust(env.work)?;
                }
                TrustEvidence::ComposerOnly => {}
            }
            println!(
                "workspace trust {} verified",
                if trusted { "enabled" } else { "disabled" }
            );
            Ok(())
        })
        .await
    })
    .await
}

/// What shows a harness's trust choice took effect, beyond reaching the composer without a trust question.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TrustEvidence {
    /// Reaching the composer without a question is all the case checks.
    ComposerOnly,
    /// Codex's process carries the trust argument; an explicit untrusted launch is checked too.
    CodexArgument,
    /// A project extension in the workspace loads only when the workspace is trusted; an explicit untrusted launch is
    /// checked too.
    PiExtension,
    /// Farhelm records the trust in Claude's config at launch, which the case reads from a private copy.
    ClaudeConfig,
}

impl TrustEvidence {
    /// Whether the harness also offers an explicit untrusted launch that the case checks in a second workspace.
    fn has_untrusted_launch(self) -> bool {
        match self {
            Self::CodexArgument | Self::PiExtension => true,
            Self::ComposerOnly | Self::ClaudeConfig => false,
        }
    }
}

/// Each harness states its trust evidence, so a newly added one cannot skip a check by default.
#[warn(clippy::wildcard_enum_match_arm)]
fn trust_evidence(harness: LaunchHarness) -> TrustEvidence {
    use LaunchHarness::*;
    match harness {
        Codex => TrustEvidence::CodexArgument,
        Pi => TrustEvidence::PiExtension,
        Claude => TrustEvidence::ClaudeConfig,
        Antigravity | Muse | Goose | Omp | Grok | Cursor | OpenCode | Bash | Zsh | Custom => {
            TrustEvidence::ComposerOnly
        }
    }
}

/// These cues are from the interactive editor, after trust has been decided. Startup banners can appear first.
fn composer_cues(harness: LaunchHarness) -> &'static [&'static str] {
    use LaunchHarness::*;
    match harness {
        Claude => &["\n❯", "shift+tab to cycle"],
        // The shortcut hint can appear before the Folder access screen. The actual composer has a context row.
        Codex => &[" · context "],
        Antigravity => &["? for shortcuts", "type a message"],
        Muse => &["\n❯"],
        Pi => &["%/", "?/"],
        Cursor | OpenCode | Goose | Grok | Omp | Bash | Zsh | Custom => &[],
    }
}

/// Recognize current approval screens without changing what the base category is willing to answer.
fn extra_trust_prompt(harness: LaunchHarness) -> Option<fn(&str) -> bool> {
    use LaunchHarness::*;
    match harness {
        Codex => Some(codex_open_restricted),
        Pi => Some(|screen| screen.contains(PI_TRUST_QUESTION)),
        Claude | Antigravity | Muse | Cursor | OpenCode | Goose | Grok | Omp | Bash | Zsh
        | Custom => None,
    }
}

/// A redraw can replace a question before the host is sampled. Check retained replay and live updates until
/// the attachment itself has seen the composer, so the test cannot race pending output preceding it.
async fn check_startup_through_composer(
    terminal: &mut Terminal<'_>,
    harness: LaunchHarness,
) -> DriftResult {
    let replay = String::from_utf8_lossy(&terminal.replay_tail).to_lowercase();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let extra_prompt = extra_trust_prompt(harness);
    loop {
        let screen = terminal.host.screen_tail(SCREEN_BYTES).to_lowercase();
        let live = terminal.output.tail(SCREEN_BYTES).to_lowercase();
        terminal.check(&screen, false).await?;
        if [&screen, &replay, &live]
            .into_iter()
            .any(|text| trust_prompt(text) || extra_prompt.is_some_and(|is_prompt| is_prompt(text)))
        {
            return Err(anyhow::anyhow!(
                "workspace trust question appeared before the composer\n{}",
                terminal.diagnostics()
            )
            .into());
        }
        if composer_cues(harness)
            .iter()
            .any(|cue| replay.contains(cue) || live.contains(cue))
        {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "composer evidence never reached the terminal attachment\n{}",
                terminal.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// The current Folder access screen offers only a restricted open when project config is disabled. A generic
/// question in a fresh directory can appear regardless of whether the explicit launch choice arrived.
async fn codex_stayed_untrusted(terminal: &mut Terminal<'_>) -> DriftResult {
    let deadline = tokio::time::Instant::now() + START_LIMIT;
    loop {
        let screen = terminal.host.screen_tail(SCREEN_BYTES).to_lowercase();
        terminal.check(&screen, false).await?;
        if codex_open_restricted(&screen) {
            println!("workspace trust disabled: Codex offered a restricted open");
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "Codex did not show restricted folder access\n{}",
                terminal.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

/// Stay scoped to the rendered decision screen and the restricted option shown by the installed CLI.
fn codex_open_restricted(screen: &str) -> bool {
    screen.contains("folder access")
        && screen.contains("open restricted")
        && screen.contains("untrusted folders stay disabled")
}

/// Read only the cwd-specific trust argument on the test terminal's pinned child. A vendor's default folder
/// workflow would not prove that Farhelm expanded the explicit choice for this directory.
fn check_codex_trust_argument(
    terminal: &Terminal<'_>,
    work: &Path,
    trusted: bool,
) -> anyhow::Result<()> {
    let child = terminal
        .host
        .status()
        .child_root()
        .context("Codex is no longer running")?;
    let work = std::fs::canonicalize(work)?;
    let key = serde_json::to_string(work.to_str().context("Codex workspace is not UTF-8")?)?;
    let level = if trusted { "trusted" } else { "untrusted" };
    let expected = format!("projects={{{key}={{trust_level=\"{level}\"}}}}");
    let cmdline = std::fs::read(format!("/proc/{}/cmdline", child.pid))?;
    let args: Vec<_> = cmdline.split(|byte| *byte == 0).collect();
    ensure!(
        terminal.host.status().child_root() == Some(child)
            && args
                .windows(2)
                .any(|pair| pair == [b"-c", expected.as_bytes()]),
        "Codex did not receive the {level} override for this workspace"
    );
    Ok(())
}

/// A project-local extension writes one file inside the owned workspace when loaded. It does not call any model,
/// modify vendor settings or require tools from the agent.
fn write_pi_extension(work: &Path) -> anyhow::Result<()> {
    let extensions = work.join(".pi/extensions");
    std::fs::create_dir_all(&extensions)?;
    std::fs::write(
        extensions.join("farhelm-drift-trust.ts"),
        format!(
            r#"import {{ writeFileSync }} from "node:fs";
import {{ join }} from "node:path";
export default function (pi) {{
    pi.on("session_start", (_event, ctx) => {{
        writeFileSync(join(ctx.cwd, "{PI_EXTENSION_WITNESS}"), "loaded\n");
    }});
}}
"#
        ),
    )?;
    ensure!(
        !work.join(PI_EXTENSION_WITNESS).exists(),
        "Pi extension ran before launch"
    );
    Ok(())
}

/// The trusted run proves the fixture loads. In the other workspace, Pi's own refusal message establishes why
/// the extension didn't run. Waiting until a deadline without an explicit refusal would prove only absence.
async fn check_pi_extension(
    terminal: &mut Terminal<'_>,
    work: &Path,
    trusted: bool,
) -> DriftResult {
    let witness = work.join(PI_EXTENSION_WITNESS);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let screen = terminal.host.screen_tail(SCREEN_BYTES).to_lowercase();
        terminal.check(&screen, false).await?;
        let loaded = witness.is_file();
        let refused = screen.contains("this project is not trusted.")
            && screen.contains("resources and packages are ignored.");
        if (trusted && loaded && !refused) || (!trusted && !loaded && refused) {
            if refused {
                println!("Pi reported that it ignored project resources");
            }
            return Ok(());
        }
        if (trusted && refused) || (!trusted && loaded) || tokio::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "Pi expected trusted={trusted}; extension loaded={loaded}, refusal shown={refused}\n{}",
                terminal.diagnostics()
            )
            .into());
        }
        pace().await;
    }
}

drift_cases! {
    execute;
    drift_antigravity => Antigravity,
    drift_claude => Claude,
    drift_codex => Codex,
    drift_muse => Muse,
    drift_pi => Pi,
}

/// Any new built-in trust setting needs a deliberate check and composer evidence.
#[farhelm_testtrace::test]
fn workspace_trust_cases_cover_the_harnesses_that_offer_the_setting() {
    for &harness in LaunchHarness::BUILT_INS {
        assert_eq!(
            HARNESSES.iter().filter(|&&case| case == harness).count(),
            usize::from(harness.offers_workspace_trust())
        );
        if harness.offers_workspace_trust() {
            assert!(!composer_cues(harness).is_empty());
        }
    }
}

/// Codex can paint its first input hint before deciding folder access. That paint cannot prove workspace trust.
#[farhelm_testtrace::test]
fn codex_startup_hint_cannot_confirm_workspace_trust() {
    let startup = "› ask codex to do anything\n? for shortcuts";
    assert!(
        !composer_cues(LaunchHarness::Codex)
            .iter()
            .any(|cue| startup.contains(cue))
    );
    let restricted = "folder access\nconfig, hooks, and exec policies from untrusted folders stay disabled.\n1. open restricted";
    assert!(extra_trust_prompt(LaunchHarness::Codex).unwrap()(
        restricted
    ));
}
