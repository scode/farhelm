//! The launcher's own words for agent choices: the order its harness chips come in, each effort's and permission's
//! name, and the help that says what a permission mode does.
//!
//! Ported from the old app's `launch_composer.rs` at reference commit `24bc0580` (`HARNESS_PICKER_ORDER`,
//! `effort_label`, `permission_display_value`, `permission_tooltip`, `permission_help`, `guarded_permission_help`). The
//! rules about which choices an agent type offers are not here: they are `farhelm_proto`'s, shared with the helm, and
//! this module only names what those rules allow. The escalation banner's names are the draft's
//! ([`super::draft::permission_name`]), which differ in two words, as the old app's did.

use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission};

/// The order the launcher's harness chips appear in, as the old launcher drew them. Checked at compile time to name
/// every agent type exactly once, so a new agent type fails the build here until someone decides where its chip goes.
pub(super) const HARNESS_PICKER_ORDER: [LaunchHarness; 9] = [
    LaunchHarness::Codex,
    LaunchHarness::Claude,
    LaunchHarness::Muse,
    LaunchHarness::Cursor,
    LaunchHarness::Grok,
    LaunchHarness::Goose,
    LaunchHarness::Pi,
    LaunchHarness::Omp,
    LaunchHarness::OpenCode,
];
const _: () = assert!(LaunchHarness::is_ordering_of_all(&HARNESS_PICKER_ORDER));

/// The page the permission help button opens: the docs' explanation of every agent's permission modes, as the old
/// launcher's "?" did.
pub(super) const PERMISSION_DOCS: &str = "https://farhelm.io/docs/agents/#permission-modes";

/// An effort's name as the launcher shows it (old `effort_label`); `None` is the model's default. "xhigh" stays lower
/// case, as the agents spell it.
pub(crate) const fn effort_label(effort: Option<LaunchEffort>) -> &'static str {
    match effort {
        Some(LaunchEffort::Off) => "Off",
        Some(LaunchEffort::Minimal) => "Minimal",
        Some(LaunchEffort::Low) => "Low",
        Some(LaunchEffort::Medium) => "Medium",
        Some(LaunchEffort::High) => "High",
        Some(LaunchEffort::Xhigh) => "xhigh",
        Some(LaunchEffort::Max) => "Max",
        Some(LaunchEffort::Ultra) => "Ultra",
        None => "Default",
    }
}

/// A permission's name in the footer and on its segment (old `permission_display_value`), with no choice as
/// "Default". It differs from the escalation banner's [`super::draft::permission_name`] in two words, as the old app's
/// did: no choice there is the agent's own setting, and Custom there is "Custom flags".
pub(crate) const fn permission_display(permission: Option<LaunchPermission>) -> &'static str {
    match permission {
        None => "Default",
        Some(LaunchPermission::Guarded) => "Guarded",
        Some(LaunchPermission::Yolo) => "YOLO",
        Some(LaunchPermission::Custom) => "Custom",
        Some(LaunchPermission::Approve) => "Approve",
        Some(LaunchPermission::SmartApprove) => "Smart approve",
        Some(LaunchPermission::Chat) => "Chat",
    }
}

/// Hover help for an explicit permission choice (old `permission_tooltip`): what the agent does under it, which the
/// mode's name alone leaves to the reader. Guarded's is the agent's own mechanism, from [`permission_help`].
pub(super) const fn permission_tooltip(permission: LaunchPermission) -> &'static str {
    match permission {
        LaunchPermission::Guarded => {
            "Guarded: the agent can approve actions through a safety mechanism that may refuse or ask you"
        }
        LaunchPermission::Yolo => "YOLO: the agent bypasses its approval and sandbox safety checks",
        LaunchPermission::Custom => {
            "Custom permission flags: your flags replace Farhelm's permission flags; Farhelm does not check them and \
             treats this as YOLO"
        }
        LaunchPermission::Approve => "Approve: the agent asks before every tool action",
        LaunchPermission::SmartApprove => {
            "Smart approve: the agent asks only before actions it judges risky"
        }
        LaunchPermission::Chat => "Chat: the agent only talks and uses no tools",
    }
}

/// Guarded's segment help before and after an agent type is chosen (old `guarded_permission_help`). Guarded can be
/// chosen first, but which mechanism it means depends on the agent, so until one is chosen the help says so rather
/// than naming any.
pub(super) fn guarded_permission_help(harness: Option<LaunchHarness>) -> String {
    harness
        .map(|harness| permission_help(harness, Some(LaunchPermission::Guarded)))
        .unwrap_or_else(|| "Guarded: choose an agent to see its automatic safety mode".to_owned())
}

/// What `permission` means for `harness`, for the permission help button and Guarded's segment (old
/// `permission_help`): the mode's heading and explanation, Guarded's flags from the same definition the helm uses, and
/// the caveat that Farhelm starts the agent this way but cannot guarantee how it behaves. Paragraphs are separated by
/// blank lines, a heading from its explanation by a line break.
pub(super) fn permission_help(
    harness: LaunchHarness,
    permission: Option<LaunchPermission>,
) -> String {
    let explanation = match permission {
        Some(LaunchPermission::Guarded) => {
            guarded_mode(harness).unwrap_or(permission_tooltip(LaunchPermission::Guarded))
        }
        Some(value) => permission_tooltip(value),
        None => {
            "Not classified · Agent's own setting\nFarhelm did not force a guarded permission mode for this launch. The \
             agent's configuration decides; hosts that ask before YOLO launches treat it as YOLO."
        }
    };
    let mut help = explanation.to_owned();
    if permission == Some(LaunchPermission::Guarded) {
        help.push_str("\n\nFlags: ");
        help.push_str(&harness.guarded_flags().join(" "));
    }
    help.push_str(
        "\n\nFarhelm starts the agent with these choices, but can't guarantee how it behaves. Your own allow rules, \
         hooks, plugins, always-allow answers, sandbox settings or switching modes inside the session can loosen it, \
         and agents change between versions. See About permission modes in the agent documentation.",
    );
    help
}

/// The agent's own name for its guarded mode, with what it does; `None` for an agent type that offers no guarded mode
/// (the old app's `unreachable!` arms, which the permission controls never reached because they offer Guarded only
/// where `offers_permission` says so).
const fn guarded_mode(harness: LaunchHarness) -> Option<&'static str> {
    match harness {
        LaunchHarness::Claude => Some(
            "Guarded · Claude auto mode\nClaude's classifier approves or blocks actions instead of asking about every \
             action.",
        ),
        LaunchHarness::Codex => Some(
            "Guarded · Codex sandboxed auto\nCodex runs within its workspace sandbox and asks before stepping outside \
             it. An approval reviewer may decide requests according to your configuration.",
        ),
        LaunchHarness::Grok => Some(
            "Guarded · Grok auto-review\nGrok's auto-review classifier approves or blocks actions instead of asking \
             about every action.",
        ),
        LaunchHarness::Muse => Some(
            "Guarded · Muse auto-review\nMuse's AI reviewer approves eligible actions; workspace commands use its \
             sandbox, and restricted actions may ask you.",
        ),
        LaunchHarness::Omp => Some(
            "Guarded · OMP write mode\nOMP approves workspace edits on its own and asks before other actions.",
        ),
        LaunchHarness::Cursor
        | LaunchHarness::Goose
        | LaunchHarness::Pi
        | LaunchHarness::OpenCode => None,
    }
}

/// The heading of [`guarded_mode`] alone ("Guarded · Claude auto mode"): the name a short label uses for a guarded
/// choice, such as a template's summary in the Templates list. Taken from the same text as the help, so a label can
/// never name a different vendor mode than the help beside it.
pub(crate) fn guarded_mode_name(harness: LaunchHarness) -> Option<&'static str> {
    guarded_mode(harness).and_then(|help| help.lines().next())
}

/// A note the Agent tab shows under an agent type's choices, for one whose sessions lack something the others have:
/// Cursor runs without Farhelm's session tracking and has no Resume (the old launcher's `cursor_launch` note).
/// OpenCode shares Cursor's runtime but the old launcher said nothing for it, so neither does this.
pub(super) const fn agent_note(harness: LaunchHarness) -> Option<&'static str> {
    match harness {
        LaunchHarness::Cursor => Some("Cursor session tracking and Resume are not supported."),
        LaunchHarness::Codex
        | LaunchHarness::Claude
        | LaunchHarness::Muse
        | LaunchHarness::Grok
        | LaunchHarness::Goose
        | LaunchHarness::Pi
        | LaunchHarness::Omp
        | LaunchHarness::OpenCode => None,
    }
}

/// The order the launcher lists agent types in its searches and its model list (the old `HARNESS_SEARCH_ORDER`).
/// It differs from [`HARNESS_PICKER_ORDER`] only in where Grok sits; both orders predate the GPUI app and are kept.
pub(super) const HARNESS_SEARCH_ORDER: [LaunchHarness; 9] = [
    LaunchHarness::Codex,
    LaunchHarness::Claude,
    LaunchHarness::Muse,
    LaunchHarness::Cursor,
    LaunchHarness::Goose,
    LaunchHarness::Pi,
    LaunchHarness::Omp,
    LaunchHarness::OpenCode,
    LaunchHarness::Grok,
];
const _: () = assert!(LaunchHarness::is_ordering_of_all(&HARNESS_SEARCH_ORDER));
