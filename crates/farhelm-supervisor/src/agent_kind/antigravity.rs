//! Antigravity's conversation hook plugin and first-record verification.

use super::{RECORD_PREFIX_BYTES, read_record_header};
use anyhow::Context as _;
use serde::Deserialize;
use std::path::Path;

/// Antigravity loads Farhelm's named hook group from a discovered plugin. Keeping both files under the plugin's own
/// directory lets older manual hooks and other named groups stay in the user's shared configuration.
pub(super) async fn persistent_hook_files(
    home: &Path,
    exe: &str,
) -> anyhow::Result<[super::persistent_hooks::HookFile; 2]> {
    use super::{AgentInstructions, hook_command, persistent_hooks::HookFile};
    use farhelm_proto::ReportVendor;

    let directory = home.join(".gemini/config/plugins/farhelm");
    HookFile::check_path(&directory)?;
    // Consent covers Farhelm's plugin directory. A symlink here can redirect hooks.json onto the shared hooks file.
    match tokio::fs::symlink_metadata(&directory).await {
        Ok(metadata) if !metadata.is_dir() => {
            anyhow::bail!(
                "{} is not a dedicated plugin directory",
                directory.display()
            );
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("checking {}", directory.display()));
        }
    }
    let command = hook_command(exe, AgentInstructions::Off, ReportVendor::Antigravity);
    let callback = serde_json::json!([{ "type": "command", "command": command, "timeout": 60 }]);
    Ok([
        HookFile::json(
            directory.join("plugin.json"),
            serde_json::json!({
                "name": "farhelm",
                "displayName": "Farhelm",
                "version": "1.0.0",
                "description": "Reports conversations to Farhelm for Restart",
            }),
        )?,
        HookFile::json(
            directory.join("hooks.json"),
            serde_json::json!({
                "farhelm": { "SessionStart": callback, "Stop": callback },
            }),
        )?,
    ])
}

/// Verify the first record of the exact transcript named by the report.
///
/// In CLI 1.5.13 both conversations report from the same process. A main
/// conversation starts with explicit user input. A subagent's transcript may
/// be absent at SessionStart and starts with a system message once present.
/// A later Stop can supply proof if the first report arrived before the file.
pub(crate) async fn verifies_main_conversation(transcript: Option<&serde_json::Value>) -> bool {
    #[derive(Deserialize)]
    struct FirstRecord<'a> {
        source: &'a str,
        #[serde(rename = "type")]
        kind: &'a str,
    }

    let Some(path) = transcript
        .and_then(serde_json::Value::as_str)
        .map(Path::new)
        .filter(|path| path.is_absolute())
    else {
        return false;
    };
    let Ok(Some(line)) = read_record_header(path).await else {
        return false;
    };
    // A valid JSON prefix followed by padding can also hit the read cap.
    // Require the record boundary when the reader filled its budget.
    if line.len() >= RECORD_PREFIX_BYTES && !line.ends_with('\n') {
        return false;
    }
    // Serde also decodes structs from positional arrays. The vendor writes
    // named fields, so a sequence supplies no evidence of either field.
    if !line.trim_start().starts_with('{') {
        return false;
    }
    serde_json::from_str::<FirstRecord<'_>>(&line)
        .is_ok_and(|record| record.source == "USER_EXPLICIT" && record.kind == "USER_INPUT")
}
