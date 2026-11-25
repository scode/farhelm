//! Drawing pieces for dialogs that start with a command field. The input still owns editing and focus, and
//! `OptionRow` still owns selection. These pieces keep the field, hints and result contents in the same style.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, px, svg,
};

use super::{Role, TextInput};
use crate::assets::Icon;
use crate::theme::{self, Palette, Token, metrics};

/// A command field's icon stays inside the input well, where pressing it focuses the same editor as its text.
pub(crate) fn command_icon_tile(icon: Icon, palette: &Palette) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.))
        .rounded(metrics::RADIUS_SM)
        .border_1()
        .border_color(theme::color(palette.border_dim))
        .bg(theme::color(palette.bg_1))
        .child(
            svg()
                .path(icon.path())
                .size(px(14.))
                .text_color(theme::color(palette.fg_0)),
        )
}

/// Dress an existing single-line input. The caller still owns its label, key handlers and the meaning of the cap.
pub(crate) fn command_field(
    field: TextInput,
    tile: Div,
    cap: Option<impl IntoElement>,
) -> TextInput {
    field
        .height(px(48.))
        .line_text_size(px(14.5))
        .line_prefix(tile)
        .when_some(cap, TextInput::line_suffix)
}

/// Keep a hint's chips and words together when its parent wraps a sequence of hints onto another line.
pub(crate) fn chips_and_words(parts: impl IntoIterator<Item = AnyElement>) -> Div {
    div().flex().items_center().gap(px(4.)).children(parts)
}

/// Section text is uppercase visually, while the accessible label keeps the ordinary spelling.
pub(crate) fn command_heading(
    id: impl Into<SharedString>,
    label: &str,
    palette: &Palette,
) -> AnyElement {
    command_heading_with_count(id, label, None, palette)
}

/// Keep the count quiet and out of the uppercase label's typography. It follows the label in reading order.
pub(crate) fn command_heading_with_count(
    id: impl Into<SharedString>,
    label: &str,
    count: Option<String>,
    palette: &Palette,
) -> AnyElement {
    let id = id.into();
    div()
        .id(id.clone())
        .debug_selector(move || id.to_string())
        .role(Role::Label)
        .aria_label(label.to_owned())
        .pt(px(4.))
        .px(px(6.))
        .pb(px(1.))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::color(palette.fg_1))
        .child(label.to_uppercase())
        .when_some(count, |heading, count| {
            heading
                .flex()
                .justify_between()
                .items_center()
                .child(div().font_weight(FontWeight::NORMAL).child(count))
        })
        .into_any_element()
}

/// The contents of an option row. A large result list supplies the measured text width so repeated layout probes
/// do not have to shape each label. The launcher's short lists let the lines take the space left by the badges.
/// The caller provides the mark at the size and alignment used by the surrounding list.
pub(crate) fn command_row_content(
    leading: Option<impl IntoElement>,
    text_width: Option<Pixels>,
    lines: impl IntoIterator<Item = AnyElement>,
) -> Div {
    let text = div().min_w_0().children(lines);
    let text = match text_width {
        Some(width) => text.w(width).flex_none(),
        None => text.flex_1(),
    };
    div()
        .flex()
        .items_start()
        .gap(px(7.))
        .children(leading)
        .child(text)
}

/// Result badges and leading marks share a size. Keep their width when a localized label needs to truncate.
pub(crate) fn command_glyph(icon: Icon, ink: Token) -> impl IntoElement {
    svg()
        .path(icon.path())
        .size(px(13.))
        .flex_none()
        .text_color(theme::color(ink))
}
