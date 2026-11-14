//! Settings → Terminal: who owns a click on a web link in a terminal, as one radio group over the helm's
//! `terminal_link_click` preference.
//!
//! Ported from the Terminal block of the old app's `settings.rs` (`TerminalLinkChoice`) at reference commit `24bc0580`,
//! with its three choices, their description lines and its help. The help no longer says other clients see the choice
//! "when they reload": shared changes arrive live in this app (the migration's orchestrator decided on 2026-11-13 that
//! each area rewords its own help as it ports it).
//!
//! The link choice saves like a switch ([`super::SettingsDialog::choose_link_click`]): the field alone, under the
//! preference row's key, showing the value being saved only while the save is in flight and the helm's stored choice
//! at every other moment, with a failure line under the group after a refusal. What a terminal does with a click is
//! the terminal's to read from the same preference; nothing here tells a terminal anything.
//!
//! NOTE: Not drawn yet, each recorded in `docs/gpui/parity/settings-and-hosts.md`. The tunnel consent switch waits for
//! the tunnel feature's own consent question ("Allow tunnels from remote sessions?"), which the terminal area builds:
//! turning tunnels on must go through that warning, so a plain switch here would grant consent unasked (decided on
//! 2026-11-14). The desktop app's **File types to open with Cmd-click** field governs opening downloaded files and
//! comes with file downloads, later in the migration.

use farhelm_helm_client::PathError;
use farhelm_proto::ui_channel::Preferences;
use gpui::{App, Div, ParentElement as _, Role, Styled as _, Window, div, px};

use super::choices;
use crate::components::{HoverHelp, RadioGroup, RadioOption};
use crate::drafts::Draft;
use crate::operations::Command;

/// The radio group's element id; each option is drawn under `settings-terminal-links-<its label>`.
pub(super) const ID: &str = "settings-terminal-links";

/// The failure line's element id.
pub(super) const ERROR_ID: &str = "settings-terminal-links-error";

/// The preference row's field, as the wire names it.
const FIELD: &str = "terminal_link_click";

/// The help above the group: the old line, with "when they reload" made live (see the module docs).
const HELP: &str = "Clicking a web link in a terminal: this choice applies to every client of this helm. Other open \
                    clients pick it up right away. This web-link choice does not change file downloads.";

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
const CHOICES: [(LinkClick, &str, &str); 3] = [
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

/// The link choices as two of the section's children, which the section spaces like any other (`.host-settings-section`
/// put its help and its group 14 pixels apart): the help, then the stacked radio group with each choice's description
/// and, after a refused save, the failure line under it. `on_change` is what a choice does with the chosen value. The
/// group shows the value being saved while its save is in flight and the stored one otherwise, as a switch does
/// ([`choices::Switch::shown`]).
pub(super) fn links(
    draft: &Draft<LinkClick>,
    preferences: Option<&Preferences>,
    on_change: impl Fn(&LinkClick, &mut Window, &mut App) + 'static,
    cx: &App,
) -> [Div; 2] {
    let shown = if draft.is_saving() {
        *draft.value()
    } else {
        stored(preferences)
    };
    let group = CHOICES.iter().fold(
        RadioGroup::new(
            ID,
            "Terminal web links",
            HoverHelp::tooltip(
                "What a click on a web link in a terminal does, for every client of this helm",
            ),
            Role::RadioGroup,
        )
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
    let choice = div()
        .flex()
        .flex_col()
        .gap(px(FAILURE_GAP))
        .child(group.on_change(on_change))
        .children(
            draft
                .refusal()
                .map(|_| choices::failure_line(ERROR_ID.to_owned(), FAILURE, cx)),
        );
    [choices::help(HELP, cx), choice]
}
