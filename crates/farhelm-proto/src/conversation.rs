//! Literal conversation selectors cross the same argument boundary whether they come from a launch request or a
//! captured report. The desktop uses these checks to show the helm's validation beside the field before submission.

use crate::{LaunchHarness, session_launch::is_reserved_placeholder};

/// Maximum byte length of a bare conversation selector. Encoded locators have their own bounds in the supervisor.
pub const MAX_CONVERSATION_ID_LEN: usize = 128;

/// Prefix on stored Pi locators.
pub const PI_LOCATOR_PREFIX: &str = "pi:";
/// Prefix on stored OMP locators.
pub const OMP_LOCATOR_PREFIX: &str = "omp:";
/// Prefix on stored Codex locators.
pub const CODEX_LOCATOR_PREFIX: &str = "codex:";
/// Prefix on stored Grok locators.
pub const GROK_LOCATOR_PREFIX: &str = "grok:";

/// Whether a token claims Farhelm's encoded locator format. Even a malformed locator keeps its prefix reserved.
pub fn is_reserved_locator_token(value: &str) -> bool {
    [
        PI_LOCATOR_PREFIX,
        OMP_LOCATOR_PREFIX,
        CODEX_LOCATOR_PREFIX,
        GROK_LOCATOR_PREFIX,
    ]
    .into_iter()
    .any(|prefix| value.starts_with(prefix))
}

/// Whether a bare id can occupy one literal resume argument. A leading dash would introduce an option. A Farhelm
/// placeholder would be read again by a later substitution pass. The check permits new vendor id formats within
/// these bounds without attempting to verify a conversation exists.
pub fn is_plausible_conversation_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_CONVERSATION_ID_LEN
        && !id.starts_with('-')
        && !is_reserved_placeholder(id)
        && id
            .chars()
            .all(|c| c.is_ascii_graphic() && c != '"' && c != '\'' && c != '\\')
}

/// Validate the one-time selector before putting it in argv. A named start takes a bare id or session-file path.
/// Encoded locators belong to Farhelm's captured restart flow.
pub fn validate_named_conversation(value: &str) -> Result<(), String> {
    if !is_plausible_conversation_id(value) || is_reserved_locator_token(value) {
        return Err("conversation id or session file must be a nonempty literal without spaces, quotes, control characters, a leading dash or Farhelm placeholders".into());
    }
    Ok(())
}

/// Check a launcher selector with its harness's path rules. File selectors go to the harness as literal paths
/// relative to the session folder or as absolute paths, so a leading tilde cannot designate the user's home.
pub fn validate_launch_conversation(harness: LaunchHarness, value: &str) -> Result<(), String> {
    validate_named_conversation(value)?;
    if harness.named_conversation_is_file() && value.starts_with('~') {
        return Err("session file paths do not expand ~; use an absolute or relative path".into());
    }
    Ok(())
}
