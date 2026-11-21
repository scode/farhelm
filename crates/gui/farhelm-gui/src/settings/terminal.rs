//! Settings → Terminal: who owns a click on a web link in a terminal, as one radio group over the helm's
//! `terminal_link_click` preference, and the tunnel consent switch over its `tunnel_consent` preference.
//!
//! Ported from the Terminal block of the old app's `settings.rs` (`TerminalLinkChoice`) and its desktop-only
//! `TunnelSettings` (`tunnels.rs`) at reference commit `24bc0580`, with the link group's three choices, their
//! description lines and its help, and the switch's label and help. The link help no longer says other clients see the
//! choice "when they reload": shared changes arrive live in this app (the migration's orchestrator decided on
//! 2026-11-13 that each area rewords its own help as it ports it). The switch's help never said so.
//!
//! The link choice saves like a switch ([`super::SettingsDialog::choose_link_click`]): the field alone, under the
//! preference row's key, showing the value being saved only while the save is in flight and the helm's stored choice
//! at every other moment, with a failure line under the group after a refusal. What a terminal does with a click is
//! the terminal's to read from the same preference; nothing here tells a terminal anything.
//!
//! The tunnel switch is not symmetric (SPEC.md: consent "is off until they accept the warning"):
//!
//! - **On** asks the tunnel security warning, the terminal's own question
//!   ([`crate::terminal::tunnels::ask_consent`]), which saves the consent on Allow. The switch shows the stored choice
//!   throughout, also while that save is in flight, since nothing the dialog holds is being saved; it turns on when the
//!   helm pushes the stored consent. Cancel saves nothing and leaves nothing to say. Whatever held focus when the
//!   warning opened gets it back: the switch when a key press or a click on Linux focused it, but not after a click on
//!   a Mac, where a click leaves a checkbox unfocused.
//! - **Off** needs no warning and saves `false` at once like any switch, showing off while the save is in flight. The
//!   helm closes every open tunnel when it stores it.
//!
//! The old switch held its stored value while either save ran ("No optimistic checked state can stand in for an
//! acknowledgment"); here only turning it on does, since turning it off is a draft's save like any switch's.
//!
//! Either way a refusal shows the helm's words under the switch, as the old switch did, until the next press that sends
//! or asks. A press while another Settings save holds the preference row's key sends and asks nothing, and says so
//! ([`super::refused`]); the warning is not shown for a press whose consent could not be saved. A refusal that arrives
//! after Settings has closed is shown nowhere, as for every Settings save; the old app put it in its tunnel notice.
//!
//! The switch is drawn only where tunnels open, the Mac ([`shown_on`]), as the old one was drawn only in the Mac app
//! (`native_desktop`, which the browser interface lacked). On Linux the terminal offers no tunnel, and the switch's
//! words and its warning's speak of this Mac.
//!
//! The section's last control, the **File types to open with Cmd-click** field, is a view of its own
//! ([`super::file_types`]).

use farhelm_helm_client::PathError;
use farhelm_input_mapping::Platform;
use farhelm_proto::ui_channel::Preferences;
use gpui::{App, Div, FocusHandle, ParentElement as _, Role, Styled as _, Window, div, px};

use super::{Refused, card, choices};
use crate::components::{Checkbox, HoverHelp, RadioGroup, RadioOption};
use crate::drafts::Draft;
use crate::host_text::{HostSentence, HostText};
use crate::operations::Command;

/// The radio group's element id; each option is drawn under `settings-terminal-links-<its label>`.
pub(super) const ID: &str = "settings-terminal-links";
pub(super) const LABEL: &str = "Terminal web links";
pub(super) const TOOLTIP: &str = "What a click on a web link in a terminal does, for this helm";

/// The failure line's element id.
pub(super) const ERROR_ID: &str = "settings-terminal-links-error";

/// The preference row's field, as the wire names it.
const FIELD: &str = "terminal_link_click";

/// The help above the group: the old line, with "when they reload" made live (see the module docs).
pub(super) const HELP: &str = "This choice does not change file downloads.";

/// The line under the group after the helm refused a save, the old app's words.
const FAILURE: &str = "Could not save how terminal links are clicked. Try again.";

/// The failure line's gap under the group: a description's gap under its own label (`.setting-group`), so the line
/// reads as the group's.
const FAILURE_GAP: f32 = 4.;

/// Who acts on a primary click on a web link in a terminal (the helm's words in [`LinkClick::word`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LinkClick {
    /// Farhelm opens the link, and the program gets neither the click nor a drag starting there. The default.
    Intercept,
    /// Farhelm opens the link and the program gets the click too.
    Both,
    /// Only the program gets the click; Farhelm opens nothing.
    Program,
}

impl LinkClick {
    /// The wire word the helm validates and stores.
    fn word(self) -> &'static str {
        match self {
            LinkClick::Intercept => "intercept",
            LinkClick::Both => "both",
            LinkClick::Program => "program",
        }
    }
}

/// The three choices in the old order: the value, its label, and the description line under it, which says what the
/// choice does and, for the latter two, its risk (SPEC.md: "The latter two choices explain the risk of a second browser
/// tab and that a program without mouse reporting cannot act on a click").
pub(super) const CHOICES: [(LinkClick, &str, &str); 3] = [
    (
        LinkClick::Intercept,
        "Open in Farhelm, don't pass the click on",
        "Farhelm opens the link once. The program does not receive that click or a drag starting there.",
    ),
    (
        LinkClick::Both,
        "Open in Farhelm and also pass the click on",
        "Use this when the program needs link clicks. A program that opens links itself may open a second tab.",
    ),
    (
        LinkClick::Program,
        "Pass the click on only",
        "Let the program handle links. Farhelm does not open them; without mouse reporting, clicking a link does \
         nothing.",
    ),
];

/// The choice the helm stores. Unset reads as Intercept, the default, and so does a word this build does not know
/// (the old app's upgrade default); `None` (no row pushed) is Intercept too.
pub(super) fn stored(preferences: Option<&Preferences>) -> LinkClick {
    match preferences.and_then(|preferences| preferences.terminal_link_click.as_deref()) {
        Some("both") => LinkClick::Both,
        Some("program") => LinkClick::Program,
        _ => LinkClick::Intercept,
    }
}

/// `PUT /api/preferences` with only the link field, set to `choice`.
pub(super) fn command(choice: LinkClick) -> Result<Command, PathError> {
    choices::patch(FIELD, serde_json::Value::String(choice.word().to_owned()))
}

/// The three link behaviors share one keyboard radio group. A failed choice leaves its explanation beside the group.
pub(super) fn links(
    draft: &Draft<LinkClick>,
    preferences: Option<&Preferences>,
    refused: &Refused,
    on_change: impl Fn(&LinkClick, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let shown = if draft.is_saving() {
        *draft.value()
    } else {
        stored(preferences)
    };
    let group = CHOICES.iter().fold(
        RadioGroup::new(ID, LABEL, HoverHelp::tooltip(TOOLTIP), Role::RadioGroup)
            .radio_rows()
            .chosen(Some(shown)),
        |group, &(value, label, description)| {
            // The description line is the option's accessible description, so the hover help repeats it, as the old
            // label's tooltip did.
            group.option(
                RadioOption::new(label, HoverHelp::tooltip(description), value)
                    .description(description),
            )
        },
    );
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(card::heading("Web links", Some(HELP), None, cx))
        .child(group.on_change(on_change))
        .children(
            draft
                .refusal()
                .map(|_| choices::failure_line(ERROR_ID.to_owned(), FAILURE, cx)),
        )
        .children(choices::busy_line(refused, ID, cx))
}

// ---- Tunnel consent ------------------------------------------------------------------------------------------------

/// The tunnel switch's element id, which tests and the test bridge find it by.
pub(super) const TUNNEL_ID: &str = "settings-tunnels";

/// Its failure line's.
pub(super) const TUNNEL_ERROR_ID: &str = "settings-tunnels-error";

/// The preference row's field, as the wire names it.
const TUNNEL_FIELD: &str = "tunnel_consent";

/// The switch's label, the old one's.
pub(super) const TUNNEL_LABEL: &str = "Allow tunnels from remote sessions";

/// The switch's hover help. The old switch had none, only the help paragraph under it, which assistive technology read
/// as its description; every control here has hover help, so it says the paragraph's first sentence.
pub(super) const TUNNEL_TOOLTIP: &str = "Open a remote session's localhost links through this Mac";

/// The help under the switch, the old words.
pub(super) const TUNNEL_HELP: &str = "Open a remote session's localhost links through this Mac. Turning this off closes every \
                           open tunnel. The next tunnel asks again.";

/// The tunnel switch's state in the open dialog (see the module docs for its two directions).
pub(super) struct TunnelConsent {
    /// Turning tunnels off: while that save is in flight its draft holds `false`, which the switch shows. Turning them
    /// on has no draft, since the warning's own save is the terminal's, not the dialog's.
    pub(super) off: Draft<bool>,
    /// The helm's words for the last press's failed save, in either direction, until the next press that sends or asks.
    pub(super) failure: Option<HostText>,
    /// Kept by the dialog, like a switch's, so the switch stays the same control across section switches. The warning
    /// returns focus to whatever held it when it opened, which is this switch only when the press focused it (see the
    /// module docs).
    pub(super) focus: FocusHandle,
}

impl TunnelConsent {
    /// A switch with nothing saved or refused yet.
    pub(super) fn new(cx: &mut App) -> Self {
        TunnelConsent {
            off: Draft::load(false),
            failure: None,
            focus: cx.focus_handle().tab_stop(true),
        }
    }

    /// What the switch shows: off while its own off save is in flight, and otherwise whether the helm stores consent
    /// (only an explicit `true`: unset is off, SPEC.md's "off until accepted").
    fn shown(&self, preferences: Option<&Preferences>) -> bool {
        !self.off.is_saving()
            && preferences.and_then(|preferences| preferences.tunnel_consent) == Some(true)
    }
}

/// Whether Settings draws the tunnel switch on `platform`: only on the Mac, where the terminal offers tunnels (see the
/// module docs). The platform is a value, as the terminal's offer takes it, so tests on any machine cover both.
pub(super) fn shown_on(platform: Platform) -> bool {
    platform == Platform::Mac
}

/// `PUT /api/preferences` with only `{"tunnel_consent": false}`: turning tunnels off. Turning them on is the warning's
/// save ([`crate::terminal::tunnels::ask_consent`]), never this dialog's.
pub(super) fn tunnels_off_command() -> Result<Command, PathError> {
    choices::patch(TUNNEL_FIELD, serde_json::Value::Bool(false))
}

/// The tunnel switch with its help, after a failed save the helm's words under it, and the line saying why a press
/// sent nothing (`refused`, [`choices::busy_line`]), in one group as the other switches are (`.setting-group`).
/// `on_toggle` is what a toggle does with the new value.
pub(super) fn tunnel_switch(
    state: &TunnelConsent,
    preferences: Option<&Preferences>,
    refused: &Refused,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let switch = Checkbox::new(
        TUNNEL_ID,
        TUNNEL_LABEL,
        HoverHelp::tooltip(TUNNEL_TOOLTIP),
        Role::Switch,
    )
    .switch()
    .content(card::switch_copy(TUNNEL_LABEL, TUNNEL_HELP, cx))
    .checked(state.shown(preferences))
    .focus(&state.focus)
    .on_toggle(on_toggle);
    // The old line was the helm's words alone (`display_peer(error)`), with no words of the app's before them.
    let failure = state.failure.as_ref().map(|reason| {
        choices::host_failure_line(
            TUNNEL_ERROR_ID,
            HostSentence::new(TUNNEL_ERROR_ID).host(reason),
            cx,
        )
    });
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .min_w_0()
        .child(card::heading("Tunnels", Some("Mac only"), None, cx))
        .child(card::rows(
            [div()
                .flex()
                .flex_col()
                .gap(px(FAILURE_GAP))
                .child(switch)
                .children(failure)
                .children(choices::busy_line(refused, TUNNEL_ID, cx))],
            cx,
        ))
}
