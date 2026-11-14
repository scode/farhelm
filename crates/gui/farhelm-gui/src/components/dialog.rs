//! The one dialog frame: a pinned header, a scrolling body and pinned answer buttons, as the old app's dialogs had.
//!
//! The old app's dialogs kept their title and their answers on screen while the body scrolled, and when even the title
//! and the answers did not fit a short window, the whole card scrolled instead (`.host-dialog-framed` in the old
//! stylesheet). That guarantee is what kept answers reachable at small window sizes, a bug class the old app fixed one
//! dialog at a time; here every dialog gets it from the frame. The frame only lays out: the layer manager places it,
//! draws the veil behind it and keeps focus inside it.
//!
//! A dialog may also have the shared header Close control SPEC.md gives editors such as Rename ("a shared title and
//! Close header"): the old `.dialog-close` "×" at the header's top right. Drawn in the header, it comes first in Tab
//! order, before the body's fields and the answers (SPEC.md for Rename: "Tab follows Close, the title field, Cancel,
//! Save"). Its label and help are each dialog's own, as they were in the old app ("Close rename", "Close rename; keep
//! the current title"), and so is what it does.
//!
//! NOTE: The old Close changed only its color on hover; here it is a ghost icon button, which gets the ghost tier's
//! hover fill. The old one was a 20-pixel "×" character; here it is the close icon at the icon buttons' size.

use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    point, px,
};

use super::{Button, HoverHelp, IconButton};
use crate::assets::Icon;
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
    close: Option<IconButton>,
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
            close: None,
        }
    }

    /// Give the frame the header Close control: `label` is its accessible name ("Close rename"), `help` its hover help,
    /// and `on_close` what it does, which is the dialog's own business (closing its layer, usually, with whatever its
    /// Cancel does to a draft). It is an ordinary button: a click or Enter or Space while it has focus runs `on_close`.
    ///
    /// `disabled` draws it unavailable, ignoring activation and out of Tab's reach, as a disabled button is. A dialog
    /// passes the same value it gives its Cancel: SPEC.md has Rename's "Close uses the same cancellation and busy-state
    /// rules as Cancel", and the old app disabled both while a rename was saving (`rename.rs` at reference commit
    /// `24bc0580`).
    pub fn close(
        mut self,
        label: impl Into<SharedString>,
        help: HoverHelp,
        disabled: bool,
        on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let id = format!("{}-close", self.id);
        self.close = Some(
            IconButton::new(id, Icon::Close, label, help, Role::Button)
                .disabled(disabled)
                .on_click(on_close),
        );
        self
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
            .child(heading)
            // `align-self: flex-start`: the Close sits level with the title, not centered on a two-line heading.
            .children(
                self.close
                    .map(|close| div().flex_shrink_0().self_start().child(close)),
            );
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
            .shadow(dialog_shadow(palette))
            .child(header)
            .child(body)
            .child(footer)
    }
}

/// `--shadow-dialog`: `0 10px 24px` in the token's color, under the dialog frame and under the one modal panel the old
/// app drew without that frame (the quick switcher's card), so the two cannot drift apart.
pub(crate) fn dialog_shadow(palette: &theme::Palette) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: theme::color(palette.shadow_dialog),
        offset: point(px(0.), px(10.)),
        blur_radius: px(24.),
        spread_radius: px(0.),
        inset: false,
    }]
}
