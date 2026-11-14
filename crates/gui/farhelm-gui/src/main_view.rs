//! The main window's root view: the sidebar with the session list beside the session pane, the connection's inert
//! states, the notice banners, and the overlay layer.
//!
//! The window is live only while the UI channel is connected. In every other state (connecting, reconnecting after a
//! break, displaced by another window, refused, or the helm gone) it shows that state instead of anything from the
//! helm, which is what makes it inert (SPEC.md, "Losing the connection to the helm" and "One GUI at a time"): the
//! session that held helm state was retired, so there is nothing stale to draw or act on.
//!
//! The open session's Delete and Rename shortcuts ([`DeleteSession`], [`RenameSession`]) and the quick switcher's
//! ([`QuickSwitcher`]) are handled on the window's ground, so they reach the session list from wherever focus is on
//! the page (a row, the terminal), and the list decides what they do. From inside a dialog or menu they reach these
//! handlers too, because `root::wrap` draws the overlay inside the ground, and each refuses while a layer is open
//! (`layers::any_open`), which is SPEC.md's "A visible modal keeps keyboard ownership".
//!
//! The live ground is a column: the posted notices' banners across the whole window, then the sidebar and the pane
//! side by side below them, so a banner pushes both down rather than covering the list's first row. The sidebar is the
//! old app's column (`.app-sidebar`): its default width, one level above the ground (`--bg-1`), with a hairline against
//! the pane.

use farhelm_helm_client::ChannelState;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div,
};

use crate::actions::{DeleteSession, QuickSwitcher, RenameSession};
use crate::components::{Banner, Button, HoverHelp, Role, Tier};
use crate::notices::{self, Notice};
use crate::session_list::{self, SessionList};
use crate::session_pane::SessionPane;
use crate::theme::metrics;
use crate::{render_count, root, session, theme};

/// The window's root view.
pub struct MainView {
    /// Keeps the theme following the operating system's appearance for this window's lifetime.
    _appearance: Subscription,
    /// This window's layer stack (which redraws this view, and so the overlay it draws, when an overlay changes), its
    /// focus state and the coordinator's focus recovery.
    root: root::Attachment,
    /// Redraws on a change of the connection's state and of the notice center.
    _observed: Vec<Subscription>,
    /// The sidebar's session list, kept for the window's life: it holds only per-UI-session state, which it starts
    /// over by itself after a reset.
    session_list: Entity<SessionList>,
    /// The pane beside the list, kept for the window's life like the list: it remakes the selected session's terminal
    /// by itself on every change of selection or UI session.
    session_pane: Entity<SessionPane>,
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
            root: root::attach(window, cx),
            _observed: observed,
            session_list: cx.new(SessionList::new),
            session_pane: cx.new(|cx| SessionPane::new(window, cx)),
        }
    }
}

#[cfg(test)]
impl MainView {
    /// The pane, for tests that reach the terminal it shows.
    pub(crate) fn session_pane(&self) -> &Entity<SessionPane> {
        &self.session_pane
    }

    /// The session list, for tests that ask it for a row's action the way its menu does.
    pub(crate) fn session_list(&self) -> &Entity<SessionList> {
        &self.session_list
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
            let sidebar = div()
                .w(metrics::SIDEBAR_WIDTH)
                .flex_none()
                .h_full()
                .overflow_hidden()
                .bg(theme::color(palette.bg_1))
                .border_r_1()
                .border_color(theme::color(palette.border_sidebar_edge))
                .child(self.session_list.clone());
            div()
                .flex()
                .flex_col()
                .size_full()
                .children(notices::posted(cx).into_iter().map(banner))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_1()
                        .min_h_0()
                        .child(sidebar)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .child(self.session_pane.clone()),
                        ),
                )
                .into_any_element()
        } else {
            inert(&state, cx)
        };
        let ground = div()
            // Taken whatever the list does with it, as a once-per-press shortcut's handler must be (`actions.rs`).
            .on_action(cx.listener(|main, _: &DeleteSession, window, cx| {
                main.session_list
                    .update(cx, |list, cx| list.delete_selected(window, cx));
            }))
            // Taken whatever the editor does with it, as a once-per-press shortcut's handler must be (`actions.rs`).
            .on_action(|_: &RenameSession, window, cx| {
                session_list::rename::open_for_selected(window, cx)
            })
            // Taken whatever the switcher does with it, as a once-per-press shortcut's handler must be (`actions.rs`).
            .on_action(|_: &QuickSwitcher, window, cx| {
                session_list::quick_switcher::open(window, cx)
            })
            .size_full()
            .bg(theme::color(palette.bg_0))
            .text_color(theme::color(palette.fg_0))
            .font_family(crate::assets::UI_FONT)
            .child(body);
        root::wrap(ground, &self.root, window, cx)
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
pub(crate) fn banner(notice: Notice) -> AnyElement {
    Banner::new(notice.text())
        .action(
            Button::new(
                SharedString::from(format!("notice-dismiss-{notice:?}")),
                "Dismiss",
                HoverHelp::tooltip("Dismiss this notice"),
                Role::Button,
            )
            .on_click(move |_, _, cx| notices::withdraw(notice, cx)),
        )
        .into_any_element()
}
