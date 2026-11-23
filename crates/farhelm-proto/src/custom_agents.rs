//! A helm-owned catalog entry for an executable the user integrates with Farhelm.
//!
//! The launch identity stays stable when the display name or command changes.
//! Existing sessions keep their compiled command and reporting capability.
//! Changes to the wrapper contract must update the contract changelog in
//! `website/src/content/docs/docs/agents/custom-agent-contract.md`.

use serde::{Deserialize, Serialize, de::value};

use crate::{LaunchHarness, session_launch::is_reserved_placeholder, text::is_presentation_unsafe};

/// The user declares capabilities without Farhelm probing the program on any host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomAgent {
    pub id: String,
    pub display_name: String,
    pub command: String,
    pub supports_resumption: bool,
    pub supports_reporting: bool,
}

impl CustomAgent {
    /// Validate a catalog write before its command or display name reaches another surface.
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_id(&self.id)?;
        if self.display_name.trim().is_empty()
            || self.display_name.chars().any(is_presentation_unsafe)
        {
            return Err("enter a display name without control or invisible formatting characters");
        }
        validate_command(&self.command)
    }
}

/// Keep IDs usable as URL segments and unambiguous in the CLI harness selector.
pub fn validate_id(id: &str) -> Result<(), &'static str> {
    let alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    if id.len() > 32
        || !id.bytes().next().is_some_and(alnum)
        || !id.bytes().last().is_some_and(alnum)
        || !id.bytes().all(|b| alnum(b) || b == b'-')
    {
        return Err("use up to 32 lowercase letters or digits, with hyphens only within the ID");
    }
    if matches!(
        id,
        "command" | "custom" | "default" | "generic" | "none" | "shell"
    ) || LaunchHarness::deserialize(value::StrDeserializer::<value::Error>::new(id)).is_ok()
    {
        return Err("this ID is reserved by Farhelm");
    }
    Ok(())
}

/// A catalog command names one program. Wrapper prefixes and full command lines have their own parsing paths.
fn validate_command(command: &str) -> Result<(), &'static str> {
    let executable_name = !command.contains('/') && !command.starts_with('~');
    if command.is_empty()
        || command.starts_with('-')
        || command.ends_with('/')
        || !(executable_name || command.starts_with('/') || command.starts_with("~/"))
        || command
            .chars()
            .any(|ch| ch.is_whitespace() || is_presentation_unsafe(ch))
        || is_reserved_placeholder(command)
    {
        return Err("use a program name on PATH, an absolute path, or a path beginning with ~/");
    }
    Ok(())
}

/// Model and effort are literal arguments in the wrapper contract.
pub fn validate_model_or_effort(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
    {
        return Err("use 1 to 256 lowercase letters, digits, hyphens, or underscores");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT: &str =
        "website/src/content/docs/docs/agents/custom-agent-contract.md, Contract changelog";

    /// A catalog ID can be used unchanged in a URL or CLI flag after a rename of its display name.
    #[farhelm_testtrace::test]
    fn custom_ids_exclude_existing_cli_choices() {
        for id in ["acme-code", "1", &"z".repeat(32)] {
            assert!(validate_id(id).is_ok(), "{id}: {CONTRACT}");
        }
        for id in [
            "",
            "ACME",
            "acme_code",
            "-acme",
            "acme-",
            "a/b",
            "codex",
            "bash",
            "custom",
            "default",
            &"z".repeat(33),
        ] {
            assert!(validate_id(id).is_err(), "{id}: {CONTRACT}");
        }
    }

    /// Preserve the separation between literal executable names and the existing shell-command surface.
    #[farhelm_testtrace::test]
    fn catalog_commands_name_one_executable() {
        for path in ["acme-code", "/opt/acme/bin/agent", "~/bin/acme-code"] {
            assert!(validate_command(path).is_ok(), "{path}: {CONTRACT}");
        }
        for path in [
            "",
            "-help",
            "./agent",
            "bin/agent",
            "~other/bin/agent",
            "~/",
            "acme code",
            "{conversation}",
            "{farhelm_args}",
            "a\0b",
        ] {
            assert!(validate_command(path).is_err(), "{path}: {CONTRACT}");
        }
    }

    /// Custom values need no catalog entry, but the wrapper receives only values within the documented grammar.
    #[farhelm_testtrace::test]
    fn custom_model_and_effort_are_bounded_literals() {
        for valid in ["fast-v2", "deep_reasoning", &"a".repeat(256)] {
            assert!(
                validate_model_or_effort(valid).is_ok(),
                "{valid}: {CONTRACT}"
            );
        }
        for invalid in [
            "",
            "FAST",
            "fast effort",
            "effort.medium",
            "é",
            &"a".repeat(257),
        ] {
            assert!(
                validate_model_or_effort(invalid).is_err(),
                "{invalid}: {CONTRACT}"
            );
        }
    }
}
