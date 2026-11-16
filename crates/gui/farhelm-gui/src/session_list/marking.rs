//! Bulk marking: Shift-click and Cmd-click (Ctrl-click on Linux) mark sessions in the list without opening them, and
//! the heading's bar acts on the marked sessions that are shown.
//!
//! SPEC.md, "Session list" (the marks paragraphs). Ported from the old app's `list/marking.rs` (`Gesture`, `mark`,
//! `effective`, `MetadataAction`, `admitted_targets`, `MarkedBar`) at reference commit `24bc0580`. Here's the TLDR of
//! its rules:
//!
//! - **Gestures.** Shift-click marks the range from the open session to the clicked row in the order the list shows
//!   (pins first, as the order hold keeps it) and replaces every earlier mark; with the open session not shown, it
//!   marks only the clicked row. Cmd-click or Ctrl-click toggles one row, the open one included;
//!   [`Gesture::from_modifiers`] takes either key on every platform, but on macOS the platform layer turns a Ctrl-click
//!   into a secondary click before it reaches the row, so there Cmd-click is the toggle, as SPEC.md now says.
//!   Shift wins when both are held, and Alt changes nothing. A plain click opens the session and clears every mark.
//!   Marking never changes which session is open. Only the row's open control reads a gesture: its menu button, its
//!   status dot and its bell keep their own actions and neither mark nor clear.
//! - **Stored and effective marks.** The window keeps the marked keys, never saved, per UI session, so a reset forgets
//!   them as the old app's reload did. Every count, offer and target is projected from the rows shown now
//!   ([`effective`], [`project`]): a filter or the Hosts panel hides a mark without forgetting it, a hidden mark is
//!   never counted or acted on, and a listing that is not available shows none. Only a listing that proves a session
//!   gone (`session::listing_proves_gone`) forgets its mark.
//! - **Targets never grow.** An action takes the targets as they were drawn, keeps only those still shown and marked
//!   when it runs, and never adds one ([`admitted`]).
//! - **The marked bar.** While any mark is effective, the heading's band holds "N marked", Stop (drawn only while some
//!   mark shown is not a retired session, which supports Delete only) and Delete (their batches and question are
//!   [`super::bulk`]'s), the marked menu's ⋯ (drawn only while one of its actions applies to a marked row) and Clear,
//!   in place of the order's title, the count, Trash, Templates and New. The cut listing's warning and the failure line
//!   stay under it. Trash's view and any dialog it opened stay alive; only its button is not drawn. New's shortcut
//!   still opens the launcher.
//! - **The metadata actions** ([`MetadataAction`]): Pin for marked rows that are not pinned, Unpin for those that are,
//!   Mark read and Mark unread for the rows whose own menu offers it, none of them for a retired containerized session,
//!   which supports Delete only (`super::retired`). Each sends the row's ordinary request, so its refusal shows on the
//!   row as the row menu's would. A row whose own commands are refused at that moment (`menu::refused`: a request in
//!   flight or held for a bulk Stop or Delete, its YOLO question or its interrupted card's Replace question open) is
//!   skipped and shows its work's "not started" line ([`super::bulk`]). The marks are cleared once the writes are sent,
//!   not when they are answered.
//! - **Escape** in the list's column clears the marks and lets the key go on, so the window's own Escape (a tooltip
//!   put away) still happens. A menu, a dialog, an inline question or the terminal that holds focus takes Escape before
//!   it reaches the list, and keeps the marks.
//! - **Focus.** Clear, Escape and a metadata action hand focus to the order's title when it was inside the bar or its
//!   menu, since the control that held it goes with the bar, as the old bar's did; a press made elsewhere since leaves
//!   focus alone. Marks that go for another reason (a filter hides them all) move no focus; the focus coordinator's
//!   recovery takes over.
//!
//! An interaction counter ([`Marks::revision`]) moves with every marking choice, so a bulk Stop or Delete accepted
//! earlier can tell whether the user has made a newer one since, and clears its own targets only if not.
//!
//! NOTE: Sandbox and recipe container rows get nothing special here yet: their environment-wide cases come with the
//! environments work. GPUI cannot report `aria-haspopup`.
//!
//! A change of the Hosts attention list above the heading closes the marked menu, as it closes the rows' menus
//! ([`super::attention`]).

use std::collections::HashSet;
use std::rc::Rc;

use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, Modifiers, ParentElement, Render, Role, StatefulInteractiveElement as _, Styled,
    WeakEntity, Window, div, px,
};

use super::seen::{self, Toggle};
use super::selection::Listed;
use super::{HEADING_ICON_SIDE, SessionList, bulk, menu, pins, retired};
use crate::assets::Icon;
use crate::components::{
    Button, HoverHelp, IconButton, MenuHeader, MenuItem, MenuStart, Tier, home_end_keys,
    toggle_arrow_keys,
};
use crate::focus::{self, Mark};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::local_state;
use crate::operations::Work;
use crate::theme::{self, metrics};

/// What a click on a row's open button asks for, read from the modifiers held (the old `Gesture::from_modifiers`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gesture {
    /// A plain click: open the session and clear every mark.
    Open,
    /// Shift: mark the range from the open session to this row.
    Range,
    /// Cmd or Ctrl: toggle this row's mark.
    Toggle,
}

impl Gesture {
    /// The gesture for a click with `modifiers`: Shift wins over Cmd and Ctrl, and Alt and Fn change nothing. A
    /// keyboard activation reports no modifiers, so it always opens.
    ///
    /// NOTE: Ctrl counts on every platform, as the old app's did, but on macOS GPUI's platform layer turns a Ctrl+left
    /// press into a right press and drops the Ctrl before any element sees it (`vendor/gpui-pre-macos/src/window.rs`),
    /// so a Ctrl-click never reaches a row there: Cmd-click is the Mac's toggle.
    pub(crate) fn from_modifiers(modifiers: &Modifiers) -> Gesture {
        if modifiers.shift {
            Gesture::Range
        } else if modifiers.platform || modifiers.control {
            Gesture::Toggle
        } else {
            Gesture::Open
        }
    }
}

/// The marks a window keeps for one UI session.
#[derive(Debug, Default)]
pub(super) struct Marks {
    /// The marked sessions' keys, shown or hidden.
    stored: HashSet<String>,
    /// How many marking choices this UI session made: each gesture, Clear, plain-click clear, opening the marked menu or
    /// a bulk question, and dispatching a metadata action counts one. A bulk batch, which clears its own targets when it
    /// finishes, compares it with the value it was accepted under.
    revision: u64,
}

impl Marks {
    /// Apply `gesture` on the row `target` to the marks, with `displayed` the keys the list shows in their order and
    /// `anchor` the open session: a plain click clears every mark wherever it landed, and a range or a toggle on a row
    /// the list does not show changes nothing. Every call counts as a choice.
    pub(super) fn mark(
        &mut self,
        displayed: &[impl AsRef<str>],
        anchor: Option<&str>,
        target: &str,
        gesture: Gesture,
    ) {
        self.revision += 1;
        let position = |key: &str| displayed.iter().position(|shown| shown.as_ref() == key);
        match gesture {
            Gesture::Open => self.stored.clear(),
            Gesture::Toggle => {
                if position(target).is_some() && !self.stored.remove(target) {
                    self.stored.insert(target.to_owned());
                }
            }
            Gesture::Range => {
                let Some(at) = position(target) else {
                    return;
                };
                let from = anchor.and_then(position).unwrap_or(at);
                let (low, high) = (from.min(at), from.max(at));
                self.stored = displayed[low..=high]
                    .iter()
                    .map(|key| key.as_ref().to_owned())
                    .collect();
            }
        }
    }

    /// Whether no session is marked, shown or not.
    pub(super) fn is_empty(&self) -> bool {
        self.stored.is_empty()
    }

    /// Forget every mark (Clear, Escape in the list, a metadata action), as a choice.
    pub(super) fn clear(&mut self) {
        self.revision += 1;
        self.stored.clear();
    }

    /// Count a choice that changes no mark: opening the marked menu or a bulk question.
    pub(super) fn touch(&mut self) {
        self.revision += 1;
    }

    /// Forget the marks `gone` names: sessions a listing proves gone, or a finished batch's own targets. Not a choice:
    /// work waiting on the revision is not disturbed.
    pub(super) fn prune(&mut self, gone: impl Fn(&str) -> bool) {
        self.stored.retain(|key| !gone(key));
    }

    /// Whether `key` is marked, shown or not.
    pub(super) fn contains(&self, key: &str) -> bool {
        self.stored.contains(key)
    }

    /// The current choice's number (see the field).
    pub(super) fn revision(&self) -> u64 {
        self.revision
    }
}

/// The marked keys among `displayed`, in its order: the marks that count, are drawn and are acted on.
pub(super) fn effective<'a>(displayed: &'a [impl AsRef<str>], marks: &Marks) -> Vec<&'a str> {
    displayed
        .iter()
        .map(AsRef::as_ref)
        .filter(|key| marks.contains(key))
        .collect()
}

/// One target as an action drew it: its key, and the row's own request the action sends it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Target {
    pub(super) key: String,
    pub(super) write: Write,
}

/// The ordinary row request a metadata action sends one target ([`MetadataAction::target`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Write {
    /// Pin to top (`true`) or Unpin.
    Pin(bool),
    /// Mark read or Mark unread, as the row's own menu would send it: Mark read carries the activity stamp the drawn
    /// row showed, so that output arriving after the draw stays unseen.
    Seen(Toggle),
}

/// The targets of `drawn` still shown (`displayed`) and still marked when the action runs, with their drawn values: a
/// session marked or shown since the draw is never added.
pub(super) fn admitted(
    drawn: &[Target],
    displayed: &[impl AsRef<str>],
    marks: &Marks,
) -> Vec<Target> {
    drawn
        .iter()
        .filter(|target| marks.contains(&target.key))
        .filter(|target| displayed.iter().any(|key| key.as_ref() == target.key))
        .cloned()
        .collect()
}

/// The marked menu's actions, in the menu's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MetadataAction {
    Pin,
    Unpin,
    Read,
    Unread,
}

impl MetadataAction {
    /// Every action, in the menu's order.
    pub(super) const ALL: [MetadataAction; 4] = [
        MetadataAction::Pin,
        MetadataAction::Unpin,
        MetadataAction::Read,
        MetadataAction::Unread,
    ];

    /// The target `row` (under `key`) gives this action, or `None` when the action does not apply to it: Pin to a row
    /// not pinned, Unpin to a pinned one, and Mark read or Mark unread to a row whose own menu offers that toggle. A
    /// retired containerized session is no target of any of them, since it supports Delete only
    /// ([`super::retired`]); answering here keeps the bar's offers, its ⋯ and the targets in agreement, as the old
    /// `applies` did by refusing such a row first.
    pub(super) fn target(self, key: &str, row: &SessionRow) -> Option<Target> {
        if super::retired::is(row) {
            return None;
        }
        let write = match self {
            MetadataAction::Pin => (!row.pinned).then_some(Write::Pin(true)),
            MetadataAction::Unpin => row.pinned.then_some(Write::Pin(false)),
            MetadataAction::Read => seen::offered(row)
                .filter(|toggle| matches!(toggle, Toggle::Read(_)))
                .map(Write::Seen),
            MetadataAction::Unread => seen::offered(row)
                .filter(|toggle| matches!(toggle, Toggle::Unread))
                .map(Write::Seen),
        }?;
        Some(Target {
            key: key.to_owned(),
            write,
        })
    }

    /// The item's label, the old app's words.
    pub(super) fn label(self) -> &'static str {
        match self {
            MetadataAction::Pin => "Pin",
            MetadataAction::Unpin => "Unpin",
            MetadataAction::Read => "Mark read",
            MetadataAction::Unread => "Mark unread",
        }
    }

    /// The item's description line, which is also its hover help, the old app's words.
    pub(super) fn description(self) -> &'static str {
        match self {
            MetadataAction::Pin => "Pin the marked sessions that are not pinned",
            MetadataAction::Unpin => "Unpin the marked sessions that are pinned",
            MetadataAction::Read => "Treat unseen output in eligible marked sessions as seen",
            MetadataAction::Unread => "Show new output again in eligible marked sessions",
        }
    }

    /// The action's place in [`MetadataAction::ALL`] (the declaration order, which `ALL` follows), which indexes the
    /// marked menu's focus handles.
    fn index(self) -> usize {
        self as usize
    }

    /// The item's element id in the marked menu.
    fn item_id(self) -> &'static str {
        match self {
            MetadataAction::Pin => "session-marked-menu-pin",
            MetadataAction::Unpin => "session-marked-menu-unpin",
            MetadataAction::Read => "session-marked-menu-read",
            MetadataAction::Unread => "session-marked-menu-unread",
        }
    }

    /// The item's glyph, the one the row menu's same action shows.
    pub(super) fn icon(self) -> Icon {
        match self {
            MetadataAction::Pin | MetadataAction::Unpin => Icon::Pin,
            MetadataAction::Read | MetadataAction::Unread => Icon::MarkSeen,
        }
    }
}

// ---- What a frame shows ----------------------------------------------------------------------------------------------

/// What one frame shows of the marks while they count ([`project`]).
#[derive(Debug)]
pub(super) struct Projection {
    /// The marked keys among the rows the list shows, in their order: what the frame counts and draws marked.
    pub(super) shown: Rc<[String]>,
    /// The metadata actions at least one of them offers, in the menu's order, each with its targets as this frame
    /// draws them: what the bar's ⋯ and the marked menu offer, and what a chosen item acts on.
    pub(super) offers: Vec<Offer>,
    /// Whether some mark shown is not a retired containerized session, which supports Delete only
    /// ([`super::retired`]): the bar draws Stop only then, as the old bar did.
    pub(super) stop: bool,
}

/// One metadata action the marks shown offer, and its targets as drawn.
#[derive(Clone, Debug)]
pub(super) struct Offer {
    pub(super) action: MetadataAction,
    pub(super) targets: Vec<Target>,
}

/// What one frame shows of `marks` over `listed`, the rows the list shows in their order ([`Projection`]). The list
/// projects nothing while marks do not count (the Hosts panel shows, or its listing is not available: SPEC.md, "failed
/// listings expose no targets"); the stored marks are kept either way.
pub(super) fn project(listed: &[Listed], marks: &Marks) -> Projection {
    let displayed: Vec<&str> = listed.iter().map(|session| session.key).collect();
    let shown = effective(&displayed, marks);
    let offers = MetadataAction::ALL
        .into_iter()
        .filter_map(|action| {
            let targets: Vec<Target> = listed
                .iter()
                .filter(|session| marks.contains(session.key))
                .filter_map(|session| action.target(session.key, session.row))
                .collect();
            (!targets.is_empty()).then_some(Offer { action, targets })
        })
        .collect();
    let stop = listed
        .iter()
        .any(|session| marks.contains(session.key) && !retired::is(session.row));
    Projection {
        shown: shown.into_iter().map(str::to_owned).collect(),
        offers,
        stop,
    }
}

/// What the list keeps for the marked bar across frames, for the window's life: the bar's focus handles, owned here so
/// the marked menu can name its toggle as where closing returns focus whichever frame drew it, the anchor the menu
/// opens beside, and what the last frame projected ([`project`]), which the row callback, the bar, the menu and a
/// chosen action read.
pub(super) struct Kept {
    /// "N marked"'s handle. Not a tab stop: Tab skips the count, as the old `tabindex="-1"` did, but the app moves
    /// focus there when the bulk question is confirmed.
    count_focus: FocusHandle,
    /// Stop's and Delete's handles, tab stops, owned here so that the bar knows when focus is on them.
    stop_focus: FocusHandle,
    delete_focus: FocusHandle,
    /// The ⋯ toggle's handle, a tab stop.
    menu_focus: FocusHandle,
    /// Clear's handle, a tab stop.
    clear_focus: FocusHandle,
    /// Where the ⋯ was last drawn. It refers to the element weakly, so a toggle no longer drawn opens nothing.
    anchor: Option<Anchor>,
    /// What the last frame projected, or `None` when marks did not count in it: the Hosts panel showed, or the listing
    /// was not available. A failed read keeps its rows on screen (the list's module docs), but SPEC.md has it expose
    /// no targets, so no gesture marks over it.
    projection: Option<Projection>,
}

impl Kept {
    /// The bar's state before anything is marked.
    pub(super) fn new(cx: &mut App) -> Kept {
        Kept {
            count_focus: cx.focus_handle(),
            stop_focus: cx.focus_handle().tab_stop(true),
            delete_focus: cx.focus_handle().tab_stop(true),
            menu_focus: cx.focus_handle().tab_stop(true),
            clear_focus: cx.focus_handle().tab_stop(true),
            anchor: None,
            projection: None,
        }
    }

    /// Whether the last frame offered Stop: some mark shown is not a retired session ([`Projection::stop`]).
    fn offers_stop(&self) -> bool {
        self.projection
            .as_ref()
            .is_some_and(|projection| projection.stop)
    }

    /// The bar's own button for `action`, Stop's or Delete's focus handle: where a bulk question returns focus on
    /// Cancel when the bar draws no ⋯ ([`super::bulk`]).
    pub(super) fn button_focus(&self, action: bulk::Lifecycle) -> &FocusHandle {
        match action {
            bulk::Lifecycle::Stop => &self.stop_focus,
            bulk::Lifecycle::Delete => &self.delete_focus,
        }
    }

    /// The marks the last frame showed, in the order shown.
    pub(super) fn shown(&self) -> Rc<[String]> {
        self.projection
            .as_ref()
            .map_or_else(|| Rc::from([]), |projection| projection.shown.clone())
    }

    /// The metadata actions the last frame offered, with their targets as drawn.
    fn offers(&self) -> &[Offer] {
        self.projection
            .as_ref()
            .map_or(&[], |projection| &projection.offers)
    }

    /// Whether one of the bar's own controls holds focus, which then goes to the order's title when the bar goes
    /// (the module docs, "Focus"), whichever way the bar goes: Clear, Escape, or a bulk batch clearing its targets.
    pub(super) fn holds_focus(&self, window: &Window) -> bool {
        [
            &self.count_focus,
            &self.stop_focus,
            &self.delete_focus,
            &self.menu_focus,
            &self.clear_focus,
        ]
        .into_iter()
        .any(|handle| handle.is_focused(window))
    }

    /// "N marked"'s focus handle, where a confirmed bulk question sends focus.
    pub(super) fn count_focus(&self) -> &FocusHandle {
        &self.count_focus
    }

    /// The ⋯ toggle's focus handle while the last frame drew the toggle, where a cancelled bulk question returns focus.
    pub(super) fn menu_toggle_drawn(&self) -> Option<&FocusHandle> {
        self.anchor.as_ref().map(|_| &self.menu_focus)
    }

    /// The ⋯ toggle's focus handle, for a test that starts from it.
    #[cfg(test)]
    pub(super) fn menu_focus(&self) -> &FocusHandle {
        &self.menu_focus
    }
}

// ---- The gestures, Clear and Escape ----------------------------------------------------------------------------------

impl SessionList {
    /// Take in what the frame just projected ([`project`]; `None` while marks do not count): keep it for the rows, the
    /// bar and the menu, and close an open marked menu whose actions on offer changed, as the old one closed when its
    /// eligibility changed under it. The close waits for the frame, since a layer is not closed while the window draws.
    /// Targets that only change their drawn values (a new activity stamp) leave the menu open.
    pub(super) fn marks_projected(
        &mut self,
        projection: Option<Projection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let actions =
            |offers: &[Offer]| offers.iter().map(|offer| offer.action).collect::<Vec<_>>();
        let changed = actions(self.marking.offers())
            != projection
                .as_ref()
                .map_or_else(Vec::new, |projection| actions(&projection.offers));
        if changed && let Some(menu) = self.marked_menu_open(window, cx) {
            window.defer(cx, move |window, cx| layers::close(menu, window, cx));
        }
        self.marking.projection = projection;
    }

    /// A click with `gesture` on the open control of `key`'s row, after the caller opened the session for a plain
    /// click (the selection rules never refuse one): a plain click clears every mark; Shift and Cmd or Ctrl mark over
    /// the rows as the last frame drew them (`entries`, the order the user clicked on), anchored on the open session,
    /// and open nothing. A marking gesture does nothing while the last frame let no mark count (a listing that is not
    /// available). Visible to the crate for the screenshot goldens, which mark rows without a click.
    pub(crate) fn row_clicked(&mut self, key: &str, gesture: Gesture, cx: &mut Context<Self>) {
        if gesture != Gesture::Open && self.marking.projection.is_none() {
            return;
        }
        let anchor = local_state::selected(cx);
        // A clone of the shared slice, so the marks can be borrowed mutably beside it.
        let entries = self.entries.clone();
        let displayed: Vec<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
        self.current
            .current(cx)
            .marks
            .mark(&displayed, anchor.as_deref(), key, gesture);
        cx.notify();
    }

    /// Forget every mark, shown or hidden (Escape in the list, Clear, a metadata action).
    pub(super) fn clear_marks(&mut self, cx: &mut Context<Self>) {
        let marks = &mut self.current.current(cx).marks;
        let had = !marks.is_empty();
        marks.clear();
        if had {
            cx.notify();
        }
    }

    /// Escape reached the list's column: clear the marks as Clear does, focus handback included, and let the key go on
    /// to the window's own Escape (SPEC.md: Escape clears marks "without preventing default handling"). Anything that
    /// owns Escape inside the column (an inline question) takes it before it gets here, and layers and the terminal are
    /// outside the column.
    pub(super) fn escape_in_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_from_bar(window, cx);
        cx.propagate();
    }

    /// Clear or Escape: clear the marks, and hand focus to the order's title if it was on one of the bar's controls,
    /// which go with the bar. The focus is read before the marks go, while the bar is still the one drawn.
    fn clear_from_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mark = Mark::take(cx);
        let hand_back = self.marking.holds_focus(window);
        self.clear_marks(cx);
        if hand_back {
            focus::focus_control(&self.order_focus, &mark, window, cx);
        }
    }
}

// ---- The marked bar --------------------------------------------------------------------------------------------------

/// "N marked"'s element id.
pub(super) const COUNT_ID: &str = "session-marked-count";

/// The ⋯ toggle's element id.
pub(super) const MENU_TOGGLE_ID: &str = "session-marked-menu-toggle";

/// Clear's element id.
pub(super) const CLEAR_ID: &str = "session-marked-clear";

/// The marked menu's element id.
pub(super) const MENU_ID: &str = "session-marked-menu";

/// The ⋯ toggle's and the menu's accessible name, the old `aria-label`.
const MENU_NAME: &str = "Marked session actions";

/// The marked menu's width: the row menu's (`.session-row-menu-panel`), which the old flyout shared.
const PANEL_WIDTH: f32 = 246.;

impl SessionList {
    /// The heading's band while marks are effective (`.marked-bar`): "N marked", Stop, Delete, the ⋯ toggle when an
    /// action applies, and Clear, on the heading's ground. `None` while no mark is effective, when the heading is drawn
    /// instead. Drawn before the store is borrowed for the rest of the frame: the toggle's anchor needs the window's
    /// element state.
    pub(super) fn marked_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let count = self.marking.shown().len();
        if count == 0 {
            self.marking.anchor = None;
            return None;
        }
        let palette = theme::palette(cx);
        let words = format!("{count} marked");
        // `.marked-count`: 13 pixels, semibold, on one line, pushing the controls after it to the band's end.
        let count = focus::track(
            div().id(COUNT_ID).debug_selector(|| COUNT_ID.into()),
            &self.marking.count_focus,
            focus::OnPress::Keep,
        )
        .role(Role::Label)
        .aria_label(words.clone())
        .mr_auto()
        .whitespace_nowrap()
        .text_size(metrics::TEXT_MD)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::color(palette.fg_0))
        .child(words);
        // The old `.marked-stop` and `.marked-delete`, with their help. Stop is left out while every mark shown is a
        // retired session, which supports Delete only, as the old bar left it out.
        let offers_stop = self.marking.offers_stop();
        let lifecycle = [
            (
                bulk::Lifecycle::Stop,
                bulk::STOP_ID,
                "Stop the agents and close terminal tabs in the marked sessions currently shown",
                Tier::Neutral,
                &self.marking.stop_focus,
            ),
            (
                bulk::Lifecycle::Delete,
                bulk::DELETE_ID,
                "Delete the marked sessions currently shown",
                Tier::Danger,
                &self.marking.delete_focus,
            ),
        ]
        .into_iter()
        .filter(|(action, ..)| offers_stop || *action != bulk::Lifecycle::Stop)
        .map(|(action, id, help, tier, focus)| {
            let list = cx.weak_entity();
            Button::new(id, action.label(), HoverHelp::tooltip(help), Role::Button)
                .tier(tier)
                .toolbar()
                .focus(focus)
                .on_click(move |_, window, cx| {
                    let _ = list.update(cx, |list, cx| list.bulk_lifecycle(action, window, cx));
                })
        })
        .collect::<Vec<_>>();
        let toggle = (!self.marking.offers().is_empty()).then(|| {
            // Beside the toggle, as the row menu opens and as the old flyout stood out past the sidebar's edge, so the
            // menu covers the session pane rather than the rows it acts on.
            let anchor = Anchor::for_element("session-marked-menu-anchor", window, cx).beside();
            self.marking.anchor = Some(anchor.clone());
            let open = self.marked_menu_open(window, cx).is_some();
            let (clicked, keyed) = (cx.weak_entity(), cx.weak_entity());
            let toggle = IconButton::new(
                MENU_TOGGLE_ID,
                Icon::More,
                MENU_NAME,
                HoverHelp::tooltip("Actions for marked sessions"),
                Role::Button,
            )
            .tier(Tier::Ghost)
            .square(px(HEADING_ICON_SIDE))
            .expanded(open)
            .focus(&self.marking.menu_focus)
            // Down and Up open the closed menu on its first or last item, as the old toggle did.
            .on_key_down(toggle_arrow_keys(move |start, window, cx| {
                let _ = keyed.update(cx, |list, cx| list.open_marked_menu(start, window, cx));
            }))
            .on_click(move |_, window, cx| {
                let _ = clicked.update(cx, |list, cx| list.toggle_marked_menu(window, cx));
            });
            div().relative().child(toggle).child(anchor.track())
        });
        if toggle.is_none() {
            self.marking.anchor = None;
        }
        let cleared = cx.weak_entity();
        let clear = IconButton::new(
            CLEAR_ID,
            Icon::Close,
            "Clear marked sessions",
            HoverHelp::tooltip("Clear marked sessions (Escape in the list)"),
            Role::Button,
        )
        .tier(Tier::Ghost)
        .square(px(HEADING_ICON_SIDE))
        .focus(&self.marking.clear_focus)
        .on_click(move |_, window, cx| {
            let _ = cleared.update(cx, |list, cx| list.clear_from_bar(window, cx));
        });
        Some(
            div()
                .id("session-marked-bar")
                .debug_selector(|| "session-marked-bar".into())
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(6.))
                .min_w_0()
                .py(metrics::LIST_HEADING_PAD.0)
                .px(metrics::LIST_HEADING_PAD.1)
                .bg(theme::color(palette.bg_2))
                .child(count)
                .children(lifecycle)
                .children(toggle)
                .child(clear)
                .into_any_element(),
        )
    }
}

// ---- The marked menu -------------------------------------------------------------------------------------------------

impl SessionList {
    /// The marked menu, if it is open.
    fn marked_menu_open(&mut self, window: &Window, cx: &App) -> Option<LayerId> {
        self.current
            .current(cx)
            .marked_menu
            .filter(|menu| layers::is_open(*menu, window, cx))
    }

    /// Close the marked menu if it is open, which returns focus to its toggle: the Hosts attention list moved the bar
    /// it opened from ([`super::attention`]). Reopened, it opens where the toggle is drawn then.
    pub(super) fn close_marked_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.marked_menu_open(window, cx) {
            layers::close(menu, window, cx);
        }
    }

    /// The ⋯ toggle was activated: open the marked menu on its first item, or close it if it is open.
    fn toggle_marked_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.marked_menu_open(window, cx) {
            layers::close(menu, window, cx);
            return;
        }
        self.open_marked_menu(MenuStart::First, window, cx);
    }

    /// Open the marked menu beside the ⋯ toggle with focus on the item `start` names, unless it is open already, the
    /// toggle is not drawn or no action applies. Opening counts as a marking choice ([`Marks::touch`]), as the old one
    /// did.
    fn open_marked_menu(&mut self, start: MenuStart, window: &mut Window, cx: &mut Context<Self>) {
        if self.marked_menu_open(window, cx).is_some() {
            return;
        }
        let Some(anchor) = self.marking.anchor.clone() else {
            return;
        };
        let offers = self.marking.offers();
        let start = match start {
            MenuStart::First => offers.first(),
            MenuStart::Last => offers.last(),
        };
        let Some(start) = start.map(|offer| offer.action) else {
            return;
        };
        let mark = Mark::take(cx);
        let items: [FocusHandle; 4] = std::array::from_fn(|_| cx.focus_handle().tab_stop(true));
        let start = items[start.index()].clone();
        let list = cx.weak_entity();
        let view = cx.new(|_| MarkedMenu { list, items });
        let opened = layers::open_popover(
            // The toggle, not what held focus before: on macOS the click that opened the menu left it unfocused.
            Popover::menu(view, cx.focus_handle(), anchor, start)
                .returns_to(focus::ReturnTo::Control(self.marking.menu_focus.clone())),
            &mark,
            window,
            cx,
        );
        let current = self.current.current(cx);
        if opened.is_some() {
            current.marks.touch();
        }
        current.marked_menu = opened;
        cx.notify();
    }

    /// A marked menu action was chosen, whose targets the menu drew as `drawn`: close the menu, send each admitted
    /// target's own request ([`admitted`]: still shown and still marked, with the values drawn), give a row whose
    /// commands are refused now its not-started line instead, clear the marks, and hand focus to the order's title,
    /// since the toggle the menu returns focus to goes with the bar. Replies are not waited for: a refusal shows on its
    /// row as the row menu's would.
    fn apply_metadata(&mut self, drawn: &[Target], window: &mut Window, cx: &mut Context<Self>) {
        let mark = Mark::take(cx);
        if let Some(menu) = self.marked_menu_open(window, cx) {
            layers::close(menu, window, cx);
        }
        let shown = self.marking.shown();
        let targets = admitted(drawn, &shown, &self.current.current(cx).marks);
        for target in targets {
            // The row menu draws these items unavailable then (`menu::refused`); the bulk action says so on the row.
            if menu::refused(&target.key, cx) {
                let (work, label) = match target.write {
                    Write::Pin(_) => (Work::Pin, "Pin"),
                    Write::Seen(_) => (Work::Seen, "Seen"),
                };
                bulk::not_started(&target.key, work, label, cx);
                continue;
            }
            match target.write {
                Write::Pin(pinned) => pins::send_pin(&target.key, pinned, cx),
                Write::Seen(toggle) => seen::send(&target.key, toggle, cx),
            }
        }
        self.clear_marks(cx);
        focus::focus_control(&self.order_focus, &mark, window, cx);
    }
}

/// The open marked menu: the shared header ("N marked", "Session metadata"), then the actions that apply to a marked
/// row shown, each with its description. Drawn from the list's projection of the same frame: the list draws before
/// the overlay the menu sits in on every frame (the row menu reads the list's Move neighbors the same way), and no view
/// in the window is cached, so a listing that changes under the open menu redraws both. The list closes it when the
/// actions on offer change ([`SessionList::marks_projected`]).
pub(super) struct MarkedMenu {
    list: WeakEntity<SessionList>,
    /// One focus handle per action, in [`MetadataAction::ALL`]'s order, whether or not the action is drawn: the menu
    /// opens on the first or last one drawn, and Home and End move there.
    items: [FocusHandle; 4],
}

impl Render for MarkedMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = layers::panel(cx)
            .id(MENU_ID)
            .debug_selector(|| MENU_ID.into())
            .w(px(PANEL_WIDTH));
        let Some(list) = self.list.upgrade() else {
            return panel;
        };
        let (count, offers) = {
            let kept = &list.read(cx).marking;
            (kept.shown().len(), kept.offers().to_vec())
        };
        // The drawn items' focus handles, in order, for Home and End.
        let mut drawn: Vec<FocusHandle> = Vec::new();
        let items: Vec<MenuItem> = offers
            .into_iter()
            .map(|Offer { action, targets }| {
                let focus = &self.items[action.index()];
                drawn.push(focus.clone());
                let list = self.list.clone();
                MenuItem::new(
                    action.item_id(),
                    action.icon(),
                    action.label(),
                    // The old item's tooltip was its description itself.
                    HoverHelp::tooltip(action.description()),
                    Role::MenuItem,
                )
                .description(action.description())
                .focus(focus)
                // `targets` are the drawn ones, with the activity each row shows for Mark read.
                .on_click(move |_, window, cx| {
                    let _ = list.update(cx, |list, cx| list.apply_metadata(&targets, window, cx));
                })
            })
            .collect();
        // Home and End, as the row menu's: the layer manager's menu context binds only Up and Down.
        if let (Some(first), Some(last)) = (drawn.first(), drawn.last()) {
            panel = panel.on_key_down(home_end_keys(first.clone(), last.clone()));
        }
        panel
            .child(MenuHeader::app(
                format!("{count} marked"),
                "Session metadata",
            ))
            .child(
                div()
                    .id("session-marked-menu-items")
                    .role(Role::Menu)
                    .aria_label(MENU_NAME)
                    .flex()
                    .flex_col()
                    .children(items),
            )
    }
}
