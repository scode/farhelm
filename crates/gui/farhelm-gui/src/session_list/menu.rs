//! A session row's actions menu: the ⋯ button at the end of the row, and the menu it opens below itself.
//!
//! Ported from the old app's `list/row.rs` (the menu toggle, `menu_label`, `menu_header_summary_parts` and the Delete
//! item) and the stylesheet's `.session-row-menu`, `.session-row-menu-panel` and `.menu-header` rules at the reference
//! commit. The menu is a popover of the layer manager, anchored to its row's button through an [`Anchor`] kept under the
//! session's key, so an open menu moves with its session when the list reorders and closes when the row is no longer
//! drawn (SPEC.md, "Things changing underneath" and "Nothing closes a dialog … it did not open").
//!
//! The menu's header names the session and says, in one line, which agent it runs and its status; below it come the
//! commands, in a [`Role::Menu`] of their own, so the header is not read out as one of them. Up and Down walk the
//! commands (the layer manager's menu context), and the pointer moves the same highlight.
//!
//! Rename comes first, as in the old app. Choosing it closes the menu, which leaves focus on the menu's button, and
//! opens the Rename editor ([`super::rename`]), which returns focus there when it closes. Its hover help names the
//! Rename shortcut on the open session's row only, the one row the shortcut acts on (SPEC.md, "Desktop window
//! chrome"). Like Delete, it is unavailable while the session has a request in flight.
//!
//! Right after Rename, in the same group with no separator between them, a live row offers Mark read or Mark unread,
//! whichever its seen state calls for ([`super::seen`]); a row that offers neither omits the item, so the commands
//! after it move up. Choosing it closes the menu, which leaves focus on the menu's button, and then sends the request.
//!
//! Then come the lifecycle commands in the old menu's groups, each after a separator: Replace ([`super::replace`]), Stop
//! ([`super::stop`]) and Delete ([`super::delete`]), all three in the danger red. Choosing one closes the menu before
//! its question opens beside the same button (the old app turned the menu into the question; here the two are separate
//! layers, one after the other), or before its request goes out when it asks nothing. While that question is open the
//! button is inert: a click on it neither reopens the menu nor answers the question, and the question's own Cancel,
//! Escape or a press elsewhere is the way out. While the session has any request in flight, every command here is
//! unavailable (SPEC.md: "a pending request refuses another lifecycle action"); opening the session stays possible, and
//! other sessions' menus are untouched. A refusal or a notice from the helm shows inside the menu as well as on the
//! row, since the open menu covers the row's own copy ([`super::outcome`]).
//!
//! NOTE: Clone and Replace with (the launcher's), Pin and the environment commands come with their areas,
//! in the old order around these, with no placeholder meanwhile; the old row menu had no Restart, which comes with the
//! session header. Also not ported: the button staying visible while the row's open control has keyboard focus (the
//! old `:focus-within`; here only the button's own focus shows it), opening the menu with the arrow keys from the
//! closed button, and placing the menu beside the sidebar with a pointer notch (the layer manager places it below its
//! anchor).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled, WeakEntity,
    Window, div, px,
};

use super::{SessionList, delete, outcome, replace, seen, selection, status, stop};
use crate::actions::RENAME_SESSION_SHORTCUT_NAME;
use crate::assets::Icon;
use crate::components::{HoverHelp, IconButton, MenuItem, Role, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Anchor, LayerId, Popover, Subject};
use crate::prompt::{self, Question};
use crate::theme::{self, metrics};
use crate::{local_state, session};

/// The id prefix of a row's menu button; the rest is the session's key.
pub(crate) const BUTTON_ID_PREFIX: &str = "session-menu-";

/// `.session-row-menu-panel:not(.menu-prompt)`'s width.
const PANEL_WIDTH: f32 = 246.;

/// How many characters of a title the menu button's accessible name keeps: a title has no length bound, and an
/// accessible name is read aloud in full. The old app's `clamp_title` kept the same.
const NAME_TITLE_CHARS: usize = 64;

/// The layer a row last opened from its menu button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Opened {
    /// The row's menu.
    Menu,
    /// A question one of its commands asked (Delete's), during which the button is inert.
    Question,
}

/// What the list keeps about its rows' menus for one UI session: where each row's button was drawn, so a command or
/// the Delete shortcut can anchor a question to it, and which layer each row opened last.
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
    open: HashMap<String, (LayerId, Opened)>,
}

/// [`RowLayers`] as the list and its item callback share it.
pub(super) type SharedRows = Rc<RefCell<RowLayers>>;

impl RowLayers {
    /// Forget the buttons drawn last frame; the list's item callback records this frame's as it draws them.
    pub(super) fn start_frame(&mut self) {
        self.anchors.clear();
    }

    /// The anchor of `key`'s menu button, as last drawn.
    pub(super) fn anchor(&self, key: &str) -> Option<Anchor> {
        self.anchors.get(key).cloned()
    }

    /// Record that `key`'s row opened `layer`.
    pub(super) fn opened(&mut self, key: &str, layer: LayerId, kind: Opened) {
        self.open.insert(key.to_owned(), (layer, kind));
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
/// `rows` for the commands and the shortcut. Called from the virtualized list's item callback, once per drawn row and
/// frame: an anchor is element state, so it is made while the row's elements are.
pub(super) fn menu_button(
    rows: &RefCell<RowLayers>,
    list: &WeakEntity<SessionList>,
    key: &str,
    title: &str,
    selected: bool,
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
    // Like the old `.session-row-menu`, invisible on a row that is neither hovered, selected nor holding the button's
    // focus and has nothing open, though still there to click and to reach with Tab.
    let visible = selected || open || focus.is_focused(window);
    let list = list.clone();
    let toggled = key.to_owned();
    let toggle = IconButton::new(
        id,
        Icon::More,
        actions_label(title),
        HoverHelp::tooltip("Open this session's actions"),
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
        .group_hover(super::row::ROW_GROUP, |style| style.opacity(1.))
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
        let (open, anchor) = {
            let mut rows = self.current.current(cx).rows.borrow_mut();
            (rows.still_open(key, window, cx), rows.anchor(key))
        };
        match open {
            Some((_, Opened::Question)) => return,
            Some((menu, Opened::Menu)) => {
                layers::close(menu, window, cx);
                return;
            }
            None => {}
        }
        let Some(anchor) = anchor else {
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
            Popover {
                view: view.into(),
                container: cx.focus_handle(),
                // The first command, as the old menu started.
                start: Some(rename_focus),
                anchor,
            },
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

    /// Close `key`'s menu if it is still open, which returns focus to where it was when the menu opened (its button):
    /// what every command chosen from the menu does first, at acceptance rather than at a reply.
    fn close_menu(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
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

    /// One of the menu's lifecycle commands was chosen at `mark`: close the menu, which returns focus to where it was
    /// when the menu opened, then go on as `command` goes on for the row (its Replace, Stop or Delete), so any question
    /// opens from there.
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
    /// and a question beside it would sit next to nothing the user can see; or the Delete shortcut over an open session
    /// the list does not show). Either way the question closes when the session is gone, and the row's button is inert
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

/// A row a lifecycle action was asked for, as listed when the user asked.
pub(super) struct Actionable {
    /// The row as the store holds it now: what any question is drawn from and any precondition is taken from.
    pub(super) row: SessionRow,
    /// Whether the helm-wide **Skip session confirmations** preference is on, so the action goes ahead on the click
    /// with the guard of what the row showed (SPEC.md, "Lifecycle operations").
    pub(super) skips_questions: bool,
}

/// `key`'s row, for a Replace, Stop or Delete the user just asked for, or `None` when nothing may happen: the session
/// is not listed, or it has a request in flight (SPEC.md: "a pending request refuses another lifecycle action").
pub(super) fn actionable(key: &str, cx: &App) -> Option<Actionable> {
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    let row = collections.get::<SessionRow>(key).cloned()?;
    if delete::busy(key, cx) {
        return None;
    }
    let skips_questions = selection::preferences(collections)
        .is_some_and(|preferences| preferences.skip_session_confirmations == Some(true));
    Some(Actionable {
        row,
        skips_questions,
    })
}

/// The menu's and its button's accessible name for a session titled `title` (as the host wrote it): "Session actions
/// for" and the title escaped as the screen shows it, cut short. Every row has a button, so the name must say whose.
fn actions_label(title: &str) -> String {
    let title = HostSentence::new("session-actions-name")
        .host(&HostText::from_host(title))
        .accessible_name();
    format!("Session actions for {}", clamp_name(&title))
}

/// `name` cut to [`NAME_TITLE_CHARS`] characters with an ellipsis, never inside an escape (`<U+202E>`), which would
/// otherwise read as a different character.
fn clamp_name(name: &str) -> String {
    let Some((cut, _)) = name.char_indices().nth(NAME_TITLE_CHARS) else {
        return name.to_owned();
    };
    let mut kept = &name[..cut];
    if let Some(open) = kept.rfind("<U+")
        && !kept[open..].contains('>')
    {
        kept = &kept[..open];
    }
    format!("{kept}…")
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
        let Some(store) = session::current_store(cx) else {
            return panel;
        };
        // A session gone from the store takes its row, and so the menu's anchor, with it; until that frame there is
        // nothing to show.
        let Some(row) = store.read(cx).collections().get::<SessionRow>(&self.key) else {
            return panel;
        };
        let title = HostText::from_host(row.info.title.as_str());
        let busy = delete::busy(&self.key, cx);
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
        let replace = lifecycle(
            "session-menu-replace",
            Icon::Replace,
            "Replace",
            replace::MENU_HELP,
            "Fresh conversation, same settings",
            LifecycleCommand::Replace,
        );
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
            .child(header(&self.key, row, &title, cx))
            .child(
                div()
                    .id("session-menu-items")
                    .role(Role::Menu)
                    .aria_label(actions_label(&row.info.title))
                    .flex()
                    .flex_col()
                    .child(rename)
                    .children(seen_item)
                    .child(separator(cx))
                    .child(replace)
                    .child(separator(cx))
                    .child(stop)
                    .child(separator(cx))
                    .child(delete),
            )
            .children(outcome::menu_line(&self.key, cx))
    }
}

/// A hairline between two of the menu's groups: the old `.menu-separator` inside the row menu's items, one pixel of
/// `--border-dim` inset 10 pixels from the panel's sides with 4 pixels above and below. Drawing only: it is no command,
/// takes no focus, and the arrow keys pass over it.
fn separator(cx: &App) -> impl IntoElement {
    div()
        .h(px(1.))
        .my(px(4.))
        .mx(px(10.))
        .bg(theme::color(theme::palette(cx).border_dim))
}

/// The menu's header, `.menu-header`: the session's title, then its agent and status on one line, above a hairline.
/// Both may quote a host (the title, and a command launch's program), so both are host text. The agent is its display
/// name for an agent Farhelm knows, and the launched program's name otherwise.
fn header(key: &str, row: &SessionRow, title: &HostText, cx: &App) -> impl IntoElement {
    let palette = theme::palette(cx);
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
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .min_w_0()
        .pt(px(6.))
        .pb(px(8.))
        .px(px(11.))
        .mb(px(4.))
        .border_b_1()
        .border_color(theme::color(palette.border_dim))
        .child(
            host_text::element(title)
                .text_size(metrics::TEXT_SM)
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::color(palette.fg_0))
                .line_clamp(3),
        )
        .child(
            div()
                .text_size(px(11.))
                .text_color(theme::color(palette.fg_1))
                .line_clamp(4)
                .child(summary),
        )
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

    /// A long title must not make the menu button's accessible name run on, and cutting it must not leave half an
    /// escape that reads as some other character. Specified: a name within the limit is kept whole; a longer one is
    /// cut at the limit with an ellipsis; a cut that would split a `<U+XXXX>` escape drops the whole escape instead.
    #[test]
    fn a_long_name_is_cut_with_an_ellipsis_and_never_inside_an_escape() {
        assert_eq!(clamp_name("short"), "short");
        let long = "a".repeat(80);
        assert_eq!(clamp_name(&long), format!("{}…", "a".repeat(64)));
        let escaped = format!("{}<U+202E>tail", "b".repeat(60));
        assert_eq!(clamp_name(&escaped), format!("{}…", "b".repeat(60)));
    }

    /// The menu header names a command launch's agent by the program it runs, since that is all Farhelm knows about
    /// it. Specified: the first word's file name, with arguments and directories dropped, and nothing for nothing.
    #[test]
    fn a_command_launch_is_named_by_its_program() {
        assert_eq!(program_name("/usr/local/bin/claude --resume x"), "claude");
        assert_eq!(program_name("codex -m gpt-5"), "codex");
        assert_eq!(program_name(""), "");
    }
}
