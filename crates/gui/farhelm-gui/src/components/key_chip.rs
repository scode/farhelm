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
        .rounded(metrics::RADIUS_SM)
        .text_size(px(11.))
        .text_color(theme::color(ink))
        .child(keys.into())
}
