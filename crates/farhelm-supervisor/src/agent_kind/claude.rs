//! Claude Code's launch-time hook refusal and its stored conversation value.
//!
//! Claude's hook argv itself (`--settings` carrying a `SessionStart` hook)
//! is built by its integration in the parent module; this file holds the
//! Claude-specific reason not to append it, and how a reported conversation
//! is stored and read back. Trust writes to Claude's settings file live in
//! `trust`, separate from command-line hook integration.
//!
//! ## The stored conversation
//!
//! Claude's hook reports the session id as soon as Claude starts, but Claude
//! writes the conversation's transcript only once the first prompt is
//! submitted, so `claude --resume <id>` fails for a session that never got a
//! prompt. Restart therefore needs the transcript file Claude named, not
//! just the id. The hook already forwards Claude's own `transcript_path`, and
//! admission stores it beside the id as a `claude:` locator
//! ([`stored_conversation`]). That is verification, not identification: the
//! path comes from Claude's report, never from Claude's directory layout.
//! A capture from an older build is a bare id, and stays resumable as one.

pub(crate) mod trust;

/// Why Claude's hook tail must not be appended to this invocation, if it
/// must not.
///
/// Plan D3: Claude Code honours only the LAST `--settings`, so a second
/// one appended after the user's would silently discard theirs, turning an
/// identity improvement into a lost configuration. Only Claude's injection
/// uses that flag, so only Claude has to yield here; a Codex invocation
/// carrying `--settings` means something else entirely (or nothing) and is
/// left alone. Both spellings count, since they are one flag to the vendor.
pub(crate) fn hook_refusal(argv: &[String]) -> Option<&'static str> {
    argv.iter()
        .any(|element| element == "--settings" || element.starts_with("--settings="))
        .then_some("invocation already passes --settings")
}

/// The value Farhelm stores for a Claude report of `session_id`, which admission has already checked is a plain,
/// plausible id: a `claude:` locator carrying the `transcript_path` Claude reported with it, or the bare id when
/// the report named no usable path (absent, not a string, relative, too long, or holding control characters).
///
/// Never fails: the file only improves Restart (it tells Restart whether Claude saved anything yet), so a report
/// whose path cannot be stored must still capture its id.
pub(crate) fn stored_conversation(
    session_id: &str,
    transcript_path: Option<&serde_json::Value>,
) -> String {
    let Some(path) = transcript_path.and_then(serde_json::Value::as_str) else {
        return session_id.to_owned();
    };
    let locator = super::SessionLocator {
        version: 1,
        session_id: session_id.to_owned(),
        session_file: Some(path.to_owned()),
    };
    super::encode_locator(super::LocatorVendor::Claude, locator)
        .unwrap_or_else(|_| session_id.to_owned())
}

/// The session id a stored Claude value resumes: a `claude:` locator's id, or a bare id from an older build (or from
/// a report that named no usable path). `None` for anything else, a malformed `claude:` token included, so no
/// reserved or option-shaped value ever becomes a resume argument.
pub(crate) fn session_id(stored: &str) -> Option<String> {
    if let Ok(locator) = super::parse_locator(super::LocatorVendor::Claude, stored) {
        return (!super::is_reserved_locator_token(&locator.session_id))
            .then_some(locator.session_id);
    }
    (!super::is_reserved_locator_token(stored) && super::is_plausible_conversation_id(stored))
        .then(|| stored.to_owned())
}

/// The transcript file a stored Claude value names, if it is a `claude:` locator carrying one. A bare id (an older
/// build's capture, or a report that named no usable path) has none, and Restart resumes it unchecked.
pub(crate) fn transcript(stored: &str) -> Option<std::path::PathBuf> {
    super::parse_locator(super::LocatorVendor::Claude, stored)
        .ok()?
        .session_file
        .map(std::path::PathBuf::from)
}
