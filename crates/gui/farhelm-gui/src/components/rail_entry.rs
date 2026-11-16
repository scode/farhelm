//! What a host rail button shows: its glyph in the rail's icon box, and in the expanded rail a label beside it.
//!
//! SPEC.md, "Session list": expanded, "All hosts, Add host and the foot controls show labels". Ported from the old
//! stylesheet's `.host-rail-icon` and `.host-rail-label` rules at reference commit `24bc0580`. The rail's buttons belong
//! to three areas (the session list's rail and foot, the help menu's `?`, the rail notices' bell), and each must put its
//! glyph and label in the same places, or the foot's icons would not line up and would move as the rail expands. So
//! the drawing lives here once. It is a drawing-only piece of the component layer: no input, no focus, nothing to name
//! (the button it sits in carries the name, and the label repeats it, so the label is hidden from assistive technology
//! by having no role).
//!
//! The icon box is 40 pixels square with a margin of -1 on every side, as the old one was: it covers the button's
//! 1-pixel edge, so the glyph centered in it sits exactly where it sat centered in the collapsed 40-pixel square. That is
//! what keeps every icon still when the rail expands and its buttons become rows laid out from the left
//! ([`super::Button::rail_row`]).

use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

/// `.host-rail-icon`'s side.
pub const RAIL_ICON_BOX: f32 = 40.;

/// The label's distance from the icon box's laid-out end: the old button's 6-pixel flex gap and the label's own
/// `margin-left: 8px`. The box ends a pixel before its 40 pixels (its -1 margin), so the label starts 52 pixels into the
/// button's content, as the old one did.
const LABEL_GAP: f32 = 6. + 8.;

/// `glyph` centered in the rail's icon box, then `label` beside it when the rail is expanded (`None` while collapsed).
/// A label that sets no ink of its own takes the button's text size and ink, as the old `.host-rail-label` inherited
/// them, so a pressed or open button's inverse ink reaches it too; the notices bell gives its label the bell's own ink,
/// since the old bell turned its whole button red while something was unread.
pub fn rail_entry(glyph: impl IntoElement, label: Option<AnyElement>) -> AnyElement {
    let icon = div()
        .relative()
        .flex_none()
        .size(px(RAIL_ICON_BOX))
        .m(px(-1.))
        .flex()
        .items_center()
        .justify_center()
        .child(glyph);
    match label {
        None => icon.into_any_element(),
        Some(label) => div()
            .flex()
            .items_center()
            .min_w_0()
            .child(icon)
            .child(div().ml(px(LABEL_GAP)).flex_none().child(label))
            .into_any_element(),
    }
}
