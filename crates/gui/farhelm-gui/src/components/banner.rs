//! The red band across the top of a surface: the old stylesheet's `.banner`, with at most one `.banner-reclaim`
//! button after its text.
//!
//! The old app had one such band, the terminal's detach banner with its Take control. Here it carries the terminal's
//! status line and the notice center's banners, and both draw it through this one builder so the two cannot drift
//! apart. It is a container, not a control: it takes no label, hover help or role of its own, because the text is its
//! content and the button it carries is a [`Button`] with all three.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div,
    prelude::FluentBuilder as _,
};

use super::{Button, Tier};
use crate::theme::{self, metrics};

/// A red band holding `content` and, optionally, one button, which the band draws in [`Tier::OnBanner`], the tier
/// made for this fill.
#[derive(IntoElement)]
pub struct Banner {
    content: AnyElement,
    action: Option<Button>,
}

impl Banner {
    /// A band showing `content` (the app's words, or a sentence holding host text).
    pub fn new(content: impl IntoElement) -> Self {
        Banner {
            content: content.into_any_element(),
            action: None,
        }
    }

    /// The band's one button, after the text rather than at the far edge, as in the old app. Whatever tier the button
    /// had, it is drawn in [`Tier::OnBanner`]: no other tier reads on the red fill.
    pub fn action(mut self, button: Button) -> Self {
        self.action = Some(button.tier(Tier::OnBanner));
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let (pad_y, pad_x) = metrics::BANNER_PAD;
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .py(pad_y)
            .px(pad_x)
            .bg(theme::color(palette.danger_bg))
            .text_color(theme::color(palette.on_fill))
            .text_size(metrics::TEXT_MD)
            .child(div().min_w_0().child(self.content))
            .when_some(self.action, |band, button| {
                band.child(div().ml(metrics::BANNER_BUTTON_GAP).child(button))
            })
    }
}
