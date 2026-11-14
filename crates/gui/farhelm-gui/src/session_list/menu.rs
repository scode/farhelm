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
//! Choosing Delete closes the menu before the question opens beside the same button (the old app turned the menu into
//! the question; here the two are separate layers, one after the other). While that question is open the button is
//! inert: a click on it neither reopens the menu nor answers the question, and the question's own Cancel, Escape or a
//! press elsewhere is the way out. A refusal from the helm shows inside the menu as well as on the row, since the open
//! menu covers the row's own copy.
//!
//! NOTE: Only Rename and Delete are ported. Clone, Replace, Replace with, Stop, Restart, Mark read, Pin and the
//! environment commands come with their areas. Also not ported: the button staying visible while the
//! row's open control has keyboard focus (the old `:focus-within`; here only the button's own focus shows it), opening
//! the menu with the arrow keys from the closed button, placing the menu beside the sidebar with a pointer notch (the
//! layer manager places it below its anchor), and centering the Delete shortcut's question when the selected row is
//! scrolled out of the list's view. Every listed row's button is drawn inside the scroll area, so its anchor exists
//! even when it is scrolled away, and the layer manager then clamps the question into the window beside no visible
//! row; telling that apart needs the anchor's drawn bounds against the list's visible area, which `layers::Anchor`
//! does not expose.

use std::collections::HashMap;

use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled,
    WeakEntity, Window, div, px,
};

use super::{SessionList, delete, selection, status};
use crate::actions::RENAME_SESSION_SHORTCUT_NAME;
use crate::assets::Icon;
use crate::components::{HoverHelp, IconButton, MenuItem, Role, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::operations::OpKey;
use crate::store::HelmStore;
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
#[derive(Default)]
pub(super) struct RowLayers {
    anchors: HashMap<String, Anchor>,
    open: HashMap<String, (LayerId, Opened)>,
}

impl RowLayers {
    /// Forget the buttons drawn last frame; the list records this frame's as it draws them.
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

impl SessionList {
    /// This frame's menu button for every row `store` lists, by session key, each with its anchor recorded for the
    /// commands and the shortcut. Made before the rows are drawn, because an anchor is element state that needs the
    /// window while the rows only read the store.
    pub(super) fn menu_buttons(
        &mut self,
        store: &Entity<HelmStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> HashMap<String, AnyElement> {
        let selected = local_state::selected(cx);
        let rows: Vec<(String, String)> = selection::listed(store.read(cx).collections())
            .iter()
            .map(|session| (session.key.to_owned(), session.row.info.title.clone()))
            .collect();
        self.current.current(cx).rows.start_frame();
        let list = cx.weak_entity();
        let mut buttons = HashMap::new();
        for (key, title) in rows {
            let anchor = Anchor::for_element(
                SharedString::from(format!("session-menu-anchor-{key}")),
                window,
                cx,
            );
            let rows = &mut self.current.current(cx).rows;
            rows.anchors.insert(key.clone(), anchor.clone());
            // Its menu, or the question that took the menu's place.
            let open = rows.still_open(&key, window, cx).is_some();
            let selected = selected.as_deref() == Some(key.as_str());
            let id: SharedString = format!("{BUTTON_ID_PREFIX}{key}").into();
            let focus =
                focus::control_handle(SharedString::from(format!("{id}-focus-handle")), window, cx);
            // Like the old `.session-row-menu`, invisible on a row that is neither hovered, selected nor holding the
            // button's focus and has nothing open, though still there to click and to reach with Tab.
            let visible = selected || open || focus.is_focused(window);
            let list = list.clone();
            let toggled = key.clone();
            let toggle = IconButton::new(
                id,
                Icon::More,
                actions_label(&title),
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
            let button = div()
                .relative()
                .flex_none()
                .mt(px(4.))
                .mr(px(2.))
                .opacity(if visible { 1. } else { 0. })
                .group_hover(super::row::ROW_GROUP, |style| style.opacity(1.))
                .child(toggle)
                .child(anchor.track())
                .into_any_element();
            buttons.insert(key, button);
        }
        buttons
    }

    /// The menu button of `key`'s row was activated: open its menu, close the menu if it is open, and do nothing while
    /// a question it asked is open. Visible to the crate for the screenshot goldens, which open a menu without a click.
    pub(crate) fn toggle_menu(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let rows = &mut self.current.current(cx).rows;
        match rows.still_open(key, window, cx) {
            Some((_, Opened::Question)) => return,
            Some((menu, Opened::Menu)) => {
                layers::close(menu, window, cx);
                return;
            }
            None => {}
        }
        let Some(anchor) = rows.anchor(key) else {
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
                start: rename_focus,
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
                .opened(key, menu, Opened::Menu);
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
        if let Some((menu, Opened::Menu)) =
            self.current.current(cx).rows.still_open(key, window, cx)
        {
            layers::close(menu, window, cx);
        }
        let _ = super::rename::open(key, mark, window, cx);
    }

    /// The menu's Delete was chosen at `mark`: close the menu, which returns focus to where it was when the menu
    /// opened, then go on as any Delete of the row does, so the question opens from there.
    fn delete_from_menu(
        &mut self,
        key: &str,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((menu, Opened::Menu)) =
            self.current.current(cx).rows.still_open(key, window, cx)
        {
            layers::close(menu, window, cx);
        }
        self.delete(key, mark, window, cx);
    }
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
        let failure = session::current_operations(cx).and_then(|operations| {
            operations
                .read(cx)
                .failure(&OpKey::session(&self.key))
                .cloned()
        });
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
        let list = self.list.clone();
        let key = self.key.clone();
        let delete = MenuItem::new(
            "session-menu-delete",
            Icon::Trash,
            "Delete",
            HoverHelp::tooltip("Delete: remove this session and its state"),
            Role::MenuItem,
        )
        .description("Removes the session and its state")
        .danger()
        .disabled(busy)
        .on_click(move |_, window, cx| {
            let mark = Mark::take(cx);
            let _ = list.update(cx, |list, cx| {
                list.delete_from_menu(&key, &mark, window, cx)
            });
        });
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
                    .child(delete),
            )
            // On the menu's own surface, so in the ordinary red whatever the row's fill.
            .children(
                failure.map(|failure| {
                    delete::failure_text(&failure.message, theme::palette(cx).danger)
                }),
            )
    }
}

/// The menu's header, `.menu-header`: the session's title, then its agent and status on one line, above a hairline.
/// Both may quote a host (the title, and a command launch's program), so both are host text. The agent is its display
/// name for an agent Farhelm knows, and the launched program's name otherwise.
fn header(key: &str, row: &SessionRow, title: &HostText, cx: &App) -> impl IntoElement {
    let palette = theme::palette(cx);
    let mut summary = HostSentence::new(SharedString::from(format!("session-menu-summary-{key}")));
    summary = match row.info.agent_kind.display_name() {
        Some(agent) => summary.text(agent),
        None => summary.host(&HostText::from_host(program_name(&row.info.invocation))),
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
