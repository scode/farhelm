//! The host settings' identity picker: the sixteen icons and the eight colors a host's mark can take, and a preview of
//! the mark as the session list draws it.
//!
//! Ported from the old dialog's Appearance section (`hosts/settings_dialog.rs`) and the stylesheet's
//! `.host-appearance-*`, `.host-icon-choice` and `.host-color-choice` rules at reference commit `24bc0580`. Each choice
//! is a button that says whether it is the one in force (`aria-pressed`), not a radio: a click saves at once, and the
//! old sheet's own comment explains why the arrows must not walk a group of them ("a radio group whose arrow navigation
//! would save and disable itself at each intermediate icon"). Tab reaches every choice, and Enter or Space chooses.
//!
//! Every choice shows the mark it would give: an icon in the host's current color (in the inverse ink while it is the
//! chosen one, on the accent fill), a color as a swatch of that color, the default as the ordinary foreground. Colors
//! come from the theme's host tokens, through the same mapping the marks use (`identity::tint`).
//!
//! Area code of the hosts module, since the host settings dialog is its one user; it becomes a component if a second
//! one appears.

use farhelm_proto::host_appearance::{HostColor, HostIcon};
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::components::{Button, HoverHelp, Role, Tier};
use crate::hosts::identity::{host_mark, tint};
use crate::theme::{self, Palette, metrics};

use super::{LABEL_LINE_HEIGHT, LABEL_TEXT, help};

/// The icon grid's columns and the gap between choices (`.host-appearance-icons`).
const ICON_COLUMNS: usize = 8;
const ICON_GAP: f32 = 6.;
/// A mark's size inside its choice, and in the preview.
const CHOICE_MARK: f32 = 14.;
/// The gap between color choices and before the color's name (`.host-appearance-colors`).
const COLOR_GAP: f32 = 8.;
/// `.host-appearance-preview`'s padding and the gaps inside it.
const PREVIEW_PAD: f32 = 12.;
const PREVIEW_GAP: f32 = 10.;
const PREVIEW_ROW_GAP: f32 = 8.;

/// The picker's parts, in the old order: "Icon" and the icon grid, "Color" and the color row with the shown color's
/// name, then the preview. `shown` is the icon and color in force (the pair being saved, while a save is in flight);
/// `on_choose` gets the pair a click chose, the other half kept as shown.
pub(super) fn picker(
    shown: (HostIcon, HostColor),
    on_choose: impl Fn((HostIcon, HostColor), &mut Window, &mut App) + Clone + 'static,
    palette: &Palette,
) -> Vec<AnyElement> {
    let (icon, color) = shown;
    let icons = HostIcon::ALL.map(|choice| {
        let chosen = choice == icon;
        let mark = host_mark(choice, color, px(CHOICE_MARK), palette);
        // The chosen icon's mark takes the inverse ink on the accent fill, whatever the host's color (the old sheet's
        // one `!important`): a tinted mark on the fill would not read.
        let mark = if chosen {
            mark.text_color(theme::color(palette.on_fill))
        } else {
            mark
        };
        let on_choose = on_choose.clone();
        // A cell of the old eight-column grid, the choice at its start: the old buttons were at most 36 pixels in
        // columns that are wider.
        div().flex_1().min_w_0().child(
            Button::new(
                format!("host-icon-{}", choice.as_str()),
                choice.display_name(),
                HoverHelp::tooltip(format!("Icon: {}", choice.display_name())),
                Role::Button,
            )
            .tier(Tier::IconChoice)
            .pressed(chosen)
            .content(mark)
            .on_click(move |_, window, cx| on_choose((choice, color), window, cx)),
        )
    });
    let mut cells = icons.into_iter();
    let grid = div()
        .id("host-appearance-icons")
        .role(Role::Group)
        .aria_label("Icon")
        .flex()
        .flex_col()
        .gap(px(ICON_GAP))
        .children((0..HostIcon::ALL.len().div_ceil(ICON_COLUMNS)).map(|_| {
            div()
                .flex()
                .gap(px(ICON_GAP))
                .children(cells.by_ref().take(ICON_COLUMNS))
        }));
    let colors = div()
        .id("host-appearance-colors")
        .role(Role::Group)
        .aria_label("Color")
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(COLOR_GAP))
        .children(HostColor::ALL.map(|choice| {
            let on_choose = on_choose.clone();
            Button::new(
                format!("host-color-{}", choice.as_str()),
                choice.display_name(),
                HoverHelp::tooltip(format!("Color: {}", choice.display_name())),
                Role::Button,
            )
            .tier(Tier::ColorChoice)
            .pressed(choice == color)
            .content(
                div()
                    .size_full()
                    .rounded_full()
                    .bg(theme::color(tint(choice, palette))),
            )
            .on_click(move |_, window, cx| on_choose((icon, choice), window, cx))
        }))
        .child(
            div()
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .child(color.display_name()),
        );
    let preview = div()
        .flex()
        .flex_col()
        .gap(px(PREVIEW_GAP))
        .p(px(PREVIEW_PAD))
        .bg(theme::color(palette.bg_1))
        .border_1()
        .border_color(theme::color(palette.border_dim))
        .rounded(metrics::RADIUS_SM)
        .child(help("In the session list", palette))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(PREVIEW_ROW_GAP))
                .text_size(px(LABEL_TEXT))
                .line_height(px(LABEL_LINE_HEIGHT))
                // The old preview's `.status-dot`, in the quiet ink: any session's status, drawn square as it was there.
                .child(
                    div()
                        .flex_none()
                        .size(metrics::STATUS_DOT)
                        .bg(theme::color(palette.fg_1)),
                )
                .child(host_mark(icon, color, px(CHOICE_MARK), palette))
                .child("Session"),
        );
    vec![
        label("Icon", palette),
        grid.into_any_element(),
        label("Color", palette),
        colors.into_any_element(),
        preview.into_any_element(),
    ]
}

/// A label over a group of choices (`.host-settings-label`), in the quieter ink at the settings' label size.
fn label(text: &'static str, palette: &Palette) -> AnyElement {
    div()
        .text_size(px(LABEL_TEXT))
        .line_height(px(LABEL_LINE_HEIGHT))
        .text_color(theme::color(palette.fg_1))
        .child(text)
        .into_any_element()
}
