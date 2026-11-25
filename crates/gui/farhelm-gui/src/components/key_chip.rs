//! The shared look of a keyboard chord in a menu or the Keyboard shortcuts dialog.

use gpui::{Div, ParentElement as _, SharedString, Styled as _, div, px};

use crate::theme::{self, Palette, Token, metrics};

/// A non-shrinking key chip, based on the old stylesheet's `.quick-switcher-template-keys kbd`.
/// The caller supplies the ink because a highlighted menu command uses inverse text.
pub(crate) fn key_chip(keys: impl Into<SharedString>, ink: Token, palette: &Palette) -> Div {
    div()
        .flex_none()
        .py(px(2.))
        .px(px(5.))
        .border_1()
        .border_color(theme::color(palette.border_dialog))
        .rounded(metrics::RADIUS_CAP)
        .text_size(px(11.))
        .text_color(theme::color(ink))
        .child(keys.into())
}

/// The main dialog action can show Enter beside its visible verb. The button retains its own accessible name and
/// key binding. The caller chooses the chip ink to match the action's enabled or unavailable state.
pub(crate) fn return_key_content(
    verb: impl Into<SharedString>,
    ink: Token,
    palette: &Palette,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(verb.into())
        .child(
            key_chip("⏎", ink, palette)
                .px(px(4.))
                .py(px(0.))
                .border_color(theme::color(ink)),
        )
}
