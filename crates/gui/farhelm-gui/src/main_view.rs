//! The main window's root view: the ground the sidebar and the session pane will be drawn on, the connection's inert
//! states, the notice banner, and the overlay layer.
//!
//! The window is live only while the UI channel is connected. In every other state (connecting, reconnecting after a
//! break, displaced by another window, refused, or the helm gone) it shows that state instead of anything from the
//! helm, which is what makes it inert (SPEC.md, "Losing the connection to the helm" and "One GUI at a time"): the
//! session that held helm state was retired, so there is nothing stale to draw or act on. The views of the end-to-end
//! slice (the session list, its row menu, the terminal) are added inside the live ground as they are built.

use farhelm_helm_client::ChannelState;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div,
};

use crate::components::{Button, HoverHelp, Role, Tier};
use crate::notices::{self, Notice};
use crate::theme::metrics;
use crate::{focus, layers, render_count, session, theme};

/// The window's root view.
pub struct MainView {
    /// Keeps the theme following the operating system's appearance for this window's lifetime.
    _appearance: Subscription,
    /// This window's layer stack; redraws this view, and so the overlay it draws, when an overlay changes.
    _layers: layers::Attachment,
    /// This window's focus state and the coordinator's focus recovery.
    focus: focus::Attachment,
    /// Redraws on a change of the connection's state and of the notice center.
    _observed: Vec<Subscription>,
}

impl MainView {
    /// The main view, following the operating system's appearance, the layer manager, the connection and the notice
    /// center while it lives.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut observed = Vec::new();
        if let Some(connection) = session::connection(cx) {
            observed.push(cx.observe(&connection, |_, _, cx| cx.notify()));
        }
        if let Some(notices) = notices::entity(cx) {
            observed.push(cx.observe(&notices, |_, _, cx| cx.notify()));
        }
        MainView {
            _appearance: theme::observe_window(window),
            _layers: layers::attach(window, cx),
            focus: focus::attach(window, cx),
            _observed: observed,
        }
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("main", cx);
        let palette = theme::palette(cx);
        let state = session::connection(cx)
            .map(|connection| connection.read(cx).state().clone())
            .unwrap_or(ChannelState::Connecting);
        let body = if state == ChannelState::Connected {
            div()
                .flex()
                .flex_col()
                .size_full()
                .children(
                    notices::posted(cx)
                        .into_iter()
                        .map(|notice| banner(notice, cx)),
                )
                .into_any_element()
        } else {
            inert(&state, cx)
        };
        focus::root(div(), &self.focus)
            .size_full()
            .bg(theme::color(palette.bg_0))
            .text_color(theme::color(palette.fg_0))
            .font_family(crate::assets::UI_FONT)
            .child(body)
            .children(layers::overlay(window, cx))
    }
}

/// What the window says while the channel is not connected, with the one action the state offers, if any.
fn inert(state: &ChannelState, cx: &App) -> AnyElement {
    let palette = theme::palette(cx);
    let (message, action): (SharedString, Option<Button>) = match state {
        ChannelState::Connecting | ChannelState::Connected => {
            ("Connecting to the helm…".into(), None)
        }
        ChannelState::Reconnecting => ("Reconnecting to the helm…".into(), None),
        ChannelState::Displaced => (
            "Another window took over this helm.".into(),
            Some(
                Button::new(
                    "connection-take-back",
                    "Take back",
                    HoverHelp::tooltip(
                        "Take this helm back from the window that took over; that window is then displaced",
                    ),
                    Role::Button,
                )
                .tier(Tier::Primary)
                .on_click(|_, _, cx| {
                    if let Some(connection) = session::connection(cx) {
                        connection.read(cx).take_back();
                    }
                }),
            ),
        ),
        ChannelState::CannotConnect { .. } => (
            "Farhelm cannot connect to its helm: the helm refused this window.".into(),
            Some(
                Button::new(
                    "connection-retry",
                    "Retry",
                    HoverHelp::tooltip("Try connecting to the helm again"),
                    Role::Button,
                )
                .tier(Tier::Primary)
                .on_click(|_, _, cx| {
                    if let Some(connection) = session::connection(cx) {
                        connection.read(cx).retry();
                    }
                }),
            ),
        ),
        ChannelState::HelmEnded => ("The helm stopped.".into(), None),
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .child(
            div()
                .text_size(metrics::TEXT_MD)
                .text_color(theme::color(palette.fg_1))
                .child(message),
        )
        .children(action)
        .into_any_element()
}

/// One posted notice as a banner across the top of the window, with the way to dismiss it.
///
/// The old app had no connection-lost notice, so this copies the nearest one it had: the red detach banner its
/// terminal painted when the view lost its session to another client (`.banner` with its "Take control" button,
/// `.banner-reclaim`, in the stylesheet at `24bc0580`). SPEC_impl.md, "Notices", asks for an existing style.
/// Like that banner, the text runs first and the button follows it rather than being pushed to the far edge. Visible to
/// the crate so the goldens can pin it.
pub(crate) fn banner(notice: Notice, cx: &App) -> AnyElement {
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
        .child(div().min_w_0().child(notice.text()))
        .child(
            div().ml(metrics::BANNER_BUTTON_GAP).child(
                Button::new(
                    SharedString::from(format!("notice-dismiss-{notice:?}")),
                    "Dismiss",
                    HoverHelp::tooltip("Dismiss this notice"),
                    Role::Button,
                )
                .tier(Tier::OnBanner)
                .on_click(move |_, _, cx| notices::withdraw(notice, cx)),
            ),
        )
        .into_any_element()
}
