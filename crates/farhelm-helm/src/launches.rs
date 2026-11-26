//! Release-owned structured harness catalog and command compiler.
//!
//! The client sends a [`LaunchSelection`] as intent, never an argv fragment.
//! This module is the single place that turns that intent into the invocation
//! the existing create path accepts. It deliberately does not probe installed
//! binaries or a vendor service: a launch must be reproducible from the
//! released catalog, and a custom model remains a literal one-argument escape
//! hatch for a newer vendor ID.

use farhelm_proto::{
    SessionLaunch,
    launch::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection},
    launcher::LaunchCatalogModel,
    session_launch::{CONVERSATION_PLACEHOLDER, FARHELM_ARGS_PLACEHOLDER},
};

const MAX_MODEL_ID_BYTES: usize = 256;

/// A catalog entry whose ID has a known owning harness.
///
/// A matching row supplies that harness's efforts. Only a current model
/// reserves its ID against use as a custom model on another harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CatalogModel {
    pub(crate) id: &'static str,
    /// Stable name for models in the same family within a harness.
    pub(crate) family: &'static str,
    /// Whether this entry is the family's current suggestion for this harness.
    pub(crate) latest: bool,
    pub(crate) harness: LaunchHarness,
    pub(crate) efforts: &'static [LaunchEffort],
}

/// Farhelm's deliberately compact Codex offering.
///
/// Codex advertises broader effort choices for some models, but this release
/// presents a stable three-level subset for both suggested and custom models.
const CODEX_EFFORTS: &[LaunchEffort] =
    &[LaunchEffort::Low, LaunchEffort::Medium, LaunchEffort::High];
const CLAUDE_EFFORTS: &[LaunchEffort] = &[
    LaunchEffort::Low,
    LaunchEffort::Medium,
    LaunchEffort::High,
    LaunchEffort::Xhigh,
    LaunchEffort::Max,
];
const MUSE_EFFORTS: &[LaunchEffort] = &[
    LaunchEffort::Low,
    LaunchEffort::Medium,
    LaunchEffort::High,
    LaunchEffort::Xhigh,
];
/// Goose exposes these request levels through `GOOSE_THINKING_EFFORT`.
/// Model providers may still clamp or reject a request at runtime.
const GOOSE_EFFORTS: &[LaunchEffort] = &[
    LaunchEffort::Off,
    LaunchEffort::Low,
    LaunchEffort::Medium,
    LaunchEffort::High,
    LaunchEffort::Max,
];
/// Pi accepts these request levels through `--thinking`; they are not a
/// promise that every OpenRouter model advertises matching reasoning support.
const PI_EFFORTS: &[LaunchEffort] = &[
    LaunchEffort::Off,
    LaunchEffort::Minimal,
    LaunchEffort::Low,
    LaunchEffort::Medium,
    LaunchEffort::High,
    LaunchEffort::Xhigh,
    LaunchEffort::Max,
];
/// OMP accepts the same request levels as Pi through its own `--thinking`
/// flag. OMP's `auto` level is deliberately NOT offered: the composer
/// presents a closed explicit vocabulary, and a mode whose meaning is
/// "vendor decides" is exactly the guessed default the structured launch
/// exists to keep out.
const OMP_EFFORTS: &[LaunchEffort] = &[
    LaunchEffort::Off,
    LaunchEffort::Minimal,
    LaunchEffort::Low,
    LaunchEffort::Medium,
    LaunchEffort::High,
    LaunchEffort::Xhigh,
    LaunchEffort::Max,
];
/// OpenCode's inspected interactive CLI has no portable effort flag. An empty
/// catalog vocabulary makes an explicit effort invalid rather than guessing a
/// translation to a provider-specific variant.
const OPENCODE_EFFORTS: &[LaunchEffort] = &[];
/// Model rows narrow this vocabulary where the CLI offers fewer efforts.
const ANTIGRAVITY_EFFORTS: &[LaunchEffort] = &[
    LaunchEffort::Low,
    LaunchEffort::Medium,
    LaunchEffort::High,
    LaunchEffort::Xhigh,
];
const ANTIGRAVITY_EFFORTS_THROUGH_HIGH: &[LaunchEffort] =
    &[LaunchEffort::Low, LaunchEffort::Medium, LaunchEffort::High];

/// The known models for this release, ordered within each generation for the launcher.
///
/// A replacement takes the latest mark within its harness and family. Keep the previous
/// entry and its accepted efforts so saved templates and exact IDs can still use it.
/// Drop an entry deliberately when its model becomes obsolete.
const CATALOG: &[CatalogModel] = &[
    CatalogModel {
        id: "gemini-4-neon",
        family: "neon",
        latest: true,
        harness: LaunchHarness::Antigravity,
        efforts: ANTIGRAVITY_EFFORTS,
    },
    CatalogModel {
        id: "gemini-4-argon",
        family: "argon",
        latest: true,
        harness: LaunchHarness::Antigravity,
        efforts: ANTIGRAVITY_EFFORTS_THROUGH_HIGH,
    },
    CatalogModel {
        id: "gemini-4-hydrogen",
        family: "hydrogen",
        latest: true,
        harness: LaunchHarness::Antigravity,
        efforts: ANTIGRAVITY_EFFORTS,
    },
    CatalogModel {
        id: "gemini-3.8-flash",
        family: "flash",
        latest: true,
        harness: LaunchHarness::Antigravity,
        efforts: ANTIGRAVITY_EFFORTS_THROUGH_HIGH,
    },
    CatalogModel {
        id: "auto",
        family: "auto",
        latest: true,
        harness: LaunchHarness::Cursor,
        efforts: &[],
    },
    CatalogModel {
        id: "composer-2.5",
        family: "composer",
        latest: true,
        harness: LaunchHarness::Cursor,
        efforts: &[],
    },
    CatalogModel {
        id: "gpt-6-luna",
        family: "luna",
        latest: true,
        harness: LaunchHarness::Codex,
        efforts: CODEX_EFFORTS,
    },
    CatalogModel {
        id: "gpt-6.2-sol",
        family: "sol",
        latest: true,
        harness: LaunchHarness::Codex,
        efforts: CODEX_EFFORTS,
    },
    CatalogModel {
        id: "gpt-6.2-astra",
        family: "astra",
        latest: true,
        harness: LaunchHarness::Codex,
        efforts: CODEX_EFFORTS,
    },
    CatalogModel {
        id: "claude-fable-5",
        family: "fable",
        latest: true,
        harness: LaunchHarness::Claude,
        efforts: CLAUDE_EFFORTS,
    },
    CatalogModel {
        id: "muse-spark-1.3-contributor",
        family: "spark",
        latest: true,
        harness: LaunchHarness::Muse,
        efforts: MUSE_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3-flash",
        family: "glm-flash",
        latest: true,
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.5",
        family: "grok",
        latest: false,
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.6",
        family: "grok",
        latest: true,
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3",
        family: "glm",
        latest: true,
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6-luna",
        family: "luna",
        latest: true,
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6.2-sol",
        family: "sol",
        latest: true,
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6.2-astra",
        family: "astra",
        latest: true,
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3-flash",
        family: "glm-flash",
        latest: true,
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.5",
        family: "grok",
        latest: false,
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.6",
        family: "grok",
        latest: true,
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3",
        family: "glm",
        latest: true,
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6-luna",
        family: "luna",
        latest: true,
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6.2-sol",
        family: "sol",
        latest: true,
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6.2-astra",
        family: "astra",
        latest: true,
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3-flash",
        family: "glm-flash",
        latest: true,
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.5",
        family: "grok",
        latest: false,
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.6",
        family: "grok",
        latest: true,
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3",
        family: "glm",
        latest: true,
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6-luna",
        family: "luna",
        latest: true,
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6.2-sol",
        family: "sol",
        latest: true,
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6.2-astra",
        family: "astra",
        latest: true,
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "opencode/glm-5.3-flash",
        family: "glm-flash",
        latest: true,
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/grok-4.5",
        family: "grok",
        latest: false,
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/grok-4.6",
        family: "grok",
        latest: true,
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/glm-5.3",
        family: "glm",
        latest: true,
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/gpt-6-luna",
        family: "luna",
        latest: true,
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/gpt-6.2-sol",
        family: "sol",
        latest: true,
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/gpt-6.2-astra",
        family: "astra",
        latest: true,
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    // Retain the effort vocabularies these IDs accepted as custom models after v0.41.0.
    CatalogModel {
        id: "gpt-6.1-sol",
        family: "sol",
        latest: false,
        harness: LaunchHarness::Codex,
        efforts: CODEX_EFFORTS,
    },
    CatalogModel {
        id: "gpt-6-astra",
        family: "astra",
        latest: false,
        harness: LaunchHarness::Codex,
        efforts: CODEX_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6.1-sol",
        family: "sol",
        latest: false,
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6-astra",
        family: "astra",
        latest: false,
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6.1-sol",
        family: "sol",
        latest: false,
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6-astra",
        family: "astra",
        latest: false,
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6.1-sol",
        family: "sol",
        latest: false,
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6-astra",
        family: "astra",
        latest: false,
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "opencode/gpt-6.1-sol",
        family: "sol",
        latest: false,
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/gpt-6-astra",
        family: "astra",
        latest: false,
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
];

/// Project the release-owned choices for the launcher and template application. The current model inherits
/// its family's word, so replacing that model only changes which row is marked latest.
pub(crate) fn launcher_catalog() -> Vec<LaunchCatalogModel> {
    CATALOG
        .iter()
        .map(|model| LaunchCatalogModel {
            id: model.id.to_owned(),
            family: model.family.to_owned(),
            latest: model.latest,
            harness: model.harness,
            efforts: model.efforts.to_vec(),
        })
        .collect()
}

/// The durable form of a submitted structured selection: what history,
/// remembered defaults, and retries record for it.
///
/// New request omissions become explicit Guarded choices on supported
/// harnesses, or YOLO for Pi, OpenCode and Goose. Shared by
/// [`compile`] and by acceptance paths that never compile (a fresh-checkout
/// create reconciled after a lost reply), so the same request records the
/// same selection whichever path accepts it. Pure: no catalog lookup, so a
/// previously accepted request stays recoverable after catalog changes. This
/// must never be applied to a stored command copied by plain replace or an
/// agent clone/inherit: its missing flags are historical facts.
pub(crate) fn normalize_selection(mut selection: LaunchSelection) -> LaunchSelection {
    if selection.permissions.is_none() {
        selection.permissions = selection.harness.omitted_permission();
    }
    selection
}

/// Compose an agent launch from an explicit choice: its start command and,
/// for an agent type with conversation reporting, its resume command, each
/// with `{farhelm_args}` where Farhelm's own arguments go (SPEC.md, "Farhelm
/// composes an agent launch's resume command from the same choices as its
/// start command").
///
/// The composed agent words are literal argv: a custom model is one element
/// after validation. An optional wrapper retains raw shell words at the front;
/// the supervisor interprets only that prefix and quotes the composed suffix.
/// An absent model adds
/// no provider or model flags, leaving the installed harness's configured
/// default in charge. `{farhelm_args}` goes last in both commands, where the
/// previous release appended Farhelm's arguments, which is also where the
/// upgrade puts it in a converted session
/// (`SessionLaunch::from_pre_launch_kinds`), so a new launch and an
/// upgraded one spawn alike.
pub(crate) fn compile(selection: LaunchSelection) -> Result<SessionLaunch, String> {
    let selection = normalize_selection(selection);
    validate_selection(&selection)?;

    // Preserve the raw prefix separately from the literal command it wraps,
    // including Goose's `env` launcher. Its word count is the supervisor's
    // execution boundary on both fresh and resume paths.
    let mut argv = selection.wrapper_prefix.clone().unwrap_or_default();
    if selection.harness == LaunchHarness::Goose
        && (selection.effort.is_some() || selection.permissions.is_some())
    {
        argv.push("env".to_string());
        if let Some(effort) = &selection.effort {
            argv.push(format!("GOOSE_THINKING_EFFORT={}", effort.as_cli_arg()));
        }
        if let Some(permission) = selection.permissions {
            let mode = match permission {
                LaunchPermission::Yolo => "auto",
                LaunchPermission::Approve => "approve",
                LaunchPermission::SmartApprove => "smart_approve",
                LaunchPermission::Chat => "chat",
                LaunchPermission::Guarded | LaunchPermission::Custom => {
                    unreachable!("Goose does not offer guarded or custom")
                }
            };
            argv.push(format!("GOOSE_MODE={mode}"));
        }
    }
    argv.push(program(selection.harness).to_string());
    if selection.harness == LaunchHarness::Grok {
        // Grok's shared leader is outside Farhelm's tracked ownership
        // boundary, so every supported launch opts into a private leader.
        argv.push("--no-leader".to_string());
    }
    if selection.harness == LaunchHarness::Goose {
        argv.push("session".to_string());
    }
    if let Some(model) = &selection.model {
        match selection.harness {
            LaunchHarness::Codex => argv.extend(["-m".to_string(), model.clone()]),
            LaunchHarness::Claude
            | LaunchHarness::Muse
            | LaunchHarness::Cursor
            | LaunchHarness::Antigravity => argv.extend(["--model".to_string(), model.clone()]),
            LaunchHarness::OpenCode => argv.extend([
                "--model".to_string(),
                opencode_model_argument(model)?.to_string(),
            ]),
            LaunchHarness::Goose | LaunchHarness::Pi | LaunchHarness::Omp => argv.extend([
                // The explicit `--provider` makes provider intent
                // unambiguous to the CLI's resolver; it is NOT a promise
                // that an unknown custom id routes literally upstream —
                // OMP's provider-scoped matching still applies — but the
                // id itself stays one argv element, stored verbatim.
                "--provider".to_string(),
                "openrouter".to_string(),
                "--model".to_string(),
                model.clone(),
            ]),
            LaunchHarness::Grok
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => {
                unreachable!("selection validated without a supported model")
            }
        }
    }
    if let Some(effort) = &selection.effort {
        match selection.harness {
            LaunchHarness::Codex => argv.extend([
                "-c".to_string(),
                format!("model_reasoning_effort={}", effort.as_cli_arg()),
            ]),
            LaunchHarness::Claude | LaunchHarness::Antigravity => {
                argv.extend(["--effort".to_string(), effort.as_cli_arg().to_string()])
            }
            LaunchHarness::Muse => argv.extend([
                "--reasoning-effort".to_string(),
                effort.as_cli_arg().to_string(),
            ]),
            // `validate_selection` has already rejected this combination.
            LaunchHarness::OpenCode => unreachable!("OpenCode has no supported effort"),
            LaunchHarness::Cursor => unreachable!("Cursor has no separate effort flag"),
            LaunchHarness::Goose => {}
            LaunchHarness::Pi | LaunchHarness::Omp => {
                argv.extend(["--thinking".to_string(), effort.as_cli_arg().to_string()])
            }
            LaunchHarness::Grok
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => {
                unreachable!("selection validated without a supported effort")
            }
        }
    }
    // OMP carries its effective permission as one explicit approval-mode flag,
    // including the YOLO default selected when the caller omits it.
    if selection.harness == LaunchHarness::Omp {
        match selection.permissions {
            Some(LaunchPermission::Yolo) => {
                argv.extend(["--approval-mode".to_string(), "yolo".to_string()])
            }
            Some(LaunchPermission::Approve) => {
                argv.extend(["--approval-mode".to_string(), "always-ask".to_string()])
            }
            _ => {}
        }
    }
    // Guarded is durable intent: keep these options before Codex's resume
    // subcommand and copy them into every other supported resume argv too.
    if selection.permissions == Some(LaunchPermission::Guarded) {
        argv.extend(
            selection
                .harness
                .guarded_flags()
                .iter()
                .map(|flag| flag.to_string()),
        );
    }
    if selection.permissions == Some(LaunchPermission::Yolo) {
        let flag = match selection.harness {
            LaunchHarness::Codex | LaunchHarness::Muse => Some("--yolo"),
            LaunchHarness::Claude | LaunchHarness::Antigravity => {
                Some("--dangerously-skip-permissions")
            }
            LaunchHarness::OpenCode => Some("--auto"),
            LaunchHarness::Cursor => Some("--force"),
            LaunchHarness::Grok => Some("--always-approve"),
            // Goose encodes every mode in the environment above; Pi has no
            // approval flag at all; OMP's own approval-mode arm above already
            // wrote its flag.
            LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::Omp
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => None,
        };
        if let Some(flag) = flag {
            argv.push(flag.to_string());
        }
    }
    if selection.permissions == Some(LaunchPermission::Custom) {
        // Keep user-authored words together so each harness's option boundary
        // stays available when adding compiler-owned arguments.
        argv.extend(
            selection
                .permission_flags
                .as_ref()
                .expect("validated custom words")
                .iter()
                .cloned(),
        );
    }
    // This final compiler-owned segment can also be replaced when cloning
    // a stored launch, without recomposing its historical permission flags.
    let positional_tail_len = custom_positional_tail_len(&selection);
    let trust_at = argv.len() - positional_tail_len;
    argv.splice(
        trust_at..trust_at,
        workspace_trust_arguments(selection.harness, selection.workspace_trust)
            .iter()
            .map(|word| (*word).to_string()),
    );

    let resume = resume_arguments(selection.harness).map(|suffix| {
        let mut resume = argv.clone();
        let resume_at = resume.len() - positional_tail_len;
        resume.splice(
            resume_at..resume_at,
            suffix.iter().map(|element| element.to_string()),
        );
        resume.push(FARHELM_ARGS_PLACEHOLDER.to_string());
        resume
    });
    argv.push(FARHELM_ARGS_PLACEHOLDER.to_string());
    Ok(SessionLaunch::Agent {
        custom_reporting: None,
        selection,
        start: argv,
        resume,
    })
}

/// Compile one registration snapshot. Existing sessions keep these argv and reporting capabilities
/// even if the registry entry is later edited or deleted.
/// See `website/src/content/docs/docs/agents/custom-agent-contract.md`, `## Contract changelog`,
/// before changing any wrapper arguments.
pub(crate) fn compile_custom(
    selection: LaunchSelection,
    agent: &farhelm_proto::custom_agents::CustomAgent,
) -> Result<SessionLaunch, String> {
    use farhelm_proto::custom_agents::validate_model_or_effort;

    if !selection.harness.uses_custom_catalog()
        || selection.custom_agent.as_deref() != Some(&agent.id)
    {
        return Err("the launch must name the custom agent it uses".into());
    }
    agent.validate()?;
    let mut selection = normalize_selection(selection);
    if selection.permissions != Some(LaunchPermission::Yolo) {
        return Err("custom agents use YOLO permissions".into());
    }
    if selection.workspace_trust.is_some() {
        return Err("custom agents have no workspace trust switch".into());
    }
    if selection.container.is_some() {
        return Err(farhelm_proto::RETIRED_CONTAINER_REASON.into());
    }
    farhelm_proto::check_permission_flags(
        selection.permissions,
        selection.permission_flags.as_deref(),
    )?;
    farhelm_proto::check_wrapper_prefix(selection.wrapper_prefix.as_deref())?;

    selection.model = selection.model.filter(|model| !model.is_empty());
    selection.effort = selection
        .effort
        .filter(|effort| !effort.as_cli_arg().is_empty());
    let mut start = selection.wrapper_prefix.clone().unwrap_or_default();
    start.push(agent.command.clone());
    for (flag, value) in [
        ("--model", selection.model.as_deref()),
        (
            "--effort",
            selection.effort.as_ref().map(|effort| effort.as_cli_arg()),
        ),
    ] {
        if let Some(value) = value {
            validate_model_or_effort(value).map_err(|reason| format!("{flag}: {reason}"))?;
            start.extend([flag.to_owned(), value.to_owned()]);
        }
    }
    let resume = agent.supports_resumption.then(|| {
        let mut resume = start.clone();
        resume.extend(
            [
                "--resume",
                CONVERSATION_PLACEHOLDER,
                FARHELM_ARGS_PLACEHOLDER,
            ]
            .map(str::to_owned),
        );
        resume
    });
    start.push(FARHELM_ARGS_PLACEHOLDER.to_owned());
    let launch = SessionLaunch::Agent {
        selection,
        start,
        resume,
        custom_reporting: Some(agent.supports_reporting),
    };
    launch.validate_new()?;
    Ok(launch)
}

/// Which all-hosts opt-in governs this harness's structured launches.
/// The compiler itself never reads configuration: callers resolve this fact
/// into the selection before composition, preserving exact YOLO verification.
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) fn workspace_trust_enabled(
    harness: LaunchHarness,
    config: &crate::checkout_config::ResolvedCheckoutConfig,
) -> bool {
    match harness {
        LaunchHarness::Claude => config.claude_trust,
        LaunchHarness::Codex => config.codex_trust,
        LaunchHarness::Muse => config.muse_trust,
        LaunchHarness::Pi => config.pi_trust,
        LaunchHarness::Antigravity => config.antigravity_trust,
        LaunchHarness::Cursor
        | LaunchHarness::Grok
        | LaunchHarness::Goose
        | LaunchHarness::OpenCode
        | LaunchHarness::Omp
        | LaunchHarness::Bash
        | LaunchHarness::Zsh
        | LaunchHarness::Custom => false,
    }
}

/// These trust mechanisms apply to a directory. Pin it before asking for approval so dispatch uses the shown path.
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) fn workspace_trust_pins_approval_cwd(harness: LaunchHarness) -> bool {
    match harness {
        LaunchHarness::Claude
        | LaunchHarness::Codex
        | LaunchHarness::Muse
        | LaunchHarness::Pi
        | LaunchHarness::Antigravity => true,
        LaunchHarness::Cursor
        | LaunchHarness::Goose
        | LaunchHarness::Grok
        | LaunchHarness::Omp
        | LaunchHarness::OpenCode
        | LaunchHarness::Bash
        | LaunchHarness::Zsh
        | LaunchHarness::Custom => false,
    }
}

/// The user's Antigravity settings take precedence over Farhelm's override.
/// Words after a bare `--` are positional values.
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) fn workspace_trust_blocked_by_selection(selection: &LaunchSelection) -> bool {
    match selection.harness {
        LaunchHarness::Antigravity => antigravity_custom_words(selection)
            .iter()
            .take_while(|word| word.as_str() != "--")
            .any(|word| word == "--settings" || word.starts_with("--settings=")),
        LaunchHarness::Claude
        | LaunchHarness::Codex
        | LaunchHarness::Cursor
        | LaunchHarness::Goose
        | LaunchHarness::Grok
        | LaunchHarness::Muse
        | LaunchHarness::Omp
        | LaunchHarness::OpenCode
        | LaunchHarness::Pi
        | LaunchHarness::Bash
        | LaunchHarness::Zsh
        | LaunchHarness::Custom => false,
    }
}

/// Only Custom flags count as user-authored CLI options. Other permission
/// modes use the compiler's own argument list.
fn antigravity_custom_words(selection: &LaunchSelection) -> &[String] {
    if selection.permissions == Some(LaunchPermission::Custom) {
        selection.permission_flags.as_deref().unwrap_or_default()
    } else {
        &[]
    }
}

/// Count the Custom words beginning with a harness's end-of-options marker.
/// Workspace settings and conversation selectors belong before that tail
/// because the CLI reads the rest as operands.
#[warn(clippy::wildcard_enum_match_arm)]
fn custom_positional_tail_len(selection: &LaunchSelection) -> usize {
    match selection.harness {
        LaunchHarness::Antigravity => {
            let words = antigravity_custom_words(selection);
            words
                .iter()
                .position(|word| word == "--")
                .map_or(0, |terminator| words.len() - terminator)
        }
        LaunchHarness::Claude
        | LaunchHarness::Codex
        | LaunchHarness::Cursor
        | LaunchHarness::Goose
        | LaunchHarness::Grok
        | LaunchHarness::Muse
        | LaunchHarness::Omp
        | LaunchHarness::OpenCode
        | LaunchHarness::Pi
        | LaunchHarness::Bash
        | LaunchHarness::Zsh
        | LaunchHarness::Custom => 0,
    }
}

/// Compiler-owned trust words, independent of tool permissions. Codex's
/// directory marker waits for the supervisor to resolve the final cwd.
/// False remains supported because stored launches must still recompose.
#[warn(clippy::wildcard_enum_match_arm)]
fn workspace_trust_arguments(
    harness: LaunchHarness,
    trust: Option<bool>,
) -> &'static [&'static str] {
    match harness {
        LaunchHarness::Codex => match trust {
            Some(true) => &[
                "-c",
                farhelm_supervisor::agent_kind::CODEX_TRUSTED_CWD_PLACEHOLDER,
            ],
            Some(false) => &[
                "-c",
                farhelm_supervisor::agent_kind::CODEX_UNTRUSTED_CWD_PLACEHOLDER,
            ],
            None => &[],
        },
        LaunchHarness::Muse => match trust {
            Some(true) => &["--trust-workspace"],
            Some(false) | None => &[],
        },
        LaunchHarness::Pi => match trust {
            Some(true) => &["--approve"],
            Some(false) => &["--no-approve"],
            None => &[],
        },
        LaunchHarness::Antigravity => match trust {
            Some(true) => &[
                "--settings",
                farhelm_supervisor::agent_kind::ANTIGRAVITY_TRUSTED_CWD_PLACEHOLDER,
            ],
            Some(false) | None => &[],
        },
        LaunchHarness::Claude
        | LaunchHarness::Cursor
        | LaunchHarness::Grok
        | LaunchHarness::Goose
        | LaunchHarness::OpenCode
        | LaunchHarness::Omp
        | LaunchHarness::Bash
        | LaunchHarness::Zsh
        | LaunchHarness::Custom => &[],
    }
}

/// Replace only a stored agent launch's compiler-owned trust segment.
///
/// Clones and inherited spawns keep the source's other words, including
/// permission omissions and models no longer offered by today's catalog. The
/// start command records the segment before its final Farhelm marker or
/// Antigravity's Custom positional tail. Resume may insert a conversation
/// selector ahead of that tail, but its trust segment stays at the same
/// index. Older saved launches can also have the selector after the tail.
/// Checking the shared boundary keeps user-authored words out of the edit.
/// Callers must check the original launch against the agent YOLO guard first:
/// rewriting a copy must never erase the evidence that a source was forged.
pub(crate) fn with_workspace_trust(
    mut launch: SessionLaunch,
    trust: Option<bool>,
) -> Result<SessionLaunch, String> {
    let SessionLaunch::Agent {
        selection,
        start,
        resume,
        ..
    } = &mut launch
    else {
        return Ok(launch);
    };
    if !selection.harness.offers_workspace_trust() || selection.workspace_trust == trust {
        return Ok(launch);
    }
    let old_words = workspace_trust_arguments(selection.harness, selection.workspace_trust);
    let new_words = workspace_trust_arguments(selection.harness, trust);
    let farhelm_slot = [FARHELM_ARGS_PLACEHOLDER.to_string()];
    let old_prefix = start
        .strip_suffix(&farhelm_slot)
        .ok_or("the stored launch has no final Farhelm argument boundary")?;
    let trust_end = old_prefix
        .len()
        .checked_sub(custom_positional_tail_len(selection))
        .ok_or("the stored launch has no workspace-trust boundary")?;
    let trust_start = trust_end
        .checked_sub(old_words.len())
        .filter(|&index| {
            old_prefix[index..trust_end]
                .iter()
                .map(String::as_str)
                .eq(old_words.iter().copied())
        })
        .ok_or("the stored launch's workspace-trust segment does not match its selection")?;
    if let Some(resume) = resume {
        if !resume.starts_with(old_prefix) {
            // A new delimited resume inserts exactly its selector between the
            // shared option prefix and the literal tail from Start.
            let (options, tail) = old_prefix.split_at(trust_end);
            let shares_delimited_boundary = !tail.is_empty()
                && resume
                    .strip_prefix(options)
                    .and_then(|rest| rest.strip_suffix(&farhelm_slot))
                    .and_then(|rest| rest.strip_suffix(tail))
                    .zip(resume_arguments(selection.harness))
                    .is_some_and(|(actual, expected)| {
                        actual
                            .iter()
                            .map(String::as_str)
                            .eq(expected.iter().copied())
                    });
            if !shares_delimited_boundary {
                return Err("the stored resume command does not share its start command's workspace-trust boundary".into());
            }
        }
        resume.splice(
            trust_start..trust_end,
            new_words.iter().map(|word| (*word).to_string()),
        );
    }
    start.splice(
        trust_start..trust_end,
        new_words.iter().map(|word| (*word).to_string()),
    );
    selection.workspace_trust = trust;
    Ok(launch)
}

/// What an agent type's resume command adds to its start command to select
/// the captured conversation, or `None` for an agent type Farhelm cannot
/// resume (Cursor, OpenCode and the plain shells).
///
/// These follow the start-command options. The compiler inserts them before
/// any Custom positional tail so an end-of-options marker cannot turn the
/// selector into an operand. Goose's `session` subcommand is already in Start.
fn resume_arguments(harness: LaunchHarness) -> Option<&'static [&'static str]> {
    match harness {
        LaunchHarness::Claude => Some(&["--resume", CONVERSATION_PLACEHOLDER]),
        LaunchHarness::Codex => Some(&["resume", CONVERSATION_PLACEHOLDER]),
        LaunchHarness::Muse => Some(&["resume", CONVERSATION_PLACEHOLDER]),
        LaunchHarness::Goose => Some(&["--resume", "--session-id", CONVERSATION_PLACEHOLDER]),
        LaunchHarness::Pi => Some(&["--session", CONVERSATION_PLACEHOLDER]),
        LaunchHarness::Omp | LaunchHarness::Grok => Some(&["--resume", CONVERSATION_PLACEHOLDER]),
        LaunchHarness::Antigravity => Some(&["--conversation", CONVERSATION_PLACEHOLDER]),
        LaunchHarness::Cursor
        | LaunchHarness::OpenCode
        | LaunchHarness::Bash
        | LaunchHarness::Zsh
        | LaunchHarness::Custom => None,
    }
}

/// Resolve a one-time conversation start while leaving the ordinary launch unchanged.
///
/// Reporting harnesses use their composed resume command directly. Cursor and
/// OpenCode have a separate selector only for this start: adding them to
/// `resume_arguments` would incorrectly offer ordinary Restart afterwards.
/// Older stored Muse launches also retain their missing resume command; named
/// start still works without backfilling an ordinary Restart command.
pub(crate) fn named_start(
    launch: &SessionLaunch,
    value: String,
) -> Result<farhelm_proto::NamedConversationStart, String> {
    let selection = launch
        .agent_selection()
        .ok_or_else(|| "a conversation selector is only offered for agent launches".to_string())?;
    if !selection.harness.offers_named_conversation() {
        return Err("this agent does not offer a named conversation start".into());
    }
    farhelm_proto::conversation::validate_launch_conversation(selection.harness, &value)?;
    let integration = farhelm_supervisor::agent_kind::IntegrationSnapshot::of(launch);
    let argv = if integration.resume_template.is_some() {
        integration.filled_named_resume_argv(&value)?
    } else {
        let suffix: &[&str] = match selection.harness {
            // Existing launches are not migrated when Muse gains capture.
            LaunchHarness::Muse => &["resume"],
            LaunchHarness::Cursor => &["--resume"],
            LaunchHarness::OpenCode => &["--session"],
            LaunchHarness::Claude
            | LaunchHarness::Codex
            | LaunchHarness::Goose
            | LaunchHarness::Pi
            | LaunchHarness::Omp
            | LaunchHarness::Grok
            | LaunchHarness::Antigravity
            | LaunchHarness::Bash
            | LaunchHarness::Zsh
            | LaunchHarness::Custom => {
                return Err("this agent launch has no resume command".into());
            }
        };
        let mut argv = launch.start_argv()?;
        let marker = argv
            .iter()
            .position(|arg| arg == FARHELM_ARGS_PLACEHOLDER)
            .ok_or_else(|| "agent start has no Farhelm argument slot".to_string())?;
        argv.splice(
            marker..marker,
            suffix
                .iter()
                .map(|word| word.to_string())
                .chain(std::iter::once(value.clone())),
        );
        argv
    };
    Ok(farhelm_proto::NamedConversationStart {
        conversation: value,
        argv,
    })
}

/// An agent launch carrying exactly `selection`, for tests that need a
/// session or reply holding one without going through the catalog (which
/// would normalize or refuse the choices under test): the harness's bare
/// program and `{farhelm_args}`, with no resume command.
#[cfg(test)]
pub(crate) fn test_agent_launch(selection: LaunchSelection) -> SessionLaunch {
    let start = vec![
        program(selection.harness).to_string(),
        FARHELM_ARGS_PLACEHOLDER.to_string(),
    ];
    SessionLaunch::Agent {
        custom_reporting: None,
        selection,
        start,
        resume: None,
    }
}

/// The literal program resolved on the host's PATH. A shell session uses the
/// ordinary program lookup, including when a launcher wrapper is present.
fn program(harness: LaunchHarness) -> &'static str {
    match harness {
        LaunchHarness::Codex => "codex",
        LaunchHarness::Claude => "claude",
        LaunchHarness::Muse => "muse",
        LaunchHarness::Cursor => "cursor-agent",
        LaunchHarness::OpenCode => "opencode",
        LaunchHarness::Goose => "goose",
        LaunchHarness::Pi => "pi",
        LaunchHarness::Omp => "omp",
        LaunchHarness::Grok => "grok",
        LaunchHarness::Antigravity => "agy",
        LaunchHarness::Bash => "bash",
        LaunchHarness::Zsh => "zsh",
        LaunchHarness::Custom => unreachable!("the built-in compiler has no catalog command"),
    }
}

/// Reject unsupported explicit choices without inventing a required model.
///
/// The catalog constrains an entered model and effort; an omitted model is
/// valid for every harness and is resolved by that harness when it starts.
fn validate_selection(selection: &LaunchSelection) -> Result<(), String> {
    if selection.harness.uses_custom_catalog() || selection.custom_agent.is_some() {
        return Err("a custom agent needs a helm catalog entry".into());
    }
    let effort = selection
        .effort
        .as_ref()
        .map(|value| {
            value
                .known()
                .ok_or_else(|| format!("unknown built-in effort {}", value.as_cli_arg()))
        })
        .transpose()?;
    farhelm_proto::check_wrapper_prefix(selection.wrapper_prefix.as_deref())?;
    if let Some(container) = &selection.container {
        container.validate()?;
    }
    farhelm_proto::check_permission_flags(
        selection.permissions,
        selection.permission_flags.as_deref(),
    )?;
    if let Some(words) = &selection.permission_flags {
        for word in words {
            if farhelm_supervisor::agent_kind::is_reserved_placeholder(word) {
                return Err("custom permission flags cannot contain Farhelm placeholders".into());
            }
        }
    }
    if selection.workspace_trust.is_some() && !selection.harness.offers_workspace_trust() {
        return Err("workspace trust is not offered by this harness".to_string());
    }
    if selection.harness == LaunchHarness::Grok && selection.model.is_some() {
        return Err("Grok does not currently expose a verified model choice".to_string());
    }
    if let Some(model) = &selection.model {
        if !selection.harness.offers_model() {
            return Err("a shell session does not accept a model choice".to_owned());
        }
        validate_model_id(model)?;
        if selection.harness == LaunchHarness::OpenCode {
            opencode_model_argument(model)?;
        }
        // Ownership and effort are the catalog's facts about the harness's
        // own spelling of the model, not the raw text: a bare OpenCode name
        // can equal another harness's catalog id (`LaunchHarness::
        // catalog_model_id`). The messages below keep the text as typed.
        let catalog_id = selection.harness.catalog_model_id(model);
        let mut known = CATALOG.iter().filter(|entry| entry.id == catalog_id);
        if let Some(known) = known
            .clone()
            .find(|entry| entry.harness == selection.harness)
        {
            if let Some(effort) = effort
                && !known.efforts.contains(&effort)
            {
                return Err(format!(
                    "Farhelm's released offering for model {model:?} does not include effort {:?}",
                    effort
                ));
            }
        } else if known.any(|entry| entry.latest) {
            // Retaining an older entry must preserve custom uses of its spelling.
            return Err(format!(
                "model {model:?} belongs to a different harness than {:?}",
                selection.harness
            ));
        }
    }
    if let Some(effort) = effort
        && !harness_efforts(selection.harness).contains(&effort)
    {
        return Err(format!(
            "Farhelm's released {:?} offering does not include effort {:?}",
            selection.harness, effort
        ));
    }
    // The harness default (`None`) is always accepted; which explicit modes
    // each harness offers is `LaunchHarness::offers_permission`'s table, the
    // same one the desktop app offers buttons from.
    if let Some(permission) = selection.permissions
        && !selection.harness.offers_permission(permission)
    {
        return Err(format!(
            "the {permission:?} permission is not offered by this harness"
        ));
    }
    Ok(())
}

/// Return OpenCode's verified provider-qualified argument without changing
/// the saved selection. Bare values are Zen model names; a slash therefore
/// names a provider and must be OpenCode itself.
fn opencode_model_argument(model: &str) -> Result<String, String> {
    match model.split_once('/') {
        None => Ok(LaunchHarness::OpenCode.catalog_model_id(model).into_owned()),
        Some(("opencode", suffix)) if !suffix.is_empty() => Ok(model.to_string()),
        Some((provider, _)) => Err(format!(
            "OpenCode model {model:?} names provider {provider:?}; only the opencode provider is supported"
        )),
    }
}

fn validate_model_id(model: &str) -> Result<(), String> {
    if model.is_empty() {
        return Err("model id cannot be empty".to_string());
    }
    if model.len() > MAX_MODEL_ID_BYTES {
        return Err(format!("model id exceeds {MAX_MODEL_ID_BYTES} bytes"));
    }
    if model.starts_with('-')
        || model.contains(char::is_control)
        || model.contains(char::is_whitespace)
    {
        return Err("model id must be one literal non-option argument without whitespace or control characters".to_string());
    }
    if farhelm_supervisor::agent_kind::is_reserved_placeholder(model) {
        return Err("model id cannot be a reserved launch placeholder".to_string());
    }
    Ok(())
}

fn harness_efforts(harness: LaunchHarness) -> &'static [LaunchEffort] {
    match harness {
        LaunchHarness::Codex => CODEX_EFFORTS,
        LaunchHarness::Claude => CLAUDE_EFFORTS,
        LaunchHarness::Muse => MUSE_EFFORTS,
        LaunchHarness::Cursor => &[],
        LaunchHarness::OpenCode => OPENCODE_EFFORTS,
        LaunchHarness::Goose => GOOSE_EFFORTS,
        LaunchHarness::Pi => PI_EFFORTS,
        LaunchHarness::Omp => OMP_EFFORTS,
        LaunchHarness::Grok | LaunchHarness::Bash | LaunchHarness::Zsh | LaunchHarness::Custom => {
            &[]
        }
        LaunchHarness::Antigravity => ANTIGRAVITY_EFFORTS,
    }
}

#[cfg(test)]
mod tests {
    /// A named start uses one literal selector and never changes saved choices.
    /// Pin every offering harness so a new resume form cannot silently make
    /// later Clone reuse the one-time conversation or widen ordinary Restart.
    #[test]
    fn named_start_uses_resume_argv_without_storing_the_selector() {
        for harness in LaunchHarness::BUILT_INS {
            let launch = super::compile(selection(*harness)).unwrap();
            let before = launch.clone();
            let named = super::named_start(&launch, "named-target".into());
            if harness.is_shell() {
                assert!(named.is_err(), "{harness:?}");
                continue;
            }
            let named = named.unwrap();
            assert_eq!(launch, before);
            assert_eq!(named.conversation, "named-target");
            assert_eq!(
                named
                    .argv
                    .iter()
                    .filter(|arg| *arg == "named-target")
                    .count(),
                1
            );
            assert!(!named.argv.iter().any(|arg| arg == "{conversation}"));
            assert!(
                !launch
                    .start_argv()
                    .unwrap()
                    .contains(&"named-target".into())
            );
            if let Some(resume) = launch.resume_argv().unwrap() {
                let expected: Vec<String> = resume
                    .into_iter()
                    .map(|arg| {
                        if arg == "{conversation}" {
                            "named-target".into()
                        } else {
                            arg
                        }
                    })
                    .collect();
                assert_eq!(named.argv, expected, "{harness:?}");
            } else {
                let selector = match harness {
                    LaunchHarness::Muse => "resume",
                    LaunchHarness::Cursor => "--resume",
                    LaunchHarness::OpenCode => "--session",
                    _ => panic!("all reporting harnesses have an ordinary resume command"),
                };
                let index = named.argv.iter().position(|word| word == selector).unwrap();
                assert_eq!(named.argv[index + 1], "named-target");
                assert_eq!(
                    harness.named_conversation_capture(),
                    farhelm_proto::NamedConversationCapture::Never
                );
            }
        }
    }

    /// Stored Muse launches gain the derived kind without gaining a saved
    /// resume command. Their one-time named start must still work, preserve
    /// wrapper and permission choices, and leave ordinary Restart unavailable.
    #[test]
    fn older_muse_launches_keep_named_start_without_backfilling_resume() {
        let mut input = selection(LaunchHarness::Muse);
        input.wrapper_prefix = Some(vec!["env".into(), "WRAPPED=yes".into()]);
        let mut launch = super::compile(input).expect("Muse compiles");
        let SessionLaunch::Agent { resume, .. } = &mut launch else {
            panic!("agent launch");
        };
        *resume = None;
        let original = launch.clone();
        assert_eq!(launch.agent_kind(), AgentKind::Muse);
        let snapshot = farhelm_supervisor::agent_kind::IntegrationSnapshot::of(&launch);
        assert_eq!(
            snapshot.restart_offer(Some("captured-id"), 0),
            farhelm_proto::RestartOffer::NoResumeCommand
        );
        let named = super::named_start(&launch, "saved-id".into()).expect("old named start works");
        assert_eq!(
            named.argv,
            [
                "env",
                "WRAPPED=yes",
                "muse",
                "--permission-profile",
                ":auto-review",
                "resume",
                "saved-id",
                FARHELM_ARGS_PLACEHOLDER
            ]
        );
        assert_eq!(launch, original);
        assert_eq!(launch.resume_argv().expect("valid launch"), None);
    }

    /// User text must not become an option or a second placeholder pass.
    /// Unknown ids remain valid launch arguments; Farhelm does not inspect
    /// vendor stores, and relative session files resolve in the session folder.
    #[test]
    fn named_start_refuses_flag_and_placeholder_inputs() {
        let launch = super::compile(selection(LaunchHarness::Codex)).unwrap();
        for invalid in [
            "",
            "-bad",
            "two words",
            "{cwd}",
            "{conversation}",
            "{farhelm_args}",
            "{codex:trusted-cwd}",
            "{codex:untrusted-cwd}",
            "codex:bad",
            "omp:bad",
            "a\0b",
            "a'b",
        ] {
            assert!(
                super::named_start(&launch, invalid.into()).is_err(),
                "{invalid:?}"
            );
        }
        for harness in [LaunchHarness::Pi, LaunchHarness::Omp] {
            let launch = super::compile(selection(harness)).unwrap();
            assert!(super::named_start(&launch, "~/session.jsonl".into()).is_err());
            assert!(super::named_start(&launch, "relative/session.jsonl".into()).is_ok());
        }
        assert!(
            super::named_start(&SessionLaunch::plain_command("echo hello"), "id".into()).is_err()
        );
    }
    /// Custom replaces only the permission slot, with identical word boundaries
    /// in fresh and resume argv. It remains YOLO even for flags that name a
    /// guarded vendor mode; Farhelm intentionally does not interpret them.
    #[test]
    fn custom_permission_words_replace_flags_on_every_supported_harness() {
        for (harness, prefix, suffix) in [
            (LaunchHarness::Claude, vec!["claude"], Some("--resume")),
            (LaunchHarness::Codex, vec!["codex"], Some("resume")),
            (
                LaunchHarness::Grok,
                vec!["grok", "--no-leader"],
                Some("--resume"),
            ),
            (LaunchHarness::Muse, vec!["muse"], Some("resume")),
            (
                LaunchHarness::Antigravity,
                vec!["agy"],
                Some("--conversation"),
            ),
            (LaunchHarness::Cursor, vec!["cursor-agent"], None),
            (LaunchHarness::Omp, vec!["omp"], Some("--resume")),
        ] {
            let words = vec!["--mode".into(), "ask me".into(), "$HOME".into()];
            let mut input = selection(harness);
            input.permissions = Some(LaunchPermission::Custom);
            input.permission_flags = Some(words.clone());
            let launch = super::compile(input.clone()).unwrap();
            assert_eq!(launch.yolo(), Some(true), "{harness:?} is always YOLO");
            assert_eq!(launch.agent_selection(), Some(&input));
            let SessionLaunch::Agent { start, resume, .. } = &launch else {
                panic!("agent")
            };
            let mut expected: Vec<String> = prefix.into_iter().map(str::to_string).collect();
            expected.extend(words);
            let mut resumed = expected.clone();
            expected.push(FARHELM_ARGS_PLACEHOLDER.into());
            assert_eq!(*start, expected, "{harness:?} fresh");
            let expected_resume = suffix.map(|suffix| {
                resumed.extend([
                    suffix.into(),
                    CONVERSATION_PLACEHOLDER.into(),
                    FARHELM_ARGS_PLACEHOLDER.into(),
                ]);
                resumed
            });
            assert_eq!(*resume, expected_resume, "{harness:?} resume");
            let restored: SessionLaunch =
                serde_json::from_str(&serde_json::to_string(&launch).unwrap()).unwrap();
            assert_eq!(restored, launch);
        }
    }

    /// Admission refuses malformed relationships and every supervisor token,
    /// so a user word cannot become a later Farhelm substitution pass.
    #[test]
    fn custom_permissions_refuse_unsupported_modes_and_placeholder_words() {
        for harness in [
            LaunchHarness::Goose,
            LaunchHarness::Pi,
            LaunchHarness::OpenCode,
        ] {
            let mut input = selection(harness);
            input.permissions = Some(LaunchPermission::Custom);
            input.permission_flags = Some(vec!["--mode".into()]);
            assert!(super::compile(input).is_err(), "{harness:?}");
        }
        let mut input = selection(LaunchHarness::Codex);
        input.permissions = Some(LaunchPermission::Custom);
        assert!(super::compile(input.clone()).is_err());
        input.permission_flags = Some(vec![]);
        assert!(super::compile(input.clone()).is_err());
        for placeholder in [
            "{cwd}",
            "{conversation}",
            "{farhelm_args}",
            "{codex:trusted-cwd}",
            "{codex:untrusted-cwd}",
        ] {
            input.permission_flags = Some(vec![placeholder.into()]);
            assert!(
                super::compile(input.clone())
                    .unwrap_err()
                    .contains("placeholders"),
                "{placeholder}"
            );
        }
        input.permissions = Some(LaunchPermission::Guarded);
        input.permission_flags = Some(vec!["--mode".into()]);
        assert!(super::compile(input).is_err());
    }
    use super::{CATALOG, normalize_selection};
    use farhelm_proto::{
        AgentKind, SessionLaunch,
        launch::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection},
        session_launch::{CONVERSATION_PLACEHOLDER, FARHELM_ARGS_PLACEHOLDER},
    };
    use std::collections::{BTreeMap, BTreeSet};

    /// A wrapper implements these literal flags. Keep the ordinary launch reusable and put the one-time
    /// conversation only in the argv used for the named start.
    #[farhelm_testtrace::test]
    fn custom_agent_compiler_pins_the_wrapper_contract() {
        const CONTRACT: &str =
            "website/src/content/docs/docs/agents/custom-agent-contract.md, Contract changelog";
        let registration = farhelm_proto::custom_agents::CustomAgent {
            id: "acme-code".into(),
            display_name: "Acme Code".into(),
            command: "~/bin/acme-code".into(),
            supports_resumption: true,
            supports_reporting: false,
        };
        let mut input = selection(LaunchHarness::Custom);
        input.custom_agent = Some(registration.id.clone());
        input.wrapper_prefix = Some(vec!["exec".into()]);
        input.model = Some("fast-v2".into());
        input.effort = Some(farhelm_proto::LaunchEffortValue::Custom("deep_work".into()));
        let launch = super::compile_custom(input.clone(), &registration).expect(CONTRACT);
        assert_eq!(launch.yolo(), Some(true), "{CONTRACT}");
        assert_eq!(launch.custom_reporting(), Some(false), "{CONTRACT}");
        assert_eq!(
            launch.start_argv().unwrap(),
            [
                "exec",
                "~/bin/acme-code",
                "--model",
                "fast-v2",
                "--effort",
                "deep_work",
                "{farhelm_args}"
            ],
            "{CONTRACT}"
        );
        assert_eq!(
            launch.resume_argv().unwrap().unwrap(),
            [
                "exec",
                "~/bin/acme-code",
                "--model",
                "fast-v2",
                "--effort",
                "deep_work",
                "--resume",
                "{conversation}",
                "{farhelm_args}"
            ],
            "{CONTRACT}"
        );
        let named = super::named_start(&launch, "conv-1".into()).expect(CONTRACT);
        assert_eq!(
            named.argv,
            [
                "exec",
                "~/bin/acme-code",
                "--model",
                "fast-v2",
                "--effort",
                "deep_work",
                "--resume",
                "conv-1",
                "{farhelm_args}"
            ],
            "{CONTRACT}"
        );

        let launch_only = farhelm_proto::custom_agents::CustomAgent {
            supports_resumption: false,
            supports_reporting: true,
            ..registration.clone()
        };
        input.model = Some(String::new());
        input.effort = None;
        let bare = super::compile_custom(input.clone(), &launch_only).expect(CONTRACT);
        assert_eq!(
            bare.start_argv().unwrap(),
            ["exec", "~/bin/acme-code", "{farhelm_args}"],
            "{CONTRACT}"
        );
        assert!(bare.resume_argv().unwrap().is_none(), "{CONTRACT}");
        assert!(
            super::named_start(&bare, "conv-1".into()).is_err(),
            "{CONTRACT}"
        );
        assert!(
            bare.validate_initial_prompt(Some("hello"), false).is_err(),
            "{CONTRACT}"
        );

        input.permissions = Some(LaunchPermission::Guarded);
        assert!(
            super::compile_custom(input.clone(), &registration).is_err(),
            "{CONTRACT}"
        );
        input.permissions = None;
        input.model = Some("not one token".into());
        assert!(
            super::compile_custom(input, &registration).is_err(),
            "{CONTRACT}"
        );
    }

    /// A family lookup needs one current row per harness. Duplicate IDs would
    /// otherwise make exact lookup and effort validation depend on table order.
    #[test]
    fn each_catalog_family_has_one_latest_per_harness() {
        let mut ids = BTreeSet::new();
        let mut families = BTreeMap::new();
        for model in CATALOG {
            assert!(
                ids.insert((model.harness, model.id)),
                "duplicate model: {model:?}"
            );
            assert!(!model.family.is_empty(), "unnamed family: {model:?}");
            let latest = families.entry((model.harness, model.family)).or_insert(0);
            *latest += usize::from(model.latest);
        }
        for ((harness, family), latest) in families {
            assert_eq!(latest, 1, "{harness:?} {family}");
        }
    }

    /// Before the catalog arrives the launcher must recognize all and only the words it will later resolve.
    #[test]
    fn catalog_families_match_the_launchers_reserved_words() {
        let catalog_families: BTreeSet<_> = CATALOG.iter().map(|model| model.family).collect();
        let reserved = farhelm_proto::launcher::CATALOG_FAMILIES;
        assert_eq!(catalog_families, reserved.iter().copied().collect());
        assert_eq!(catalog_families.len(), reserved.len());
    }

    /// The shorthand follows Grok 4.6 on every offering, and the older ID stays in the same family.
    #[test]
    fn grok_family_keeps_the_older_id_and_uses_its_successor() {
        let catalog = super::launcher_catalog();
        for (harness, prefix) in [
            (LaunchHarness::Goose, "x-ai"),
            (LaunchHarness::Pi, "x-ai"),
            (LaunchHarness::Omp, "x-ai"),
            (LaunchHarness::OpenCode, "opencode"),
        ] {
            for (version, latest) in [("4.5", false), ("4.6", true)] {
                let id = format!("{prefix}/grok-{version}");
                let model = catalog
                    .iter()
                    .find(|model| model.harness == harness && model.id == id)
                    .unwrap_or_else(|| panic!("{harness:?} must still offer {id}"));
                assert_eq!(model.family, "grok", "{model:?}");
                assert_eq!(model.latest, latest, "{model:?}");
                assert_eq!(model.matches_family_word("GrOk"), latest, "{model:?}");
            }
        }
    }

    /// Registering an older name preserves the efforts 0.41.0 accepted on its
    /// original harness and preserves a deliberate custom selection on another one.
    #[test]
    fn an_older_astra_keeps_its_efforts_and_custom_cursor_spelling() {
        for effort in [LaunchEffort::Low, LaunchEffort::Medium, LaunchEffort::High] {
            let codex = compile(LaunchSelection {
                model: Some("gpt-6-astra".into()),
                effort: Some(effort.into()),
                ..selection(LaunchHarness::Codex)
            })
            .unwrap();
            assert_eq!(codex.selection.effort, Some(effort.into()));
        }
        let cursor = compile(LaunchSelection {
            model: Some("gpt-6-astra".into()),
            ..selection(LaunchHarness::Cursor)
        })
        .unwrap();
        assert!(
            shell_words::split(&cursor.invocation)
                .unwrap()
                .windows(2)
                .any(|args| args == ["--model", "gpt-6-astra"])
        );
    }

    /// A compiled agent launch, viewed the way these tests assert on it:
    /// the start command as the text a session displays (Farhelm's
    /// `{farhelm_args}` left out), the resume command likewise, and the
    /// launch itself for the tests about its placeholders.
    #[derive(Debug)]
    struct Compiled {
        invocation: String,
        agent_kind: AgentKind,
        selection: LaunchSelection,
        resume_template: Option<Vec<String>>,
        launch: SessionLaunch,
    }

    /// [`super::compile`], viewed as a [`Compiled`].
    fn compile(selection: LaunchSelection) -> Result<Compiled, String> {
        let launch = super::compile(selection)?;
        let SessionLaunch::Agent {
            selection,
            start: _,
            resume,
            ..
        } = &launch
        else {
            panic!("compile always makes an agent launch");
        };
        let without_args = |argv: &[String]| -> Vec<String> {
            argv.iter()
                .filter(|element| *element != FARHELM_ARGS_PLACEHOLDER)
                .cloned()
                .collect()
        };
        Ok(Compiled {
            invocation: launch.display_command(),
            agent_kind: launch.agent_kind(),
            selection: selection.clone(),
            resume_template: resume.as_deref().map(without_args),
            launch,
        })
    }

    /// Spec: every agent type's composed start command ends in
    /// `{farhelm_args}`; an agent type with conversation reporting also gets
    /// a resume command, which is the start command, that type's resume
    /// selector with `{conversation}`, then `{farhelm_args}`; Cursor and
    /// OpenCode get none. Every composed launch passes the launch rules.
    ///
    /// Why: SPEC.md has Farhelm compose both commands from the same choices
    /// and Restart run the resume command as composed. `{farhelm_args}` at
    /// the end is where the previous release appended Farhelm's arguments,
    /// and where the upgrade puts it in converted sessions, so a new launch
    /// and an upgraded one spawn alike.
    #[farhelm_testtrace::test]
    fn every_agent_type_composes_start_and_resume_with_farhelm_args() {
        for harness in LaunchHarness::BUILT_INS.iter().copied() {
            let launch = super::compile(selection(harness)).expect("default compiles");
            launch
                .validate_new()
                .expect("a composed launch passes the rules");
            let SessionLaunch::Agent { start, resume, .. } = &launch else {
                panic!("an agent launch");
            };
            assert_eq!(
                start.last().map(String::as_str),
                Some(FARHELM_ARGS_PLACEHOLDER),
                "{harness:?}"
            );
            let suffix: Option<&[&str]> = match harness {
                LaunchHarness::Claude => Some(&["--resume"]),
                LaunchHarness::Codex => Some(&["resume"]),
                LaunchHarness::Muse => Some(&["resume"]),
                LaunchHarness::Goose => Some(&["--resume", "--session-id"]),
                LaunchHarness::Pi => Some(&["--session"]),
                LaunchHarness::Omp | LaunchHarness::Grok => Some(&["--resume"]),
                LaunchHarness::Antigravity => Some(&["--conversation"]),
                LaunchHarness::Cursor
                | LaunchHarness::OpenCode
                | LaunchHarness::Bash
                | LaunchHarness::Zsh => None,
                LaunchHarness::Custom => unreachable!("only built-in choices are in this matrix"),
            };
            let expected = suffix.map(|suffix| {
                let mut resume = start[..start.len() - 1].to_vec();
                resume.extend(suffix.iter().map(|element| element.to_string()));
                resume.push(CONVERSATION_PLACEHOLDER.to_string());
                resume.push(FARHELM_ARGS_PLACEHOLDER.to_string());
                resume
            });
            assert_eq!(resume, &expected, "{harness:?}");
        }
    }

    /// Both shells resolve as bare PATH programs in the existing terminal.
    /// Model, permission and resume flags would alter the shell's invocation.
    #[test]
    fn shell_launches_compile_to_plain_programs_with_no_permission_mode() {
        for (harness, program) in [(LaunchHarness::Bash, "bash"), (LaunchHarness::Zsh, "zsh")] {
            let input = selection(harness);
            let launch = super::compile(input.clone()).expect("plain shell compiles");
            assert_eq!(launch.agent_kind(), farhelm_proto::AgentKind::Generic);
            assert_eq!(
                launch.start_argv().unwrap(),
                [program, FARHELM_ARGS_PLACEHOLDER]
            );
            assert_eq!(launch.agent_selection().unwrap().permissions, None);
            assert!(launch.resume_argv().unwrap().is_none());
            assert_eq!(launch.yolo(), Some(false));
            assert!(
                super::compile(LaunchSelection {
                    model: Some("custom-model".into()),
                    ..input.clone()
                })
                .is_err(),
                "unsupported options are refused before argv composition"
            );
            assert!(
                super::compile(LaunchSelection {
                    permissions: Some(LaunchPermission::Yolo),
                    ..input
                })
                .is_err()
            );
        }
    }

    /// A wrapper belongs to the stored launch, so every way to start that
    /// launch must preserve its raw spelling ahead of the program and its arguments.
    /// Goose is the sensitive case: its environment-setting `env` executable
    /// is part of the command being wrapped, not a replacement for the prefix.
    /// Shells still use the wrapper for a fresh start, but refuse a named start.
    #[farhelm_testtrace::test]
    fn wrapper_prefix_surrounds_every_agent_start_and_resume() {
        let prefix = vec![
            "with-endpoint".to_string(),
            "\"$PROFILE\"".to_string(),
            "'two words'".to_string(),
            "--".to_string(),
        ];
        for harness in LaunchHarness::BUILT_INS {
            let bare = super::compile(selection(*harness)).expect("bare agent compiles");
            let wrapped = super::compile(LaunchSelection {
                wrapper_prefix: Some(prefix.clone()),
                container: None,
                ..selection(*harness)
            })
            .expect("wrapped agent compiles");
            let bare_start = bare.start_argv().unwrap();
            let wrapped_start = wrapped.start_argv().unwrap();
            assert_eq!(wrapped_start[..prefix.len()], prefix, "{harness:?}");
            assert_eq!(wrapped_start[prefix.len()..], bare_start, "{harness:?}");
            if *harness == LaunchHarness::Goose {
                assert_eq!(wrapped_start[prefix.len()], "env");
            }
            if let Some(bare_resume) = bare.resume_argv().unwrap() {
                let wrapped_resume = wrapped.resume_argv().unwrap().unwrap();
                assert_eq!(wrapped_resume[..prefix.len()], prefix, "{harness:?}");
                assert_eq!(wrapped_resume[prefix.len()..], bare_resume, "{harness:?}");
            }
            let named = super::named_start(&wrapped, "saved".into());
            if harness.offers_named_conversation() {
                let named = named.expect("this harness supports a named start");
                assert_eq!(named.argv[..prefix.len()], prefix, "{harness:?}");
            } else {
                assert_eq!(
                    named.expect_err("this harness cannot start a named conversation"),
                    "this agent does not offer a named conversation start",
                    "{harness:?}"
                );
            }
        }
    }

    /// Isolation is launch intent, not an executable prefix. The supervisor must
    /// receive the choice intact and run the ordinary harness argv inside it.
    #[test]
    fn container_choice_survives_compilation_without_rewriting_harness_argv() {
        let mut input = selection(LaunchHarness::Claude);
        let baseline = super::compile(input.clone()).unwrap();
        let config = farhelm_proto::ContainerConfig {
            engine: farhelm_proto::ContainerEngine::Podman,
            image: "localhost/agent:latest".into(),
            api_proxy_port: 42000,
            github_token_file: Some("/credentials/token".into()),
        };
        input.container = Some(config.clone());
        let isolated = super::compile(input.clone()).unwrap();
        assert_eq!(isolated.agent_selection().unwrap().container, Some(config));
        assert_eq!(
            isolated.start_argv().unwrap(),
            baseline.start_argv().unwrap()
        );
        assert_eq!(
            isolated.resume_argv().unwrap(),
            baseline.resume_argv().unwrap()
        );
        input.container.as_mut().unwrap().api_proxy_port = 0;
        assert!(super::compile(input).is_err());
    }

    fn selection(harness: LaunchHarness) -> LaunchSelection {
        LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness,
            model: None,
            effort: None,
            permissions: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        }
    }

    /// Spec: `normalize_selection` is exactly the selection `compile`
    /// records, including omitted Cursor and Pi permissions becoming YOLO.
    ///
    /// Why: a fresh-checkout create reconciled after a lost reply records
    /// its selection without compiling. If the two disagreed, the same
    /// request would leave a different remembered permission default
    /// depending on whether its first reply arrived.
    #[farhelm_testtrace::test]
    fn normalized_and_compiled_selections_agree() {
        for harness in [LaunchHarness::Pi, LaunchHarness::Cursor] {
            let draft = selection(harness);
            let normalized = normalize_selection(draft.clone());
            assert_eq!(normalized.permissions, Some(LaunchPermission::Yolo));
            assert_eq!(compile(draft).expect("compile draft").selection, normalized);
        }
    }

    /// A saved template can reset Cursor's permission to the default. The
    /// resulting command uses the same explicit flag as a fresh Cursor launch.
    #[test]
    fn a_saved_cursor_template_can_reset_to_explicit_yolo() {
        use farhelm_proto::launcher::{
            LaunchTemplate, LauncherState, TemplateContext, apply_template,
        };

        let template: LaunchTemplate = serde_json::from_str(
            r#"{"name":"old-cursor","fields":{"agent":"cursor","permissions":null}}"#,
        )
        .expect("read the saved template");
        assert_eq!(template.fields.permissions, Some(None));
        let draft = apply_template(
            &LauncherState::default(),
            &template,
            &TemplateContext {
                catalog: &[],
                known_hosts: &[],
                known_recipes: &[],
                known_places: &[],
                host_fixed: false,
            },
        )
        .expect("apply Cursor template");
        assert_eq!(draft.harness, Some(LaunchHarness::Cursor));
        assert_eq!(draft.permissions, None, "the template cleared the choice");
        let launch = compile(LaunchSelection {
            permissions: draft.permissions,
            ..selection(draft.harness.unwrap())
        })
        .expect("compile Cursor from the saved template");
        assert_eq!(launch.selection.permissions, Some(LaunchPermission::Yolo));
        assert_eq!(launch.invocation, "cursor-agent --force");
    }

    /// A retired model no longer implies a harness. An old template can still pass the literal model ID once it
    /// names the harness explicitly.
    #[test]
    fn a_retired_catalog_model_requires_an_explicit_agent() {
        use farhelm_proto::launcher::{
            LaunchTemplate, LauncherState, TemplateContext, apply_template,
        };

        let model = "gpt-5.6-terra";
        let catalog = super::launcher_catalog();
        assert!(catalog.iter().all(|entry| !entry.id.ends_with(model)));
        let mut template: LaunchTemplate = serde_json::from_value(serde_json::json!({
            "name": "old-codex",
            "fields": { "model": model },
        }))
        .expect("read the saved template");
        let context = TemplateContext {
            catalog: &catalog,
            known_hosts: &[],
            known_recipes: &[],
            known_places: &[],
            host_fixed: false,
        };
        let empty = LauncherState::default();
        let refusal = apply_template(&empty, &template, &context)
            .expect_err("the retired model no longer supplies the agent");
        assert_eq!(refusal.field, "model");
        assert_eq!(refusal.reason, "needs an agent type, and none is chosen");

        template.fields.agent = Some(LaunchHarness::Codex);
        let draft = apply_template(&empty, &template, &context)
            .expect("apply a deliberately chosen retired model");
        assert_eq!(draft.harness, Some(LaunchHarness::Codex));
        assert_eq!(draft.model.as_deref(), Some(model));
        let launch = compile(LaunchSelection {
            model: draft.model,
            ..selection(draft.harness.unwrap())
        })
        .expect("compile the model as a custom choice");
        let argv = shell_words::split(&launch.invocation).unwrap();
        assert!(argv.windows(2).any(|words| words == ["-m", model]));
    }

    /// Cursor defaults to the same argv as an explicit YOLO choice. Its
    /// permissions and model do not grant conversation reporting or Restart.
    #[test]
    fn cursor_launches_preserve_intent_without_conversation_integration() {
        for model in [None, Some("composer-2.5"), Some("custom[effort=high]")] {
            for permissions in [None, Some(LaunchPermission::Yolo)] {
                let input = LaunchSelection {
                    custom_agent: None,
                    permission_flags: None,
                    harness: LaunchHarness::Cursor,
                    model: model.map(str::to_owned),
                    effort: None,
                    permissions,
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                };
                let launch = compile(input.clone()).unwrap();
                let mut expected = vec!["cursor-agent".to_string()];
                if let Some(model) = model {
                    expected.extend(["--model".into(), model.into()]);
                }
                expected.push("--force".into());
                assert_eq!(shell_words::split(&launch.invocation).unwrap(), expected);
                assert_eq!(
                    launch.selection,
                    LaunchSelection {
                        permissions: Some(LaunchPermission::Yolo),
                        ..input
                    }
                );
                assert_eq!(launch.agent_kind, AgentKind::Generic);
                let snapshot =
                    farhelm_supervisor::agent_kind::IntegrationSnapshot::of(&launch.launch);
                assert!(snapshot.integration().is_none());
                assert!(snapshot.resume_template.is_none());
                assert_eq!(
                    snapshot.restart_offer(Some("saved-id"), 0),
                    farhelm_proto::RestartOffer::NoConversationReporting
                );
            }
        }
        let mut input = selection(LaunchHarness::Cursor);
        input.effort = Some(LaunchEffort::High.into());
        assert!(compile(input).is_err());
        let mut input = selection(LaunchHarness::Cursor);
        input.permissions = Some(LaunchPermission::Approve);
        assert!(compile(input).is_err());
    }

    /// New omissions select explicit guarded flags while model and effort
    /// remain the vendor's choice. Pin the actual argv rather than deriving
    /// the expectation from the shared flag table, so UI and compiler cannot
    /// agree on the same wrong mapping unnoticed.
    #[test]
    fn request_defaults_compile_to_guarded_flags() {
        assert_eq!(
            compile(selection(LaunchHarness::Codex)).unwrap().invocation,
            "codex -a on-request -s workspace-write"
        );
        assert_eq!(
            compile(selection(LaunchHarness::Claude))
                .unwrap()
                .invocation,
            "claude --permission-mode auto"
        );
        assert_eq!(
            compile(selection(LaunchHarness::Muse)).unwrap().invocation,
            "muse --permission-profile :auto-review"
        );
        assert_eq!(
            compile(selection(LaunchHarness::Omp)).unwrap().invocation,
            "omp --approval-mode write"
        );
    }

    /// Guarded flags survive every supported resume selector, and cannot
    /// silently become a vendor default on a fresh launch either. OMP's old
    /// stored omissions remain YOLO; new omissions compile Guarded explicitly.
    #[test]
    fn guarded_modes_pin_fresh_and_resume_argv() {
        for (harness, flags, selector) in [
            (
                LaunchHarness::Claude,
                vec!["--permission-mode", "auto"],
                Some("--resume"),
            ),
            (
                LaunchHarness::Codex,
                vec!["-a", "on-request", "-s", "workspace-write"],
                Some("resume"),
            ),
            (
                LaunchHarness::Grok,
                vec!["--no-leader", "--permission-mode", "auto"],
                Some("--resume"),
            ),
            (
                LaunchHarness::Antigravity,
                vec!["--mode", "auto"],
                Some("--conversation"),
            ),
            (
                LaunchHarness::Muse,
                vec!["--permission-profile", ":auto-review"],
                Some("resume"),
            ),
            (
                LaunchHarness::Omp,
                vec!["--approval-mode", "write"],
                Some("--resume"),
            ),
        ] {
            let mut input = selection(harness);
            input.permissions = Some(LaunchPermission::Guarded);
            let launch = super::compile(input).unwrap();
            let SessionLaunch::Agent {
                selection,
                start,
                resume,
                ..
            } = launch
            else {
                panic!("agent launch")
            };
            assert_eq!(selection.permissions, Some(LaunchPermission::Guarded));
            let mut expected = vec![super::program(harness).to_string()];
            expected.extend(flags.iter().map(|flag| flag.to_string()));
            expected.push(FARHELM_ARGS_PLACEHOLDER.to_string());
            assert_eq!(start, expected, "{harness:?} fresh");
            let expected_resume = selector.map(|selector| {
                let mut argv = expected[..expected.len() - 1].to_vec();
                argv.extend([
                    selector.to_string(),
                    CONVERSATION_PLACEHOLDER.to_string(),
                    FARHELM_ARGS_PLACEHOLDER.to_string(),
                ]);
                argv
            });
            assert_eq!(resume, expected_resume, "{harness:?} resume");
        }
    }

    /// Each harness has a different CLI spelling even though the composer
    /// presents one consistent model/effort/permission vocabulary.
    #[test]
    fn explicit_choices_use_each_harnesses_documented_flags() {
        let codex = LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Codex,
            model: Some("gpt-6.2-sol".to_string()),
            effort: Some(LaunchEffort::High.into()),
            permissions: Some(LaunchPermission::Yolo),
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };
        assert_eq!(
            shell_words::split(&compile(codex).unwrap().invocation).unwrap(),
            [
                "codex",
                "-m",
                "gpt-6.2-sol",
                "-c",
                "model_reasoning_effort=high",
                "--yolo"
            ]
        );

        let claude = LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Claude,
            model: Some("claude-fable-5".to_string()),
            effort: Some(LaunchEffort::High.into()),
            permissions: Some(LaunchPermission::Yolo),
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };
        assert_eq!(
            shell_words::split(&compile(claude).unwrap().invocation).unwrap(),
            [
                "claude",
                "--model",
                "claude-fable-5",
                "--effort",
                "high",
                "--dangerously-skip-permissions"
            ]
        );

        let muse = LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Muse,
            model: Some("muse-spark-1.3-contributor".to_string()),
            effort: Some(LaunchEffort::Xhigh.into()),
            permissions: Some(LaunchPermission::Yolo),
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };
        assert_eq!(
            shell_words::split(&compile(muse).unwrap().invocation).unwrap(),
            [
                "muse",
                "--model",
                "muse-spark-1.3-contributor",
                "--reasoning-effort",
                "xhigh",
                "--yolo"
            ]
        );

        let opencode = LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::OpenCode,
            model: Some("opencode/grok-4.6".to_string()),
            effort: None,
            permissions: Some(LaunchPermission::Yolo),
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };
        let compiled = compile(opencode).expect("OpenCode's documented flags compile");
        assert_eq!(compiled.agent_kind, AgentKind::Generic);
        assert_eq!(
            shell_words::split(&compiled.invocation).unwrap(),
            ["opencode", "--model", "opencode/grok-4.6", "--auto"]
        );
    }

    /// An omitted OpenCode model leaves the vendor's configured choice in
    /// charge; explicitly entered Zen ids retain their argv qualification.
    #[test]
    fn opencode_default_and_explicit_zen_model_keep_distinct_argv() {
        let default = compile(selection(LaunchHarness::OpenCode)).expect("OpenCode default");
        assert_eq!(default.selection.model, None);
        assert_eq!(
            shell_words::split(&default.invocation).unwrap(),
            ["opencode", "--auto"]
        );

        let bare = LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::OpenCode,
            model: Some("private-zen-model".to_string()),
            effort: None,
            permissions: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };
        let compiled = compile(bare.clone()).expect("bare Zen model");
        assert_eq!(
            compiled.selection,
            LaunchSelection {
                permissions: Some(LaunchPermission::Yolo),
                ..bare.clone()
            },
            "the selection records typed intent"
        );
        assert_eq!(
            shell_words::split(&compiled.invocation).unwrap(),
            [
                "opencode",
                "--model",
                "opencode/private-zen-model",
                "--auto"
            ]
        );

        let other_provider = LaunchSelection {
            model: Some("anthropic/claude".to_string()),
            ..bare
        };
        assert!(
            compile(other_provider)
                .unwrap_err()
                .contains("only the opencode provider")
        );
    }

    /// Why: several bare Zen names are also another harness's catalog ids,
    /// and comparing the typed text refused an OpenCode launch of
    /// `gpt-6-luna` as "a different harness" although `opencode/gpt-6-luna`
    /// worked. Spec (SPEC.md, OpenCode's bare-name contract): every catalog
    /// OpenCode model launches under either spelling, the selection keeps
    /// the spelling typed, and the argv is provider-qualified. The same bare
    /// name stays its own harness's model for that harness. A current
    /// qualified name remains reserved for OpenCode.
    #[test]
    fn opencode_accepts_both_spellings_of_names_other_harnesses_also_list() {
        let overlapping = CATALOG
            .iter()
            .filter(|entry| entry.harness == LaunchHarness::OpenCode)
            .filter_map(|entry| entry.id.strip_prefix("opencode/"))
            .filter(|bare| CATALOG.iter().any(|entry| entry.id == *bare))
            .collect::<Vec<_>>();
        // Premise: the catalog still has the collisions this test is about.
        assert!(overlapping.contains(&"gpt-6-luna"), "{overlapping:?}");
        assert!(overlapping.contains(&"gpt-6-astra"), "{overlapping:?}");
        for bare in overlapping {
            for typed in [bare.to_string(), format!("opencode/{bare}")] {
                let opencode = LaunchSelection {
                    custom_agent: None,
                    permission_flags: None,
                    harness: LaunchHarness::OpenCode,
                    model: Some(typed.clone()),
                    effort: None,
                    permissions: None,
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                };
                let compiled = compile(opencode.clone())
                    .unwrap_or_else(|error| panic!("OpenCode {typed}: {error}"));
                assert_eq!(
                    compiled.selection,
                    LaunchSelection {
                        permissions: Some(LaunchPermission::Yolo),
                        ..opencode
                    },
                    "typed model intent kept with effective permission"
                );
                assert_eq!(
                    shell_words::split(&compiled.invocation).unwrap(),
                    ["opencode", "--model", &format!("opencode/{bare}"), "--auto"]
                );
            }
            let owner = CATALOG
                .iter()
                .find(|entry| entry.id == bare)
                .expect("overlap")
                .harness;
            assert!(
                compile(LaunchSelection {
                    model: Some(bare.to_string()),
                    ..selection(owner)
                })
                .is_ok(),
                "{bare} stays a {owner:?} model"
            );
            let qualified = format!("opencode/{bare}");
            let result = compile(LaunchSelection {
                model: Some(qualified.clone()),
                ..selection(owner)
            });
            if CATALOG
                .iter()
                .any(|entry| entry.id == qualified && entry.latest)
            {
                assert!(
                    result
                        .unwrap_err()
                        .contains("belongs to a different harness")
                );
            } else {
                assert_eq!(
                    result.unwrap().selection.model.as_deref(),
                    Some(qualified.as_str())
                );
            }
        }
    }

    /// The OpenCode CLI's `--auto` option is a permission policy, not an
    /// effort mechanism; a stale structured selection must not invent one.
    #[test]
    fn opencode_refuses_an_unsupported_effort() {
        let selection = LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::OpenCode,
            model: Some("opencode/glm-5.3".to_string()),
            effort: Some(LaunchEffort::High.into()),
            permissions: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };

        assert!(
            compile(selection)
                .unwrap_err()
                .contains("does not include effort")
        );
    }

    /// Suggested Zen models compile without effort or an implicit permission
    /// override; choosing a model must not change either policy.
    #[test]
    fn opencode_suggestions_compile_without_implicit_options() {
        let offered: Vec<_> = CATALOG
            .iter()
            .filter(|row| row.harness == LaunchHarness::OpenCode)
            .collect();
        for row in offered {
            assert!(row.efforts.is_empty());
            let compiled = compile(LaunchSelection {
                model: Some(row.id.to_string()),
                ..selection(LaunchHarness::OpenCode)
            })
            .expect("every suggested model compiles");
            assert_eq!(
                shell_words::split(&compiled.invocation).unwrap(),
                ["opencode", "--model", row.id, "--auto"]
            );
        }
    }

    /// Provider qualification must not turn shell-sensitive custom model
    /// text into syntax or rewrite the intent later used by clone/history.
    #[test]
    fn opencode_custom_qualification_preserves_one_literal_argument() {
        for model in ["custom'42;$literal", "opencode/custom'42;$literal"] {
            let input = LaunchSelection {
                model: Some(model.to_string()),
                ..selection(LaunchHarness::OpenCode)
            };
            let compiled = compile(input.clone()).expect("literal custom Zen model");
            assert_eq!(
                compiled.selection,
                LaunchSelection {
                    permissions: Some(LaunchPermission::Yolo),
                    ..input
                }
            );
            assert_eq!(
                shell_words::split(&compiled.invocation).unwrap(),
                [
                    "opencode",
                    "--model",
                    "opencode/custom'42;$literal",
                    "--auto"
                ]
            );
        }
    }

    /// A custom model remains one literal argv element even when it needs
    /// shell quoting; accepting it must not turn it into two user arguments.
    #[test]
    fn a_custom_model_id_is_quoted_as_one_argv_element() {
        let mut input = selection(LaunchHarness::Muse);
        input.model = Some("release/candidate'42;$literal".to_string());
        input.effort = Some(LaunchEffort::Xhigh.into());
        let compiled = compile(input).unwrap();
        assert_eq!(compiled.agent_kind, AgentKind::Muse);
        assert_eq!(
            shell_words::split(&compiled.invocation).unwrap(),
            [
                "muse",
                "--model",
                "release/candidate'42;$literal",
                "--reasoning-effort",
                "xhigh",
                "--permission-profile",
                ":auto-review"
            ]
        );
    }

    /// The known catalog protects cross-harness choices and its curated
    /// offerings, while malformed custom IDs cannot smuggle options or
    /// placeholder substitution through.
    #[test]
    fn incompatible_and_malformed_models_are_refused() {
        let mut wrong_harness = selection(LaunchHarness::Claude);
        wrong_harness.model = Some("gpt-6.2-sol".to_string());
        assert!(compile(wrong_harness).unwrap_err().contains("belongs"));

        for model in ["", "--model", "two words", "line\nbreak"]
            .into_iter()
            .chain(
                farhelm_supervisor::agent_kind::RESERVED_PLACEHOLDERS
                    .iter()
                    .copied(),
            )
        {
            let mut malformed = selection(LaunchHarness::Codex);
            malformed.model = Some(model.to_string());
            assert!(
                compile(malformed).is_err(),
                "model {model:?} must be refused"
            );
        }
        // Substitution only matches a whole element, so a marker inside a
        // longer id is ordinary text and stays accepted.
        let mut embedded = selection(LaunchHarness::Codex);
        embedded.model = Some("x{codex:trusted-cwd}".to_string());
        assert!(compile(embedded).is_ok());
    }

    /// Muse Contributor 1.3 deliberately stops at xhigh in Farhelm's
    /// released catalog. This is a model-specific evidence boundary: the
    /// broader Muse CLI vocabulary and a different non-contributor row do
    /// not establish max or ultra for this contributor offering.
    #[test]
    fn muse_contributor_catalog_stops_at_xhigh() {
        for effort in [LaunchEffort::Max, LaunchEffort::Ultra] {
            let selection = LaunchSelection {
                custom_agent: None,
                permission_flags: None,
                harness: LaunchHarness::Muse,
                model: Some("muse-spark-1.3-contributor".to_string()),
                effort: Some(effort.into()),
                permissions: None,
                wrapper_prefix: None,
                container: None,
                workspace_trust: None,
            };
            assert!(
                compile(selection).is_err(),
                "the contributor catalog must not imply {effort:?} support"
            );
        }
    }

    /// Goose and Pi share OpenRouter IDs but compile distinct, literal argv
    /// contracts; choosing one must never silently route to the other.
    #[test]
    fn goose_and_pi_compile_their_native_openrouter_contracts() {
        let goose = compile(LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Goose,
            model: Some("x-ai/grok-4.6".into()),
            effort: Some(LaunchEffort::Max.into()),
            permissions: Some(LaunchPermission::SmartApprove),
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        })
        .expect("Goose selection compiles");
        assert_eq!(
            shell_words::split(&goose.invocation).unwrap(),
            [
                "env",
                "GOOSE_THINKING_EFFORT=max",
                "GOOSE_MODE=smart_approve",
                "goose",
                "session",
                "--provider",
                "openrouter",
                "--model",
                "x-ai/grok-4.6"
            ]
        );

        let pi = compile(LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Pi,
            model: Some("x-ai/grok-4.6".into()),
            effort: Some(LaunchEffort::Minimal.into()),
            permissions: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        })
        .expect("Pi selection compiles");
        assert_eq!(pi.selection.permissions, Some(LaunchPermission::Yolo));
        assert_eq!(
            shell_words::split(&pi.invocation).unwrap(),
            [
                "pi",
                "--provider",
                "openrouter",
                "--model",
                "x-ai/grok-4.6",
                "--thinking",
                "minimal"
            ]
        );
    }

    /// Goose's four visible permission modes are environment values, while
    /// an omitted mode now selects Goose's stock autonomous mode explicitly.
    /// This pins the full mapping rather than one representative mode.
    #[test]
    fn goose_maps_every_permission_without_unsolicited_overrides() {
        let base = LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Goose,
            model: Some("z-ai/glm-5.3".into()),
            effort: None,
            permissions: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };
        assert_eq!(
            shell_words::split(&compile(base.clone()).unwrap().invocation).unwrap(),
            [
                "env",
                "GOOSE_MODE=auto",
                "goose",
                "session",
                "--provider",
                "openrouter",
                "--model",
                "z-ai/glm-5.3"
            ],
            "an omitted Goose permission uses its stock autonomous mode"
        );
        for (permission, mode) in [
            (LaunchPermission::Yolo, "auto"),
            (LaunchPermission::Approve, "approve"),
            (LaunchPermission::SmartApprove, "smart_approve"),
            (LaunchPermission::Chat, "chat"),
        ] {
            let compiled = compile(LaunchSelection {
                permissions: Some(permission),
                wrapper_prefix: None,
                container: None,
                workspace_trust: None,
                ..base.clone()
            })
            .expect("every offered Goose permission compiles");
            assert_eq!(
                shell_words::split(&compiled.invocation).unwrap(),
                [
                    "env",
                    &format!("GOOSE_MODE={mode}"),
                    "goose",
                    "session",
                    "--provider",
                    "openrouter",
                    "--model",
                    "z-ai/glm-5.3"
                ]
            );
        }
    }

    /// Pi has no tool-approval flag. Omission and explicit YOLO compile to
    /// the same argv, and Goose's approval modes are all rejected.
    #[test]
    fn pi_accepts_only_its_yolo_label_without_an_approve_flag() {
        let base = LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Pi,
            model: Some("z-ai/glm-5.3".into()),
            effort: None,
            permissions: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        };
        let omitted = shell_words::split(&compile(base.clone()).unwrap().invocation).unwrap();
        let yolo = shell_words::split(
            &compile(LaunchSelection {
                permissions: Some(LaunchPermission::Yolo),
                wrapper_prefix: None,
                container: None,
                workspace_trust: None,
                ..base.clone()
            })
            .unwrap()
            .invocation,
        )
        .unwrap();
        assert_eq!(omitted, yolo);
        assert!(!yolo.iter().any(|argument| argument == "--approve"));
        for permission in [
            LaunchPermission::Approve,
            LaunchPermission::SmartApprove,
            LaunchPermission::Chat,
        ] {
            assert!(
                compile(LaunchSelection {
                    permissions: Some(permission),
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                    ..base.clone()
                })
                .is_err(),
                "Pi must reject {permission:?}"
            );
        }
    }

    /// OMP compiles its own OpenRouter contract beside Pi's: the same
    /// provider-qualified model form, its own `--thinking` effort flag, and
    /// its own `--approval-mode` pair. Omission compiles Guarded explicitly,
    /// while an explicit approval choice carries its exact value.
    #[test]
    fn omp_compiles_its_native_openrouter_contract_with_an_explicit_approval_mode() {
        // Every optional flag at once, with the OMP-specific Approve mode.
        let full = compile(LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Omp,
            model: Some("x-ai/grok-4.6".into()),
            effort: Some(LaunchEffort::Minimal.into()),
            permissions: Some(LaunchPermission::Approve),
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        })
        .expect("OMP selection compiles");
        assert_eq!(full.agent_kind, AgentKind::Omp);
        assert_eq!(
            shell_words::split(&full.invocation).unwrap(),
            [
                "omp",
                "--provider",
                "openrouter",
                "--model",
                "x-ai/grok-4.6",
                "--thinking",
                "minimal",
                "--approval-mode",
                "always-ask"
            ]
        );

        // YOLO carries its own approval-mode value, never a bare flag.
        let yolo = compile(LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Omp,
            model: Some("z-ai/glm-5.3".into()),
            effort: Some(LaunchEffort::Xhigh.into()),
            permissions: Some(LaunchPermission::Yolo),
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        })
        .expect("OMP yolo compiles");
        assert_eq!(
            shell_words::split(&yolo.invocation).unwrap(),
            [
                "omp",
                "--provider",
                "openrouter",
                "--model",
                "z-ai/glm-5.3",
                "--thinking",
                "xhigh",
                "--approval-mode",
                "yolo"
            ]
        );

        // An omitted OMP permission is Farhelm's explicit Guarded default.
        let omitted = compile(LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Omp,
            model: Some("z-ai/glm-5.3".into()),
            effort: None,
            permissions: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        })
        .expect("OMP default compiles");
        assert_eq!(
            omitted.selection.permissions,
            Some(LaunchPermission::Guarded),
            "an omitted OMP permission normalizes to Guarded"
        );
        assert_eq!(
            shell_words::split(&omitted.invocation).unwrap(),
            [
                "omp",
                "--provider",
                "openrouter",
                "--model",
                "z-ai/glm-5.3",
                "--approval-mode",
                "write"
            ]
        );
    }

    /// OMP's effort list stops at max (the enum's `ultra` is not offered),
    /// while its model may defer to the configured default. Goose-only
    /// permission labels remain refused.
    #[test]
    fn omp_vocabulary_is_closed_and_model_optional() {
        for row in CATALOG
            .iter()
            .filter(|row| row.harness == LaunchHarness::Omp)
        {
            assert_eq!(row.efforts, super::OMP_EFFORTS);
            assert!(
                compile(LaunchSelection {
                    model: Some(row.id.to_string()),
                    ..selection(LaunchHarness::Omp)
                })
                .is_ok(),
                "every suggested OMP model compiles"
            );
        }

        // A custom id stays one literal argv element under OMP's contract.
        let custom = compile(LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Omp,
            model: Some("release/candidate'42;$literal".to_string()),
            effort: None,
            permissions: None,
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        })
        .expect("a custom OMP id is the escape hatch");
        assert_eq!(
            shell_words::split(&custom.invocation).unwrap(),
            [
                "omp",
                "--provider",
                "openrouter",
                "--model",
                "release/candidate'42;$literal",
                "--approval-mode",
                "write"
            ]
        );

        let default = compile(selection(LaunchHarness::Omp)).expect("OMP default");
        assert_eq!(default.selection.model, None);
        assert_eq!(
            shell_words::split(&default.invocation).unwrap(),
            ["omp", "--approval-mode", "write"]
        );
        // OMP's own effort vocabulary is OMP_EFFORTS: `ultra` (the shared
        // enum's extra level) is refused; every listed level compiles.
        for effort in super::OMP_EFFORTS {
            assert!(
                compile(LaunchSelection {
                    custom_agent: None,
                    permission_flags: None,
                    harness: LaunchHarness::Omp,
                    model: Some("z-ai/glm-5.3".into()),
                    effort: Some((*effort).into()),
                    permissions: None,
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                })
                .is_ok(),
                "OMP's catalog offers {effort:?}"
            );
        }
        assert!(
            compile(LaunchSelection {
                custom_agent: None,
                permission_flags: None,
                harness: LaunchHarness::Omp,
                model: Some("z-ai/glm-5.3".into()),
                effort: Some(LaunchEffort::Ultra.into()),
                permissions: None,
                wrapper_prefix: None,
                container: None,
                workspace_trust: None,
            })
            .is_err(),
            "ultra is not in OMP's released offering"
        );
        // OMP offers no permission beyond Approve and YOLO: the Goose-only
        // labels refuse exactly as they do for Pi.
        for permission in [LaunchPermission::SmartApprove, LaunchPermission::Chat] {
            assert!(
                compile(LaunchSelection {
                    custom_agent: None,
                    permission_flags: None,
                    harness: LaunchHarness::Omp,
                    model: Some("z-ai/glm-5.3".into()),
                    effort: None,
                    permissions: Some(permission),
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                })
                .is_err(),
                "OMP must reject {permission:?}"
            );
        }
    }

    /// Omitted models must leave provider and model configuration to each
    /// harness, including Goose's session subcommand. Pi's mandatory YOLO
    /// label is retained as metadata without adding model or provider flags.
    #[test]
    fn omitted_models_use_vendor_default_argv() {
        for (harness, expected) in [
            (LaunchHarness::OpenCode, vec!["opencode", "--auto"]),
            (
                LaunchHarness::Goose,
                vec!["env", "GOOSE_MODE=auto", "goose", "session"],
            ),
            (LaunchHarness::Pi, vec!["pi"]),
            (LaunchHarness::Omp, vec!["omp", "--approval-mode", "write"]),
        ] {
            let compiled = compile(selection(harness)).expect("model omission is valid");
            assert_eq!(compiled.selection.model, None);
            assert_eq!(shell_words::split(&compiled.invocation).unwrap(), expected);
            assert_eq!(compiled.selection.permissions, harness.omitted_permission());
        }
    }

    /// Grok's tracked launch keeps each ownership and resume token explicit;
    /// the permissive choice changes only the deliberate approval flag.
    #[test]
    fn grok_compiles_native_launch_and_resume_template() {
        let fresh = compile(selection(LaunchHarness::Grok)).expect("compile Grok launch");
        assert_eq!(fresh.invocation, "grok --no-leader --permission-mode auto");
        assert_eq!(fresh.agent_kind, AgentKind::Grok);
        assert_eq!(
            fresh.resume_template,
            Some(vec![
                "grok".to_string(),
                "--no-leader".to_string(),
                "--permission-mode".to_string(),
                "auto".to_string(),
                "--resume".to_string(),
                "{conversation}".to_string(),
            ])
        );

        let permissive = compile(LaunchSelection {
            custom_agent: None,
            permission_flags: None,
            harness: LaunchHarness::Grok,
            model: None,
            effort: None,
            permissions: Some(LaunchPermission::Yolo),
            wrapper_prefix: None,
            container: None,
            workspace_trust: None,
        })
        .expect("compile permissive Grok launch");
        assert_eq!(permissive.invocation, "grok --no-leader --always-approve");
        assert_eq!(
            permissive.resume_template,
            Some(vec![
                "grok".to_string(),
                "--no-leader".to_string(),
                "--always-approve".to_string(),
                "--resume".to_string(),
                "{conversation}".to_string(),
            ])
        );
    }

    /// Known Antigravity models constrain effort, while a custom ID has the
    /// full CLI vocabulary. Restart and named starts use the same selector
    /// and keep the literal model and permission arguments.
    #[test]
    fn antigravity_catalog_and_compiler_preserve_model_effort_and_conversation() {
        let catalog = super::launcher_catalog();
        let models: Vec<_> = catalog
            .iter()
            .filter(|row| row.harness == LaunchHarness::Antigravity)
            .collect();
        assert_eq!(models.len(), 4);
        for (id, family, accepts_xhigh) in [
            ("gemini-4-neon", "neon", true),
            ("gemini-4-argon", "argon", false),
            ("gemini-4-hydrogen", "hydrogen", true),
            ("gemini-3.8-flash", "flash", false),
        ] {
            let row = models
                .iter()
                .find(|row| row.id == id)
                .expect("Antigravity model");
            assert!(row.latest);
            assert_eq!(row.family, family);
            let mut choice = selection(LaunchHarness::Antigravity);
            choice.model = Some(id.into());
            choice.effort = Some(LaunchEffort::Xhigh.into());
            assert_eq!(super::compile(choice).is_ok(), accepts_xhigh, "{id}");
        }
        let mut choice = selection(LaunchHarness::Antigravity);
        choice.model = Some("gemini-custom".into());
        choice.effort = Some(LaunchEffort::Xhigh.into());
        choice.permissions = Some(LaunchPermission::Yolo);
        let launch = super::compile(choice).expect("custom Antigravity model");
        let expected = [
            "agy",
            "--model",
            "gemini-custom",
            "--effort",
            "xhigh",
            "--dangerously-skip-permissions",
        ];
        let start = launch.start_argv().expect("fresh Antigravity start");
        assert_eq!(&start[..expected.len()], &expected);
        assert_eq!(&start[expected.len()..], [FARHELM_ARGS_PLACEHOLDER]);
        let named =
            super::named_start(&launch, "explicit-id".into()).expect("named Antigravity start");
        assert_eq!(&named.argv[..expected.len()], &expected);
        assert_eq!(
            &named.argv[expected.len()..],
            ["--conversation", "explicit-id", FARHELM_ARGS_PLACEHOLDER]
        );
    }

    /// The same saved selector feeds Restart and a one-time named start. If it falls after Custom's delimiter,
    /// Antigravity sees it as a positional word and can launch without selecting the intended conversation.
    #[test]
    fn antigravity_custom_delimiter_keeps_both_conversation_selectors_in_options() {
        let mut choice = selection(LaunchHarness::Antigravity);
        choice.permissions = Some(LaunchPermission::Custom);
        choice.permission_flags = Some(
            [
                "--mode",
                "auto",
                "--settings",
                "{}",
                "--",
                "literal operand",
            ]
            .map(String::from)
            .into(),
        );
        let launch = super::compile(choice).unwrap();
        assert_eq!(
            launch.start_argv().unwrap(),
            [
                "agy",
                "--mode",
                "auto",
                "--settings",
                "{}",
                "--",
                "literal operand",
                FARHELM_ARGS_PLACEHOLDER,
            ]
        );
        let expected = [
            "agy",
            "--mode",
            "auto",
            "--settings",
            "{}",
            "--conversation",
            "saved-thread",
            "--",
            "literal operand",
            FARHELM_ARGS_PLACEHOLDER,
        ];
        let integration = farhelm_supervisor::agent_kind::IntegrationSnapshot::of(&launch);
        assert_eq!(
            integration.filled_resume_argv("saved-thread").unwrap(),
            expected
        );
        assert_eq!(
            super::named_start(&launch, "saved-thread".into())
                .unwrap()
                .argv,
            expected
        );
    }

    /// An unverified Grok model remains outside the structured contract.
    #[test]
    fn grok_rejects_unverified_model_choices() {
        let mut choice = selection(LaunchHarness::Grok);
        choice.model = Some("grok-experimental".to_string());
        assert_eq!(
            compile(choice).unwrap_err(),
            "Grok does not currently expose a verified model choice"
        );
    }

    /// Shared model IDs are valid for every OpenRouter owner, while
    /// Goose-only modes remain invalid for every other harness.
    #[test]
    fn shared_models_keep_owner_and_goose_modes_do_not_cross_harnesses() {
        for harness in [LaunchHarness::Goose, LaunchHarness::Pi, LaunchHarness::Omp] {
            assert!(
                compile(LaunchSelection {
                    custom_agent: None,
                    permission_flags: None,
                    harness,
                    model: Some("z-ai/glm-5.3".into()),
                    effort: None,
                    permissions: None,
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                })
                .is_ok()
            );
        }
        assert!(
            compile(LaunchSelection {
                custom_agent: None,
                permission_flags: None,
                harness: LaunchHarness::Pi,
                model: Some("z-ai/glm-5.3".into()),
                effort: None,
                permissions: Some(LaunchPermission::Chat),
                wrapper_prefix: None,
                container: None,
                workspace_trust: None,
            })
            .is_err()
        );
    }

    /// A workspace-trust choice changes only a supported harness's one-run
    /// project-content setting. Pi's mandatory YOLO tool mode remains separate,
    /// and Codex's path waits for the supervisor's resolved launch directory.
    #[test]
    fn workspace_trust_compiles_only_for_supported_harnesses() {
        for (harness, trust, expected_flag) in [
            (LaunchHarness::Claude, Some(true), None),
            (LaunchHarness::Claude, Some(false), None),
            (LaunchHarness::Codex, Some(true), None),
            (LaunchHarness::Codex, Some(false), None),
            (LaunchHarness::Muse, Some(true), Some("--trust-workspace")),
            (LaunchHarness::Muse, Some(false), None),
            (LaunchHarness::Pi, Some(true), Some("--approve")),
            (LaunchHarness::Pi, Some(false), Some("--no-approve")),
        ] {
            let mut choice = selection(harness);
            if harness == LaunchHarness::Pi {
                choice.model = Some("z-ai/glm-5.3".into());
            }
            choice.workspace_trust = trust;
            let compiled = compile(choice).expect("supported trust choice compiles");
            let argv = shell_words::split(&compiled.invocation).unwrap();
            assert_eq!(
                argv.iter()
                    .filter(|word| matches!(
                        word.as_str(),
                        "--trust-workspace" | "--approve" | "--no-approve"
                    ))
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                expected_flag.into_iter().collect::<Vec<_>>()
            );
            assert_eq!(compiled.selection.workspace_trust, trust);
            if harness == LaunchHarness::Claude {
                assert_eq!(
                    compiled.invocation,
                    compile(selection(harness)).unwrap().invocation
                );
            }
            if harness == LaunchHarness::Codex {
                assert_eq!(argv[0], "codex");
                assert_eq!(&argv[1..5], ["-a", "on-request", "-s", "workspace-write"]);
                assert_eq!(argv[5], "-c");
                assert_eq!(
                    argv[6],
                    if trust == Some(true) {
                        farhelm_supervisor::agent_kind::CODEX_TRUSTED_CWD_PLACEHOLDER
                    } else {
                        farhelm_supervisor::agent_kind::CODEX_UNTRUSTED_CWD_PLACEHOLDER
                    }
                );
            }
            assert_eq!(compiled.selection.permissions, harness.omitted_permission());
        }
        for harness in [LaunchHarness::Cursor, LaunchHarness::Goose] {
            let mut choice = selection(harness);
            choice.model = Some("z-ai/glm-5.3".into());
            choice.workspace_trust = Some(true);
            assert_eq!(
                compile(choice).unwrap_err(),
                "workspace trust is not offered by this harness"
            );
        }
    }

    /// Every supported old trust value can be replaced with Settings' result
    /// without changing the compiler's other words or its resume contract.
    /// This also pins the old false forms that retained sessions still use.
    #[test]
    fn stored_trust_replacement_matches_current_composition() {
        for harness in LaunchHarness::BUILT_INS
            .iter()
            .copied()
            .filter(|harness| harness.offers_workspace_trust())
        {
            for previous in [None, Some(false), Some(true)] {
                let mut choice = selection(harness);
                choice.workspace_trust = previous;
                let source = super::compile(choice.clone()).unwrap();
                assert_eq!(source.agent_selection().unwrap().workspace_trust, previous);
                for next in [None, Some(true)] {
                    choice.workspace_trust = next;
                    assert_eq!(
                        super::with_workspace_trust(source.clone(), next).unwrap(),
                        super::compile(choice.clone()).unwrap(),
                        "{harness:?}: {previous:?} to {next:?}",
                    );
                }
            }
        }
    }

    /// Cloning is not permission migration or model validation. A historic
    /// launch may omit today's safety flags and carry words the current
    /// catalog would not compose; only its trust segment may change.
    #[test]
    fn stored_trust_replacement_preserves_historical_nontrust_words() {
        for (harness, program) in [
            (LaunchHarness::Codex, "codex"),
            (LaunchHarness::Muse, "muse"),
            (LaunchHarness::Pi, "pi"),
        ] {
            let mut choice = selection(harness);
            choice.permissions = None;
            choice.model = Some("historical model spelling".into());
            choice.workspace_trust = Some(true);
            let mut prefix = vec![
                program.to_string(),
                "--historic-option".into(),
                "old-value".into(),
            ];
            let nontrust_prefix = prefix.clone();
            prefix.extend(
                super::workspace_trust_arguments(harness, Some(true))
                    .iter()
                    .map(|word| (*word).to_string()),
            );
            let mut start = prefix.clone();
            start.push(FARHELM_ARGS_PLACEHOLDER.into());
            let suffix = vec![
                "historic-resume-selector".to_string(),
                CONVERSATION_PLACEHOLDER.into(),
                FARHELM_ARGS_PLACEHOLDER.into(),
            ];
            let mut resume = prefix;
            resume.extend(suffix.clone());
            let source = SessionLaunch::Agent {
                custom_reporting: None,
                selection: choice,
                start,
                resume: Some(resume),
            };
            assert_eq!(source.agent_selection().unwrap().permissions, None);
            let cleared = super::with_workspace_trust(source, None).unwrap();
            let SessionLaunch::Agent {
                selection,
                start,
                resume,
                ..
            } = cleared
            else {
                panic!("agent")
            };
            assert_eq!(selection.permissions, None);
            assert_eq!(
                selection.model.as_deref(),
                Some("historical model spelling")
            );
            assert_eq!(selection.workspace_trust, None);
            let mut expected_start = nontrust_prefix.clone();
            expected_start.push(FARHELM_ARGS_PLACEHOLDER.into());
            assert_eq!(start, expected_start);
            let mut expected_resume = nontrust_prefix;
            expected_resume.extend(suffix);
            assert_eq!(resume, Some(expected_resume));
        }
    }

    /// The pre-launch-kinds compiler stored a shell-joined start, while the
    /// Codex/Pi integrations copied that original argv and appended their
    /// selector. Conversion adds the final Farhelm markers. These are those
    /// actual generated shapes, including Codex's historic permission omission;
    /// enabling Settings must not strand their otherwise supported clones.
    #[test]
    fn stored_trust_replacement_accepts_pre_launch_kind_agent_conversions() {
        for (harness, kind, model, permissions, base, suffix) in [
            (
                LaunchHarness::Codex,
                AgentKind::Codex,
                "gpt-6-luna",
                None,
                vec![
                    "codex",
                    "-m",
                    "gpt-6-luna",
                    "-c",
                    "model_reasoning_effort=high",
                ],
                vec!["resume", CONVERSATION_PLACEHOLDER],
            ),
            (
                LaunchHarness::Pi,
                AgentKind::Pi,
                "z-ai/glm-5.3",
                Some(LaunchPermission::Yolo),
                vec![
                    "pi",
                    "--provider",
                    "openrouter",
                    "--model",
                    "z-ai/glm-5.3",
                    "--thinking",
                    "high",
                ],
                vec!["--session", CONVERSATION_PLACEHOLDER],
            ),
        ] {
            for previous in [None, Some(false), Some(true)] {
                let mut original: Vec<String> =
                    base.iter().map(|word| (*word).to_string()).collect();
                original.extend(
                    super::workspace_trust_arguments(harness, previous)
                        .iter()
                        .map(|word| (*word).to_string()),
                );
                let mut old_resume = original.clone();
                old_resume.extend(suffix.iter().map(|word| (*word).to_string()));
                let selection = LaunchSelection {
                    custom_agent: None,
                    harness,
                    model: Some(model.into()),
                    effort: Some(LaunchEffort::High.into()),
                    permissions,
                    permission_flags: None,
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: previous,
                };
                let converted = SessionLaunch::from_pre_launch_kinds(
                    &shell_words::join(original),
                    kind,
                    Some(old_resume),
                    Some(selection),
                );
                assert!(
                    matches!(converted, SessionLaunch::Agent { .. }),
                    "premise: the historical structured row remains Agent, not a refused Legacy copy"
                );
                converted.validate_new().unwrap();
                for next in [None, Some(true)] {
                    let normalized = super::with_workspace_trust(converted.clone(), next).unwrap();
                    let SessionLaunch::Agent {
                        selection,
                        start,
                        resume,
                        ..
                    } = normalized
                    else {
                        panic!("agent")
                    };
                    assert_eq!(selection.permissions, permissions);
                    assert_eq!(selection.model.as_deref(), Some(model));
                    assert_eq!(selection.workspace_trust, next);
                    let mut expected: Vec<String> =
                        base.iter().map(|word| (*word).to_string()).collect();
                    expected.extend(
                        super::workspace_trust_arguments(harness, next)
                            .iter()
                            .map(|word| (*word).to_string()),
                    );
                    let mut expected_resume = expected.clone();
                    expected.push(FARHELM_ARGS_PLACEHOLDER.into());
                    expected_resume.extend(suffix.iter().map(|word| (*word).to_string()));
                    expected_resume.push(FARHELM_ARGS_PLACEHOLDER.into());
                    assert_eq!(start, expected, "{harness:?}, {previous:?} to {next:?}");
                    assert_eq!(resume, Some(expected_resume));
                }
            }
        }
    }

    /// Trust-looking custom permission words belong to the user, not this
    /// setting. Matching only the compiler's trailing segment avoids stripping
    /// those words while normalizing the separately recorded trust choice.
    #[test]
    fn stored_trust_replacement_keeps_custom_words_and_command_launches() {
        for harness in [LaunchHarness::Codex, LaunchHarness::Muse] {
            let mut choice = selection(harness);
            choice.permissions = Some(LaunchPermission::Custom);
            choice.permission_flags = Some(vec!["--trust-workspace".into(), "--approve".into()]);
            choice.workspace_trust = Some(true);
            let source = super::compile(choice.clone()).unwrap();
            choice.workspace_trust = None;
            assert_eq!(
                super::with_workspace_trust(source, None).unwrap(),
                super::compile(choice).unwrap(),
            );
        }
        let command = SessionLaunch::plain_command("codex -c '{codex:trusted-cwd}'");
        assert_eq!(
            super::with_workspace_trust(command.clone(), None).unwrap(),
            command
        );
        for harness in LaunchHarness::BUILT_INS
            .iter()
            .copied()
            .filter(|harness| !harness.offers_workspace_trust())
        {
            let source = super::compile(selection(harness)).unwrap();
            assert_eq!(
                super::with_workspace_trust(source.clone(), Some(true)).unwrap(),
                source
            );
        }
    }

    /// A source whose selected trust does not occupy the compiler boundary
    /// cannot be repaired by guessing. Rejecting the edit preserves the
    /// original-source YOLO proof instead of laundering arbitrary argv.
    #[test]
    fn stored_trust_replacement_refuses_unrecognized_boundaries() {
        let mut choice = selection(LaunchHarness::Codex);
        choice.workspace_trust = Some(true);
        let source = super::compile(choice).unwrap();
        let SessionLaunch::Agent { start, .. } = &source else {
            panic!("agent")
        };
        assert_eq!(
            start.last().map(String::as_str),
            Some(FARHELM_ARGS_PLACEHOLDER)
        );
        let mut missing_marker = source.clone();
        let SessionLaunch::Agent { start, .. } = &mut missing_marker else {
            panic!("agent")
        };
        start.pop();
        assert!(super::with_workspace_trust(missing_marker, None).is_err());
        let mut wrong_trust = source.clone();
        let SessionLaunch::Agent { start, .. } = &mut wrong_trust else {
            panic!("agent")
        };
        let index = start.len() - 2;
        start[index] = "unrecognized-trust-word".into();
        assert!(super::with_workspace_trust(wrong_trust, None).is_err());
        let mut wrong_resume = source;
        let SessionLaunch::Agent { resume, .. } = &mut wrong_resume else {
            panic!("agent")
        };
        resume.as_mut().unwrap()[0] = "another-program".into();
        assert!(super::with_workspace_trust(wrong_resume, None).is_err());
    }
}
