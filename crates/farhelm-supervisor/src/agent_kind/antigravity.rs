//! First-record verification for Antigravity's manual conversation hooks.

use super::{RECORD_PREFIX_BYTES, read_record_header};
use serde::Deserialize;
use std::path::Path;

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
