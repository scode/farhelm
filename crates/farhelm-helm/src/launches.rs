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
        }
    }
    if selection.permissions == Some(LaunchPermission::Yolo) {
        argv.push(
            match selection.harness {
                LaunchHarness::Codex | LaunchHarness::Muse => "--yolo",
                LaunchHarness::Claude => "--dangerously-skip-permissions",
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
    }
}

fn validate_selection(selection: &LaunchSelection) -> Result<(), String> {
    if let Some(model) = &selection.model {
        validate_model_id(model)?;
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
