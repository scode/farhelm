//! Settings shows related rows inside one card. Padding and separators live here so a page can build its controls
//! without handling the card's geometry.

use farhelm_proto::LaunchHarness;
use gpui::{
    AnyElement, App, Div, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Role, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _,
    Svg, div, prelude::FluentBuilder as _, px, svg,
};

use crate::theme::{self, metrics};

/// Reuse the session list's drawing and identity ink when an agent leads a Settings row.
pub(super) fn agent_mark(agent: LaunchHarness, cx: &App) -> Svg {
    let palette = theme::palette(cx);
    let mark = crate::harness::glyph(agent);
    svg()
        .path(crate::harness::icon(mark).path())
        .size(px(16.))
        .flex_none()
        .text_color(theme::color(
            crate::harness::ink(mark, palette).unwrap_or(palette.fg_1),
        ))
}

/// Draw the section's content on the card ground, one line of padding per row and a hairline between rows.
pub(super) fn rows<T: IntoElement>(rows: impl IntoIterator<Item = T>, cx: &App) -> Div {
    let palette = theme::palette(cx);
    surface(cx).children(rows.into_iter().enumerate().map(|(index, row)| {
        div()
            .px(px(14.))
            .py(px(10.))
            .min_w_0()
            .when(index != 0, |row| {
                row.border_t_1()
                    .border_color(theme::color(palette.border_dim))
            })
            .child(row)
    }))
}

/// The outline and ground, for controls whose own focus styles determine their padding.
pub(super) fn surface(cx: &App) -> Div {
    let palette = theme::palette(cx);
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .rounded(px(6.))
        .border_1()
        .border_color(theme::color(palette.border_dim))
        .bg(theme::color(palette.card))
}

/// A card's title, optional note and action share a heading. The action stays at the end as the note wraps.
pub(super) fn heading(
    title: &'static str,
    note: Option<&'static str>,
    action: Option<AnyElement>,
    cx: &App,
) -> Div {
    let palette = theme::palette(cx);
    div()
        .flex()
        .justify_between()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .min_w_0()
                .flex()
                .items_start()
                .gap_x(px(8.))
                .child(
                    div()
                        .flex_none()
                        .text_size(metrics::TEXT_MD)
                        .line_height(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .children(note.map(|note| {
                    div()
                        .min_w_0()
                        .text_size(metrics::TEXT_SM)
                        .line_height(px(18.))
                        .text_color(theme::color(palette.fg_1))
                        .child(note)
                })),
        )
        .children(action.map(|action| div().flex_none().child(action)))
}

/// Keep field captions below the card heading in the type hierarchy. The unsaved dot still belongs to the caption.
pub(super) fn caption(label: &'static str, edited: bool, palette: &theme::Palette) -> Div {
    div()
        .text_size(metrics::TEXT_SM)
        .line_height(px(17.))
        .text_color(theme::color(palette.fg_1))
        .child(super::editing::caption(label, edited.then_some(palette)))
}

/// Secondary typography shared by help paragraphs and their decorative marks.
pub(super) fn help_style(cx: &App) -> Div {
    div()
        .text_size(metrics::TEXT_SM)
        .line_height(px(17.))
        .text_color(theme::color(theme::palette(cx).fg_1))
}

/// Keep the visible help readable in the accessibility tree. Label values supply the text to platform adapters.
pub(super) fn help(
    id: impl Into<ElementId>,
    description: impl Into<SharedString>,
    cx: &App,
) -> Stateful<Div> {
    let text = description.into();
    help_style(cx)
        .id(id)
        .role(Role::Label)
        .aria_value(text.clone())
        .child(text)
}

/// A switch title and its visible description share a wrapping column. Keep the type scale the same across cards
/// whether the toggle owns the whole row or stands beside a separate preview control.
pub(super) fn switch_copy(title: impl IntoElement, description: &'static str, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .min_w_0()
        .child(
            div()
                .text_size(px(13.))
                .line_height(px(18.))
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
        .children((!description.is_empty()).then(|| help("settings-switch-help", description, cx)))
}
