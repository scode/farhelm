//! The main window's root view: the sidebar with the session list beside the session pane, the connection's inert
//! states, the notice banners, and the overlay layer.
//!
//! The window is live only while the UI channel is connected. In every other state (connecting, reconnecting after a
//! break, displaced by another window, refused, or the helm gone) it shows that state instead of anything from the
//! helm, which is what makes it inert (SPEC.md, "Losing the connection to the helm" and "One GUI at a time"): the
//! session that held helm state was retired, so there is nothing stale to draw or act on.
//!
//! The open session's Delete and Rename shortcuts ([`DeleteSession`], [`RenameSession`]), the quick switcher's
//! ([`QuickSwitcher`]) and the template list's ([`TemplateList`], the switcher in its templates-only mode) are handled
//! on the window's ground, so they reach the session pane's header (Delete) and the session list (Rename, the switcher)
//! from wherever focus is on the page (a row, the terminal), and those decide what they do. From inside a dialog or
//! menu they reach these handlers too, because `root::wrap` draws the overlay inside the ground, and each refuses while
//! a layer that holds the keyboard is open (`layers::focus_layer_open`), which is SPEC.md's "A visible modal keeps
//! keyboard ownership". A popover that takes no focus, such as a terminal link's destination display, does not count:
//! it owns no keys.
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
//! The Keyboard shortcuts overview's chord ([`ShowKeyboardShortcuts`]) is handled on the ground like the session
//! shortcuts and the switcher's above, and the overview refuses it while a layer holds the keyboard as they do
//! ([`crate::shortcuts`]).
//!
//! The quick-launch sequence's prefix ([`BeginSequence`]) is handled on the ground too, so it reaches the templates
//! module from a terminal, a row or empty space; the templates module refuses it while a dialog keeps the keyboard. The
//! main pane is what its pill is centered over ([`crate::templates::armed`]): the view draws an anchor there in every
//! connected state, session open or not, so a pill shown on it stays put when a session opens or closes. The ground also
//! draws the sequence's one window-wide press watcher, since a click anywhere cancels it. The ⚡ chord ([`QuickLaunch`])
//! and the number keys ([`TemplateNumberKey`]) are handled on the ground for the same reason, and the templates module's
//! dispatcher decides what they do.
//!
//! The main view also keeps the owner of the dialogs SPEC.md opens by themselves when the window starts or resets, the
//! Welcome card and the first batch of announcements ([`welcome::StartDialogs`]), for the window's life; it is made
//! after the session pane, so the selected session's terminal is home by the time a card opens over it. It keeps the
//! status sounds' owner ([`sounds::StatusSounds`]) for the window's life the same way, over the sidebar's session list,
//! whose query tells it when the sidebar's view changes.
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

use crate::actions::{BeginSequence, QuickLaunch, ShowKeyboardShortcuts, TemplateNumberKey};
use crate::actions::{DeleteSession, NewSession, QuickSwitcher, RenameSession, TemplateList};
use crate::components::{Banner, Button, HoverHelp, Role, Tier};
use crate::desktop::linux_title_bar;
use crate::desktop::title_band::{self, Row};
use crate::focus::Mark;
use crate::layers::Anchor;
use crate::notices::{self, Notice};
use crate::session_list::quick_switcher::Mode;
use crate::session_list::{self, SessionList};
use crate::session_pane::{SessionPane, header};
use crate::sidebar::{self, Sidebar};
use crate::templates;
use crate::theme::metrics;
use crate::{launcher, layers, render_count, root, session, settings, sounds, theme, welcome};

/// The window's root view.
pub struct MainView {
    /// Keeps the theme following the operating system's appearance for this window's lifetime.
    _appearance: Subscription,
    /// Redraws the window when the compositor's answer about decorations changes, so the Linux window's own title bar
    /// appears or goes with it (`desktop::linux_title_bar`).
    _decorations: Subscription,
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
    /// The owner of the window-start dialogs (the Welcome card and the first batch of announcements), kept for the
    /// window's life; it starts over by itself with each UI session.
    _start_dialogs: Entity<welcome::StartDialogs>,
    /// The owner of the status sounds, kept for the window's life; it starts over by itself with each UI session.
    _sounds: Entity<sounds::StatusSounds>,
    /// The main pane's anchor as last drawn, which the sequence pill is centered over (the module docs); `None` before
    /// the first connected frame. Kept weakly by the anchor itself, so a pill on it goes when the pane is not drawn.
    pane_anchor: Option<Anchor>,
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
        let status_sounds = cx.new(|cx| sounds::StatusSounds::new(&session_list, window, cx));
        // The pane makes New home when there are no sessions, so it holds New's handle.
        let new = session_list.read(cx).new_focus().clone();
        let root = root::attach(window, cx);
        // The agent approval cards' slot, once per window, as the layer stack it belongs to is attached.
        crate::approvals::region::install(window, cx);
        MainView {
            _appearance: theme::observe_window(window),
            _decorations: linux_title_bar::follow_decorations(window),
            _appearance_choice: session::observe_current(cx, |_, cx| {
                settings::follow_stored_appearance(cx)
            }),
            root,
            _observed: observed,
            sidebar: cx.new(|cx| Sidebar::new(session_list.clone(), cx)),
            session_list,
            session_pane: cx.new(|cx| SessionPane::new(new, window, cx)),
            // After the session pane, whose first sync is deferred the same way and so runs first.
            _start_dialogs: cx.new(|cx| welcome::StartDialogs::new(window, cx)),
            _sounds: status_sounds,
            pane_anchor: None,
        }
    }

    /// The session list, for a hand-off that starts outside the list and must still show Sessions
    /// ([`session_list::show_sessions`]), and for tests that ask it for a row's action the way its menu does.
    pub(crate) fn session_list(&self) -> &Entity<SessionList> {
        &self.session_list
    }
}

#[cfg(test)]
impl MainView {
    /// The pane, for tests that reach the terminal it shows.
    pub(crate) fn session_pane(&self) -> &Entity<SessionPane> {
        &self.session_pane
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("main", cx);
        let palette = theme::palette(cx);
        let list = self.session_list.downgrade();
        let new_list = list.clone();
        let state = session::connection(cx)
            .map(|connection| connection.read(cx).state().clone())
            .unwrap_or(ChannelState::Connecting);
        let body = if state == ChannelState::Connected {
            // Under its own key, apart from the pane's id, as the anchor's element state must be.
            let pane_anchor = Anchor::for_element("main-pane-anchor", window, cx);
            let pane_tracker = pane_anchor.track();
            self.pane_anchor = Some(pane_anchor.clone());
            // The approval region centers its cards in this pane and reaches at most its height.
            crate::approvals::region::set_pane(pane_anchor.clone(), cx);
            let posted: Vec<Notice> = notices::posted(cx)
                .into_iter()
                .filter(|notice| is_banner(*notice))
                .collect();
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
                                .debug_selector(|| "main-pane".into())
                                .relative()
                                .flex_1()
                                .min_w(sidebar::MAIN_PANE_MIN_WIDTH)
                                .h_full()
                                .child(self.session_pane.clone())
                                .child(pane_tracker),
                        )
                        // Drawn last, so it takes the pointer over the pane's edge. It is placed from the sidebar's
                        // shown width, which assumes the sidebar starts at this row's left edge and that the window
                        // redraws when that width changes (`Sidebar::grip`).
                        .child(
                            self.sidebar
                                .update(cx, |sidebar, cx| sidebar.grip(window, cx)),
                        ),
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
            .on_action({
                let list = list.clone();
                move |_: &QuickSwitcher, window, cx| {
                    session_list::quick_switcher::open(list.clone(), Mode::Sessions, window, cx)
                }
            })
            // The template list is the switcher listing templates only; taken the same way, for the same reason.
            .on_action(move |_: &TemplateList, window, cx| {
                session_list::quick_switcher::open(list.clone(), Mode::Templates, window, cx)
            })
            // Taken whatever New does with it, as a once-per-press shortcut's handler must be (`actions.rs`). New
            // itself refuses while a dialog or menu is open, a launch is pending or a host change is in flight
            // (SPEC.md: "unless a modal is open or New is disabled"); through the list, so that an accepted New
            // leaves the Hosts panel for Sessions, as the heading's New and the switcher's row do.
            .on_action(move |_: &NewSession, window, cx| {
                session_list::open_new(&new_list, None, window, cx);
            })
            // Taken whatever the overview does with it, as a once-per-press shortcut's handler must be (`actions.rs`).
            // The interceptor dispatches the physical-key row to the focused element, a terminal or a dialog's control
            // included, so it bubbles up to here like the bound shortcuts above.
            .on_action(|_: &ShowKeyboardShortcuts, window, cx| {
                crate::shortcuts::open_from_chord(window, cx)
            })
            // Taken whatever the sequence does with it, as a once-per-press shortcut's handler must be (`actions.rs`).
            .on_action(cx.listener(|main, _: &BeginSequence, window, cx| {
                templates::begin_sequence(main.pane_anchor.clone(), window, cx)
            }))
            // The terminal text size shortcut works wherever focus is while a terminal is open (SPEC.md, "Terminal
            // experience"); the terminal area steps every open terminal together.
            .on_action(|_: &crate::actions::IncreaseTerminalTextSize, _, cx| {
                if crate::terminal::text_size::any_terminal_open(cx) {
                    crate::terminal::text_size::step(1, cx);
                }
            })
            .on_action(|_: &crate::actions::DecreaseTerminalTextSize, _, cx| {
                if crate::terminal::text_size::any_terminal_open(cx) {
                    crate::terminal::text_size::step(-1, cx);
                }
            })
            // Taken whatever the quick launch does with it, as a once-per-press shortcut's handler must be
            // (`actions.rs`); the dispatcher decides whether it may act here at all.
            .on_action(|_: &QuickLaunch, window, cx| templates::quick_launch(window, cx))
            // A number key's chord, the same way: it reaches the ground from wherever focus is, since the physical-key
            // table dispatches it to the focused element (`actions.rs`).
            .on_action(|key: &TemplateNumberKey, window, cx| {
                templates::number_key(key.digit, window, cx)
            })
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::color(palette.bg_0))
            .text_color(theme::color(palette.fg_0))
            .font_family(crate::assets::UI_FONT)
            // The Linux window's own title bar, where the compositor draws none, above everything else.
            .children(linux_title_bar::bar(
                "main-title-bar",
                crate::bootstrap::WINDOW_TITLE,
                "Close Farhelm",
                window,
                cx,
            ))
            .child(div().flex_1().min_h_0().w_full().child(body))
            // Once for the whole window: it lets the title band pair a double-click with the press before it, wherever
            // that press was (`desktop::title_band`).
            .child(title_band::watch_presses())
            // Once for the whole window: a press anywhere cancels an armed sequence (`templates::armed`).
            .child(templates::watch_sequence_presses());
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

/// Whether `notice` shows as a banner across the top of the window. Exhaustive, so a new notice decides where it is
/// drawn: a late answer to an approval card shows in the approval region above its waiting list, as the old app drew it
/// (`approvals.rs` at `24bc0580`), not across the window.
fn is_banner(notice: Notice) -> bool {
    match notice {
        Notice::ConnectionLost
        | Notice::LaunchNotStarted
        | Notice::LaunchMayStillStart
        | Notice::EnvironmentLaunchUnknown => true,
        Notice::LateAnswer { .. } => false,
    }
}

/// A launch notice's banner: its words with Open launcher, which brings back the launcher waiting behind it
/// (`launcher::launch::open_waiting`), hovering as `help` says.
fn open_launcher_banner(notice: Notice, help: &'static str) -> Banner {
    Banner::new(notice.text()).lead_action(
        Button::new(
            "notice-open-launcher",
            "Open launcher",
            HoverHelp::tooltip(help),
            Role::Button,
        )
        .on_click(|_, window, cx| {
            launcher::launch::open_waiting(window, cx);
        }),
    )
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
        // The connection-lost notice offers nothing but Dismiss. A late answer is never drawn as a banner
        // ([`is_banner`]); were it one, its text and Dismiss would be all it offers too.
        Notice::ConnectionLost | Notice::LateAnswer { .. } => Banner::new(notice.text()),
        // SPEC.md, "Creation": a refused launch the user had moved on from is offered by a notice, "and the launcher
        // returns when the user opens that notice".
        Notice::LaunchNotStarted => open_launcher_banner(
            notice,
            "Open launcher: show the refused launch with its settings and why it did not start",
        ),
        // A launch that may still start is offered the same way (its launcher holds the helm's timeout reply).
        Notice::LaunchMayStillStart => open_launcher_banner(
            notice,
            "Open launcher: show the launch with its settings and the helm's reply",
        ),
        // So is an environment launch whose end is unknown (its launcher holds what is known of it).
        Notice::EnvironmentLaunchUnknown => open_launcher_banner(
            notice,
            "Open launcher: show the launch with its settings and what is known of how it ended",
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
