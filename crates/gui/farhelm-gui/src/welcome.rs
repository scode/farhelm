//! Welcome in the empty pane and under Help, followed by the owner of the automatic announcement and upgrade dialogs.
//!
//! Welcome opens as a dialog only when asked for from Help. The window-start owner instead checks for pending
//! announcements once per UI session, then checks for upgrade highlights.

mod content;
#[cfg(test)]
pub(crate) use content::PANE_START_ID;
pub(crate) use content::{PANE_PREFIX, WelcomeContent};

#[cfg(test)]
pub(crate) mod tests;

use gpui::{
    App, AppContext as _, Context, FocusHandle, FontWeight, ParentElement as _, Render,
    Styled as _, Subscription, Task, Window, div, px,
};
use std::rc::Rc;

use crate::assets::Icon;
use crate::components::{DialogFrame, HoverHelp, command_icon_tile};
use crate::focus::{Mark, ReturnTo};
use crate::launcher::entry;
use crate::layers::{self, Dialog, LayerId, MainAction};
use crate::session::{self, PerSession};
use crate::theme;

/// The title shared by the empty pane and the Help dialog.
pub(crate) const TITLE: &str = "Welcome to Farhelm";

/// The Get started guide's first-session page.
pub(crate) const GET_STARTED_URL: &str = "https://farhelm.io/docs/get-started/first-session/";

/// The introduction shared by both Welcome surfaces.
const INTRODUCTION: &str = "One window for every coding agent you run, on your Mac and on any Linux box you can SSH \
                            to. Each agent keeps running on its machine when you close the window, and you pick it up \
                            where it was.";

/// Open Welcome after Help closes. Escape and Close return to the Help button. The same button receives focus
/// when the user closes Add host or Send feedback after opening either from Welcome.
pub fn open(
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let can_start = entry::new_available(cx);
    let view = cx.new(|cx| WelcomeDialog {
        opener: opener.clone(),
        content: WelcomeContent::new("welcome-dialog", cx),
        layer: None,
    });
    let start = view.read(cx).content.dialog_start(can_start);
    let layer = layers::open_dialog(
        Dialog {
            view: view.clone().into(),
            container: cx.focus_handle(),
            start,
            subject: None,
            main_action: None,
            keeps_focus_on_empty_click: false,
            returns_to: ReturnTo::Control(opener.clone()),
            on_dismiss: None,
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    // Complete the dialog in this update so its first frame draws the controls for the registered layer.
    view.update(cx, |view, _| view.layer = Some(layer));
    Some(layer)
}

/// Own the modal instance's controls and the Help button that opened it.
struct WelcomeDialog {
    opener: FocusHandle,
    content: WelcomeContent,
    layer: Option<LayerId>,
}

impl Render for WelcomeDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let layer = self.layer;
        let body = div()
            .flex()
            .justify_center()
            .children(layer.map(|layer| self.content.render_dialog(layer, &self.opener, cx)));
        let palette = theme::palette(cx);
        let heading = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(theme::metrics::TEXT_TITLE)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme::color(palette.fg_0))
            .child(command_icon_tile(Icon::App, palette))
            .child(TITLE);
        DialogFrame::new("welcome", TITLE, body, vec![])
            .width(px(520.))
            .heading(heading)
            .close(
                "Close Welcome",
                HoverHelp::tooltip("Close Welcome"),
                false,
                move |_, window, cx| {
                    if let Some(layer) = layer {
                        layers::dismiss(layer, window, cx);
                    }
                },
            )
    }
}

// ---- The window-start dialogs ----------------------------------------------------------------------------------------

/// Own the window-start decisions for announcements and upgrade highlights. The main view keeps one
/// for the window's life and starts the decisions over when the UI session resets.
pub struct StartDialogs {
    start: PerSession<Start>,
    /// Follows the connection, whose change is how a new UI session arrives.
    _connection: Option<Subscription>,
}

/// The signed-feed brick launch reserves the batch for the bell even if the first snapshot already has it.
/// The upgrade highlights check still runs.
pub(crate) struct AnnouncementsFromBell;

impl gpui::Global for AnnouncementsFromBell {}

/// What one UI session's start has decided and still owes.
#[derive(Default)]
struct Start {
    /// The mark taken when the owner first saw this UI session ([`Mark::at_ui_session_start`]); `None` before.
    mark: Option<Mark>,
    /// Whether the decision ran: once per UI session, whatever it came to.
    decided: bool,
    /// The current session owns the window side of the lookup. Resetting drops it before a later session can use
    /// its answer.
    _highlights: Option<Task<()>>,
}

impl StartDialogs {
    /// The owner for the window being built. A UI session that exists already (the snapshot came before the window)
    /// is seen here, before the window's first frame draws it; the decision itself runs once the window exists, since
    /// nothing may open inside the window's construction (SPEC_impl.md, "GPUI cookbook: traps").
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let connection = session::connection(cx).map(|connection| {
            cx.observe_in(&connection, window, |owner: &mut Self, _, window, cx| {
                owner.see(cx);
                cx.defer_in(window, Self::decide);
            })
        });
        let mut owner = StartDialogs {
            start: PerSession::default(),
            _connection: connection,
        };
        owner.see(cx);
        cx.defer_in(window, Self::decide);
        owner
    }

    /// Take the window-start mark if this is the first sight of the current UI session. A connection change that brings
    /// no new session (a state change, the window going inert) takes nothing: the mark belongs to the session, told
    /// apart by its identity through [`PerSession`], not to whichever callback came first.
    fn see(&mut self, cx: &App) {
        let live = session::connection(cx)
            .is_some_and(|connection| connection.read(cx).session().is_some());
        let start = self.start.current(cx);
        if live && start.mark.is_none() {
            start.mark = Some(Mark::at_ui_session_start(cx));
        }
    }

    /// Run the automatic dialog checks once for this UI session.
    ///
    /// Deferred from wherever the session was seen to the end of that update, after every other view has heard of the
    /// new session (the session pane has made its terminal home by then). The decision runs with the mark taken when
    /// the session was seen, so a press that lands in between refuses the open.
    fn decide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start = self.start.current(cx);
        let Some(mark) = start.mark.filter(|_| !start.decided) else {
            return;
        };
        start.decided = true;
        // Agent runs can suppress the automatic dialogs. Help still opens a dialog when the user asks.
        if crate::agent_startup::start_dialogs_off(cx) {
            return;
        }
        self.remaining_dialogs(mark, window, cx);
    }

    /// Upgrade highlights follow the automatic announcements. A queued dismissal continues only within
    /// the connection session that supplied the batch.
    fn remaining_dialogs(&mut self, mark: Mark, window: &mut Window, cx: &mut Context<Self>) {
        let store = session::current_store(cx)
            .as_ref()
            .map(|store| store.entity_id());
        let owner = cx.weak_entity();
        let after: MainAction = Rc::new(move |window, cx| {
            if session::current_store(cx)
                .as_ref()
                .map(|store| store.entity_id())
                == store
            {
                let _ = owner.update(cx, |owner, cx| {
                    let task = crate::whats_new::check(Mark::take(cx), window, cx);
                    owner.start.current(cx)._highlights = task;
                });
            }
        });
        if cx.try_global::<AnnouncementsFromBell>().is_some()
            || crate::announcements::open_pending(&mark, after, window, cx).is_none()
        {
            let task = crate::whats_new::check(mark, window, cx);
            self.start.current(cx)._highlights = task;
        }
    }
}
