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
pub(crate) fn compile(selection: LaunchSelection) -> Result<CompiledLaunch, String> {
    validate_selection(&selection)?;

    let mut argv = vec![program(selection.harness).to_string()];
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
        }
    }
    if selection.permissions == Some(LaunchPermission::Yolo) {
        argv.push(
            match selection.harness {
                LaunchHarness::Codex | LaunchHarness::Muse => "--yolo",
                LaunchHarness::Claude => "--dangerously-skip-permissions",
                LaunchHarness::OpenCode => "--auto",
            }
            .to_string(),
        );
    }

    Ok(CompiledLaunch {
        invocation: shell_words::join(argv),
        agent_kind: match selection.harness {
            LaunchHarness::Codex => AgentKind::Codex,
            LaunchHarness::Claude => AgentKind::Claude,
            LaunchHarness::Muse => AgentKind::Generic,
            LaunchHarness::OpenCode => AgentKind::Generic,
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
    }
}

fn validate_selection(selection: &LaunchSelection) -> Result<(), String> {
    if selection.harness == LaunchHarness::OpenCode && selection.model.is_none() {
        return Err("choose an OpenCode model before launching".to_string());
    }
    if let Some(model) = &selection.model {
        validate_model_id(model)?;
        if selection.harness == LaunchHarness::OpenCode {
            opencode_model_argument(model)?;
        }
        if let Some(known) = CATALOG.iter().find(|entry| entry.id == model) {
            if known.harness != selection.harness {
                return Err(format!(
                    "model {model:?} belongs to {:?}, not {:?}",
                    known.harness, selection.harness
                ));
            }
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

    /// The four suggestions are the release offering, not a default model or
    /// a provider discovery result. Each must compile without effort or an
    /// implicit permission override.
    #[test]
    fn opencode_catalog_contains_exactly_the_four_zen_suggestions() {
        let ids = [
            "opencode/glm-5.3-flash",
            "opencode/grok-4.5",
            "opencode/grok-4.6",
            "opencode/glm-5.3",
        ];
        let offered: Vec<_> = super::catalog()
            .iter()
            .filter(|row| row.harness == LaunchHarness::OpenCode)
            .collect();
        assert_eq!(offered.iter().map(|row| row.id).collect::<Vec<_>>(), ids);
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
}
