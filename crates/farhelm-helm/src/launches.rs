//! Release-owned structured harness catalog and command compiler.
//!
//! The browser sends a [`LaunchSelection`] as intent, never an argv fragment.
//! This module is the single place that turns that intent into the invocation
//! the existing create path accepts. It deliberately does not probe installed
//! binaries or a vendor service: a launch must be reproducible from the
//! released catalog, and a custom model remains a literal one-argument escape
//! hatch for a newer vendor ID.

use farhelm_proto::{
    AgentKind,
    launch::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection},
};
use serde::Serialize;

const MAX_MODEL_ID_BYTES: usize = 256;

/// A catalog entry whose ID has a known owning harness.
///
/// The catalog is intentionally small. It prevents a known Codex model from
/// being submitted as Claude, while an unknown but well-formed ID remains a
/// custom model for the already selected harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct CatalogModel {
    pub(crate) id: &'static str,
    pub(crate) harness: LaunchHarness,
    pub(crate) efforts: &'static [LaunchEffort],
}

/// Farhelm's deliberately compact Codex offering.
///
/// Codex advertises broader effort choices for some models, but this release
/// presents a stable three-level subset for its ordinary released models.
/// Astra is the explicit exception: Farhelm curates it as high-only. These
/// are product choices, not assertions about a vendor rejecting omissions.
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

const CATALOG: &[CatalogModel] = &[
    CatalogModel {
        id: "gpt-5.6-luna",
        harness: LaunchHarness::Codex,
        efforts: CODEX_EFFORTS,
    },
    CatalogModel {
        id: "gpt-5.6-terra",
        harness: LaunchHarness::Codex,
        efforts: CODEX_EFFORTS,
    },
    CatalogModel {
        id: "gpt-5.6-sol",
        harness: LaunchHarness::Codex,
        efforts: CODEX_EFFORTS,
    },
    CatalogModel {
        id: "gpt-6-astra",
        harness: LaunchHarness::Codex,
        efforts: &[LaunchEffort::High],
    },
    CatalogModel {
        id: "claude-fable-5",
        harness: LaunchHarness::Claude,
        efforts: CLAUDE_EFFORTS,
    },
    CatalogModel {
        id: "muse-spark-1.3-contributor",
        harness: LaunchHarness::Muse,
        efforts: MUSE_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3-flash",
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.5",
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.6",
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3",
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-5.6-luna",
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-5.6-terra",
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-5.6-sol",
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6-astra",
        harness: LaunchHarness::Goose,
        efforts: GOOSE_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3-flash",
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.5",
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.6",
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3",
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-5.6-luna",
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-5.6-terra",
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-5.6-sol",
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6-astra",
        harness: LaunchHarness::Pi,
        efforts: PI_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3-flash",
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.5",
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "x-ai/grok-4.6",
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "z-ai/glm-5.3",
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-5.6-luna",
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-5.6-terra",
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-5.6-sol",
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "openai/gpt-6-astra",
        harness: LaunchHarness::Omp,
        efforts: OMP_EFFORTS,
    },
    CatalogModel {
        id: "opencode/glm-5.3-flash",
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/grok-4.5",
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/grok-4.6",
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/glm-5.3",
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/gpt-5.6-luna",
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/gpt-5.6-terra",
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/gpt-5.6-sol",
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
    CatalogModel {
        id: "opencode/gpt-6-astra",
        harness: LaunchHarness::OpenCode,
        efforts: OPENCODE_EFFORTS,
    },
];

/// Return the release-owned known-model catalog served to composer clients.
///
/// The list is static for one helm build, so a borrowed slice avoids a second
/// mutable catalog that could drift from [`compile`]'s compatibility checks.
pub(crate) fn catalog() -> &'static [CatalogModel] {
    CATALOG
}

/// The resolved launch bundle that the existing supervisor create path needs.
///
/// `selection` is retained separately because a compiled command alone loses
/// whether an omitted flag was a harness default or an older catalog's
/// explicit choice. The supervisor will later persist both values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledLaunch {
    pub(crate) invocation: String,
    pub(crate) agent_kind: AgentKind,
    /// A copied structured bundle can preserve a source's durable resume
    /// argv. Fresh catalog compilation leaves this absent and keeps the
    /// supervisor's established integration-derived default.
    pub(crate) resume_template: Option<Vec<String>>,
    pub(crate) selection: LaunchSelection,
}

/// Compile an explicit structured choice into one shell-word-safe invocation.
///
/// The result contains no user-provided shell syntax. A custom model is one
/// argv element after validation, and `shell_words::join` does the only
/// quoting at the boundary where the supervisor later splits the invocation.
pub(crate) fn compile(mut selection: LaunchSelection) -> Result<CompiledLaunch, String> {
    // Pi has no vendor approval mode. Its only offered safety label records
    // that absence, so make an omitted Pi choice durable before history and
    // retry paths receive the compiled selection.
    if selection.harness == LaunchHarness::Pi && selection.permissions.is_none() {
        selection.permissions = Some(LaunchPermission::Yolo);
    }
    validate_selection(&selection)?;

    let mut argv = Vec::new();
    if selection.harness == LaunchHarness::Goose
        && (selection.effort.is_some() || selection.permissions.is_some())
    {
        argv.push("env".to_string());
        if let Some(effort) = selection.effort {
            argv.push(format!("GOOSE_THINKING_EFFORT={}", effort.as_cli_arg()));
        }
        if let Some(permission) = selection.permissions {
            let mode = match permission {
                LaunchPermission::Yolo => "auto",
                LaunchPermission::Approve => "approve",
                LaunchPermission::SmartApprove => "smart_approve",
                LaunchPermission::Chat => "chat",
            };
            argv.push(format!("GOOSE_MODE={mode}"));
        }
    }
    argv.push(program(selection.harness).to_string());
    if selection.harness == LaunchHarness::Goose {
        argv.push("session".to_string());
    }
    if let Some(model) = &selection.model {
        match selection.harness {
            LaunchHarness::Codex => argv.extend(["-m".to_string(), model.clone()]),
            LaunchHarness::Claude | LaunchHarness::Muse => {
                argv.extend(["--model".to_string(), model.clone()])
            }
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
        }
    }
    if let Some(effort) = selection.effort {
        match selection.harness {
            LaunchHarness::Codex => argv.extend([
                "-c".to_string(),
                format!("model_reasoning_effort={}", effort.as_cli_arg()),
            ]),
            LaunchHarness::Claude => {
                argv.extend(["--effort".to_string(), effort.as_cli_arg().to_string()])
            }
            LaunchHarness::Muse => argv.extend([
                "--reasoning-effort".to_string(),
                effort.as_cli_arg().to_string(),
            ]),
            // `validate_selection` has already rejected this combination.
            LaunchHarness::OpenCode => unreachable!("OpenCode has no supported effort"),
            LaunchHarness::Goose => {}
            LaunchHarness::Pi | LaunchHarness::Omp => {
                argv.extend(["--thinking".to_string(), effort.as_cli_arg().to_string()])
            }
        }
    }
    // OMP carries its permission as one explicit approval-mode flag; the
    // harness default adds NOTHING, so an omitted OMP permission stays
    // omitted rather than being rewritten the way Pi's is above.
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
    if selection.permissions == Some(LaunchPermission::Yolo) {
        let flag = match selection.harness {
            LaunchHarness::Codex | LaunchHarness::Muse => Some("--yolo"),
            LaunchHarness::Claude => Some("--dangerously-skip-permissions"),
            LaunchHarness::OpenCode => Some("--auto"),
            // Goose encodes every mode in the environment above; Pi has no
            // approval flag at all; OMP's own approval-mode arm above already
            // wrote its flag.
            LaunchHarness::Goose | LaunchHarness::Pi | LaunchHarness::Omp => None,
        };
        if let Some(flag) = flag {
            argv.push(flag.to_string());
        }
    }

    Ok(CompiledLaunch {
        invocation: shell_words::join(argv),
        agent_kind: match selection.harness {
            LaunchHarness::Codex => AgentKind::Codex,
            LaunchHarness::Claude => AgentKind::Claude,
            LaunchHarness::Muse => AgentKind::Generic,
            LaunchHarness::OpenCode => AgentKind::Generic,
            LaunchHarness::Goose => AgentKind::Goose,
            LaunchHarness::Pi => AgentKind::Pi,
            LaunchHarness::Omp => AgentKind::Omp,
        },
        resume_template: None,
        selection,
    })
}

fn program(harness: LaunchHarness) -> &'static str {
    match harness {
        LaunchHarness::Codex => "codex",
        LaunchHarness::Claude => "claude",
        LaunchHarness::Muse => "muse",
        LaunchHarness::OpenCode => "opencode",
        LaunchHarness::Goose => "goose",
        LaunchHarness::Pi => "pi",
        LaunchHarness::Omp => "omp",
    }
}

fn validate_selection(selection: &LaunchSelection) -> Result<(), String> {
    if selection.model.is_none() {
        let message = match selection.harness {
            LaunchHarness::OpenCode => Some("choose an OpenCode model before launching"),
            LaunchHarness::Goose => Some("choose a Goose model before launching"),
            LaunchHarness::Pi => Some("choose a Pi model before launching"),
            LaunchHarness::Omp => Some("choose an OMP model before launching"),
            LaunchHarness::Codex | LaunchHarness::Claude | LaunchHarness::Muse => None,
        };
        if let Some(message) = message {
            return Err(message.to_string());
        }
    }
    if let Some(model) = &selection.model {
        validate_model_id(model)?;
        if selection.harness == LaunchHarness::OpenCode {
            opencode_model_argument(model)?;
        }
        let known = CATALOG
            .iter()
            .filter(|entry| entry.id == model)
            .collect::<Vec<_>>();
        if !known.is_empty() {
            if !known.iter().any(|entry| entry.harness == selection.harness) {
                return Err(format!(
                    "model {model:?} belongs to a different harness than {:?}",
                    selection.harness
                ));
            }
            let known = known
                .into_iter()
                .find(|entry| entry.harness == selection.harness)
                .expect("owner checked");
            if let Some(effort) = selection.effort
                && !known.efforts.contains(&effort)
            {
                return Err(format!(
                    "Farhelm's released offering for model {model:?} does not include effort {:?}",
                    effort
                ));
            }
        }
    }
    if let Some(effort) = selection.effort
        && !harness_efforts(selection.harness).contains(&effort)
    {
        return Err(format!(
            "Farhelm's released {:?} offering does not include effort {:?}",
            selection.harness, effort
        ));
    }
    match (selection.harness, selection.permissions) {
        (LaunchHarness::Goose, _)
        | (LaunchHarness::Pi, Some(LaunchPermission::Yolo))
        // OMP offers the harness default (no flag), Approve, and YOLO; its
        // `write` mode is not a Farhelm choice and the Goose-only labels do
        // not cross harnesses.
        | (LaunchHarness::Omp, None)
        | (LaunchHarness::Omp, Some(LaunchPermission::Yolo))
        | (LaunchHarness::Omp, Some(LaunchPermission::Approve))
        | (_, None)
        | (_, Some(LaunchPermission::Yolo)) => {}
        (_, Some(permission)) => {
            return Err(format!("the {permission:?} permission is not offered by this harness"));
        }
    }
    Ok(())
}

/// Return OpenCode's verified provider-qualified argument without changing
/// the saved selection. Bare values are Zen model names; a slash therefore
/// names a provider and must be OpenCode itself.
fn opencode_model_argument(model: &str) -> Result<String, String> {
    match model.split_once('/') {
        None => Ok(format!("opencode/{model}")),
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
    if model == "{cwd}" || model == "{conversation}" {
        return Err("model id cannot be a reserved launch placeholder".to_string());
    }
    Ok(())
}

fn harness_efforts(harness: LaunchHarness) -> &'static [LaunchEffort] {
    match harness {
        LaunchHarness::Codex => CODEX_EFFORTS,
        LaunchHarness::Claude => CLAUDE_EFFORTS,
        LaunchHarness::Muse => MUSE_EFFORTS,
        LaunchHarness::OpenCode => OPENCODE_EFFORTS,
        LaunchHarness::Goose => GOOSE_EFFORTS,
        LaunchHarness::Pi => PI_EFFORTS,
        LaunchHarness::Omp => OMP_EFFORTS,
    }
}

#[cfg(test)]
mod tests {
    use super::compile;
    use farhelm_proto::{
        AgentKind,
        launch::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection},
    };

    fn selection(harness: LaunchHarness) -> LaunchSelection {
        LaunchSelection {
            harness,
            model: None,
            effort: None,
            permissions: None,
        }
    }

    /// Omitted choices must omit flags, because “default” is the vendor's
    /// decision and cannot be represented by a guessed model or mode.
    #[test]
    fn defaults_compile_to_the_harness_program_only() {
        assert_eq!(
            compile(selection(LaunchHarness::Codex)).unwrap().invocation,
            "codex"
        );
        assert_eq!(
            compile(selection(LaunchHarness::Claude))
                .unwrap()
                .invocation,
            "claude"
        );
        assert_eq!(
            compile(selection(LaunchHarness::Muse)).unwrap().invocation,
            "muse"
        );
    }

    /// Each harness has a different CLI spelling even though the composer
    /// presents one consistent model/effort/permission vocabulary.
    #[test]
    fn explicit_choices_use_each_harnesses_documented_flags() {
        let codex = LaunchSelection {
            harness: LaunchHarness::Codex,
            model: Some("gpt-5.6-terra".to_string()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::Yolo),
        };
        assert_eq!(
            shell_words::split(&compile(codex).unwrap().invocation).unwrap(),
            [
                "codex",
                "-m",
                "gpt-5.6-terra",
                "-c",
                "model_reasoning_effort=high",
                "--yolo"
            ]
        );

        let claude = LaunchSelection {
            harness: LaunchHarness::Claude,
            model: Some("claude-fable-5".to_string()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::Yolo),
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
            harness: LaunchHarness::Muse,
            model: Some("muse-spark-1.3-contributor".to_string()),
            effort: Some(LaunchEffort::Xhigh),
            permissions: Some(LaunchPermission::Yolo),
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
            harness: LaunchHarness::OpenCode,
            model: Some("opencode/grok-4.6".to_string()),
            effort: None,
            permissions: Some(LaunchPermission::Yolo),
        };
        let compiled = compile(opencode).expect("OpenCode's documented flags compile");
        assert_eq!(compiled.agent_kind, AgentKind::Generic);
        assert_eq!(
            shell_words::split(&compiled.invocation).unwrap(),
            ["opencode", "--model", "opencode/grok-4.6", "--auto"]
        );
    }

    /// OpenCode launches must name Zen explicitly. This prevents a changed
    /// local OpenCode configuration from silently choosing a model Farhelm's
    /// durable selection never recorded.
    #[test]
    fn opencode_requires_a_zen_model_and_normalizes_only_its_argv() {
        assert!(
            compile(selection(LaunchHarness::OpenCode))
                .unwrap_err()
                .contains("choose an OpenCode model")
        );

        let bare = LaunchSelection {
            harness: LaunchHarness::OpenCode,
            model: Some("private-zen-model".to_string()),
            effort: None,
            permissions: None,
        };
        let compiled = compile(bare.clone()).expect("bare Zen model");
        assert_eq!(
            compiled.selection, bare,
            "the selection records typed intent"
        );
        assert_eq!(
            shell_words::split(&compiled.invocation).unwrap(),
            ["opencode", "--model", "opencode/private-zen-model"]
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

    /// The OpenCode CLI's `--auto` option is a permission policy, not an
    /// effort mechanism; a stale structured selection must not invent one.
    #[test]
    fn opencode_refuses_an_unsupported_effort() {
        let selection = LaunchSelection {
            harness: LaunchHarness::OpenCode,
            model: Some("opencode/glm-5.3".to_string()),
            effort: Some(LaunchEffort::High),
            permissions: None,
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
        let offered: Vec<_> = super::catalog()
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
                ["opencode", "--model", row.id]
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
            assert_eq!(compiled.selection, input);
            assert_eq!(
                shell_words::split(&compiled.invocation).unwrap(),
                ["opencode", "--model", "opencode/custom'42;$literal"]
            );
        }
    }

    /// A custom model remains one literal argv element even when it needs
    /// shell quoting; accepting it must not turn it into two user arguments.
    #[test]
    fn a_custom_model_id_is_quoted_as_one_argv_element() {
        let mut input = selection(LaunchHarness::Muse);
        input.model = Some("release/candidate'42;$literal".to_string());
        input.effort = Some(LaunchEffort::Xhigh);
        let compiled = compile(input).unwrap();
        assert_eq!(compiled.agent_kind, AgentKind::Generic);
        assert_eq!(
            shell_words::split(&compiled.invocation).unwrap(),
            [
                "muse",
                "--model",
                "release/candidate'42;$literal",
                "--reasoning-effort",
                "xhigh"
            ]
        );
    }

    /// The known catalog protects cross-harness choices and its curated
    /// offerings, while malformed custom IDs cannot smuggle options or
    /// placeholder substitution through.
    #[test]
    fn incompatible_and_malformed_models_are_refused() {
        let mut wrong_harness = selection(LaunchHarness::Claude);
        wrong_harness.model = Some("gpt-5.6-terra".to_string());
        assert!(compile(wrong_harness).unwrap_err().contains("belongs"));

        for model in ["", "--model", "two words", "{cwd}", "line\nbreak"] {
            let mut malformed = selection(LaunchHarness::Codex);
            malformed.model = Some(model.to_string());
            assert!(
                compile(malformed).is_err(),
                "model {model:?} must be refused"
            );
        }
    }

    /// Muse Contributor 1.3 deliberately stops at xhigh in Farhelm's
    /// released catalog. This is a model-specific evidence boundary: the
    /// broader Muse CLI vocabulary and a different non-contributor row do
    /// not establish max or ultra for this contributor offering.
    #[test]
    fn muse_contributor_catalog_stops_at_xhigh() {
        for effort in [LaunchEffort::Max, LaunchEffort::Ultra] {
            let selection = LaunchSelection {
                harness: LaunchHarness::Muse,
                model: Some("muse-spark-1.3-contributor".to_string()),
                effort: Some(effort),
                permissions: None,
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
            harness: LaunchHarness::Goose,
            model: Some("x-ai/grok-4.6".into()),
            effort: Some(LaunchEffort::Max),
            permissions: Some(LaunchPermission::SmartApprove),
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
            harness: LaunchHarness::Pi,
            model: Some("x-ai/grok-4.6".into()),
            effort: Some(LaunchEffort::Minimal),
            permissions: None,
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
    /// an omitted mode and effort must leave the process environment alone.
    /// This pins the full mapping rather than one representative mode.
    #[test]
    fn goose_maps_every_permission_without_unsolicited_overrides() {
        let base = LaunchSelection {
            harness: LaunchHarness::Goose,
            model: Some("z-ai/glm-5.3".into()),
            effort: None,
            permissions: None,
        };
        assert_eq!(
            shell_words::split(&compile(base.clone()).unwrap().invocation).unwrap(),
            [
                "goose",
                "session",
                "--provider",
                "openrouter",
                "--model",
                "z-ai/glm-5.3"
            ],
            "omitted choices must not write Goose environment overrides"
        );
        for (permission, mode) in [
            (LaunchPermission::Yolo, "auto"),
            (LaunchPermission::Approve, "approve"),
            (LaunchPermission::SmartApprove, "smart_approve"),
            (LaunchPermission::Chat, "chat"),
        ] {
            let compiled = compile(LaunchSelection {
                permissions: Some(permission),
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
            harness: LaunchHarness::Pi,
            model: Some("z-ai/glm-5.3".into()),
            effort: None,
            permissions: None,
        };
        let omitted = shell_words::split(&compile(base.clone()).unwrap().invocation).unwrap();
        let yolo = shell_words::split(
            &compile(LaunchSelection {
                permissions: Some(LaunchPermission::Yolo),
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
                    ..base.clone()
                })
                .is_err(),
                "Pi must reject {permission:?}"
            );
        }
    }

    /// OMP compiles its own OpenRouter contract beside Pi's: the same
    /// provider-qualified model form, its own `--thinking` effort flag, and
    /// its own `--approval-mode` pair — where an OMITTED permission stays
    /// omitted (the harness default is a real choice, unlike Pi's rewritten
    /// yolo) and both explicit modes carry their exact value.
    #[test]
    fn omp_compiles_its_native_openrouter_contract_with_an_explicit_approval_mode() {
        // Every optional flag at once, with the OMP-specific Approve mode.
        let full = compile(LaunchSelection {
            harness: LaunchHarness::Omp,
            model: Some("x-ai/grok-4.6".into()),
            effort: Some(LaunchEffort::Minimal),
            permissions: Some(LaunchPermission::Approve),
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
            harness: LaunchHarness::Omp,
            model: Some("z-ai/glm-5.3".into()),
            effort: Some(LaunchEffort::Xhigh),
            permissions: Some(LaunchPermission::Yolo),
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

        // The harness default is real: an omitted permission adds NO flag —
        // the selection keeps recording the omission, never a rewrite.
        let omitted = compile(LaunchSelection {
            harness: LaunchHarness::Omp,
            model: Some("z-ai/glm-5.3".into()),
            effort: None,
            permissions: None,
        })
        .expect("OMP default compiles");
        assert_eq!(
            omitted.selection.permissions, None,
            "an omitted OMP permission must stay omitted, unlike Pi's rewrite"
        );
        assert_eq!(
            shell_words::split(&omitted.invocation).unwrap(),
            ["omp", "--provider", "openrouter", "--model", "z-ai/glm-5.3"]
        );
    }

    /// OMP's effort list stops at max (the enum's `ultra` is not offered),
    /// a model is required, and Goose-only permission labels are refused.
    #[test]
    fn omp_vocabulary_is_closed_and_model_required() {
        for row in super::catalog()
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
            harness: LaunchHarness::Omp,
            model: Some("release/candidate'42;$literal".to_string()),
            effort: None,
            permissions: None,
        })
        .expect("a custom OMP id is the escape hatch");
        assert_eq!(
            shell_words::split(&custom.invocation).unwrap(),
            [
                "omp",
                "--provider",
                "openrouter",
                "--model",
                "release/candidate'42;$literal"
            ]
        );

        assert_eq!(
            compile(selection(LaunchHarness::Omp)).unwrap_err(),
            "choose an OMP model before launching"
        );
        // OMP's own effort vocabulary is OMP_EFFORTS: `ultra` (the shared
        // enum's extra level) is refused; every listed level compiles.
        for effort in super::OMP_EFFORTS {
            assert!(
                compile(LaunchSelection {
                    harness: LaunchHarness::Omp,
                    model: Some("z-ai/glm-5.3".into()),
                    effort: Some(*effort),
                    permissions: None,
                })
                .is_ok(),
                "OMP's catalog offers {effort:?}"
            );
        }
        assert!(
            compile(LaunchSelection {
                harness: LaunchHarness::Omp,
                model: Some("z-ai/glm-5.3".into()),
                effort: Some(LaunchEffort::Ultra),
                permissions: None,
            })
            .is_err(),
            "ultra is not in OMP's released offering"
        );
        // OMP offers no permission beyond Approve and YOLO: the Goose-only
        // labels refuse exactly as they do for Pi.
        for permission in [LaunchPermission::SmartApprove, LaunchPermission::Chat] {
            assert!(
                compile(LaunchSelection {
                    harness: LaunchHarness::Omp,
                    model: Some("z-ai/glm-5.3".into()),
                    effort: None,
                    permissions: Some(permission),
                })
                .is_err(),
                "OMP must reject {permission:?}"
            );
        }
    }

    /// Every explicit-model harness names itself in the refusal. OpenCode's
    /// established wording remains stable while Goose and Pi avoid claiming
    /// the wrong catalog.
    #[test]
    fn missing_model_errors_name_the_selected_harness() {
        for (harness, expected) in [
            (
                LaunchHarness::OpenCode,
                "choose an OpenCode model before launching",
            ),
            (
                LaunchHarness::Goose,
                "choose a Goose model before launching",
            ),
            (LaunchHarness::Pi, "choose a Pi model before launching"),
            (LaunchHarness::Omp, "choose an OMP model before launching"),
        ] {
            assert_eq!(compile(selection(harness)).unwrap_err(), expected);
        }
    }

    /// Shared model IDs are valid for every OpenRouter owner, while
    /// Goose-only modes remain invalid for every other harness.
    #[test]
    fn shared_models_keep_owner_and_goose_modes_do_not_cross_harnesses() {
        for harness in [LaunchHarness::Goose, LaunchHarness::Pi, LaunchHarness::Omp] {
            assert!(
                compile(LaunchSelection {
                    harness,
                    model: Some("z-ai/glm-5.3".into()),
                    effort: None,
                    permissions: None,
                })
                .is_ok()
            );
        }
        assert!(
            compile(LaunchSelection {
                harness: LaunchHarness::Pi,
                model: Some("z-ai/glm-5.3".into()),
                effort: None,
                permissions: Some(LaunchPermission::Chat),
            })
            .is_err()
        );
    }
}
