//! The one dialog frame: a pinned header, a scrolling body and pinned answer buttons, as the old app's dialogs had.
//!
//! The old app's dialogs kept their title and their answers on screen while the body scrolled, and when even the title
//! and the answers did not fit a short window, the whole card scrolled instead (`.host-dialog-framed` in the old
//! stylesheet). That guarantee is what kept answers reachable at small window sizes, a bug class the old app fixed one
//! dialog at a time; here every dialog gets it from the frame. The frame only lays out: the layer manager places it,
//! draws the veil behind it and keeps focus inside it.

use gpui::{
    AnyElement, App, BoxShadow, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    point, px,
};

use super::Button;
use crate::host_text::Line;
use crate::theme::{self, metrics};

/// The body's floor before the whole frame starts scrolling instead: about three lines, as the old `min-height: 3em`.
const BODY_MIN_HEIGHT: f32 = 36.;

/// A dialog's frame. The title is the accessible name, and is always the app's own words; the summary may name a host
/// value through a [`Line`], which then is a labelled node of its own inside the dialog. The answers are drawn
/// right-aligned in the order given, the old footer's order (Cancel first, the main action last).
#[derive(IntoElement)]
pub struct DialogFrame {
    id: SharedString,
    title: SharedString,
    summary: Option<Line>,
    body: AnyElement,
    answers: Vec<Button>,
}

impl DialogFrame {
    /// A frame titled `title` around `body`, with `answers` pinned at the bottom.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        body: impl IntoElement,
        answers: Vec<Button>,
    ) -> Self {
        DialogFrame {
            id: id.into(),
            title: title.into(),
            summary: None,
            body: body.into_any_element(),
            answers,
        }
    }

    /// A one-line summary under the title (the old `.dialog-summary`): app wording as a string, or a
    /// [`HostSentence`](crate::host_text::HostSentence) when it names a host value (the session a delete removes).
    pub fn summary(mut self, summary: impl Into<Line>) -> Self {
        self.summary = Some(summary.into());
        self
    }
}

impl RenderOnce for DialogFrame {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let (pad_y, pad_x) = metrics::DIALOG_PAD;
        let (foot_y, foot_x) = metrics::DIALOG_FOOTER_PAD;
        let max_height = window.viewport_size().height - metrics::DIALOG_VERTICAL_MARGIN * 2.;

        let mut heading = div().flex_1().min_w_0().child(
            div()
                .text_size(metrics::TEXT_TITLE)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::color(palette.fg_0))
                .child(self.title.clone()),
        );
        if let Some(summary) = self.summary {
            heading = heading.child(
                div()
                    .mt(px(6.))
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.fg_1))
                    .child(summary),
            );
        }
        let header = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(14.))
            .py(pad_y)
            .px(pad_x)
            .bg(theme::color(palette.control_hover))
            .border_b_1()
            .border_color(theme::color(palette.border_dim))
            .child(heading);
        let body = div()
            .id(ElementId::Name(format!("{}-body", self.id).into()))
            .debug_selector(|| format!("{}-body", self.id))
            .flex_shrink_1()
            .min_h(px(BODY_MIN_HEIGHT))
            .overflow_y_scroll()
            .py(pad_y)
            .px(pad_x)
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_0))
            .child(self.body);
        let footer = div()
            .debug_selector(|| format!("{}-answers", self.id))
            .flex()
            .flex_shrink_0()
            .justify_end()
            .items_center()
            .flex_wrap()
            .gap(px(8.))
            .py(foot_y)
            .px(foot_x)
            .bg(theme::color(palette.bg_1))
            .border_t_1()
            .border_color(theme::color(palette.border_dim))
            .children(self.answers);

        div()
            .id(ElementId::Name(self.id.clone()))
            .debug_selector(|| self.id.to_string())
            .role(Role::Dialog)
            .aria_label(self.title)
            .flex()
            .flex_col()
            .w(metrics::DIALOG_WIDTH)
            .max_w_full()
            .max_h(max_height)
            .overflow_y_scroll()
            .bg(theme::color(palette.bg_2))
            .border_1()
            .border_color(theme::color(palette.border_dialog))
            .rounded(metrics::RADIUS_MD)
            .shadow(vec![BoxShadow {
                color: theme::color(palette.shadow_dialog),
                offset: point(px(0.), px(10.)),
                blur_radius: px(24.),
                spread_radius: px(0.),
                inset: false,
            }])
            .child(header)
            .child(body)
            .child(footer)
    }
}
