//! Grok uses positional attribution and requires the emitter's private backend option.

use super::*;
use positional::PositionalAttribution;

/// Attribute the hook to the process that ran it and confirm that process uses an owned backend.
///
/// Grok installs can use a versioned executable name. The shared corridor finds the emitter
/// without an image rule. Grok still requires `--no-leader` on that exact process because a
/// shared backend can carry another session's conversation.
pub(crate) fn grok_corridor(chain: &[ChainLink]) -> Result<PositionalAttribution, String> {
    let attribution = positional_corridor(chain, PositionalReporter::BuiltIn)?;
    let argv = chain[attribution.emitter_index]
        .argv
        .as_deref()
        .ok_or_else(|| "the Grok emitter's command line is unavailable".to_string())?;
    if !grok_has_owned_backend(argv) {
        return Err(
            "the Grok emitter was not launched with --no-leader before its option boundary"
                .to_string(),
        );
    }
    Ok(attribution)
}

/// Only a standalone option before `--` selects Grok's owned backend.
fn grok_has_owned_backend(argv: &[Vec<u8>]) -> bool {
    argv.iter()
        .take_while(|argument| argument.as_slice() != b"--")
        .skip(1)
        .any(|argument| argument.as_slice() == b"--no-leader")
}
