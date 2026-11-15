//! The main window's root view: the sidebar with the session list beside the session pane, the connection's inert
//! states, the notice banners, and the overlay layer.
//!
//! The window is live only while the UI channel is connected. In every other state (connecting, reconnecting after a
//! break, displaced by another window, refused, or the helm gone) it shows that state instead of anything from the
//! helm, which is what makes it inert (SPEC.md, "Losing the connection to the helm" and "One GUI at a time"): the
//! session that held helm state was retired, so there is nothing stale to draw or act on.
//!
//! The open session's Delete and Rename shortcuts ([`DeleteSession`], [`RenameSession`]) and the quick switcher's
//! ([`QuickSwitcher`]) are handled on the window's ground, so they reach the session pane's header (Delete) and the
//! session list (Rename, the switcher) from wherever focus is on the page (a row, the terminal), and those decide what
//! they do. From inside a dialog or menu they reach these handlers too, because `root::wrap` draws the overlay inside
//! the ground, and each refuses while a layer that holds the keyboard is open (`layers::focus_layer_open`), which is
//! SPEC.md's "A visible modal keeps keyboard ownership". A popover that takes no focus, such as a terminal link's
//! destination display, does not count: it owns no keys.
//!
//! The live ground is a column: the posted notices' banners across the whole window, then the sidebar and the pane
//! side by side below them, so a banner pushes both down rather than covering the list's first row. The sidebar
//! ([`Sidebar`]) draws its own column at the width the user chose, with the divider that changes it, whose grip the row
//! draws after the pane ([`Sidebar::grip`]). The pane keeps its floor ([`sidebar::MAIN_PANE_MIN_WIDTH`]); in a window
//! too narrow for both, the row scrolls sideways rather than crushing the pane, as the old `.app-shell` did. Only a
//! sideways wheel scrolls it, so a wheel over the list or the terminal scrolls only those.
//!
//! The main view also keeps the theme following the helm's stored appearance, beside the operating system's: it holds
//! the subscription for the window's life, and the Settings area decides what a change does
//! ([`settings::follow_stored_appearance`]). Together they give SPEC.md's rule ("System follows each device's current
//! appearance ... an explicit choice overrides it"); the theme module decides how a choice and the system resolve to
//! a palette.
//!
//! In the macOS window the sidebar's app bar and the session pane's top row are the window's title band
//! (`desktop::title_band`); this view draws the band's one window-wide press watcher, gives the connection screens a
//! band strip of their own above their content, and puts a strip of the band above any notice banner.

use farhelm_helm_client::ChannelState;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled, Subscription,
    Window, div,
};

use crate::actions::{DeleteSession, QuickSwitcher, RenameSession};
use crate::components::{Banner, Button, HoverHelp, Role, Tier};
use crate::desktop::title_band::{self, Row};
use crate::focus::Mark;
use crate::notices::{self, Notice};
use crate::session_list::{self, SessionList};
use crate::session_pane::{SessionPane, header};
use crate::sidebar::{self, Sidebar};
use crate::theme::metrics;
use crate::{launcher, layers, render_count, root, session, settings, theme};

/// The window's root view.
pub struct MainView {
    /// Keeps the theme following the operating system's appearance for this window's lifetime.
    _appearance: Subscription,
    /// Keeps the theme following the helm's stored appearance for this window's lifetime: at the first snapshot, on
    /// every change of the store (a no-op unless the choice changed), and again after a reset
    /// ([`settings::follow_stored_appearance`]).
    _appearance_choice: session::ObservingCurrent,
    /// This window's layer stack (which redraws this view, and so the overlay it draws, when an overlay changes), its
    /// focus state and the coordinator's focus recovery.
    root: root::Attachment,
    /// Redraws on a change of the connection's state and of the notice center.
    _observed: Vec<Subscription>,
    /// The sidebar's session list, which the sidebar keeps for the window's life (it holds only per-UI-session state,
    /// which it starts over by itself after a reset). A handle is kept here for the quick switcher, which this view
    /// opens and which tells the list what a pick opened, and for tests, which ask the list for a row's action the way
    /// its menu does.
    session_list: Entity<SessionList>,
    /// The sidebar around the list, kept for the window's life like the list.
    sidebar: Entity<Sidebar>,
    /// The pane beside the list, kept for the window's life like the list: it remakes the selected session's terminal
    /// by itself on every change of selection or UI session.
    session_pane: Entity<SessionPane>,
}

impl MainView {
    /// The main view, following the operating system's appearance, the layer manager, the connection, the notice
    /// center and the helm's stored appearance while it lives.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut observed = Vec::new();
        if let Some(connection) = session::connection(cx) {
            observed.push(cx.observe(&connection, |_, _, cx| cx.notify()));
        }
        if let Some(notices) = notices::entity(cx) {
            observed.push(cx.observe(&notices, |_, _, cx| cx.notify()));
        }
        let session_list = cx.new(SessionList::new);
        MainView {
            _appearance: theme::observe_window(window),
            _appearance_choice: session::observe_current(cx, |_, cx| {
                settings::follow_stored_appearance(cx)
            }),
            root: root::attach(window, cx),
            _observed: observed,
            sidebar: cx.new(|cx| Sidebar::new(session_list.clone(), cx)),
            session_list,
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
        let list = self.session_list.downgrade();
        let state = session::connection(cx)
            .map(|connection| connection.read(cx).state().clone())
            .unwrap_or(ChannelState::Connecting);
        let body = if state == ChannelState::Connected {
            let posted = notices::posted(cx);
            // In the macOS window a posted notice's banner would sit under the traffic lights, so a strip of the title
            // band goes above the banners while any is shown.
            let above_banners = (!posted.is_empty())
                .then(|| title_band::strip("above-banners", Row::AboveBanners))
                .flatten();
            div()
                .flex()
                .flex_col()
                .size_full()
                .children(above_banners)
                .children(posted.into_iter().map(banner))
                // In a narrow macOS window the app header leaves the sidebar's column and runs across the window's top
                // here, above the row that scrolls sideways (`Sidebar::floating_header`).
                .children(
                    self.sidebar
                        .update(cx, |sidebar, cx| sidebar.floating_header(window, cx)),
                )
                .child(
                    div()
                        .id("main-shell")
                        .relative()
                        .flex()
                        .flex_row()
                        .flex_1()
                        .min_h_0()
                        .overflow_x_scroll()
                        .restrict_scroll_to_axis()
                        .child(self.sidebar.clone())
                        .child(
                            div()
                                .id("main-pane")
                                .flex_1()
                                .min_w(sidebar::MAIN_PANE_MIN_WIDTH)
                                .h_full()
                                .child(self.session_pane.clone()),
                        )
                        // Drawn last, so it takes the pointer over the pane's edge. It is placed from the sidebar's
                        // width, which assumes the sidebar starts at this row's left edge and that the window redraws
                        // when the width changes (`Sidebar::grip`); rail expansion must keep both true.
                        .child(self.sidebar.update(cx, |sidebar, cx| sidebar.grip(cx))),
                )
                .into_any_element()
        } else {
            inert(&state, cx)
        };
        let ground = div()
            // The open session's header Delete, with its ordinary question (SPEC.md, "Desktop window chrome"), unless a
            // dialog, menu or question keeps the keyboard (a popover that never takes focus, such as a terminal link's
            // destination display, does not). Taken whatever the header does with it, as a once-per-press
            // shortcut's handler must be (`actions.rs`).
            .on_action(cx.listener(|main, _: &DeleteSession, window, cx| {
                if layers::focus_layer_open(window, cx) {
                    return;
                }
                let mark = Mark::take(cx);
                main.session_pane.update(cx, |pane, cx| {
                    pane.header_action(header::Action::Delete, &mark, window, cx)
                });
            }))
            // Taken whatever the editor does with it, as a once-per-press shortcut's handler must be (`actions.rs`).
            .on_action(|_: &RenameSession, window, cx| {
                session_list::rename::open_for_selected(window, cx)
            })
            // Taken whatever the switcher does with it, as a once-per-press shortcut's handler must be (`actions.rs`).
            // The switcher holds the list weakly, so that a pick can show the session it opened.
            .on_action(move |_: &QuickSwitcher, window, cx| {
                session_list::quick_switcher::open(list.clone(), window, cx)
            })
            .size_full()
            .bg(theme::color(palette.bg_0))
            .text_color(theme::color(palette.fg_0))
            .font_family(crate::assets::UI_FONT)
            .child(body)
            // Once for the whole window: it lets the title band pair a double-click with the press before it, wherever
            // that press was (`desktop::title_band`).
            .child(title_band::watch_presses());
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
    // SPEC.md, "Desktop window chrome": startup screens keep their content clear of the native controls. In the macOS
    // window a strip of the title band runs across the screen's top, as tall as the band, as the old window's startup
    // pages kept 48 pixels there (`padding-top: 48px`); unlike the old padding the strip also moves the window, which
    // with the app owning the title bar's drag would otherwise have no place to be grabbed until the helm connects.
    let content = div()
        .flex_1()
        .min_h_0()
        .w_full()
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
        .children(action);
    div()
        .size_full()
        .flex()
        .flex_col()
        .children(title_band::strip("connection-titlebar", Row::EmptyPane))
        .child(content)
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
    // What a notice offers besides dismissing it, before Dismiss. Exhaustive, so a new notice decides.
    let banner = match notice {
        Notice::ConnectionLost => Banner::new(notice.text()),
        // SPEC.md, "Creation": a refused launch the user had moved on from is offered by a notice, "and the launcher
        // returns when the user opens that notice".
        Notice::LaunchNotStarted => Banner::new(notice.text()).lead_action(
            Button::new(
                "notice-open-launcher",
                "Open launcher",
                HoverHelp::tooltip("Open launcher: show the refused launch with its settings and why it did not start"),
                Role::Button,
            )
            .on_click(|_, window, cx| {
                launcher::launch::open_waiting(window, cx);
            }),
        ),
    };
    banner
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
