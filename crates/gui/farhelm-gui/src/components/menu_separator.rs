//! The hairline between two groups of a menu's commands: the old stylesheet's `.menu-separator`.
//!
//! Two menus draw it: a session row's menu (`session_list::menu`, between its lifecycle groups) and a host's menu
//! (`hosts::menu`, before its destructive group). Both copy the same old rule, so it lives here once rather than in
//! either area.

use gpui::{App, IntoElement, Styled as _, div, px};

use crate::theme;

/// A hairline between two of a menu's groups (`.menu-separator`): one pixel of `--border-dim`, inset 10 pixels from the
/// panel's sides, with 4 pixels above and below. Drawing only: it is no command, takes no focus, and the arrow keys pass
/// over it.
pub fn menu_separator(cx: &App) -> impl IntoElement {
    div()
        .h(px(1.))
        .my(px(4.))
        .mx(px(10.))
        .bg(theme::color(theme::palette(cx).border_dim))
}
