//! The Settings switches over the helm's shared preference row: what each one is called and explains, how it reads the
//! pushed preferences, the command that saves it, and how one is drawn with its help and its failure line.
//!
//! Each switch is ported from the old Settings dialog (`settings.rs` at reference commit `24bc0580`) with its label,
//! hover help and help text, except that the help no longer says other clients see the choice "when they reload": the
//! migration's orchestrator decided on 2026-11-13 that such shared changes arrive live in the new app, whose helm
//! pushes its state, and that each area rewords its own help as it ports it. The help names who else changes these
//! choices, the command line and agents, rather than other open clients: one desktop app talks to its helm (the
//! orchestrator's drift scan of 2026-11-15). The three status sounds' switches come from the old `SoundSettings`
//! (`sounds.rs` at the same commit), which saved them on the device; here they are fields of the preference row like
//! the rest (the maintainer's decision of 2026-11-13).
//!
//! A switch saves one field of the preference row with `PUT /api/preferences` and a sparse body naming only that field
//! (the helm's `put_preferences`: a field the patch leaves out is untouched), so it never overwrites another field with
//! a stale copy. Each switch always sends a value, never `null`: turning Skip session confirmations off sends `false`,
//! as the old app's `restore_session_confirmations` did.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::Preferences;
use gpui::{
    App, Div, ElementId, FocusHandle, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Role, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};
use serde::de::DeserializeOwned;

use super::Refused;
use crate::assets::Icon;
use crate::components::{Button, Checkbox, HoverHelp, IconButton, Tier};
use crate::drafts::Draft;
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Command, Work};
use crate::sounds::Sound;
use crate::sounds::rules::Switches;
use crate::theme::{self, metrics};

/// The old labels' size: the browser's default 16 pixels, which the Settings rows inherited (the stylesheet's own
/// comment calls them "the 16px field labels").
pub(super) const LABEL_TEXT: f32 = 16.;
/// The labels' line: `line-height: normal` at 16 pixels in the bundled face, which keeps a row as high as its 20-pixel
/// switch, as in the old app. GPUI's default line is taller, and would push each help paragraph down from its label.
pub(super) const LABEL_LINE_HEIGHT: f32 = 19.4;
/// `.host-settings-help`'s line height in Settings: `line-height: 1.6` at its 12 pixels.
pub(super) const HELP_LINE_HEIGHT: f32 = 19.2;
/// `.host-settings-section .setting-group`'s gap: a description sits this close under its own label.
pub(super) const GROUP_GAP: f32 = 4.;
/// `.checkout-root-label`'s gap between a field's words and the field, which every labelled text field in Settings
/// shares ([`labelled`]).
pub(super) const LABEL_GAP: f32 = 6.;
/// `.checkout-root-label input`'s `min-height`: the single-line text fields' height in Settings.
pub(super) const FIELD_HEIGHT: f32 = 36.;
/// `.checkout-root-error` and `.checkout-root-success`: `line-height: 1.5` at 12 pixels, the explicitly saved forms'
/// outcome lines and notes ([`outcome_line`]).
pub(super) const OUTCOME_LINE_HEIGHT: f32 = 18.;

/// One preference switch in Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    /// Sessions: Skip session confirmations, off by default.
    SkipSessionConfirmations,
    /// Sessions: Name sessions on quick launch, on by default, including when the helm never stored a choice.
    NameSessionsOnQuickLaunch,
    /// Sessions: Color terminal frames by host, off by default.
    ColorTerminalFrames,
    /// Hosts: Update remote hosts automatically, on until the user turns it off.
    AutoUpdateRemoteHosts,
    /// Hosts: Set up new hosts without asking, on when the setup confirmation is skipped.
    SkipHostSetupConfirmation,
    /// Hosts: Remove hosts without asking, on when the removal confirmation is skipped.
    SkipHostRemoveConfirmation,
    /// Claude's page shows this hint switch. It is on unless the shared dismissal is stored,
    /// so the switch is the dismissal inverted: turning it off stores `claude_trust_hint_dismissed: true`, which the
    /// band's Don't show again stores too (SPEC.md: "a shared dismissal, reversible beside the switch").
    ShowClaudeTrustHint,
    /// Codex's page uses its own shared hint dismissal.
    ShowCodexTrustHint,
    /// Muse's page uses its own shared hint dismissal.
    ShowMuseTrustHint,
    /// Antigravity's hint covers ordinary directories and managed checkouts.
    ShowAntigravityTrustHint,
    /// Notifications, Sounds: Session waiting, on by default (the status sounds' switches read unset through
    /// [`Switches::from_preferences`], which the sounds' owner reads too).
    SoundWaiting,
    /// Notifications, Sounds: Approval request, on by default.
    SoundApproval,
    /// Notifications, Sounds: Turn finished, off by default.
    SoundTurnFinished,
    /// Notifications, Sounds: SSH sign-in, on by default.
    SoundSshSignIn,
    /// App: Show finalized highlights after the next upgrade, shared by windows on this helm.
    ShowWhatsNew,
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
        Choice::ShowClaudeTrustHint,
        Choice::ShowCodexTrustHint,
        Choice::ShowMuseTrustHint,
        Choice::ShowAntigravityTrustHint,
        Choice::SoundWaiting,
        Choice::SoundApproval,
        Choice::SoundTurnFinished,
        Choice::SoundSshSignIn,
        Choice::ShowWhatsNew,
    ];

    /// Search groups these preferences in the section that owns each control.
    pub(super) fn section(self) -> super::Section {
        use super::Section;
        match self {
            Self::ShowWhatsNew => Section::App,
            Self::SkipSessionConfirmations
            | Self::NameSessionsOnQuickLaunch
            | Self::ColorTerminalFrames => Section::Sessions,
            Self::AutoUpdateRemoteHosts
            | Self::SkipHostSetupConfirmation
            | Self::SkipHostRemoveConfirmation => Section::Hosts,
            Self::ShowClaudeTrustHint => Section::Agent(super::TrustAgent::Claude.harness()),
            Self::ShowCodexTrustHint => Section::Agent(super::TrustAgent::Codex.harness()),
            Self::ShowMuseTrustHint => Section::Agent(super::TrustAgent::Muse.harness()),
            Self::ShowAntigravityTrustHint => {
                Section::Agent(super::TrustAgent::Antigravity.harness())
            }
            Self::SoundWaiting
            | Self::SoundApproval
            | Self::SoundTurnFinished
            | Self::SoundSshSignIn => Section::Notifications,
        }
    }

    /// The preference row's field this switch saves, as the wire names it.
    fn field(self) -> &'static str {
        match self {
            Choice::ShowWhatsNew => "show_whats_new",
            Choice::SkipSessionConfirmations => "skip_session_confirmations",
            Choice::NameSessionsOnQuickLaunch => "name_session_on_quick_launch",
            Choice::ColorTerminalFrames => "color_terminal_frames",
            Choice::AutoUpdateRemoteHosts => "auto_update_remote_hosts",
            Choice::SkipHostSetupConfirmation => "skip_host_setup_confirmation",
            Choice::SkipHostRemoveConfirmation => "skip_host_remove_confirmation",
            Choice::ShowClaudeTrustHint => "claude_trust_hint_dismissed",
            Choice::ShowCodexTrustHint => "codex_trust_hint_dismissed",
            Choice::ShowMuseTrustHint => "muse_trust_hint_dismissed",
            Choice::ShowAntigravityTrustHint => "antigravity_trust_hint_dismissed",
            Choice::SoundWaiting => "sound_waiting",
            Choice::SoundApproval => "sound_approval",
            Choice::SoundTurnFinished => "sound_turn_finished",
            Choice::SoundSshSignIn => "sound_ssh_sign_in",
        }
    }

    /// Whether the helm's stored choice is on. An absent row and an unset field both use the default. Quick-launch
    /// naming, automatic remote-host updates and upgrade highlights stay on until the user stores `false`.
    /// A trust hint's switch shows the hint unless its dismissal is stored `true` (the field's doc: "Unset and false
    /// both allow the hint"). A sound's switch reads as the sounds' owner reads it ([`Switches::from_preferences`]),
    /// so what Settings shows is what plays.
    pub(super) fn stored(self, preferences: Option<&Preferences>) -> bool {
        let sound = |sound| Switches::from_preferences(preferences).enabled(sound);
        let field = preferences.and_then(|preferences| match self {
            Choice::ShowWhatsNew => preferences.show_whats_new,
            Choice::SkipSessionConfirmations => preferences.skip_session_confirmations,
            Choice::NameSessionsOnQuickLaunch => preferences.name_session_on_quick_launch,
            Choice::ColorTerminalFrames => preferences.color_terminal_frames,
            Choice::AutoUpdateRemoteHosts => preferences.auto_update_remote_hosts,
            Choice::SkipHostSetupConfirmation => preferences.skip_host_setup_confirmation,
            Choice::SkipHostRemoveConfirmation => preferences.skip_host_remove_confirmation,
            Choice::ShowClaudeTrustHint => preferences.claude_trust_hint_dismissed,
            Choice::ShowCodexTrustHint => preferences.codex_trust_hint_dismissed,
            Choice::ShowMuseTrustHint => preferences.muse_trust_hint_dismissed,
            Choice::ShowAntigravityTrustHint => preferences.antigravity_trust_hint_dismissed,
            Choice::SoundWaiting
            | Choice::SoundApproval
            | Choice::SoundTurnFinished
            | Choice::SoundSshSignIn => None,
        });
        match self {
            Choice::SoundWaiting => sound(Sound::Waiting),
            Choice::SoundApproval => sound(Sound::Approval),
            Choice::SoundTurnFinished => sound(Sound::TurnFinished),
            Choice::SoundSshSignIn => sound(Sound::SshSignIn),
            Choice::NameSessionsOnQuickLaunch
            | Choice::AutoUpdateRemoteHosts
            | Choice::ShowWhatsNew => field != Some(false),
            Choice::ShowClaudeTrustHint
            | Choice::ShowCodexTrustHint
            | Choice::ShowMuseTrustHint
            | Choice::ShowAntigravityTrustHint => field != Some(true),
            Choice::SkipSessionConfirmations
            | Choice::ColorTerminalFrames
            | Choice::SkipHostSetupConfirmation
            | Choice::SkipHostRemoveConfirmation => field == Some(true),
        }
    }

    /// `PUT /api/preferences` with only this switch's field, set to `on`, or for a trust hint's switch to the dismissal
    /// `!on` (see the module docs).
    pub(super) fn command(self, on: bool) -> Result<Command, PathError> {
        let stored = match self {
            Choice::ShowClaudeTrustHint
            | Choice::ShowCodexTrustHint
            | Choice::ShowMuseTrustHint
            | Choice::ShowAntigravityTrustHint => !on,
            Choice::SkipSessionConfirmations
            | Choice::ShowWhatsNew
            | Choice::NameSessionsOnQuickLaunch
            | Choice::ColorTerminalFrames
            | Choice::AutoUpdateRemoteHosts
            | Choice::SkipHostSetupConfirmation
            | Choice::SkipHostRemoveConfirmation
            | Choice::SoundWaiting
            | Choice::SoundApproval
            | Choice::SoundTurnFinished
            | Choice::SoundSshSignIn => on,
        };
        patch(self.field(), serde_json::Value::Bool(stored))
    }

    /// The switch's element id, which tests and the test bridge find it by.
    pub(super) fn id(self) -> &'static str {
        match self {
            Choice::ShowWhatsNew => "settings-show-whats-new",
            Choice::SkipSessionConfirmations => "settings-skip-session-confirmations",
            Choice::NameSessionsOnQuickLaunch => "settings-name-quick-launches",
            Choice::ColorTerminalFrames => "settings-color-terminal-frames",
            Choice::AutoUpdateRemoteHosts => "settings-auto-update-remote-hosts",
            Choice::SkipHostSetupConfirmation => "settings-skip-host-setup",
            Choice::SkipHostRemoveConfirmation => "settings-skip-host-remove",
            Choice::ShowClaudeTrustHint => "settings-claude-trust-hint",
            Choice::ShowCodexTrustHint => "settings-codex-trust-hint",
            Choice::ShowMuseTrustHint => "settings-muse-trust-hint",
            Choice::ShowAntigravityTrustHint => "settings-antigravity-trust-hint",
            Choice::SoundWaiting => "settings-sound-waiting",
            Choice::SoundApproval => "settings-sound-approval",
            Choice::SoundTurnFinished => "settings-sound-turn-finished",
            Choice::SoundSshSignIn => "settings-sound-ssh-sign-in",
        }
    }

    /// The failure line's element id: the switch's, with `-error`.
    pub(super) fn error_id(self) -> String {
        format!("{}-error", self.id())
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Choice::ShowWhatsNew => "Show what's new after updates",
            Choice::SkipSessionConfirmations => "Skip session confirmations",
            Choice::NameSessionsOnQuickLaunch => "Name sessions on quick launch",
            Choice::ColorTerminalFrames => "Color terminal frames by host",
            Choice::AutoUpdateRemoteHosts => "Update remote hosts automatically",
            Choice::SkipHostSetupConfirmation => "Set up new hosts without asking",
            Choice::SkipHostRemoveConfirmation => "Remove hosts without asking",
            Choice::ShowClaudeTrustHint => {
                "Show the hint when Claude asks about a managed checkout"
            }
            Choice::ShowCodexTrustHint => "Show the hint when Codex asks about a managed checkout",
            Choice::ShowMuseTrustHint => "Show the hint when Muse asks about a managed checkout",
            Choice::ShowAntigravityTrustHint => {
                "Show the hint when Antigravity asks about workspace trust"
            }
            Choice::SoundWaiting => "Session waiting",
            Choice::SoundApproval => "Approval request",
            Choice::SoundTurnFinished => "Turn finished",
            Choice::SoundSshSignIn => "SSH sign-in",
        }
    }

    /// The hover help, the old row's `data-tooltip`.
    pub(super) fn tooltip(self) -> &'static str {
        match self {
            Choice::ShowWhatsNew => "Show release highlights after Farhelm upgrades",
            Choice::SkipSessionConfirmations => {
                "Stop, delete, replace, restart and close terminal tabs on the first click"
            }
            Choice::NameSessionsOnQuickLaunch => {
                "Offer to name a quick-launched session while its agent starts"
            }
            Choice::ColorTerminalFrames => "Color each terminal's frame to match its host icon",
            Choice::AutoUpdateRemoteHosts => {
                "Update remote hosts as soon as an update is available, while Farhelm is open; never update the \
                 helm's own machine"
            }
            Choice::SkipHostSetupConfirmation => {
                "Set up a new host as soon as it has been checked, without showing the plan first"
            }
            Choice::SkipHostRemoveConfirmation => {
                "Forget a host as soon as you choose remove, without asking; its sessions keep running"
            }
            Choice::ShowClaudeTrustHint => {
                "Show a note above the terminal when Claude asks whether to trust a new managed checkout"
            }
            Choice::ShowCodexTrustHint => {
                "Show a note above the terminal when Codex asks about trust in a managed checkout"
            }
            Choice::ShowMuseTrustHint => {
                "Show a note above the terminal when Muse asks about trust in a managed checkout"
            }
            Choice::ShowAntigravityTrustHint => {
                "Show a note above the terminal when Antigravity asks whether to trust the folder"
            }
            Choice::SoundWaiting => "Play a bell when a session starts waiting for your answer",
            Choice::SoundApproval => {
                "Play a ding-dong when an agent asks Farhelm to do something that needs your approval"
            }
            Choice::SoundTurnFinished => {
                "Play a soft pluck when an agent goes from running to idle"
            }
            Choice::SoundSshSignIn => "Play a chime when an SSH host needs you to sign in",
        }
    }

    /// The help under the switch, which for the two host switches explains the behavior the shown value gives
    /// (SPEC.md: "Each switch explains the current behavior"). A sound uses its tooltip as its description.
    pub(super) fn help(self, on: bool) -> &'static str {
        match self {
            Choice::ShowWhatsNew if on => {
                "Show highlights at the next upgrade. All windows on this helm use this choice."
            }
            Choice::ShowWhatsNew => {
                "Skip upgrade highlights. All windows on this helm use this choice."
            }
            Choice::SkipSessionConfirmations => {
                "Stop, delete, replace, restart and close terminal tabs on the first click. YOLO, host and trash \
                 confirmations still follow their own rules."
            }
            Choice::NameSessionsOnQuickLaunch => {
                "Offer to name a quick-launched session while its agent starts. Enter keeps its default name."
            }
            Choice::ColorTerminalFrames => {
                "Match the thin frame around every session's terminal to its host icon color. The terminal \
                 background stays the same."
            }
            Choice::AutoUpdateRemoteHosts => {
                "Update remote hosts as soon as an update is available, while Farhelm is open. This never updates \
                 the helm's own machine."
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
            Choice::ShowClaudeTrustHint
            | Choice::ShowCodexTrustHint
            | Choice::ShowMuseTrustHint
            | Choice::ShowAntigravityTrustHint => "",
            Choice::SoundWaiting
            | Choice::SoundApproval
            | Choice::SoundTurnFinished
            | Choice::SoundSshSignIn => self.tooltip(),
        }
    }

    /// The manual preview goes to the installed player without observing session state or changing the switch.
    fn preview(self) -> Option<Sound> {
        match self {
            Self::SoundWaiting => Some(Sound::Waiting),
            Self::SoundApproval => Some(Sound::Approval),
            Self::SoundTurnFinished => Some(Sound::TurnFinished),
            Self::SoundSshSignIn => Some(Sound::SshSignIn),
            _ => None,
        }
    }

    /// The line shown under the switch after the helm refused its save of `sent`. The old app's words where it had
    /// them; the sounds' switches had none, since the old app saved them on the device, so theirs are new, worded like
    /// the others. It kept turning the session-confirmation opt-out on and the two host switches silent, a best-effort
    /// exception that assumed an optimistic copy; the migration's orchestrator decided on 2026-11-14 that they report a
    /// refusal too (SPEC.md, "Errors and diagnostics"), so those lines are new, worded like the others.
    fn failure(self, sent: bool) -> &'static str {
        match self {
            Choice::ShowWhatsNew => "Could not save whether to show upgrade highlights. Try again.",
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
            Choice::ShowClaudeTrustHint => {
                "Could not save whether to show the Claude trust hint. Try again."
            }
            Choice::ShowCodexTrustHint => {
                "Could not save whether to show the Codex trust hint. Try again."
            }
            Choice::ShowMuseTrustHint => {
                "Could not save whether to show the Muse trust hint. Try again."
            }
            Choice::ShowAntigravityTrustHint => {
                "Could not save whether to show the Antigravity trust hint. Try again."
            }
            Choice::SoundWaiting => "Could not save whether to play the waiting sound. Try again.",
            Choice::SoundApproval => {
                "Could not save whether to play the approval sound. Try again."
            }
            Choice::SoundTurnFinished => {
                "Could not save whether to play the turn finished sound. Try again."
            }
            Choice::SoundSshSignIn => {
                "Could not save whether to play the SSH sign-in sound. Try again."
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

/// `PUT /api/preferences` with a sparse body naming only `field`, set to `value`: how every choice in Settings saves
/// its one field of the preference row (see the module docs for why the body names nothing else).
pub(super) fn patch(field: &str, value: serde_json::Value) -> Result<Command, PathError> {
    let mut body = serde_json::Map::new();
    body.insert(field.to_owned(), value);
    preference_command(serde_json::Value::Object(body))
}

/// Both upgrade fields travel together so re-enabling starts from the running release.
pub(super) fn whats_new_command(running: &str, on: bool) -> Result<Command, PathError> {
    preference_command(crate::whats_new::preferences_patch(running, Some(on)))
}

fn preference_command(body: serde_json::Value) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Save,
        Method::Put,
        ApiPath::api(&["preferences"])?,
        Some(body),
        "PUT /api/preferences",
    ))
}

/// Read a route the helm does not push, as Settings' sections do once per opening (the pattern the migration's
/// orchestrator settled for such reads): `on_read` gets the answer decoded as `T`, or why the helm did not give one. A
/// path that does not build is a failed read too, so a control never waits on a read that was not sent.
///
/// `on_read` runs once whenever a request was sent (or the path failed), and never without a UI session, which leaves
/// nothing waiting: Settings does not open without a session, and a reset closes it. Whenever it runs, it runs on its
/// own, after the update that called this has returned, as [`crate::operations::read`]'s answers do, so a caller may
/// send the read from inside its own update and have `on_read` update it.
pub(super) fn read_route<T: DeserializeOwned + 'static>(
    path: Result<ApiPath, PathError>,
    shown: impl Into<String>,
    cx: &mut App,
    on_read: impl FnOnce(Result<T, HostText>, &mut App) + 'static,
) {
    match path {
        Ok(path) => {
            crate::operations::read(path, shown, cx, on_read);
        }
        Err(error) => {
            let reason = HostText::from_host(error.to_string());
            cx.defer(move |cx| on_read(Err(reason), cx));
        }
    }
}

/// The button that reads a failed read again, labelled `label` (the old forms' "Retry loading", or a longer label where
/// the section reads more than one thing) with `help` as its hover help, in the neutral tier, running `on_retry` when
/// pressed. Every Settings form whose read can fail offers one; a form that draws the helm's reason above it on a line
/// of its own uses [`failure_with_retry`].
pub(super) fn retry_loading(
    id: impl Into<SharedString>,
    label: &'static str,
    help: &'static str,
    on_retry: impl Fn(&mut App) + 'static,
) -> Button {
    Button::new(id, label, HoverHelp::tooltip(help), Role::Button)
        .tier(Tier::Neutral)
        .on_click(move |_, _, cx| on_retry(cx))
}

/// A failed read's two lines: `line`, saying why (usually the helm's reason as a [`failure_outcome`]), then `retry`
/// ([`retry_loading`]) on a row of its own, as the old forms drew them.
pub(super) fn failure_with_retry(line: impl IntoElement, retry: Button) -> [gpui::AnyElement; 2] {
    [
        line.into_any_element(),
        div().flex().child(retry).into_any_element(),
    ]
}

/// The line under a choice after its save failed, drawn as an alert under the element id `id` so tests and the test
/// bridge find it next to its control: the danger ink at the help's size and line. The text is the app's own words
/// (fixed, or App's updater reason); the helm's words go through [`host_failure_line`].
pub(super) fn failure_line(id: String, text: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let palette = theme::palette(cx);
    let text = text.into();
    div()
        .id(gpui::ElementId::Name(id.clone().into()))
        .debug_selector(move || id.clone())
        .role(Role::Alert)
        .aria_label(text.clone())
        .text_size(metrics::TEXT_SM)
        .line_height(px(HELP_LINE_HEIGHT))
        .text_color(theme::color(palette.danger))
        .child(text)
}

/// A failure line that carries the helm's words, drawn as [`failure_line`] is under the element id `id`: `sentence`
/// holds the app's words, if any, and the helm's as host text, which reach the screen and assistive technology only
/// escaped (SPEC_impl.md, "Host-supplied text"). The configuration switches' lines and the tunnel switch's are drawn
/// this way.
pub(super) fn host_failure_line(
    id: &'static str,
    sentence: HostSentence,
    cx: &App,
) -> Stateful<Div> {
    let palette = theme::palette(cx);
    div()
        .id(id)
        .debug_selector(|| id.to_owned())
        .role(Role::Alert)
        .aria_label(sentence.accessible_name())
        .text_size(metrics::TEXT_SM)
        .line_height(px(HELP_LINE_HEIGHT))
        .text_color(theme::color(palette.danger))
        .child(sentence)
}

/// The line beside the Settings control `control` (its element id) when `refused` holds its press as the one refused last
/// for a save in flight ([`Refused::line`]), drawn as the choices' failure lines are.
pub(super) fn busy_line(refused: &Refused, control: &str, cx: &App) -> Option<Stateful<Div>> {
    refused.line(control, |id, words| failure_line(id, words, cx))
}

/// A help paragraph: the old `.host-settings-help` in Settings, in the quieter text color at its line height. Takes
/// fixed text or text built at run time (such as a trust read or save failure).
pub(super) fn help(text: impl Into<SharedString>, cx: &App) -> Div {
    let palette = theme::palette(cx);
    div()
        .text_size(metrics::TEXT_SM)
        .line_height(px(HELP_LINE_HEIGHT))
        .text_color(theme::color(palette.fg_1))
        .child(text.into())
}

/// A text field with its words above it, as wide as its column (`.checkout-root-label`): the label at the old labels'
/// size over `field`, which is wrapped in an element drawn under `id`'s debug selector, because tests click a field by
/// where it is drawn and GPUI Kit's field draws no selector of its own. `label` should be the field's accessible name
/// too, as the old `<label>` made it.
pub(super) fn labelled(
    label: &'static str,
    id: &'static str,
    field: impl IntoElement,
    edited: Option<&theme::Palette>,
) -> Div {
    field_with_caption(
        id,
        field,
        div()
            .text_size(px(LABEL_TEXT))
            .line_height(px(LABEL_LINE_HEIGHT))
            .child(super::editing::caption(label, edited)),
    )
}

/// Keep the field's measured target stable when its owner chooses a different caption style.
pub(super) fn field_with_caption(id: &'static str, field: impl IntoElement, caption: Div) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(LABEL_GAP))
        .w_full()
        .min_w_0()
        .child(caption)
        .child(
            div()
                .w_full()
                .debug_selector(move || id.to_owned())
                .child(field),
        )
}

/// A line under an explicitly saved form (its outcome, its working line, a failure), drawn under the element id `id`
/// with `role`, in `ink`, at the forms' 12 pixels on an 18-pixel line, as wide as its column so long words wrap there.
/// The caller adds the words and their accessible name.
pub(super) fn outcome_line(id: impl Into<SharedString>, role: Role, ink: Hsla) -> Stateful<Div> {
    let id: SharedString = id.into();
    div()
        .id(ElementId::Name(id.clone()))
        .debug_selector(move || id.to_string())
        .role(role)
        .w_full()
        .text_size(metrics::TEXT_SM)
        .line_height(px(OUTCOME_LINE_HEIGHT))
        .text_color(ink)
}

/// An [`outcome_line`] holding a failure: the helm's words (or the form's own, carried the same way) as host text, so
/// they reach the screen and assistive technology only escaped, as an alert in the danger ink.
pub(super) fn failure_outcome(
    id: impl Into<SharedString>,
    reason: &HostText,
    cx: &App,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let sentence = HostSentence::new(ElementId::Name(id.clone())).host(reason);
    outcome_line(id, Role::Alert, theme::color(theme::palette(cx).danger))
        .aria_label(sentence.accessible_name())
        .child(sentence)
}

/// One switch with its help under it and, after a refused save, its failure line under that, and the line saying why a
/// press sent nothing (`refused`, [`busy_line`]), all in one group so the help stays with its own label
/// (`.setting-group`). `on_toggle` is what a toggle does with the new value.
pub(super) fn group(
    switch: &Switch,
    preferences: Option<&Preferences>,
    refused: &Refused,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let choice = switch.choice;
    let shown = switch.shown(preferences);
    let sound = choice.preview();
    let copy = super::card::switch_copy(choice.label(), choice.help(shown), cx);
    let toggle = Checkbox::new(
        choice.id(),
        choice.label(),
        HoverHelp::tooltip(choice.tooltip()),
        Role::Switch,
    )
    .switch()
    .checked(shown)
    .focus(&switch.focus)
    .on_toggle(on_toggle);
    let row = if let Some(sound) = sound {
        let label = format!("Preview {}", choice.label());
        let preview = IconButton::new(
            format!("{}-preview", choice.id()),
            Icon::Play,
            label.clone(),
            HoverHelp::tooltip(label),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .square(px(24.))
        .on_click(move |_, _, cx| crate::sounds::play(sound, cx));
        // Leave room before the switch's mark for the preview button. It has its own hitbox over the row, so
        // the label and description still toggle the sound while a press on Play only previews it.
        div()
            .relative()
            .child(toggle.content(copy.pr(px(26.))))
            .child(
                div()
                    .absolute()
                    .right(Checkbox::SWITCH_MARK_WIDTH + px(8.))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .block_mouse_except_scroll()
                    .child(preview),
            )
            .into_any_element()
    } else {
        toggle.content(copy).into_any_element()
    };
    let failure = switch
        .draft
        .refusal()
        .map(|_| choice.failure(*switch.draft.value()));
    div()
        .flex()
        .flex_col()
        .gap(px(GROUP_GAP))
        .min_w_0()
        .child(row)
        .children(failure.map(|failure| failure_line(choice.error_id(), failure, cx)))
        .children(busy_line(refused, choice.id(), cx))
}
