//! The open session's top row and its lifecycle actions: Stop, Restart ▾, Replace ▾, Clone and Delete, at the right of
//! the row, with their questions anchored under their buttons and the split buttons' menus under their carets.
//!
//! SPEC.md, "Terminal experience" (the session view's one top row, and the 650-pixel rule), "Lifecycle operations" and
//! "Keyboard focus". Ported from the old app's `.session-topbar` and `.titlebar-actions` (`session_view.rs` and
//! `app.css` at the reference commit). Here's the TLDR of the rules:
//!
//! - **The row.** A plain flex row, at least 30 pixels high, on `--bg-1`: a flexible left region that may shrink to
//!   nothing ([`TABS_SLOT`], where the terminal area's tab strip and open-tunnels pill go; the pill is the terminal's
//!   to draw, [`AttachedTerminal::tunnel_pill`](crate::terminal::attached::AttachedTerminal::tunnel_pill)), then the
//!   actions, which never shrink. The row never wraps; a pane too narrow for it clips it. SPEC.md's floor is a
//!   650-pixel pane, where every action stays fully visible and keyboard reachable.
//! - **The actions, in the old order.** Stop, Restart, Replace, Clone, Delete, each a compact button with the old icon,
//!   label and hover help; Delete in the danger tier, the rest neutral. The terminal area adds the text size group at
//!   the start of the actions.
//! - **Restart and Replace are split buttons** ([`Split`]): the main part performs the plain action, question included,
//!   and the caret beside it opens a menu of that action and its "with…" variant, which opens the launcher
//!   (`launcher::entry::open_restart_with`, `open_replace_with`). The menu ([`CaretMenu`]) is a popover below its
//!   caret that names the caret as where closing returns focus; Escape or Tab closes it there, as SPEC.md asks of these
//!   menus alone. A choice that cannot run stays in the menu, greyed but focusable, with its reason as its description
//!   and help, and a caret whose choices all are unavailable still opens, muted, since its menu is where the reasons
//!   are. Clone opens the launcher too (`launcher::entry::open_clone`). Clone and the "with…" choices are unavailable
//!   while the launcher cannot open (`session_list::menu::launcher_wait`: a launch pending or a host change in
//!   flight), as the launcher's entry points refuse then, with that reason as their help and description; Clone stays
//!   focusable then, as Restart and Replace do while they wait. The launcher returns focus to what held it when it
//!   opened: Clone, or the caret, since a chosen menu item closes the menu first. A caret menu's anchor is named for
//!   its session, so the menu closes when another session opens (a Delete from elsewhere, say).
//! - **The row's code does the work.** Each action reads the open session's row as the session list reads it and calls
//!   the same `begin` and `confirm` the row's menu calls (`session_list::stop`, `replace`, `delete`), which hold each
//!   action's ask-or-send rule: the same questions, the same frozen preconditions, the same commands, the same pending
//!   lines and refusal words. Restart, which the old row menu never had, is `session_list::restart`. What is the
//!   header's own: where each question is anchored, and two decisions that differ from the row's because SPEC.md says
//!   so: Delete asks even for a session with nothing alive (`delete::begin`'s flag), and Restart's button is greyed but
//!   focusable, its hover help and accessible name saying why, when the session cannot resume
//!   ([`crate::components::Button::busy`]).
//! - **The Delete shortcut** is this Delete (SPEC.md, "Desktop window chrome"): the main view hands it to
//!   [`SessionPane::header_action`].
//! - **Exclusion.** While the session has any request in flight, or the YOLO question its Replace met or the
//!   interrupted card's Replace question is open, every action is unavailable, carets and Clone included, as the row's
//!   menu items are (`session_list::menu::refused`), and Restart and Replace also wait for a host change
//!   (`session_list::menu::launch_wait`). A pending Stop shows its spinner and "Stopping…" on the Stop button, and a
//!   pending Delete shows its progress in place of the actions until it settles (they come back with a refusal).
//!   While a question an action asked is open, its button reads as pressed and a click on it does nothing; a press
//!   anywhere else closes the question unanswered.
//! - **The open session by key.** Each action reads the session's row from the store by key, and the helm keeps the
//!   open session there even while a filter or the list's cap leaves it out of the list (`session_list::filters`,
//!   "The query names the open session"), so the actions work on a session the list does not show exactly as on a
//!   listed one. With no row at all (the moment before the helm answers the query that names a newly opened session,
//!   or a selected key that names no session), the row draws no actions, since there is nothing they could ask from.
//! - **Focus.** Each question returns focus home (the terminal) on Escape and to where it was on any other close, which
//!   is SPEC.md's "Escape cancels the header's Stop, Restart, Replace or Delete question and returns focus to the
//!   terminal" ([`crate::prompt::Question::returns_home_on_escape`]).
//! - **The YOLO question.** A Replace from the header that the source's host refuses for YOLO asks the launcher's
//!   question below the top row, among the header's lines, rather than at the source's row
//!   ([`crate::session_list::replace_yolo`]). It is drawn only while the source is the open session.
//!
//! The row is drawn by the pane on every frame, uncached, and reads everything it shows from the store, the operation
//! store and the shared clocks while it draws; the only state it keeps is where each button and caret was drawn, and
//! which question or caret menu it opened ([`Header`]).
//!
//! NOTE: Not ported from the old caret: `aria-haspopup`, which GPUI cannot report, and placing the menu at its caret's
//! right edge (the layer manager places a popover below its anchor). The old header's compact mode under tunnel
//! pressure is the terminal area's, and is not here.

use std::collections::HashMap;

use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled, WeakEntity, Window, div, px,
};

use super::SessionPane;
use crate::actions::DELETE_SESSION_SHORTCUT_NAME;
use crate::actions::{FocusNext, FocusPrevious};
use crate::assets::Icon;
use crate::components::{
    Button, HoverHelp, IconButton, MenuHeader, MenuItem, MenuStart, SplitPart, Tier, home_end_keys,
    spinner_icon, toggle_arrow_keys,
};
use crate::desktop::title_band::{self, Row};
use crate::focus::{self, Mark};
use crate::launcher::entry;
use crate::layers::{self, Anchor, LayerId, Popover, Subject};
use crate::operations::Work;
use crate::prompt::{self, Question};
use crate::session;
use crate::session_list::replace_yolo::{self, Surface};
use crate::session_list::{delete, menu, outcome, replace, restart, stop};
use crate::theme::{self, metrics};

/// The id of the top row's left region, where the terminal area's tab strip and open-tunnels pill go. The pill shows
/// while the session has tunnels open; the strip always, its Agent tab at least. The row's layout (a region that takes
/// the width the actions leave, and shrinks to nothing first) is already the one they need.
pub(crate) const TABS_SLOT: &str = "session-header-tabs";

/// `.session-topbar`'s `min-height`.
const ROW_MIN_HEIGHT: f32 = 30.;
/// `.session-topbar`'s horizontal padding.
const ROW_PAD_X: f32 = 10.;
/// `.session-topbar`'s gap between the left region and the actions.
const ROW_GAP: f32 = 10.;
/// `.titlebar-actions`'s gap between actions.
const ACTIONS_GAP: f32 = 6.;

/// One of the header's actions, in the old order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Action {
    Stop,
    Restart,
    Replace,
    Clone,
    Delete,
}

impl Action {
    /// Every action, in the order the row draws them.
    pub(crate) const ALL: [Action; 5] = [
        Action::Stop,
        Action::Restart,
        Action::Replace,
        Action::Clone,
        Action::Delete,
    ];

    /// The button's id, by which tests and the bridge find it.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Action::Stop => "session-header-stop",
            Action::Restart => "session-header-restart",
            Action::Replace => "session-header-replace",
            Action::Clone => "session-header-clone",
            Action::Delete => "session-header-delete",
        }
    }

    /// The visible label.
    fn label(self) -> &'static str {
        match self {
            Action::Stop => "Stop",
            Action::Restart => "Restart",
            Action::Replace => "Replace",
            Action::Clone => "Clone",
            Action::Delete => "Delete",
        }
    }

    /// The old icon.
    fn icon(self) -> Icon {
        match self {
            Action::Stop => Icon::Stop,
            Action::Restart => Icon::Restart,
            Action::Replace => Icon::Replace,
            Action::Clone => Icon::Clone,
            Action::Delete => Icon::Trash,
        }
    }

    /// Whether the action on `row` would start a session now, and so waits for a host change ([`menu::launch_wait`]).
    /// A Restart that cannot resume does not: it keeps its own reason, which outlasts the change. Exhaustive, so a new
    /// action decides.
    fn would_launch(self, row: &SessionRow) -> bool {
        match self {
            Action::Restart => restart::offered(row),
            Action::Replace => true,
            // Clone opens the launcher rather than starting a session, and waits while the launcher cannot open,
            // which covers a host change already (`menu::launcher_wait`).
            Action::Stop | Action::Clone | Action::Delete => false,
        }
    }

    /// Delete is the one destructive action, so it alone is in the danger tier; the rest are neutral.
    fn tier(self) -> Tier {
        match self {
            Action::Delete => Tier::Danger,
            Action::Stop | Action::Restart | Action::Replace | Action::Clone => Tier::Neutral,
        }
    }
}

/// The hover help of `action` on `row`, as listed and drawn now, with Skip session confirmations as `skipped`. Delete
/// names the platform's Delete shortcut, and drops "after you confirm" when the question is skipped; Restart's says
/// whether the conversation can be resumed and why not ([`restart::hover_help`]).
pub(crate) fn hover_help(action: Action, row: &SessionRow, skipped: bool) -> String {
    match action {
        Action::Stop => "Stop the agent and close its terminal tabs".to_owned(),
        Action::Restart => restart::hover_help(row),
        Action::Replace => replace::MENU_HELP.to_owned(),
        Action::Clone => menu::CLONE_HELP.to_owned(),
        Action::Delete if skipped => {
            format!("Delete: remove this session and its state ({DELETE_SESSION_SHORTCUT_NAME})")
        }
        Action::Delete => format!(
            "Delete: remove this session and its state, after you confirm ({DELETE_SESSION_SHORTCUT_NAME})"
        ),
    }
}

// ---- The split buttons' carets and their menus ----------------------------------------------------------------------

/// One of the two split buttons: Restart and Replace, whose caret opens a menu of the plain action and its "with…"
/// variant (the old `action_menu::ActionKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Split {
    Restart,
    Replace,
}

impl Split {
    /// The split button whose main part is `action`, if it is one.
    fn of(action: Action) -> Option<Split> {
        match action {
            Action::Restart => Some(Split::Restart),
            Action::Replace => Some(Split::Replace),
            Action::Stop | Action::Clone | Action::Delete => None,
        }
    }

    /// The plain action, the main part's.
    pub(crate) fn action(self) -> Action {
        match self {
            Split::Restart => Action::Restart,
            Split::Replace => Action::Replace,
        }
    }

    /// The caret's id, by which tests and the bridge find it; its menu is `<id>-menu`, and the menu's items are
    /// `<id>-plain` and `<id>-with`.
    pub(crate) fn caret_id(self) -> &'static str {
        match self {
            Split::Restart => "session-header-restart-options",
            Split::Replace => "session-header-replace-options",
        }
    }

    /// The action's name, the menu's title and the plain item's label.
    fn label(self) -> &'static str {
        self.action().label()
    }

    /// The variant's label: "Restart with…" or "Replace with…".
    fn with_label(self) -> &'static str {
        match self {
            Split::Restart => "Restart with…",
            Split::Replace => "Replace with…",
        }
    }

    /// Open the variant's launcher on `session` (the launcher's entry points), returning whether it opened.
    fn open_with(self, session: &str, window: &mut Window, cx: &mut App) -> bool {
        match self {
            Split::Restart => entry::open_restart_with(session, window, cx),
            Split::Replace => entry::open_replace_with(session, window, cx),
        }
    }
}

/// What a split button's menu offers for a session now: whether each choice can run, and the description that says what
/// it does or why it cannot. The caret reads it for its look and help, and the open menu for its items, so the two
/// agree.
struct Choices {
    plain_unavailable: bool,
    with_unavailable: bool,
    plain_description: String,
    with_description: String,
}

impl Choices {
    /// `split`'s choices for the session `key`, listed as `row`, with `launcher_wait` saying why the launcher cannot
    /// open now, if it cannot ([`menu::launcher_wait`]). Restart's plain choice is unavailable when the session cannot
    /// resume, and its variant when the launcher says why it is not offered
    /// ([`entry::restart_with_unavailable`]) or cannot open; Replace's plain choice is offered, and its
    /// variant only while the launcher can open. Both plain choices also wait for a host change
    /// ([`menu::launch_wait`]), as the main parts do. An unavailable choice's description is its reason; the others
    /// are the old menu's, through [`menu_line`].
    fn of(
        split: Split,
        key: &str,
        row: &SessionRow,
        launcher_wait: Option<&'static str>,
        cx: &App,
    ) -> Choices {
        // A plain choice that would start a session waits for a host change, as its main part does, saying why.
        let wait = menu::launch_wait(cx);
        match split {
            Split::Restart => {
                let reason = entry::restart_with_unavailable(key, cx);
                let offered = restart::offered(row);
                let plain = match wait.filter(|_| offered) {
                    Some(wait) => wait.to_owned(),
                    None => restart::offer_continuation(
                        &row.info.status,
                        row.info.restart_offer,
                        row.info.agent_kind,
                    ),
                };
                Choices {
                    plain_unavailable: !offered || wait.is_some(),
                    with_unavailable: reason.is_some() || launcher_wait.is_some(),
                    plain_description: menu_line(&plain),
                    with_description: menu_line(reason.as_deref().or(launcher_wait).unwrap_or(
                        "Resume this session's conversation with changed launch settings",
                    )),
                }
            }
            Split::Replace => Choices {
                plain_unavailable: wait.is_some(),
                with_unavailable: launcher_wait.is_some(),
                plain_description: wait
                    .unwrap_or("Swap this session for a fresh conversation with the same settings")
                    .to_owned(),
                with_description: launcher_wait
                    .unwrap_or("Edit the settings, then swap this session for a new one")
                    .to_owned(),
            },
        }
    }

    /// Whether neither choice can run, so the caret is muted and its help says to open it for the reasons.
    fn none_available(&self) -> bool {
        self.plain_unavailable && self.with_unavailable
    }
}

/// An explanation as a menu item's description, the old `menu_description`: sentence case with no closing period, and
/// "Replace with" named with its ellipsis, as the menu item that offers it is labelled. Only the app's own words go
/// through it: it changes the first letter, which in a host value would be a different value.
fn menu_line(text: &str) -> String {
    let text = text.replace("use Replace with", "use Replace with…");
    let text = text.trim_end_matches('.');
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

// ---- What the header keeps -------------------------------------------------------------------------------------------

/// What the header keeps between frames: where each button was last drawn, so its question can hang under it, the
/// question it last opened, during which that button reads as pressed and is inert, and the same for the carets and
/// their menus. Kept by the pane, and checked with the layer manager whenever it is read, so a question or menu closed
/// some other way needs no bookkeeping here.
#[derive(Default)]
pub(super) struct Header {
    anchors: HashMap<Action, Anchor>,
    asked: Option<(Action, LayerId)>,
    /// Each caret as last drawn: the anchor its menu opens below, and its focus handle, which the menu names as where
    /// closing returns focus. Both are element state, made while the caret draws, so its handlers read them from here.
    carets: HashMap<Split, (Anchor, FocusHandle)>,
    /// The caret menu open now, if any, while its caret reads as open.
    menu: Option<(Split, LayerId)>,
}

impl Header {
    /// The split button whose caret menu is open now, if any.
    pub(super) fn open_menu(&mut self, window: &Window, cx: &App) -> Option<Split> {
        let (split, layer) = self.menu?;
        if layers::is_open(layer, window, cx) {
            Some(split)
        } else {
            self.menu = None;
            None
        }
    }

    /// The action whose question is open now, if any.
    pub(super) fn open_question(&mut self, window: &Window, cx: &App) -> Option<Action> {
        let (action, layer) = self.asked?;
        if layers::is_open(layer, window, cx) {
            Some(action)
        } else {
            self.asked = None;
            None
        }
    }
}

// ---- Drawing ---------------------------------------------------------------------------------------------------------

/// What the top row draws for the open session, read from the store and the operation store now.
struct Shown<'a> {
    /// The session's row as the store holds it, listed or carried outside the list.
    row: &'a SessionRow,
    skipped: bool,
    busy: bool,
    /// [`menu::launch_wait`].
    launch_wait: Option<&'static str>,
    /// Why the launcher cannot open now, if it cannot ([`menu::launcher_wait`]), for Clone and the "with…" choices.
    launcher_wait: Option<&'static str>,
    stopping: bool,
    /// The spinner's step, from the pane's spinner.
    step: u8,
}

impl SessionPane {
    /// The top row for the open session filed under `key`, its spinner at `step`, recording where each button is drawn.
    pub(super) fn header_row(
        &mut self,
        key: &str,
        step: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = theme::palette(cx);
        // In the macOS window the row is the session's half of the window's title band: as tall as the sidebar's app
        // bar, and a press on its empty background drags the window (`desktop::title_band`).
        let min_height = if title_band::ON {
            title_band::HEIGHT
        } else {
            px(ROW_MIN_HEIGHT)
        };
        // The terminal area's part of the left region: the strip of the session's terminals, then the open-tunnels pill
        // while the session has tunnels open (the agent terminal's: SPEC.md's tunnels belong to the session). Buttons,
        // so they keep their presses inside the title band.
        let set = self
            .mounted
            .current(cx)
            .as_ref()
            .and_then(|mounted| mounted.set.clone());
        let strip = set
            .as_ref()
            .map(|set| crate::terminal::set::TerminalSet::strip(set, window, cx));
        let pill = set
            .map(|set| set.read(cx).agent().clone())
            .and_then(|terminal| {
                terminal.update(cx, |terminal, cx| terminal.tunnel_pill(window, cx))
            });
        let row = div()
            .id("session-header")
            .debug_selector(|| "session-header".into())
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(px(ROW_GAP))
            .min_h(min_height)
            .px(px(ROW_PAD_X))
            .overflow_hidden()
            .bg(theme::color(palette.bg_1))
            .text_size(metrics::TEXT_MD)
            // The left region: the terminal area's tabs and tunnel pill go here. It takes the width the actions leave
            // and gives it up first (`min_w_0`), so the actions never shrink at the 650-pixel floor.
            .child(
                div()
                    .id(TABS_SLOT)
                    .debug_selector(|| TABS_SLOT.into())
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .gap(px(ROW_GAP))
                    .children(strip)
                    .children(pill),
            );
        let row = title_band::row(row, Row::SessionTop);
        let Some(store) = session::current_store(cx) else {
            return row.into_any_element();
        };
        let collections = store.read(cx).collections();
        let row_now = collections.get::<SessionRow>(key).cloned();
        let skipped = menu::skips_questions(collections);
        if let Some(guard) = delete::in_flight(key, cx) {
            self.header.anchors.clear();
            self.header.carets.clear();
            return row
                // Text in the title band keeps its presses: it must not move the window (`desktop::title_band`).
                .child(title_band::claims_presses(
                    div().child(delete_progress(key, guard, step, cx)),
                ))
                .into_any_element();
        }
        // No row, no actions (the module docs' "The open session by key").
        let Some(row_now) = row_now else {
            self.header.anchors.clear();
            self.header.carets.clear();
            return row.into_any_element();
        };
        let shown = Shown {
            row: &row_now,
            skipped,
            // The interrupted card's open Replace question refuses these too, as any open lifecycle question does.
            busy: menu::refused(key, cx) || self.card_asking(cx),
            launch_wait: menu::launch_wait(cx),
            launcher_wait: menu::launcher_wait(cx),
            stopping: stop::in_flight(key, cx),
            step,
        };
        let open = self.header.open_question(window, cx);
        let menu_open = self.header.open_menu(window, cx);
        let pane = cx.weak_entity();
        let mut anchors = HashMap::new();
        let mut carets = HashMap::new();
        let mut buttons: Vec<AnyElement> = Vec::new();
        for action in Action::ALL {
            let anchor = Anchor::for_element(
                SharedString::from(format!("{}-anchor", action.id())),
                window,
                cx,
            );
            anchors.insert(action, anchor.clone());
            let button = action_button(action, &shown, open == Some(action), &pane, cx);
            // Each action is its own anchor, exactly as wide as its button, as the old `.restart-offer`
            // wrappers were, so its question hangs under it.
            let main = div()
                .relative()
                .flex_none()
                .child(button)
                .child(anchor.track());
            let Some(split) = Split::of(action) else {
                buttons.push(main.into_any_element());
                continue;
            };
            let choices = Choices::of(split, key, shown.row, shown.launcher_wait, cx);
            // The session's key is in the anchor's id, so a caret menu closes with its session: the next session's
            // caret is another anchor, and an anchor that is no longer drawn closes its popover.
            let anchor = Anchor::for_element(
                SharedString::from(format!("{}-anchor-{key}", split.caret_id())),
                window,
                cx,
            );
            let focus = focus::control_handle(
                SharedString::from(format!("{}-focus-handle", split.caret_id())),
                window,
                cx,
            );
            carets.insert(split, (anchor.clone(), focus.clone()));
            let caret = caret_button(
                split,
                &choices,
                shown.busy,
                menu_open == Some(split),
                &focus,
                &pane,
            );
            // The two parts side by side with no gap, as the old `.header-action-split`; the row's default
            // stretch gives the icon-only caret the main part's height. The caret is its menu's anchor.
            buttons.push(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .child(main)
                    .child(div().relative().flex().child(caret).child(anchor.track()))
                    .into_any_element(),
            );
        }
        self.header.anchors = anchors;
        self.header.carets = carets;
        row.child(
            div()
                .id("session-header-actions")
                .debug_selector(|| "session-header-actions".into())
                .flex()
                .flex_row()
                .flex_none()
                .items_center()
                .gap(px(ACTIONS_GAP))
                // The terminal area's text size group starts the right-hand group (SPEC.md's top row).
                .child(crate::terminal::text_size::controls(cx))
                .children(buttons),
        )
        .into_any_element()
    }

    /// The lines between the top row and the terminal for `key`: a Restart's or Replace's wait once it has lasted two
    /// seconds, then the session's refusal or notice, as the old header's progress and error lines were, then the YOLO
    /// question a Replace from this header met ([`replace_yolo`]). The wait sits below the row, never in it, so a long
    /// host name cannot push an action out of the 650-pixel floor; the question sits there for the same reason. None
    /// while the session shows its interrupted card (`card`), which draws all three itself, so nothing shows twice.
    pub(super) fn header_lines(
        &self,
        key: &str,
        step: u8,
        card: bool,
        cx: &App,
    ) -> Vec<AnyElement> {
        let mut lines = Vec::new();
        if card {
            return lines;
        }
        let ink = theme::color(theme::palette(cx).fg_0);
        let row = session::current_store(cx)
            .and_then(|store| store.read(cx).collections().get::<SessionRow>(key).cloned());
        if let Some(row) = &row {
            for work in [Work::Restart, Work::Replace] {
                if let Some(line) =
                    outcome::wait_line("session-header", work, true, key, row, step, ink, cx)
                {
                    lines.push(line.py(px(4.)).px(px(12.)).into_any_element());
                }
            }
        }
        if let Some(line) = outcome::pane_line(key, cx) {
            lines.push(line.into_any_element());
        }
        lines.extend(replace_yolo::header_question(key, cx));
        lines
    }

    /// Whether the header shows anything that turns the spinner for `key`: a Stop in flight, or a Restart or Replace
    /// that has waited long enough to say so.
    pub(super) fn header_spins(key: &str, cx: &App) -> bool {
        stop::in_flight(key, cx)
            || outcome::waited_seconds(key, Work::Restart, cx).is_some()
            || outcome::waited_seconds(key, Work::Replace, cx).is_some()
    }

    /// The user activated `action` in the open session's header at `mark`. It goes as the row's menu goes for the same
    /// session, through the same functions, with the question (when it asks) hung under the action's button. Nothing
    /// happens while that question is open, for a session the store holds no row for, or for one with a request in
    /// flight.
    pub(crate) fn header_action(
        &mut self,
        action: Action,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self.open_key(cx) else {
            return;
        };
        if self.header.open_question(window, cx).is_some() || self.card_asking(cx) {
            return;
        }
        let Some(target) = menu::actionable(&key, cx) else {
            return;
        };
        let (row, skipped) = (&target.row, target.skips_questions);
        let asking = Asking {
            action,
            key: &key,
            mark,
        };
        // Each action's own `begin` decides whether it asks or goes ahead, the same decision the row's menu makes, so
        // the two cannot drift apart; the header adds only where its questions hang.
        match action {
            Action::Stop => {
                // A session on a host that is not connected cannot be stopped; the button is drawn disabled, and a
                // stale press does nothing.
                if row.stale {
                    return;
                }
                if let Some(ask) = stop::begin(&key, row, skipped, cx) {
                    self.ask(asking, ask, stop::confirm, window, cx);
                }
            }
            Action::Restart => {
                if let Some(ask) = restart::begin(&key, row, skipped, cx) {
                    self.ask(asking, ask, restart::confirm, window, cx);
                }
            }
            Action::Replace => {
                if let Some(ask) = replace::begin(
                    &key,
                    row,
                    skipped,
                    Surface::Header,
                    window.window_handle(),
                    cx,
                ) {
                    self.ask(asking, ask, replace::confirm, window, cx);
                }
            }
            Action::Clone => {
                // The launcher refuses while New is unavailable, as the button is drawn then; a press that beats
                // that frame opens nothing.
                if !entry::open_clone(&key, window, cx) {
                    tracing::debug!("the launcher refused to open Clone from the header");
                }
            }
            Action::Delete => {
                // SPEC.md: the header's Delete "asks first in the same anchored way, even for a session that has
                // ended", unlike the row's, which deletes a session with nothing alive unasked.
                if let Some(ask) = delete::begin(&key, row, skipped, true, cx) {
                    self.ask(asking, ask, delete::confirm, window, cx);
                }
            }
        }
    }

    /// Ask the question `question` builds for `asking`'s action about its session, hung under the action's button as
    /// last drawn (centered if it was not drawn), returning focus home on Escape. Nothing opens when the user has moved
    /// on since `asking`'s mark. The question closes when the session is gone.
    fn ask<T: Clone + 'static>(
        &mut self,
        asking: Asking,
        question: impl Fn() -> Question<T>,
        on_confirm: impl FnOnce(T, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Asking { action, key, mark } = asking;
        let question = question().returns_home_on_escape();
        let subject = Subject::session(key, cx);
        let asked = match self.header.anchors.get(&action).cloned() {
            Some(anchor) => {
                prompt::ask_anchored(question, anchor, subject, mark, on_confirm, window, cx)
            }
            None => prompt::ask(question, subject, mark, on_confirm, window, cx),
        };
        if let Some(layer) = asked {
            self.header.asked = Some((action, layer));
            cx.notify();
        }
    }
}

/// Which action asks a question, about which session, for which of the user's gestures: what [`SessionPane::ask`] needs
/// besides the question itself.
#[derive(Clone, Copy)]
struct Asking<'a> {
    action: Action,
    key: &'a str,
    mark: &'a Mark,
}

/// `action`'s button as `shown` says, pressed while its question is open (`asking`), activating through `pane`.
fn action_button(
    action: Action,
    shown: &Shown,
    asking: bool,
    pane: &WeakEntity<SessionPane>,
    cx: &App,
) -> Button {
    let pane = pane.clone();
    let row = shown.row;
    // A launch waiting for a host change says why in place of what it does, and so does Clone while the launcher
    // cannot open.
    let wait = match action {
        Action::Clone => shown.launcher_wait,
        _ => shown.launch_wait.filter(|_| action.would_launch(row)),
    };
    let help = wait.map_or_else(|| hover_help(action, row, shown.skipped), str::to_owned);
    let stopping = action == Action::Stop && shown.stopping;
    let label = if stopping {
        stop::PROGRESS_LABEL
    } else {
        action.label()
    };
    let mut button = Button::new(action.id(), label, HoverHelp::tooltip(help), Role::Button)
        .icon(action.icon())
        .tier(action.tier())
        .compact()
        .expanded(asking)
        .on_click(move |_, window, cx| {
            let mark = Mark::take(cx);
            // The pane may be gone with its window; then there is nothing to act on.
            let _ = pane.update(cx, |pane, cx| pane.header_action(action, &mark, window, cx));
        });
    if stopping {
        // The old header's Stop while stopping: its icon, then the spinner and "Stopping…" in place of the label, which
        // stays the button's accessible name.
        let ink = theme::color(theme::palette(cx).fg_0);
        button = button.content(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .child(spinner_icon(shown.step, px(12.), ink))
                .child(stop::PROGRESS_LABEL),
        );
    }
    if Split::of(action).is_some() {
        button = button.split(SplitPart::Main);
    }
    match action {
        // Busy rather than disabled while waiting, so the reason in the hover help stays reachable.
        Action::Restart => button
            .accessible_name(restart::offer_label(row.info.restart_offer))
            .busy(!restart::offered(row) || wait.is_some())
            .disabled(shown.busy),
        Action::Replace => button.busy(wait.is_some()).disabled(shown.busy),
        Action::Stop => button.disabled(shown.busy || row.stale),
        // Clone opens the launcher, so it waits while the launcher cannot open, as Restart and Replace wait.
        Action::Clone => button.busy(wait.is_some()).disabled(shown.busy),
        Action::Delete => button.disabled(shown.busy),
    }
}

/// `split`'s caret, offering `choices`, drawn open while its menu is (`open`), tracking `focus`, and opening its menu
/// through `pane`. It is unavailable only while the session's commands are refused (`busy`), as the old caret was:
/// while neither choice can run it is muted but still opens the menu, which is where the reasons are (SPEC.md: "the
/// menu remains available to inspect reasons"). Down and Up open the closed menu on its first or last choice, as the
/// old caret did.
fn caret_button(
    split: Split,
    choices: &Choices,
    busy: bool,
    open: bool,
    focus: &FocusHandle,
    pane: &WeakEntity<SessionPane>,
) -> IconButton {
    let help = if choices.none_available() {
        "Neither option is available. Open the menu for the reasons.".to_owned()
    } else {
        format!(
            "{} options: use the current settings or edit them first",
            split.label()
        )
    };
    let (clicked, keyed) = (pane.clone(), pane.clone());
    IconButton::new(
        split.caret_id(),
        Icon::ChevronDown,
        format!("{} options", split.label()),
        HoverHelp::tooltip(help),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .compact()
    .split(SplitPart::Caret {
        muted: choices.none_available(),
    })
    .expanded(open)
    .disabled(busy)
    .focus(focus)
    .on_key_down(toggle_arrow_keys(move |start, window, cx| {
        let _ = keyed.update(cx, |pane, cx| {
            pane.open_caret_menu(split, start, window, cx)
        });
    }))
    .on_click(move |_, window, cx| {
        // The pane may be gone with its window; then there is no menu to open.
        let _ = clicked.update(cx, |pane, cx| pane.toggle_caret_menu(split, window, cx));
    })
}

impl SessionPane {
    /// `split`'s caret was activated (a click, Enter or Space): open its menu on the first choice, or close it if it is
    /// open.
    fn toggle_caret_menu(&mut self, split: Split, window: &mut Window, cx: &mut Context<Self>) {
        if self.header.open_menu(window, cx) == Some(split) {
            self.close_caret_menu(window, cx);
        } else {
            self.open_caret_menu(split, MenuStart::First, window, cx);
        }
    }

    /// Open `split`'s menu below its caret with focus on the choice `start` names, unless a caret menu is open
    /// already, the caret was not drawn, or the open session's commands are refused. The menu keeps Tab, as every menu
    /// does, and names the caret as where closing returns focus; its own Tab closes it ([`CaretMenu`]).
    fn open_caret_menu(
        &mut self,
        split: Split,
        start: MenuStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.header.open_menu(window, cx).is_some() {
            return;
        }
        let Some(key) = self.open_key(cx) else {
            return;
        };
        if menu::refused(&key, cx) {
            return;
        }
        let Some((anchor, caret)) = self.header.carets.get(&split).cloned() else {
            return;
        };
        let mark = Mark::take(cx);
        let items: [FocusHandle; 2] = std::array::from_fn(|_| cx.focus_handle().tab_stop(true));
        let start = match start {
            MenuStart::First => items[0].clone(),
            MenuStart::Last => items[1].clone(),
        };
        let pane = cx.weak_entity();
        let view = cx.new(|_| CaretMenu {
            pane,
            key,
            split,
            items,
        });
        let opened = layers::open_popover(
            Popover::menu(view, cx.focus_handle(), anchor, start)
                // The caret, not what held focus before: on macOS the click that opened the menu left the caret
                // unfocused, and SPEC.md returns focus to the caret when the menu closes.
                .returns_to(focus::ReturnTo::Control(caret)),
            &mark,
            window,
            cx,
        );
        self.header.menu = opened.map(|layer| (split, layer));
        cx.notify();
    }

    /// Close the caret menu if one is open, which returns focus to its caret.
    fn close_caret_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, layer)) = self.header.menu.take() {
            layers::close(layer, window, cx);
            cx.notify();
        }
    }

    /// A choice of `split`'s menu, opened on the session `key`, was activated at `mark`: close the menu, which returns
    /// focus to the caret, then go on as the plain action's button goes on ([`SessionPane::header_action`], so the
    /// same question hangs under the main part), or open the variant's launcher, which returns focus to the caret when
    /// it closes (SPEC.md: "Cancelling Restart with returns to that persistent caret"). Nothing happens when the open
    /// session is no longer `key`.
    fn caret_choice(
        &mut self,
        split: Split,
        key: &str,
        with: bool,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_caret_menu(window, cx);
        if self.open_key(cx).as_deref() != Some(key) {
            return;
        }
        if !with {
            self.header_action(split.action(), mark, window, cx);
        } else if menu::actionable(key, cx).is_some() && !split.open_with(key, window, cx) {
            tracing::debug!(
                ?split,
                "the launcher refused to open from the header's menu"
            );
        }
    }
}

/// A split button's open menu (the old `ActionMenu`'s flyout): a header naming the action, then the plain choice and
/// its "with…" variant, each with an icon, a label and a line saying what it does or why it cannot. Drawn from the
/// store while it is open, so availability and reasons follow the session.
///
/// An unavailable choice stays focusable, so its reason is reachable by keyboard, and does nothing when activated
/// ([`MenuItem::disabled`]). Up and Down walk the two choices (the layer manager's menu context), Home and End go to
/// the first and the last ([`home_end_keys`]), and Escape closes the menu with focus back on the caret, all as the old
/// menu did. Tab closes it the same way, which SPEC.md asks of these menus alone ("Escape or Tab closes the menu and
/// returns focus to its caret"): the menu takes Tab's action itself, before the focus coordinator's trap would keep it
/// inside, so it needs nothing from the layer manager.
pub(crate) struct CaretMenu {
    pane: WeakEntity<SessionPane>,
    key: String,
    split: Split,
    /// The two choices' focus handles, plain first: the menu starts on either, and Home and End move there.
    items: [FocusHandle; 2],
}

/// The old `.header-action-menu`'s width.
const CARET_MENU_WIDTH: f32 = 260.;

impl Render for CaretMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.split.caret_id();
        let panel = layers::panel(cx)
            .id(SharedString::from(format!("{id}-menu")))
            .debug_selector(move || format!("{id}-menu"))
            .w(px(CARET_MENU_WIDTH));
        let Some(store) = session::current_store(cx) else {
            return panel;
        };
        let Some(row) = store
            .read(cx)
            .collections()
            .get::<SessionRow>(&self.key)
            .cloned()
        else {
            return panel;
        };
        let launcher_wait = menu::launcher_wait(cx);
        let choices = Choices::of(self.split, &self.key, &row, launcher_wait, cx);
        let busy = menu::refused(&self.key, cx);
        let summary = if choices.none_available() {
            "Unavailable for this session"
        } else {
            match self.split {
                Split::Restart => "Resume a conversation in this session",
                Split::Replace => "Start a fresh conversation",
            }
        };
        let item = |with: bool| {
            let (label, icon, description, unavailable) = if with {
                (
                    self.split.with_label(),
                    Icon::Adjust,
                    choices.with_description.clone(),
                    choices.with_unavailable,
                )
            } else {
                (
                    self.split.label(),
                    self.split.action().icon(),
                    choices.plain_description.clone(),
                    choices.plain_unavailable,
                )
            };
            let (pane, key, split) = (self.pane.clone(), self.key.clone(), self.split);
            MenuItem::new(
                format!("{id}-{}", if with { "with" } else { "plain" }),
                icon,
                label,
                HoverHelp::tooltip(description.clone()),
                Role::MenuItem,
            )
            .description(description)
            .disabled(busy || unavailable)
            .focus(&self.items[usize::from(with)])
            .on_click(move |_, window, cx| {
                let mark = Mark::take(cx);
                let _ = pane.update(cx, |pane, cx| {
                    pane.caret_choice(split, &key, with, &mark, window, cx)
                });
            })
        };
        let (first, last) = (self.items[0].clone(), self.items[1].clone());
        let (tabbed, shift_tabbed) = (self.pane.clone(), self.pane.clone());
        panel
            // Home and End, the old menu's: the layer manager's menu context binds only Up and Down.
            .on_key_down(home_end_keys(first, last))
            // Tab and Shift+Tab close the menu with focus back on the caret (see the type's docs). Handled before the
            // root's handler, which would walk the menu's choices.
            .on_action(move |_: &FocusNext, window, cx| tab_out(&tabbed, window, cx))
            .on_action(move |_: &FocusPrevious, window, cx| tab_out(&shift_tabbed, window, cx))
            .child(MenuHeader::app(self.split.label(), summary))
            .child(
                div()
                    .id(SharedString::from(format!("{id}-items")))
                    .role(Role::Menu)
                    .aria_label(self.split.label())
                    .flex()
                    .flex_col()
                    .child(item(false))
                    .child(item(true)),
            )
    }
}

/// Tab or Shift+Tab in `pane`'s open caret menu: dismiss it, as Escape does, which returns focus to its caret. A
/// dismissal rather than a plain close, because the focus coordinator then swallows only the key's held repeats: a
/// plain close would also swallow a second Tab pressed within half a second, which SPEC.md's caret rule means to go on
/// from the caret to the next control.
fn tab_out(pane: &WeakEntity<SessionPane>, window: &mut Window, cx: &mut App) {
    let open = pane
        .upgrade()
        .and_then(|pane| pane.read(cx).header.menu.map(|(_, layer)| layer));
    if let Some(layer) = open {
        layers::dismiss(layer, window, cx);
    }
}

/// What takes the actions' place while the session's Delete, sent with `guard`, is in flight: the spinner at `step` and
/// the row's own words for it ([`delete::progress_label`]), in the accent, as the old `.header-delete-progress` was.
fn delete_progress(
    key: &str,
    guard: farhelm_proto::DeleteGuard,
    step: u8,
    cx: &App,
) -> impl IntoElement {
    let ink = theme::color(theme::palette(cx).accent);
    let label = delete::progress_label(guard);
    div()
        .id(SharedString::from(format!(
            "session-header-delete-progress-{key}"
        )))
        .debug_selector(|| "session-header-delete-progress".into())
        .role(Role::Status)
        .aria_label(label)
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .whitespace_nowrap()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(spinner_icon(step, px(12.), ink))
        .child(label)
}
