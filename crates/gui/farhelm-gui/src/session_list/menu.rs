//! A session row's actions menu: the ⋯ button at the end of the row, and the menu it opens below itself.
//!
//! Ported from the old app's `list/row.rs` (the menu toggle, `menu_label`, `menu_header_summary_parts` and the Delete
//! item) and the stylesheet's `.session-row-menu` and `.session-row-menu-panel` rules at the reference commit. The menu
//! is a popover of the layer manager, anchored to its row's button through an [`Anchor`] kept under the session's key,
//! so an open menu moves with its session when the list reorders and closes when the row is no longer drawn (SPEC.md,
//! "Things changing underneath" and "Nothing closes a dialog … it did not open").
//!
//! The menu's header is the shared [`MenuHeader`], which the host menu draws too: it names the session and says, in one
//! line, which agent it runs and its status. Below it come the commands, in a [`Role::Menu`] of their own, so the
//! header is not read out as one of them. Up and Down walk the commands (the layer manager's menu context), and the
//! pointer moves the same highlight.
//!
//! The menu names its button as where closing returns focus ([`focus::ReturnTo::Control`]), so Escape and every command
//! chosen from it leave focus on the button, also on macOS, where the click that opened the menu left the button
//! unfocused (the old app named the button through `RETURN_FOCUS`). When the button is no longer drawn, focus goes back
//! to what held it before the menu opened, or, when that was the button itself (a click focuses it on Linux), home.
//!
//! Rename comes first, as in the old app. Choosing it closes the menu, which leaves focus on the menu's button, and
//! opens the Rename editor ([`super::rename`]), which returns focus there when it closes. Its hover help names the
//! Rename shortcut on the open session's row only, the one row the shortcut acts on (SPEC.md, "Desktop window
//! chrome"). Like Delete, it is unavailable while the session has a request in flight.
//!
//! Right after Rename, in the same group with no separator between them, a live row offers Mark read or Mark unread,
//! whichever its seen state calls for ([`super::seen`]); a row that offers neither omits the item, so the commands
//! after it move up. Choosing it closes the menu, which leaves focus on the menu's button, and then sends the request.
//! In the same group come Pin to top or Unpin, then Move up and Move down where the row has a displayed pin that way
//! ([`super::pins`]), each closing the menu the same way before its request goes out, so focus stays on the moved row's
//! button.
//!
//! After a separator come the launcher's two, Clone and Replace with…, in the old menu's group of "make a new
//! session from this one" commands, ahead of Replace. Choosing one closes the menu, which leaves focus on its button,
//! then opens the launcher on the row's session ([`launcher::entry::open_clone`],
//! [`launcher::entry::open_replace_with`]), which returns focus to the button when it closes. Both are unavailable
//! while the session's commands are refused (below), and while the launcher cannot open ([`launcher_wait`]: a launch
//! pending or a host change in flight), when their help and description say why. A press drawn a frame early opens
//! nothing: the entry points refuse while the launcher cannot open, and the click checks the refusal itself. The old
//! menu offered no Restart with, which the open session's header offers in its Restart menu
//! (`session_pane/header.rs`).
//!
//! Then come the lifecycle commands in the old menu's groups, each after a separator: Replace ([`super::replace`]),
//! Stop ([`super::stop`]) and Delete ([`super::delete`]), all three in the danger red. Choosing one closes the menu
//! before its question opens beside the same button (the old app turned the menu into the question; here the two are
//! separate layers, one after the other), or before its request goes out when it asks nothing. While that question is
//! open the button is inert: a click on it neither reopens the menu nor answers the question, and the question's own
//! Cancel, Escape or a press elsewhere is the way out. While the session has any request in flight, or the YOLO
//! question its Replace met is open, every command here is unavailable ([`refused`]; SPEC.md: "an open lifecycle
//! confirmation or a pending request refuses another lifecycle action"); opening the session stays possible, and other
//! sessions' menus are untouched. Replace also waits for a host change ([`launch_wait`]). A refusal or a notice from
//! the helm shows inside the menu as well as on the row, since the open menu covers the row's own copy
//! ([`super::outcome`]).
//!
//! **The button's reveal.** The button is drawn on every row but shows only while there is a reason to: the pointer is
//! over its row, any control in its row holds focus, the row is the open session's, or the button's own menu or
//! question is open. Otherwise it is transparent but still there to click and to reach with Tab. These are the old
//! `.session-row-menu` rules (`:hover`, `:focus-within`, `.selected` and `[aria-expanded="true"]` on the row); an open
//! menu keeps the button shown with the pointer elsewhere, since the pointer leaves the row to reach the menu's
//! commands. The row's hover is recorded by its list item (`super::row::RowHover`) rather than left to a hover style,
//! so that a key typed while the pointer rests on the row does not hide the button (GPUI's hover styles stand down
//! after keyboard input; the old `:hover` did not), and so that the reveal is one decision the tests can read: the
//! button's wrapper names it in its debug selector, `session-menu-<key>:shown` or `:hidden`.
//!
//! While any row's menu or a question it asked is open, the list's order hold stays on ([`RowLayers::any_open`],
//! [`super::hold`]), so the row the menu belongs to does not move away from it.
//!
//! NOTE: The environment commands come with their area, in the old order around these, with no placeholder
//! meanwhile; the old row menu had no Restart, which the open session's header offers instead
//! (`session_pane/header.rs`). Also not ported: opening the menu with the arrow keys from the closed button, placing
//! the menu beside the sidebar with a pointer notch (the layer manager places it below its anchor), the
//! old reveal's 100 ms fade (the button appears and goes at once: a fade per row would need a ticker animation for
//! each row in motion, which nothing else in the list holds), and its rule that a touch screen always shows the button
//! (GPUI reports no coarse pointer to ask about).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use farhelm_helm_client::{Collections, PathError};
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled, WeakEntity,
    Window, div, px,
};

use super::{SessionList, delete, outcome, replace, seen, selection, status, stop};
use crate::actions::RENAME_SESSION_SHORTCUT_NAME;
use crate::assets::Icon;
use crate::components::{HoverHelp, IconButton, MenuHeader, MenuItem, Role, Tier, menu_separator};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::launcher::entry;
use crate::layers::{self, Anchor, LayerId, Popover, Subject};
use crate::operations::{Command, OpKey, Work};
use crate::prompt::{self, Question};
use crate::{local_state, session};

/// The id prefix of a row's menu button; the rest is the session's key.
pub(crate) const BUTTON_ID_PREFIX: &str = "session-menu-";

/// `.session-row-menu-panel:not(.menu-prompt)`'s width.
const PANEL_WIDTH: f32 = 246.;

/// The menu button's hover help, the old toggle's `data-tooltip` word for word.
pub(crate) const ACTIONS_HELP: &str =
    "Session actions: rename, clone, replace, stop, delete and more";

/// Clone's hover help, in the row's menu and on the open session's header: the old header button's tooltip, since the
/// old menu item had none.
pub(crate) const CLONE_HELP: &str =
    "Clone: open the new-session form with these settings; this session keeps running";

/// Replace with's hover help in the row's menu: the old interrupted card's Replace with tooltip, since the old menu
/// item had none.
const REPLACE_WITH_HELP: &str =
    "Replace with: edit the settings, then swap this session for a new one";

/// A launcher entry point a menu item opens on its session ([`entry::open_clone`],
/// [`entry::open_replace_with`]): it returns whether the launcher opened.
type LauncherEntry = fn(&str, &mut Window, &mut App) -> bool;

/// The layer a row last opened from its menu button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Opened {
    /// The row's menu.
    Menu,
    /// A question one of its commands asked (Delete's), during which the button is inert.
    Question,
}

/// What the list keeps about its rows' menus for one UI session: where each row's button was drawn, so a command or
/// the Delete shortcut can anchor a question to it, each drawn button's focus handle, which its menu names as where
/// closing returns focus, and which layer each row opened last.
///
/// The layers are kept by id and checked with the layer manager whenever they are read, so a layer closed some other
/// way (Escape, a press outside, its row gone) needs no bookkeeping here.
///
/// Shared ([`SharedRows`]) because the rows are drawn by the virtualized list's item callback, after the list's own
/// render has returned: the callback records each drawn row's anchor here, and only the rows the list draws (those in
/// view, and a few beyond its edges) have one. A row scrolled further away has none, so its open menu closes as one
/// whose row went (the old app closed a row's menu on any move of the row), and the Delete shortcut asks its question
/// centered rather than beside a row nobody can see.
#[derive(Default)]
pub(super) struct RowLayers {
    anchors: HashMap<String, Anchor>,
    /// Each drawn button's focus handle, recorded with its anchor: the handle is element state, made only while the
    /// row draws, so the menu's opener (a click handler) reads it from here.
    buttons: HashMap<String, FocusHandle>,
    open: HashMap<String, (LayerId, Opened)>,
}

/// [`RowLayers`] as the list and its item callback share it.
pub(super) type SharedRows = Rc<RefCell<RowLayers>>;

impl RowLayers {
    /// Forget the buttons drawn last frame; the list's item callback records this frame's as it draws them.
    pub(super) fn start_frame(&mut self) {
        self.anchors.clear();
        self.buttons.clear();
    }

    /// The anchor of `key`'s menu button, as last drawn.
    pub(super) fn anchor(&self, key: &str) -> Option<Anchor> {
        self.anchors.get(key).cloned()
    }

    /// Record that `key`'s row opened `layer`.
    pub(super) fn opened(&mut self, key: &str, layer: LayerId, kind: Opened) {
        self.open.insert(key.to_owned(), (layer, kind));
    }

    /// Whether any row's menu, or a question one of its commands asked, is still open: what keeps the order hold on
    /// ([`super::hold`]).
    pub(super) fn any_open(&mut self, window: &Window, cx: &App) -> bool {
        self.open
            .retain(|_, (layer, _)| layers::is_open(*layer, window, cx));
        !self.open.is_empty()
    }

    /// The layer `key`'s row opened that is still open, if any.
    fn still_open(&mut self, key: &str, window: &Window, cx: &App) -> Option<(LayerId, Opened)> {
        let open = *self.open.get(key)?;
        if layers::is_open(open.0, window, cx) {
            Some(open)
        } else {
            self.open.remove(key);
            None
        }
    }
}

/// The menu button of the row of `key`, a session titled `title` (as the host wrote it), with its anchor recorded in
/// `rows` for the commands and the shortcut, and its focus handle for the menu to return focus to. `row_active` says
/// whether the pointer is over the row or any control in it holds focus, two of the reveal's reasons (the module docs,
/// "The button's reveal"), which the row's drawing knows. Called from the virtualized list's item callback, once per
/// drawn row and frame: an anchor and a control's focus handle are element state, so they are made while the row's
/// elements are.
#[allow(
    clippy::too_many_arguments,
    reason = "one row's button: its row's identity and state, and the window it is drawn in"
)]
pub(super) fn menu_button(
    rows: &RefCell<RowLayers>,
    list: &WeakEntity<SessionList>,
    key: &str,
    title: &str,
    selected: bool,
    row_active: bool,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let anchor = Anchor::for_element(
        SharedString::from(format!("session-menu-anchor-{key}")),
        window,
        cx,
    );
    let open = {
        let mut rows = rows.borrow_mut();
        rows.anchors.insert(key.to_owned(), anchor.clone());
        // Its menu, or the question that took the menu's place.
        rows.still_open(key, window, cx).is_some()
    };
    let id: SharedString = format!("{BUTTON_ID_PREFIX}{key}").into();
    let focus = focus::control_handle(SharedString::from(format!("{id}-focus-handle")), window, cx);
    rows.borrow_mut()
        .buttons
        .insert(key.to_owned(), focus.clone());
    // Like the old `.session-row-menu`, invisible while no reveal reason holds, though still there to click and to
    // reach with Tab. None of them latches: the button goes once none holds.
    let visible = selected || open || row_active;
    let selector_id = id.clone();
    let list = list.clone();
    let toggled = key.to_owned();
    let toggle = IconButton::new(
        id,
        Icon::More,
        actions_label(title),
        HoverHelp::tooltip(ACTIONS_HELP),
        Role::Button,
    )
    .tier(Tier::RowMenu { on_fill: selected })
    .expanded(open)
    .focus(&focus)
    .on_click(move |_, window, cx| {
        // The list may be gone with its window; then there is no menu to open.
        let _ = list.update(cx, |list, cx| list.toggle_menu(&toggled, window, cx));
    });
    // `margin: 4px 2px 0 0` and `align-self: flex-start`: beside the title line, not centered on the row.
    div()
        .relative()
        .flex_none()
        .mt(px(4.))
        .mr(px(2.))
        .opacity(if visible { 1. } else { 0. })
        // Tests read the reveal here, since the frame's record carries no opacity. The closure runs only in builds that
        // record selectors, so other builds never format the string.
        .debug_selector(move || {
            format!("{selector_id}:{}", if visible { "shown" } else { "hidden" })
        })
        .child(toggle)
        .child(anchor.track())
        .into_any_element()
}

impl SessionList {
    /// The menu button of `key`'s row was activated: open its menu, close the menu if it is open, and do nothing while
    /// a question it asked is open. A row scrolled out of view is scrolled back first, so the menu opens beside a row
    /// the user can see. Visible to the crate for the screenshot goldens, which open a menu without a click.
    pub(crate) fn toggle_menu(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.reveal_row(key, cx);
        let (open, anchor, button) = {
            let mut rows = self.current.current(cx).rows.borrow_mut();
            (
                rows.still_open(key, window, cx),
                rows.anchor(key),
                rows.buttons.get(key).cloned(),
            )
        };
        match open {
            Some((_, Opened::Question)) => return,
            Some((menu, Opened::Menu)) => {
                layers::close(menu, window, cx);
                return;
            }
            None => {}
        }
        let (Some(anchor), Some(button)) = (anchor, button) else {
            return;
        };
        let mark = Mark::take(cx);
        let rename_focus = cx.focus_handle().tab_stop(true);
        let list = cx.weak_entity();
        let view = cx.new(|_| RowMenu {
            key: key.to_owned(),
            list,
            rename_focus: rename_focus.clone(),
        });
        let opened = layers::open_popover(
            // Focus starts on the first command, as the old menu started.
            Popover::menu(view, cx.focus_handle(), anchor, rename_focus)
                // The button, not what held focus before: on macOS the click that opened the menu left the button
                // unfocused, and SPEC.md returns focus to the button when the menu closes.
                .returns_to(focus::ReturnTo::Control(button)),
            &mark,
            window,
            cx,
        );
        if let Some(menu) = opened {
            self.current
                .current(cx)
                .rows
                .borrow_mut()
                .opened(key, menu, Opened::Menu);
        }
    }

    /// Close `key`'s menu if it is still open, which returns focus to its button (the menu names it, so on every
    /// platform): what every command chosen from the menu does first, at acceptance rather than at a reply.
    pub(super) fn close_menu(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let open = self
            .current
            .current(cx)
            .rows
            .borrow_mut()
            .still_open(key, window, cx);
        if let Some((menu, Opened::Menu)) = open {
            layers::close(menu, window, cx);
        }
    }

    /// The menu's Rename was chosen at `mark`: close the menu, which returns focus to its button, then open the Rename
    /// editor, which returns focus there when it closes.
    fn rename_from_menu(
        &mut self,
        key: &str,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_menu(key, window, cx);
        let _ = super::rename::open(key, mark, window, cx);
    }

    /// The menu's Mark read or Mark unread was chosen: close the menu, then send `toggle`, the action the item was drawn
    /// with. A refusal shows on the row, so nothing waits with the menu open.
    fn seen_from_menu(
        &mut self,
        key: &str,
        toggle: seen::Toggle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_menu(key, window, cx);
        seen::send(key, toggle, cx);
    }

    /// One of the menu's lifecycle commands was chosen at `mark`: close the menu, which returns focus to its button,
    /// then go on as `command` goes on for the row (its Replace, Stop or Delete), so any question opens from there.
    fn lifecycle_from_menu(
        &mut self,
        key: &str,
        mark: &Mark,
        command: LifecycleCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_menu(key, window, cx);
        match command {
            LifecycleCommand::Replace => self.replace(key, mark, window, cx),
            LifecycleCommand::Stop => self.stop(key, mark, window, cx),
            LifecycleCommand::Delete => self.delete(key, mark, window, cx),
        }
    }

    /// Ask the question `question` builds about `key`'s row at `mark`: beside the row's menu button while the row is in
    /// the list's view, and centered when it is not (a row scrolled away may still be drawn just past the list's edge,
    /// and a question beside it would sit next to nothing the user can see). Either way the question closes when the session is gone, and the row's button is inert
    /// while it is open ([`Opened::Question`]). Nothing opens when the user has moved on since `mark`. `on_confirm`
    /// runs at most once, with the question's frozen value. A builder rather than a question, because a question cannot
    /// be copied and the anchored attempt consumes the one it is given even when it opens nothing.
    pub(super) fn ask_about_row<T: Clone + 'static>(
        &mut self,
        key: &str,
        question: impl Fn() -> Question<T>,
        mark: &Mark,
        on_confirm: impl FnOnce(T, &mut Window, &mut App) + Clone + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let anchor = if self.row_in_view(key) {
            self.current.current(cx).rows.borrow().anchor(key)
        } else {
            None
        };
        let asked = anchor.and_then(|anchor| {
            prompt::ask_anchored(
                question(),
                anchor,
                Subject::session(key, cx),
                mark,
                on_confirm.clone(),
                window,
                cx,
            )
        });
        let asked = asked.or_else(|| {
            prompt::ask(
                question(),
                Subject::session(key, cx),
                mark,
                on_confirm,
                window,
                cx,
            )
        });
        if let Some(layer) = asked {
            self.current
                .current(cx)
                .rows
                .borrow_mut()
                .opened(key, layer, Opened::Question);
        }
    }
}

/// Which of the menu's lifecycle commands was chosen.
#[derive(Clone, Copy)]
enum LifecycleCommand {
    Replace,
    Stop,
    Delete,
}

/// A row a lifecycle action was asked for, as the store held it when the user asked.
pub(crate) struct Actionable {
    /// The row as the store holds it now: what any question is drawn from and any precondition is taken from.
    pub(crate) row: SessionRow,
    /// Whether the helm-wide **Skip session confirmations** preference is on, so the action goes ahead on the click
    /// with the guard of what the row showed (SPEC.md, "Lifecycle operations").
    pub(crate) skips_questions: bool,
}

/// `key`'s row, for a lifecycle action the user just asked for (from the row's menu, or the open session's header), or
/// `None` when nothing may happen: the store holds no row for the session, or its lifecycle commands are refused
/// ([`refused`]: a request in flight, or its YOLO question open). The row is read by key, so the open session the helm
/// carries outside a filtered or cut list is actionable like a listed one.
pub(crate) fn actionable(key: &str, cx: &App) -> Option<Actionable> {
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    let row = collections.get::<SessionRow>(key).cloned()?;
    if refused(key, cx) {
        return None;
    }
    Some(Actionable {
        row,
        skips_questions: skips_questions(collections),
    })
}

/// Whether `key`'s lifecycle commands (Rename, Clone, Replace with, Replace, Stop and Delete here, and Restart, Restart
/// with and Clone in the open session's header) are refused now, and drawn unavailable: SPEC.md's "an open lifecycle
/// confirmation or a pending request refuses another lifecycle action". A request of the session in flight is one
/// ([`delete::busy`]); the YOLO question a Replace of the session met, while it is open, is the other
/// ([`super::replace_yolo::open`]). The row's and the header's own questions refuse through their buttons, which are
/// inert while their question is open.
pub(crate) fn refused(key: &str, cx: &App) -> bool {
    delete::busy(key, cx) || super::replace_yolo::open(key, cx)
}

/// Send `command`, one of the row's own requests (a seen toggle, a pin, a move), through the operation store under
/// `key`'s session key, so it is that session's one request in flight and its other actions wait for it. Nothing waits
/// for the reply: a refusal is the store's to keep and the row's to show ([`outcome`]), and a success needs nothing,
/// since the helm has pushed its effect by the time it replies. Nothing is sent while the session already has a
/// request in flight, while the window is inert, or for a key that cannot be a path segment (which no helm serves);
/// each is logged.
pub(super) fn send(key: &str, command: Result<Command, PathError>, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let Ok(command) = command else {
        tracing::warn!("a session key that cannot be a path segment was not sent a row request");
        return;
    };
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::session(key), command, cx, |_, _| {})
    });
    if let Err(refused) = started {
        tracing::debug!(?refused, "a row request was not sent");
    }
}

/// Why every session's plain Restart and Replace wait now, if they do: while one of this window's host-registry changes
/// is in flight ([`crate::hosts::host_change_in_flight`]), the launcher's own reason for its Launch
/// ([`crate::launcher::launch::HOST_CHANGE_IN_FLIGHT`]). This menu's Replace and the header's Restart and Replace draw
/// themselves unavailable with it as their hover help, and their sends refuse ([`refuse_for_host_change`]).
///
/// SPEC.md, "Creation": "Launches and changes to the host registry still exclude each other". The migration's
/// orchestrator decided on 2026-11-15 that Restart and Replace count as launches for this half of the exclusion, as
/// they already do for the other ([`Work::creates_a_session`]); the old app's wider rule, every write waiting, is not
/// carried over. So it is not part of [`refused`]: it concerns the window, not one session, and Rename, Stop and
/// Delete do not wait for it. A new surface that offers either action gets the refusal through their send functions
/// but must draw the wait itself, from this function, or its control looks ready while it does nothing.
pub(crate) fn launch_wait(cx: &App) -> Option<&'static str> {
    crate::hosts::host_change_in_flight(cx)
        .then_some(crate::launcher::launch::HOST_CHANGE_IN_FLIGHT)
}

/// Why the launcher cannot open now, if it cannot ([`entry::new_available`]): a host change in flight
/// ([`launch_wait`]'s words), or a launch pending ([`LAUNCH_PENDING`]). This menu's Clone and Replace with… and the
/// header's Clone, Restart with… and Replace with… draw themselves unavailable with it as their hover help and
/// description, as the plain choices say [`launch_wait`]'s, so a greyed launcher trigger always says why (SPEC.md,
/// "Terminal experience": the greyed entries' "tooltips and accessible descriptions explain the specific reason").
pub(crate) fn launcher_wait(cx: &mut App) -> Option<&'static str> {
    if entry::new_available(cx) {
        return None;
    }
    Some(launch_wait(cx).unwrap_or(LAUNCH_PENDING))
}

/// [`launcher_wait`]'s words while a launch is pending (the launcher holds one launch at a time). The old app had none:
/// it never greyed these triggers, and its launcher refused a second opening silently.
const LAUNCH_PENDING: &str = "Wait for the pending launch to finish";

/// Refuse `key`'s `work` (a Restart or a Replace about to be sent) while a host change is in flight, and say why on
/// the session: the reason goes in the operation store's slot for the key, which the row, its menu and the header draw
/// as a refusal ("Replace: Wait for the host change to finish"; [`outcome`]). Returns whether it refused.
///
/// What reaches this is a stale press or a confirmation of a question that opened before the change started (a YOLO
/// question stays open inline while the user goes on). SPEC.md, "Errors and diagnostics", has every failed operation
/// surface its error and no dialog close as though it succeeded, so the refusal cannot be silent. The note
/// ([`Operations::note`]) is written only while nothing runs on the key, since a note beside running work would
/// outlive it, and the store refuses a second request on the key anyway.
///
/// [`Operations::note`]: crate::operations::Operations::note
pub(crate) fn refuse_for_host_change(key: &str, work: Work, cx: &mut App) -> bool {
    let Some(reason) = launch_wait(cx) else {
        return false;
    };
    tracing::debug!(
        ?work,
        "a lifecycle request waited for a host change and was not sent"
    );
    if let Some(operations) = session::current_operations(cx) {
        let key = OpKey::session(key);
        operations.update(cx, |operations, cx| {
            if operations.in_flight(&key).is_none() {
                operations.note(&key, work, HostText::from_host(reason), cx);
            }
        });
    }
    true
}

/// Whether the helm-wide **Skip session confirmations** preference is on in `collections`: the lifecycle actions then go
/// ahead on the click, and the open session's header drops "after you confirm" from its Delete's help.
pub(crate) fn skips_questions(collections: &Collections) -> bool {
    selection::preferences(collections)
        .is_some_and(|preferences| preferences.skip_session_confirmations == Some(true))
}

/// The menu's and its button's accessible name for a session titled `title` (as the host wrote it): "Session actions
/// for" and the title escaped as the screen shows it, cut short ([`MenuHeader::clamp_name`], which the host menu's
/// name uses too). Every row has a button, so the name must say whose.
fn actions_label(title: &str) -> String {
    format!(
        "Session actions for {}",
        MenuHeader::clamp_name(&HostText::from_host(title))
    )
}

/// The open menu of one row: drawn from the store while it is open, so its header and its commands' availability
/// follow the session.
pub(super) struct RowMenu {
    key: String,
    list: WeakEntity<SessionList>,
    /// The Rename command's focus handle, where the menu starts.
    rename_focus: FocusHandle,
}

impl Render for RowMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = layers::panel(cx)
            .id("session-menu")
            .debug_selector(|| "session-menu".into())
            .w(px(PANEL_WIDTH));
        // Read before the row, which borrows the store from the context.
        let launcher_wait = launcher_wait(cx);
        let Some(store) = session::current_store(cx) else {
            return panel;
        };
        // A session gone from the store takes its row, and so the menu's anchor, with it; until that frame there is
        // nothing to show.
        let Some(row) = store.read(cx).collections().get::<SessionRow>(&self.key) else {
            return panel;
        };
        let title = HostText::from_host(row.info.title.as_str());
        let busy = refused(&self.key, cx);
        let open = local_state::selected(cx).as_deref() == Some(self.key.as_str());
        let rename_help = if open {
            format!("Rename: give this session a new title ({RENAME_SESSION_SHORTCUT_NAME})")
        } else {
            "Rename: give this session a new title".to_owned()
        };
        let (list, key) = (self.list.clone(), self.key.clone());
        let rename = MenuItem::new(
            "session-menu-rename",
            Icon::Edit,
            "Rename",
            HoverHelp::tooltip(rename_help),
            Role::MenuItem,
        )
        .description("Give this session a new title")
        .disabled(busy)
        .focus(&self.rename_focus)
        .on_click(move |_, window, cx| {
            let mark = Mark::take(cx);
            let _ = list.update(cx, |list, cx| {
                list.rename_from_menu(&key, &mark, window, cx)
            });
        });
        // Mark read or Mark unread, as the row's seen state reads now, for a live row only.
        let seen_item = seen::offered(row).map(|toggle| {
            let (list, key) = (self.list.clone(), self.key.clone());
            MenuItem::new(
                "session-menu-seen",
                Icon::MarkSeen,
                toggle.label(),
                HoverHelp::tooltip(toggle.help()),
                Role::MenuItem,
            )
            .description(toggle.description())
            .disabled(busy)
            .on_click(move |_, window, cx| {
                let _ = list.update(cx, |list, cx| list.seen_from_menu(&key, toggle, window, cx));
            })
        });
        // The lifecycle commands: in the old menu's danger red, each in a group of its own, and unavailable while
        // the session has a request in flight, as Rename is.
        let lifecycle = |id: &'static str,
                         icon: Icon,
                         label: &'static str,
                         help: &'static str,
                         description: &'static str,
                         command: LifecycleCommand| {
            let (list, key) = (self.list.clone(), self.key.clone());
            MenuItem::new(id, icon, label, HoverHelp::tooltip(help), Role::MenuItem)
                .description(description)
                .danger()
                .disabled(busy)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    let _ = list.update(cx, |list, cx| {
                        list.lifecycle_from_menu(&key, &mark, command, window, cx)
                    });
                })
        };
        // Replace waits for a host change, its help saying why ([`launch_wait`]).
        let launch_wait = launch_wait(cx);
        // Clone and Replace with open the launcher on this session. They are unavailable while the session's own
        // commands are refused, as Rename is, and while the launcher cannot open ([`launcher_wait`]: a launch pending,
        // a host change in flight), saying why then in place of their help and description.
        let launcher_busy = busy || launcher_wait.is_some();
        let launcher_item = |id: &'static str,
                             icon: Icon,
                             label: &'static str,
                             help: &'static str,
                             description: &'static str,
                             open: LauncherEntry| {
            let (list, key) = (self.list.clone(), self.key.clone());
            MenuItem::new(
                id,
                icon,
                label,
                HoverHelp::tooltip(launcher_wait.unwrap_or(help)),
                Role::MenuItem,
            )
            .description(launcher_wait.unwrap_or(description))
            .disabled(launcher_busy)
            .on_click(move |_, window, cx| {
                // Closed first, which returns focus to the menu's button: the entry point refuses while a menu is open,
                // and the launcher returns focus to what held it when it opened.
                let _ = list.update(cx, |list, cx| list.close_menu(&key, window, cx));
                // A press drawn a frame before the session's commands were refused: the entry points check only
                // whether the launcher may open, so the refusal is checked here, as the other commands check it.
                if actionable(&key, cx).is_none() {
                    return;
                }
                if !open(&key, window, cx) {
                    tracing::debug!(label, "the launcher refused to open from the row menu");
                }
            })
        };
        let clone = launcher_item(
            "session-menu-clone",
            Icon::Clone,
            "Clone",
            CLONE_HELP,
            "New session, keep this one",
            entry::open_clone,
        );
        let replace_with = launcher_item(
            "session-menu-replace-with",
            Icon::Adjust,
            "Replace with…",
            REPLACE_WITH_HELP,
            "Edit settings, then swap",
            entry::open_replace_with,
        );
        let replace = lifecycle(
            "session-menu-replace",
            Icon::Replace,
            "Replace",
            launch_wait.unwrap_or(replace::MENU_HELP),
            "Fresh conversation, same settings",
            LifecycleCommand::Replace,
        )
        .disabled(busy || launch_wait.is_some());
        let stop = lifecycle(
            "session-menu-stop",
            Icon::Stop,
            "Stop",
            stop::MENU_HELP,
            "Ends the agent and its processes, and closes terminal tabs",
            LifecycleCommand::Stop,
        );
        let delete = lifecycle(
            "session-menu-delete",
            Icon::Trash,
            "Delete",
            "Delete: remove this session and its state",
            "Removes the session and its state",
            LifecycleCommand::Delete,
        );
        panel
            .child(MenuHeader::new(&title, summary(&self.key, row)))
            .child(
                div()
                    .id("session-menu-items")
                    .role(Role::Menu)
                    .aria_label(actions_label(&row.info.title))
                    .flex()
                    .flex_col()
                    .child(rename)
                    .children(seen_item)
                    .children(super::pins::menu_items(
                        &self.list, &self.key, row, busy, cx,
                    ))
                    .child(menu_separator(cx))
                    .child(clone)
                    .child(replace_with)
                    .child(menu_separator(cx))
                    .child(replace)
                    .child(menu_separator(cx))
                    .child(stop)
                    .child(menu_separator(cx))
                    .child(delete),
            )
            .children(outcome::menu_line(&self.key, cx))
    }
}

/// The line under the session's title in the menu's header: its agent, then its status, as "Codex · Running". It may
/// quote a host (a command launch's program), so it is host text. The agent is its display name for an agent Farhelm
/// knows, and the launched program's name otherwise; a status that draws no badge leaves the agent alone, with no
/// separator.
fn summary(key: &str, row: &SessionRow) -> HostSentence {
    let mut summary = HostSentence::new(SharedString::from(format!("session-menu-summary-{key}")));
    summary = match agent_name(row) {
        AgentName::Known(agent) => summary.text(agent),
        AgentName::Program(program) => summary.host(&program),
    };
    if let Some(word) = status::Badge::word(
        status::badge(
            row,
            SharedString::from(format!("session-menu-status-{key}")),
        )
        .as_ref(),
    ) {
        summary = summary.text(" · ").text(word);
    }
    summary
}

/// What a session runs, by the name the user knows it by: the one rule for the menu's header and the quick switcher's
/// agent column, so the two never name the same session differently.
pub(super) enum AgentName {
    /// An agent Farhelm knows, by its display name (the app's own words).
    Known(&'static str),
    /// Anything else, by the program its command line runs ([`program_name`]), which the host supplied.
    Program(HostText),
}

/// The name `row` runs under ([`AgentName`]).
pub(super) fn agent_name(row: &SessionRow) -> AgentName {
    match row.info.agent_kind.display_name() {
        Some(agent) => AgentName::Known(agent),
        None => AgentName::Program(HostText::from_host(program_name(&row.info.invocation))),
    }
}

/// The program a command line runs, by its file name: its first word with any directory dropped (`/usr/bin/claude
/// --resume` is `claude`). A host value: the caller escapes it.
fn program_name(invocation: &str) -> &str {
    let program = invocation.split_whitespace().next().unwrap_or("");
    program.rsplit('/').next().unwrap_or(program)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The menu header names a command launch's agent by the program it runs, since that is all Farhelm knows about
    /// it. Specified: the first word's file name, with arguments and directories dropped, and nothing for nothing.
    #[test]
    fn a_command_launch_is_named_by_its_program() {
        assert_eq!(program_name("/usr/local/bin/claude --resume x"), "claude");
        assert_eq!(program_name("codex -m gpt-5"), "codex");
        assert_eq!(program_name(""), "");
    }
}
