//! Settings → Appearance: System, Dark or Light as one radio group over the helm's `appearance` preference.
//!
//! The three tiles are previews of the interface chrome. The terminal in each miniature stays dark, even when the
//! surrounding interface follows a device in its light appearance.
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
use gpui::{App, Div, ParentElement as _, Role, Styled as _, Window, div, px, relative};

use super::{Refused, card, choices};
use crate::components::{HoverHelp, RadioGroup, RadioOption};
use crate::drafts::Draft;
use crate::operations::Command;
use crate::theme::{self, Preference};
use crate::{agent_startup, session};

/// The radio group's element id; each option is drawn under `settings-appearance-<its label>`.
pub(super) const ID: &str = "settings-appearance";
pub(super) const LABEL: &str = "Color theme";
pub(super) const TOOLTIP: &str = "The interface colors for this helm";

/// The failure line's element id.
pub(super) const ERROR_ID: &str = "settings-appearance-error";

/// The preference row's field, as the wire names it.
const FIELD: &str = "appearance";

/// The heading describes what System follows and which part of the app keeps its colors.
pub(super) const HELP: &str =
    "System follows each device's own appearance. The terminal stays dark.";

/// The line under the group after the helm refused a save.
const FAILURE: &str = "Could not save the color theme. Try again.";

/// The three choices in the old order, each with its label and hover help.
pub(super) const CHOICES: [(Preference, &str, &str); 3] = [
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

/// The illustrations sit inside a single keyboard radio group, with error and busy lines just below it.
pub(super) fn section(
    draft: &Draft<Preference>,
    preferences: Option<&Preferences>,
    refused: &Refused,
    on_change: impl Fn(&Preference, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let group = CHOICES.iter().fold(
        RadioGroup::new(ID, LABEL, HoverHelp::tooltip(TOOLTIP), Role::RadioGroup)
            .cards()
            .chosen(Some(shown(draft, preferences))),
        |group, &(value, label, help)| {
            group.option(
                RadioOption::new(label, HoverHelp::tooltip(help), value)
                    .preview(preview(value, theme::palette(cx))),
            )
        },
    );
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(card::heading(LABEL, Some(HELP), None, cx))
        .child(card::rows([group.on_change(on_change)], cx))
        .children(
            draft
                .refusal()
                .map(|_| choices::failure_line(ERROR_ID.to_owned(), FAILURE, cx)),
        )
        .children(choices::busy_line(refused, ID, cx))
}

/// The preview borrows actual theme roles so a change in the main palette also reaches its miniature.
fn preview(preference: Preference, palette: &theme::Palette) -> Div {
    let swatch = |fill, width, height| {
        div()
            .w(relative(width))
            .h(px(height))
            .rounded(px(2.))
            .bg(theme::color(fill))
    };
    let sidebar = |palette: &theme::Palette| {
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .p(px(6.))
            .bg(theme::color(palette.bg_1))
            .child(swatch(palette.accent_fill, 0.88, 6.))
            .child(swatch(palette.border, 0.65, 5.))
    };
    let frame = div()
        .flex()
        .h(px(42.))
        .w_full()
        .rounded(px(4.))
        .border_1()
        .border_color(theme::color(palette.border_dim))
        .overflow_hidden();
    match preference {
        Preference::System => frame
            .child(sidebar(&theme::LIGHT).flex_1().min_w_0())
            .child(sidebar(&theme::DARK).flex_1().min_w_0()),
        Preference::Dark | Preference::Light => {
            let palette = if preference == Preference::Dark {
                &theme::DARK
            } else {
                &theme::LIGHT
            };
            frame.child(sidebar(palette).w(relative(0.38))).child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .p(px(6.))
                    .bg(theme::color(theme::DARK.bg_0))
                    .child(swatch(theme::DARK.ok, 0.55, 4.))
                    .child(swatch(theme::DARK.fg_2, 0.75, 4.)),
            )
        }
    }
}
