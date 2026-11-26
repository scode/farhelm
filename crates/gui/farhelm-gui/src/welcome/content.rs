//! The same first actions appear in the empty pane and in the Welcome dialog. Each instance owns its focus handles.

use gpui::{
    App, Div, FocusHandle, FontWeight, InteractiveElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, svg,
};

use super::{GET_STARTED_URL, INTRODUCTION, TITLE};
use crate::actions::NEW_SESSION_SHORTCUT_NAME;
use crate::components::{Button, HoverHelp, Role, StatusMark, Tier, key_chip};
use crate::focus::Mark;
use crate::launcher::{HARNESS_PICKER_ORDER, entry};
use crate::layers::{self, LayerId};
use crate::{feedback, harness, hosts, session_list, theme};

pub(crate) const PANE_PREFIX: &str = "welcome-pane";
#[cfg(test)]
pub(crate) const PANE_START_ID: &str = "welcome-pane-start";

/// The empty pane's focus stays with these controls across redraws. The Start handle is also the window's home while
/// the listing proves the fleet empty.
pub(crate) struct WelcomeContent {
    prefix: &'static str,
    pub(crate) start: FocusHandle,
    host: FocusHandle,
    guide: FocusHandle,
    feedback: FocusHandle,
}

impl WelcomeContent {
    /// Each surface owns its focus handles and uses a distinct prefix so both can appear in one frame.
    pub(crate) fn new(prefix: &'static str, cx: &App) -> Self {
        Self {
            prefix,
            start: cx.focus_handle().tab_stop(true),
            host: cx.focus_handle().tab_stop(true),
            guide: cx.focus_handle().tab_stop(true),
            feedback: cx.focus_handle().tab_stop(true),
        }
    }

    /// Offer the same New entry point as the sidebar. Only an accepted open switches the sidebar back to Sessions.
    /// The host and feedback dialogs each return to the link that opened them.
    pub(crate) fn render(&self, cx: &mut App) -> Div {
        self.render_at(None, cx)
    }

    /// The dialog starts on its primary action when available and on the first link while a launch or host change
    /// disables that action. Both are already tab stops inside the body.
    pub(super) fn dialog_start(&self, can_start: bool) -> FocusHandle {
        if can_start {
            self.start.clone()
        } else {
            self.host.clone()
        }
    }

    /// The frame supplies the title. Host and feedback actions close this layer and use Help as their opener.
    pub(super) fn render_dialog(&self, layer: LayerId, opener: &FocusHandle, cx: &mut App) -> Div {
        self.render_at(Some((layer, opener)), cx)
    }

    /// A click that starts a new layer leaves the Welcome dialog first. In the pane, each link stays drawn and can
    /// receive focus after the layer closes. The guide simply opens a URL in either surface.
    fn render_at(&self, dialog: Option<(LayerId, &FocusHandle)>, cx: &mut App) -> Div {
        let layer = dialog.map(|(layer, _)| layer);
        let opener = dialog.map(|(_, opener)| opener);
        let close_dialog = move |window: &mut Window, cx: &mut App| {
            if let Some(layer) = layer {
                layers::close(layer, window, cx);
            }
        };
        let available = entry::new_available(cx);
        let palette = theme::palette(cx);
        let start = Button::new(
            format!("{}-start", self.prefix),
            "Start a session",
            HoverHelp::tooltip("Start a session"),
            Role::Button,
        )
        .tier(Tier::Primary)
        .focus(&self.start)
        .disabled(!available)
        .content(
            div()
                .h(px(22.))
                .px(px(6.))
                .flex()
                .items_center()
                .gap(px(6.))
                .child("Start a session")
                .children(
                    NEW_SESSION_SHORTCUT_NAME.map(|name| key_chip(name, palette.on_fill, palette)),
                ),
        )
        .on_click(move |_, window, cx| {
            // New can become unavailable after this button was drawn. Leave Welcome open if the launcher can't take over.
            if layer.is_some() && !entry::new_available(cx) {
                return;
            }
            close_dialog(window, cx);
            if entry::open_new(window, cx) {
                session_list::show_sessions(window, cx);
            }
        });
        let host = self
            .link("add-host", "Add a remote host", &self.host)
            .on_click({
                let opener = opener.unwrap_or(&self.host).clone();
                move |_, window, cx| {
                    close_dialog(window, cx);
                    hosts::add::open(&opener, &Mark::take(cx), window, cx);
                }
            });
        let guide = self
            .link("get-started", "Read Get started ↗", &self.guide)
            .on_click(|_, _, cx| cx.open_url(GET_STARTED_URL));
        let feedback = self
            .link("feedback", "Send feedback", &self.feedback)
            .on_click({
                let opener = opener.unwrap_or(&self.feedback).clone();
                move |_, window, cx| {
                    close_dialog(window, cx);
                    feedback::open(&opener, &Mark::take(cx), window, cx);
                }
            });

        div()
            .w_full()
            .max_w(px(420.))
            .min_w_0()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .child(marks(self.prefix, palette))
            // A future introduction video belongs above the title, inside the same width as the copy.
            .children(dialog.is_none().then(|| {
                div()
                    .id(format!("{}-title", self.prefix))
                    .role(Role::Heading)
                    .aria_label(TITLE)
                    .mt(px(14.))
                    .text_size(px(20.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child(TITLE)
            }))
            .child(
                div()
                    .mt(px(8.))
                    .mb(px(18.))
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(theme::color(palette.fg_dim))
                    .child(INTRODUCTION),
            )
            .child(start)
            .child(
                div()
                    .mt(px(14.))
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap_x(px(16.))
                    .gap_y(px(6.))
                    .child(host)
                    .child(guide)
                    .child(feedback),
            )
    }

    /// Keep the secondary actions in the same tab order and return path as the other Welcome controls.
    fn link(&self, id: &str, label: &'static str, focus: &FocusHandle) -> Button {
        Button::new(
            format!("{}-{id}", self.prefix),
            label,
            HoverHelp::tooltip(label),
            Role::Link,
        )
        .tier(Tier::Link)
        .focus(focus)
    }
}

/// Introduce the built-in agents in launcher order. The marks wrap as a group if the pane is too narrow for the row.
fn marks(prefix: &str, palette: &theme::Palette) -> impl gpui::IntoElement + use<> {
    let agents = HARNESS_PICKER_ORDER
        .into_iter()
        .filter(|agent| !agent.is_shell());
    let names = agents
        .clone()
        .map(|agent| agent.display_name())
        .collect::<Vec<_>>();
    div()
        .id(format!("{prefix}-marks"))
        .role(Role::Group)
        .aria_label(format!("Works with {}", names.join(", ")))
        .w_full()
        .flex()
        .flex_wrap()
        .justify_center()
        .gap(px(8.))
        .children(agents.map(|agent| {
            let name = agent.display_name();
            let glyph = harness::glyph(agent);
            let ink = harness::ink(glyph, palette).unwrap_or(palette.fg_1);
            StatusMark::new(
                format!("{prefix}-mark-{agent:?}"),
                name,
                HoverHelp::tooltip(name),
                Role::Image,
                "",
            )
            .described(false)
            .content(
                svg()
                    .path(harness::icon(glyph).path())
                    .size(px(16.))
                    .text_color(theme::color(ink)),
            )
        }))
}
