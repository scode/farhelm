//! Settings → Appearance: System, Dark or Light as one radio group over the helm's `appearance` preference.
//!
//! Ported from the old app's `AppearanceSettings` (`appearance.rs` at reference commit `24bc0580`) with its help line,
//! its "Color theme" legend over the segmented well, and its three choices. The old app kept the choice per device, in
//! the web view's storage; here it is one field of the helm's preference row like every other Settings choice (the
//! maintainer decided on 2026-11-13 that appearance is a helm preference), so the help no longer says "for this device"
//! and says instead that other open clients pick the choice up right away. Nothing is carried over from the old
//! storage: every user starts at System after the switch to this app.
//!
//! The radio saves like a switch does ([`super::SettingsDialog::choose_appearance`]): the field alone, under the
//! preference row's key, showing the value being saved only while the save is in flight and the helm's stored choice
//! at every other moment, with a failure line under the group after a refusal.
//!
//! The stored choice also reaches the theme from here ([`follow_stored`], called through the main view's subscription
//! to the current UI session). Which palette that paints is not decided here: the theme resolves it from the choice and
//! the operating system's appearance.

use farhelm_helm_client::PathError;
use farhelm_proto::ui_channel::Preferences;
use gpui::{App, Div, FontWeight, ParentElement as _, Role, Styled as _, Window, div, px};

use super::choices;
use crate::components::{HoverHelp, RadioGroup, RadioOption};
use crate::drafts::Draft;
use crate::operations::Command;
use crate::theme::{self, Preference};
use crate::{agent_startup, session};

/// The radio group's element id; each option is drawn under `settings-appearance-<its label>`.
pub(super) const ID: &str = "settings-appearance";

/// The failure line's element id.
pub(super) const ERROR_ID: &str = "settings-appearance-error";

/// The preference row's field, as the wire names it.
const FIELD: &str = "appearance";

/// `.appearance-legend`: 13-pixel semibold words, 8 pixels above the well.
const LEGEND_TEXT: f32 = 13.;
const LEGEND_GAP: f32 = 8.;

/// The help above the group: the old line, reworded for a choice the helm holds (see the module docs).
const HELP: &str = "Choose the interface colors. This choice applies to this helm; other open clients pick it up right \
                    away. System follows each device's own appearance. The terminal stays dark.";

/// The line under the group after the helm refused a save.
const FAILURE: &str = "Could not save the color theme. Try again.";

/// The three choices in the old order, each with its label and hover help.
const CHOICES: [(Preference, &str, &str); 3] = [
    (
        Preference::System,
        "System",
        "Follow this device's appearance, light or dark",
    ),
    (
        Preference::Dark,
        "Dark",
        "Use the dark colors whatever this device's appearance is",
    ),
    (
        Preference::Light,
        "Light",
        "Use the light colors whatever this device's appearance is",
    ),
];

/// The choice the helm stores. Unset reads as System, the default, and so does a word this build does not know, which
/// the helm itself already reads back as unset (`Preferences::appearance`); `None` (no row pushed) is System too.
pub(super) fn stored(preferences: Option<&Preferences>) -> Preference {
    match preferences.and_then(|preferences| preferences.appearance.as_deref()) {
        Some("dark") => Preference::Dark,
        Some("light") => Preference::Light,
        _ => Preference::System,
    }
}

/// Hand the helm's stored appearance to the theme, which repaints every window if the palette changes and leaves System
/// following the operating system. The main view calls this from [`session::observe_current`], so it runs at the first
/// snapshot, on every change of the store and after a reset; it costs one row lookup when the choice is unchanged,
/// since [`theme::set_preference`] then does nothing.
///
/// Nothing changes while the window is inert: with no UI session there is no stored choice, and the palette in use
/// stays until the next snapshot brings one. Before the first snapshot that palette is System's, the theme's starting
/// choice, so the connecting screen is in the operating system's appearance and switches when a stored Dark or Light
/// arrives (a deviation recorded in `docs/gpui/parity/settings-and-hosts.md`).
///
/// NOTE: An agent build started with `--appearance` keeps that palette for the whole run and ignores the stored choice
/// ([`agent_startup`]). The hook exists so a screenshot batch captures the app light and dark whatever the helm or the
/// machine says; letting the first snapshot replace it would capture every picture in the stored appearance.
pub(crate) fn follow_stored(cx: &mut App) {
    if agent_startup::current(cx).preference.is_some() {
        return;
    }
    let Some(store) = session::current_store(cx) else {
        return;
    };
    let choice = stored(super::preferences(store.read(cx).collections()));
    theme::set_preference(choice, cx);
}

/// The wire word for `preference`. System is sent as `system`, not as a cleared field: the helm reads both as System,
/// and an explicit word keeps the patch the same shape as the other two.
fn word(preference: Preference) -> &'static str {
    match preference {
        Preference::System => "system",
        Preference::Dark => "dark",
        Preference::Light => "light",
    }
}

/// `PUT /api/preferences` with only the appearance field, set to `preference`.
pub(super) fn command(preference: Preference) -> Result<Command, PathError> {
    choices::patch(
        FIELD,
        serde_json::Value::String(word(preference).to_owned()),
    )
}

/// The value the group shows: the one being saved while its save is in flight, and the helm's stored choice at every
/// other moment, as a switch's ([`choices::Switch::shown`]).
fn shown(draft: &Draft<Preference>, preferences: Option<&Preferences>) -> Preference {
    if draft.is_saving() {
        *draft.value()
    } else {
        stored(preferences)
    }
}

/// The section's content: the help, then the "Color theme" legend over the segmented radios, then the failure line
/// after a refused save. `on_change` is what a choice does with the chosen value.
pub(super) fn section(
    draft: &Draft<Preference>,
    preferences: Option<&Preferences>,
    on_change: impl Fn(&Preference, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let palette = theme::palette(cx);
    let group = CHOICES.iter().fold(
        RadioGroup::new(
            ID,
            "Color theme",
            HoverHelp::tooltip("The interface colors for every client of this helm"),
            Role::RadioGroup,
        )
        .chosen(Some(shown(draft, preferences))),
        |group, &(value, label, help)| {
            group.option(RadioOption::new(label, HoverHelp::tooltip(help), value))
        },
    );
    // `.appearance-settings`: the help and the fieldset 14 pixels apart, the same gap the section keeps under its
    // heading.
    div()
        .flex()
        .flex_col()
        .gap(px(super::SECTION_GAP))
        .child(choices::help(HELP, cx))
        .child(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(LEGEND_GAP))
                .child(
                    div()
                        .text_size(px(LEGEND_TEXT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::color(palette.fg_0))
                        .child("Color theme"),
                )
                .child(group.on_change(on_change))
                .children(
                    draft
                        .refusal()
                        .map(|_| choices::failure_line(ERROR_ID.to_owned(), FAILURE, cx)),
                ),
        )
}
