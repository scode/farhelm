//! Claude Code's launch-time hook refusal.
//!
//! Claude's hook argv itself (`--settings` carrying a `SessionStart` hook)
//! is built by its integration in the parent module; this file holds the
//! Claude-specific reason not to append it.

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
