//! The Settings switches over the helm's shared preference row: what each one is called and explains, how it reads the
//! pushed preferences, the command that saves it, and how one is drawn with its help and its failure line.
//!
//! Each switch is ported from the old Settings dialog (`settings.rs` at reference commit `24bc0580`) with its label,
//! hover help and help text, except that the help no longer says other clients see the choice "when they reload": the
//! migration's orchestrator decided on 2026-11-13 that such shared changes arrive live in the new app, whose helm pushes
//! its state, and that each area rewords its own help as it ports it.
//!
//! A switch saves one field of the preference row with `PUT /api/preferences` and a sparse body naming only that field
//! (the helm's `put_preferences`: a field the patch leaves out is untouched), so it never overwrites another field with
//! a stale copy. Each switch always sends a value, never `null`: turning Skip session confirmations off sends `false`,
//! as the old app's `restore_session_confirmations` did.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::Preferences;
use gpui::{
    App, Div, FocusHandle, InteractiveElement as _, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::components::{Checkbox, HoverHelp};
use crate::drafts::Draft;
use crate::operations::{Command, Work};
use crate::theme::{self, metrics};

/// The old labels' size: the browser's default 16 pixels, which the Settings rows inherited (the stylesheet's own
/// comment calls them "the 16px field labels").
const LABEL_TEXT: f32 = 16.;
/// The labels' line: `line-height: normal` at 16 pixels in the bundled face, which keeps a row as high as its 20-pixel
/// switch, as in the old app. GPUI's default line is taller, and would push each help paragraph down from its label.
const LABEL_LINE_HEIGHT: f32 = 19.4;
/// `.host-settings-help`'s line height in Settings: `line-height: 1.6` at its 12 pixels.
const HELP_LINE_HEIGHT: f32 = 19.2;
/// `.host-settings-section .setting-group`'s gap: a description sits this close under its own label.
const GROUP_GAP: f32 = 4.;

/// One preference switch in Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    /// Sessions: Skip session confirmations, off by default.
    SkipSessionConfirmations,
    /// Sessions: Name sessions on quick launch, on by default, including when the helm never stored a choice.
    NameSessionsOnQuickLaunch,
    /// Sessions: Color terminal frames by host, off by default.
    ColorTerminalFrames,
    /// Hosts: Update remote hosts automatically, off unless explicitly enabled.
    AutoUpdateRemoteHosts,
    /// Hosts: Set up new hosts without asking, on when the setup confirmation is skipped.
    SkipHostSetupConfirmation,
    /// Hosts: Remove hosts without asking, on when the removal confirmation is skipped.
    SkipHostRemoveConfirmation,
}

impl Choice {
    /// Every switch, in the order the dialog keeps their state (the sections choose the order they draw them in).
    pub(super) const ALL: &[Choice] = &[
        Choice::SkipSessionConfirmations,
        Choice::NameSessionsOnQuickLaunch,
        Choice::ColorTerminalFrames,
        Choice::AutoUpdateRemoteHosts,
        Choice::SkipHostSetupConfirmation,
        Choice::SkipHostRemoveConfirmation,
    ];

    /// The preference row's field this switch saves, as the wire names it.
    fn field(self) -> &'static str {
        match self {
            Choice::SkipSessionConfirmations => "skip_session_confirmations",
            Choice::NameSessionsOnQuickLaunch => "name_session_on_quick_launch",
            Choice::ColorTerminalFrames => "color_terminal_frames",
            Choice::AutoUpdateRemoteHosts => "auto_update_remote_hosts",
            Choice::SkipHostSetupConfirmation => "skip_host_setup_confirmation",
            Choice::SkipHostRemoveConfirmation => "skip_host_remove_confirmation",
        }
    }

    /// Whether the helm's stored choice is on. `preferences` is `None` when the helm pushed no row, which reads as
    /// nothing chosen. Every field but naming reads unset as off; naming is a positive choice that defaults on, so only
    /// an explicit `false` turns it off (the old app: "treating an absent value like the confirmation opt-outs would
    /// disable it", and SPEC.md: on "including when the helm has never stored a choice").
    pub(super) fn stored(self, preferences: Option<&Preferences>) -> bool {
        let field = preferences.and_then(|preferences| match self {
            Choice::SkipSessionConfirmations => preferences.skip_session_confirmations,
            Choice::NameSessionsOnQuickLaunch => preferences.name_session_on_quick_launch,
            Choice::ColorTerminalFrames => preferences.color_terminal_frames,
            Choice::AutoUpdateRemoteHosts => preferences.auto_update_remote_hosts,
            Choice::SkipHostSetupConfirmation => preferences.skip_host_setup_confirmation,
            Choice::SkipHostRemoveConfirmation => preferences.skip_host_remove_confirmation,
        });
        match self {
            Choice::NameSessionsOnQuickLaunch => field != Some(false),
            Choice::SkipSessionConfirmations
            | Choice::ColorTerminalFrames
            | Choice::AutoUpdateRemoteHosts
            | Choice::SkipHostSetupConfirmation
            | Choice::SkipHostRemoveConfirmation => field == Some(true),
        }
    }

    /// `PUT /api/preferences` with only this switch's field, set to `on` (see the module docs).
    pub(super) fn command(self, on: bool) -> Result<Command, PathError> {
        let mut body = serde_json::Map::new();
        body.insert(self.field().to_owned(), serde_json::Value::Bool(on));
        Ok(Command::new(
            Work::Save,
            Method::Put,
            ApiPath::api(&["preferences"])?,
            Some(serde_json::Value::Object(body)),
            "PUT /api/preferences",
        ))
    }

    /// The switch's element id, which tests and the test bridge find it by.
    pub(super) fn id(self) -> &'static str {
        match self {
            Choice::SkipSessionConfirmations => "settings-skip-session-confirmations",
            Choice::NameSessionsOnQuickLaunch => "settings-name-quick-launches",
            Choice::ColorTerminalFrames => "settings-color-terminal-frames",
            Choice::AutoUpdateRemoteHosts => "settings-auto-update-remote-hosts",
            Choice::SkipHostSetupConfirmation => "settings-skip-host-setup",
            Choice::SkipHostRemoveConfirmation => "settings-skip-host-remove",
        }
    }

    /// The failure line's element id: the switch's, with `-error`.
    pub(super) fn error_id(self) -> String {
        format!("{}-error", self.id())
    }

    fn label(self) -> &'static str {
        match self {
            Choice::SkipSessionConfirmations => "Skip session confirmations",
            Choice::NameSessionsOnQuickLaunch => "Name sessions on quick launch",
            Choice::ColorTerminalFrames => "Color terminal frames by host",
            Choice::AutoUpdateRemoteHosts => "Update remote hosts automatically",
            Choice::SkipHostSetupConfirmation => "Set up new hosts without asking",
            Choice::SkipHostRemoveConfirmation => "Remove hosts without asking",
        }
    }

    /// The hover help, the old row's `data-tooltip`.
    fn tooltip(self) -> &'static str {
        match self {
            Choice::SkipSessionConfirmations => {
                "Stop, delete, replace, restart and close terminal tabs on the first click"
            }
            Choice::NameSessionsOnQuickLaunch => {
                "Offer to name a quick-launched session while its agent starts"
            }
            Choice::ColorTerminalFrames => "Color each terminal's frame to match its host icon",
            Choice::AutoUpdateRemoteHosts => {
                "Update remote hosts as soon as an update is available, even with no window open; never update the \
                 helm's own machine"
            }
            Choice::SkipHostSetupConfirmation => {
                "Set up a new host as soon as it has been checked, without showing the plan first"
            }
            Choice::SkipHostRemoveConfirmation => {
                "Forget a host as soon as you choose remove, without asking; its sessions keep running"
            }
        }
    }

    /// The help under the switch, which for the two host switches explains the behavior the shown value gives (SPEC.md:
    /// "Each switch explains the current behavior").
    fn help(self, on: bool) -> &'static str {
        match self {
            Choice::SkipSessionConfirmations => {
                "Stop, delete, replace, restart and close terminal tabs on the first click. YOLO, host and trash \
                 confirmations still follow their own rules. This choice applies to this helm; other open clients \
                 pick it up right away."
            }
            Choice::NameSessionsOnQuickLaunch => {
                "Offer to name a quick-launched session while its agent starts. Enter keeps its default name. This \
                 choice applies to this helm; other open clients pick it up right away."
            }
            Choice::ColorTerminalFrames => {
                "Match the thin frame around every session's terminal to its host icon color. The terminal \
                 background stays the same. This choice applies to this helm; other open clients pick it up right \
                 away."
            }
            Choice::AutoUpdateRemoteHosts => {
                "Update remote hosts as soon as an update is available, even with no window open. This never updates \
                 the helm's own machine. Other open clients pick up this choice right away."
            }
            Choice::SkipHostSetupConfirmation if on => {
                "Farhelm sets up a new host as soon as it has checked it, without asking."
            }
            Choice::SkipHostSetupConfirmation => {
                "Farhelm shows what setup will change on a new host and asks before doing it."
            }
            Choice::SkipHostRemoveConfirmation if on => {
                "Farhelm forgets a host as soon as you choose remove. Its sessions keep running."
            }
            Choice::SkipHostRemoveConfirmation => {
                "Farhelm asks before forgetting a host. Its sessions keep running."
            }
        }
    }

    /// The line shown under the switch after the helm refused its save of `sent`. The old app's words where it had
    /// them. It kept turning the session-confirmation opt-out on and the two host switches silent, a best-effort
    /// exception that assumed an optimistic copy; the migration's orchestrator decided on 2026-11-14 that they report a
    /// refusal too (SPEC.md, "Errors and diagnostics"), so those lines are new, worded like the others.
    fn failure(self, sent: bool) -> &'static str {
        match self {
            Choice::SkipSessionConfirmations if !sent => {
                "Confirmations may still be off for this helm. Try again."
            }
            Choice::SkipSessionConfirmations => {
                "Could not save whether to skip session confirmations. Try again."
            }
            Choice::NameSessionsOnQuickLaunch => {
                "Could not save whether to name quick-launched sessions. Try again."
            }
            Choice::ColorTerminalFrames => {
                "Could not save whether to color terminal frames. Try again."
            }
            Choice::AutoUpdateRemoteHosts => {
                "Could not save whether to update remote hosts automatically. Try again."
            }
            Choice::SkipHostSetupConfirmation => {
                "Could not save whether to set up new hosts without asking. Try again."
            }
            Choice::SkipHostRemoveConfirmation => {
                "Could not save whether to remove hosts without asking. Try again."
            }
        }
    }
}

/// One switch's state in the open dialog: its draft, which holds the value being saved and the last refusal, and the
/// focus handle the dialog keeps for it, so focus can start on it and it stays the same control while its section is
/// hidden and shown again.
pub(super) struct Switch {
    pub(super) choice: Choice,
    pub(super) draft: Draft<bool>,
    pub(super) focus: FocusHandle,
}

impl Switch {
    /// The value the switch shows: the one being saved while its save is in flight, and the helm's stored choice at
    /// every other moment (SPEC_impl.md, "The UI channel": the store already holds a saved value when the reply
    /// arrives). There is no optimistic copy, so a refused save shows the stored value again with nothing to restore.
    pub(super) fn shown(&self, preferences: Option<&Preferences>) -> bool {
        if self.draft.is_saving() {
            *self.draft.value()
        } else {
            self.choice.stored(preferences)
        }
    }
}

/// A help paragraph: the old `.host-settings-help` in Settings, in the quieter text color at its line height.
pub(super) fn help(text: &'static str, cx: &App) -> Div {
    let palette = theme::palette(cx);
    div()
        .text_size(metrics::TEXT_SM)
        .line_height(px(HELP_LINE_HEIGHT))
        .text_color(theme::color(palette.fg_1))
        .child(text)
}

/// One switch with its help under it and, after a refused save, its failure line under that, all in one group so the
/// help stays with its own label (`.setting-group`). `on_toggle` is what a toggle does with the new value.
pub(super) fn group(
    switch: &Switch,
    preferences: Option<&Preferences>,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let palette = theme::palette(cx);
    let choice = switch.choice;
    let shown = switch.shown(preferences);
    let failure = switch
        .draft
        .refusal()
        .map(|_| choice.failure(*switch.draft.value()));
    div()
        .flex()
        .flex_col()
        .gap(px(GROUP_GAP))
        .min_w_0()
        .child(
            div()
                .text_size(px(LABEL_TEXT))
                .line_height(px(LABEL_LINE_HEIGHT))
                .child(
                    Checkbox::new(
                        choice.id(),
                        choice.label(),
                        HoverHelp::tooltip(choice.tooltip()),
                        Role::Switch,
                    )
                    .switch()
                    .checked(shown)
                    .focus(&switch.focus)
                    .on_toggle(on_toggle),
                ),
        )
        .child(help(choice.help(shown), cx))
        .children(failure.map(|failure| {
            div()
                .id(gpui::ElementId::Name(choice.error_id().into()))
                .debug_selector(move || choice.error_id())
                .role(Role::Alert)
                .aria_label(failure)
                .text_size(metrics::TEXT_SM)
                .line_height(px(HELP_LINE_HEIGHT))
                .text_color(theme::color(palette.danger))
                .child(failure)
        }))
}
