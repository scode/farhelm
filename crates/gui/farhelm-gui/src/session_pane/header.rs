//! The open session's top row and its lifecycle actions: Stop, Restart, Replace and Delete, at the right of the row,
//! with their questions anchored under their buttons.
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
//! - **The actions, in the old order.** Stop, Restart, Replace, Delete, each a compact button with the old icon, label
//!   and hover help; Delete in the danger tier, the rest neutral. Restart's split caret, Replace's, and Clone between
//!   Replace and Delete are the launcher's and are not drawn yet; the terminal area adds the text size group at the
//!   start of the actions.
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
//! - **Exclusion.** While the session has any request in flight, or the YOLO question its Replace met is open, every
//!   action is unavailable, as the row's menu items are (`session_list::menu::refused`); a pending Stop shows its
//!   spinner and "Stopping…" on the Stop button, and a pending Delete shows its progress in place of the actions until
//!   it settles (they come back with a refusal). While a question an action asked is open, its button reads as pressed
//!   and a click on it does nothing; a press anywhere else closes the question unanswered.
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
//! store and the shared clocks while it draws; the only state it keeps is where each button was drawn and which
//! question it opened ([`Header`]).

use std::collections::HashMap;

use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    Role, SharedString, StatefulInteractiveElement as _, Styled, WeakEntity, Window, div, px,
};

use super::SessionPane;
use crate::actions::DELETE_SESSION_SHORTCUT_NAME;
use crate::assets::Icon;
use crate::components::{Button, HoverHelp, Tier, spinner_icon};
use crate::desktop::title_band::{self, Row};
use crate::focus::Mark;
use crate::layers::{self, Anchor, LayerId, Subject};
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
    Delete,
}

impl Action {
    /// Every action, in the order the row draws them.
    pub(crate) const ALL: [Action; 4] = [
        Action::Stop,
        Action::Restart,
        Action::Replace,
        Action::Delete,
    ];

    /// The button's id, by which tests and the bridge find it.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Action::Stop => "session-header-stop",
            Action::Restart => "session-header-restart",
            Action::Replace => "session-header-replace",
            Action::Delete => "session-header-delete",
        }
    }

    /// The visible label.
    fn label(self) -> &'static str {
        match self {
            Action::Stop => "Stop",
            Action::Restart => "Restart",
            Action::Replace => "Replace",
            Action::Delete => "Delete",
        }
    }

    /// The old icon.
    fn icon(self) -> Icon {
        match self {
            Action::Stop => Icon::Stop,
            Action::Restart => Icon::Restart,
            Action::Replace => Icon::Replace,
            Action::Delete => Icon::Trash,
        }
    }

    /// Delete is the one destructive action, so it alone is in the danger tier; the rest are neutral.
    fn tier(self) -> Tier {
        match self {
            Action::Delete => Tier::Danger,
            Action::Stop | Action::Restart | Action::Replace => Tier::Neutral,
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
        Action::Delete if skipped => {
            format!("Delete: remove this session and its state ({DELETE_SESSION_SHORTCUT_NAME})")
        }
        Action::Delete => format!(
            "Delete: remove this session and its state, after you confirm ({DELETE_SESSION_SHORTCUT_NAME})"
        ),
    }
}

// ---- What the header keeps -------------------------------------------------------------------------------------------

/// What the header keeps between frames: where each button was last drawn, so its question can hang under it, and the
/// question it last opened, during which that button reads as pressed and is inert. Kept by the pane, and checked with
/// the layer manager whenever it is read, so a question closed some other way needs no bookkeeping here.
#[derive(Default)]
pub(super) struct Header {
    anchors: HashMap<Action, Anchor>,
    asked: Option<(Action, LayerId)>,
}

impl Header {
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
            .map(|mounted| mounted.set.clone());
        let strip = set.as_ref().map(|set| set.read(cx).strip(set, window));
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
            return row.into_any_element();
        };
        let shown = Shown {
            row: &row_now,
            skipped,
            busy: menu::refused(key, cx),
            stopping: stop::in_flight(key, cx),
            step,
        };
        let open = self.header.open_question(window, cx);
        let pane = cx.weak_entity();
        let mut anchors = HashMap::new();
        let buttons: Vec<AnyElement> = Action::ALL
            .into_iter()
            .map(|action| {
                let anchor = Anchor::for_element(
                    SharedString::from(format!("{}-anchor", action.id())),
                    window,
                    cx,
                );
                anchors.insert(action, anchor.clone());
                let button = action_button(action, &shown, open == Some(action), &pane, cx);
                // Each action is its own anchor, exactly as wide as its button, as the old `.restart-offer`
                // wrappers were, so its question hangs under it.
                div()
                    .relative()
                    .flex_none()
                    .child(button)
                    .child(anchor.track())
                    .into_any_element()
            })
            .collect();
        self.header.anchors = anchors;
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
    /// host name cannot push an action out of the 650-pixel floor; the question sits there for the same reason.
    pub(super) fn header_lines(&self, key: &str, step: u8, cx: &App) -> Vec<AnyElement> {
        let mut lines = Vec::new();
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
        if self.header.open_question(window, cx).is_some() {
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
    let help = hover_help(action, row, shown.skipped);
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
    match action {
        Action::Restart => button
            .accessible_name(restart::offer_label(row.info.restart_offer))
            .busy(!restart::offered(row))
            .disabled(shown.busy),
        Action::Stop => button.disabled(shown.busy || row.stale),
        Action::Replace | Action::Delete => button.disabled(shown.busy),
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
