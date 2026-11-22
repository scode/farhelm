//! The red band across the top of a surface: the old stylesheet's `.banner`, with its `.banner-reclaim` button after
//! its text, and at most one more before that button.
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
    lead: Option<Button>,
    action: Option<Button>,
    flush_wrapped_actions: bool,
}

impl Banner {
    /// A band showing `content` (the app's words, or a sentence holding host text).
    pub fn new(content: impl IntoElement) -> Self {
        Banner {
            content: content.into_any_element(),
            lead: None,
            action: None,
            flush_wrapped_actions: false,
        }
    }

    /// The band's one button, after the text rather than at the far edge, as in the old app. Whatever tier the button
    /// had, it is drawn in [`Tier::OnBanner`]: no other tier reads on the red fill.
    pub fn action(mut self, button: Button) -> Self {
        self.action = Some(button.tier(Tier::OnBanner).help_below());
        self
    }

    /// A button before the band's [`Banner::action`], for a band that offers something besides its one action: a
    /// notice whose Dismiss follows what it offers (Open launcher). Drawn in [`Tier::OnBanner`] for the same reason.
    pub fn lead_action(mut self, button: Button) -> Self {
        self.lead = Some(button.tier(Tier::OnBanner).help_below());
        self
    }

    /// Align buttons that wrap below long notice text with its left edge, keeping the gap between inline items.
    pub(crate) fn flush_wrapped_actions(mut self) -> Self {
        self.flush_wrapped_actions = true;
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let (pad_y, pad_x) = metrics::BANNER_PAD;
        let flush = self.flush_wrapped_actions;
        let action = |button| {
            div()
                .when(!flush, |slot| slot.ml(metrics::BANNER_BUTTON_GAP))
                .child(button)
        };
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .when(flush, |band| band.gap_x(metrics::BANNER_BUTTON_GAP))
            .items_center()
            .py(pad_y)
            .px(pad_x)
            .bg(theme::color(palette.danger_bg))
            .text_color(theme::color(palette.on_fill))
            .text_size(metrics::TEXT_MD)
            .child(div().min_w_0().child(self.content))
            .when_some(self.lead, |band, button| band.child(action(button)))
            .when_some(self.action, |band, button| band.child(action(button)))
    }
}
