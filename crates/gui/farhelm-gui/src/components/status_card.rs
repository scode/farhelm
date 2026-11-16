//! The calm accent card that stands over a terminal to say what is happening to it: the session pane's Delete scrim
//! card (`.terminal-delete-overlay-card` at the reference commit) and the terminal's reconnect card
//! (`.session-unavailable-card`), which the old stylesheet drew alike on purpose. SPEC.md, "Session list": a dropped
//! terminal stream "uses the same card" as an unreachable host's outage, and the old outage card took Delete's accent
//! card and scrim, so an expected wait never reads as a failure. The look lives here once so those surfaces cannot
//! drift apart.
//!
//! Drawing only, like the spinner beside it: the card takes no input or focus and names nothing. Whoever draws it
//! decides the scrim around it, whether it takes the pointer, and what accessible status it carries, since those
//! differ (the Delete scrim lets every press through to the terminal; the reconnect card holds a button). The
//! component rule of a label, hover help and a role reaches only the controls put inside it.

use gpui::{App, Div, FontWeight, ParentElement as _, SharedString, Styled as _, div, px};

use super::spinner_icon;
use crate::layers;
use crate::theme::{self, metrics};

/// The card's padding, vertical then horizontal: both old cards' `padding: 14px 20px`.
const PAD: (f32, f32) = (14., 20.);

/// The heading's text size: both old headings' `font-size: 15px`.
const HEADING_TEXT: f32 = 15.;

/// The gap between the heading's spinner and its words: both old headings' `gap: 10px`.
const HEADING_GAP: f32 = 10.;

/// The heading's spinner: `.delete-spinner` at the cards' 16 pixels rather than its usual 12.
const HEADING_SPINNER: f32 = 16.;

/// The empty card, a column for its caller to fill: the second background, a one-pixel accent edge, the medium radius
/// and the menu shadow (`--shadow-menu`), floating over whatever scrim the caller draws. It sets no width: it is as wide
/// as what it holds, and a caller that may hold long text caps it.
pub fn status_card(cx: &App) -> Div {
    let palette = theme::palette(cx);
    div()
        .flex()
        .flex_col()
        .py(px(PAD.0))
        .px(px(PAD.1))
        .bg(theme::color(palette.bg_2))
        .border_1()
        .border_color(theme::color(palette.accent))
        .rounded(metrics::RADIUS_MD)
        .shadow(layers::menu_shadow(palette))
}

/// The card's heading: the stepped spinner at `step` beside `label`, both in the accent at 15 pixels, semibold. The
/// accent rather than danger red, because both cards show a wait the app expects to end by itself.
pub fn status_heading(step: u8, label: impl Into<SharedString>, cx: &App) -> Div {
    let ink = theme::color(theme::palette(cx).accent);
    let label: SharedString = label.into();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(HEADING_GAP))
        .text_size(px(HEADING_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(spinner_icon(step, px(HEADING_SPINNER), ink))
        .child(label)
}
