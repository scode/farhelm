//! The launcher's own words for agent choices: the order its harness chips come in, the permission docs link, Guarded's
//! help before an agent is chosen, and the notes an agent type's choices carry. The names and help for permission and
//! effort choices that other surfaces show too are [`crate::permission::words`].
//!
//! Ported from the old app's `launch_composer.rs` at reference commit `24bc0580` (`HARNESS_PICKER_ORDER`,
//! `guarded_permission_help`).

use farhelm_proto::{LaunchHarness, LaunchPermission};

use crate::permission::words::permission_help;

/// The order the launcher's harness chips appear in, as the old launcher drew them. Checked at compile time to name
/// every agent type exactly once, so a new agent type fails the build here until someone decides where its chip goes.
pub(super) const HARNESS_PICKER_ORDER: [LaunchHarness; 12] = [
    LaunchHarness::Codex,
    LaunchHarness::Claude,
    LaunchHarness::Antigravity,
    LaunchHarness::Muse,
    LaunchHarness::Cursor,
    LaunchHarness::Grok,
    LaunchHarness::Goose,
    LaunchHarness::Pi,
    LaunchHarness::Omp,
    LaunchHarness::OpenCode,
    LaunchHarness::Bash,
    LaunchHarness::Zsh,
];
const _: () = assert!(LaunchHarness::is_ordering_of_built_ins(
    &HARNESS_PICKER_ORDER
));

/// The page the permission help button opens: the docs' explanation of every agent's permission modes, as the old
/// launcher's "?" did.
pub(super) const PERMISSION_DOCS: &str = "https://farhelm.io/docs/agents/#permission-modes";

/// Guarded's segment help before and after an agent type is chosen (old `guarded_permission_help`). Guarded can be
/// chosen first, but which mechanism it means depends on the agent, so until one is chosen the help says so rather
/// than naming any.
pub(super) fn guarded_permission_help(harness: Option<LaunchHarness>) -> String {
    harness
        .map(|harness| permission_help(harness, Some(LaunchPermission::Guarded)))
        .unwrap_or_else(|| "Guarded: choose an agent to see its automatic safety mode".to_owned())
}

/// A note the Agent tab shows under an agent type's choices, for one whose sessions lack something the others have:
/// Cursor runs without Farhelm's session tracking and has no Resume (the old launcher's `cursor_launch` note).
/// OpenCode shares Cursor's runtime but the old launcher said nothing for it, so neither does this.
pub(super) const fn agent_note(harness: LaunchHarness) -> Option<&'static str> {
    match harness {
        LaunchHarness::Cursor => Some("Cursor session tracking and Resume are not supported."),
        LaunchHarness::Codex
        | LaunchHarness::Claude
        | LaunchHarness::Antigravity
        | LaunchHarness::Muse
        | LaunchHarness::Grok
        | LaunchHarness::Goose
        | LaunchHarness::Pi
        | LaunchHarness::Omp
        | LaunchHarness::OpenCode
        | LaunchHarness::Bash
        | LaunchHarness::Zsh
        | LaunchHarness::Custom => None,
    }
}

/// The order the launcher lists agent types in its searches and its model list (the old `HARNESS_SEARCH_ORDER`).
/// It differs from [`HARNESS_PICKER_ORDER`] only in where Grok sits; both orders predate the GPUI app and are kept.
pub(super) const HARNESS_SEARCH_ORDER: [LaunchHarness; 12] = [
    LaunchHarness::Codex,
    LaunchHarness::Claude,
    LaunchHarness::Antigravity,
    LaunchHarness::Muse,
    LaunchHarness::Cursor,
    LaunchHarness::Goose,
    LaunchHarness::Pi,
    LaunchHarness::Omp,
    LaunchHarness::OpenCode,
    LaunchHarness::Grok,
    LaunchHarness::Bash,
    LaunchHarness::Zsh,
];
const _: () = assert!(LaunchHarness::is_ordering_of_built_ins(
    &HARNESS_SEARCH_ORDER
));
