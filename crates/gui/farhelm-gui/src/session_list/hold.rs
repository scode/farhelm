//! The order hold: while the pointer is over the list's rows, the rows keep the order they had on screen, so a row does
//! not move out from under the pointer on its way to a click.
//!
//! SPEC.md, "Session list". Ported from the old app's `rows.rs` (`HeldOrder`, `held_display_order`,
//! `hold_should_release`) and `list/view.rs` (`order_hold`, `note_list_pointer`, `leave_list`,
//! `try_release_order_hold`) at the reference commit. Here's the TLDR of the rules:
//!
//! - **Taken by the pointer.** Any pointer activity over the rows (entering them, moving, a press of any button, a
//!   wheel scroll, which moves the rows under a still pointer) takes the hold when none is on, snapshotting the order
//!   on screen and which of those rows were pinned. The heading and the segments above the rows are not part of it.
//! - **Released by leaving or by stillness, never under a row's menu.** The hold ends once the pointer has left the
//!   rows, or has been completely still over them for 5 seconds ([`STILLNESS`]); but while a row's menu or a question
//!   one of its commands asked is open it stays, whatever the pointer did, since releasing it would move the very row
//!   the menu belongs to. The rule is checked against the facts as they are ([`Hold::release_if_due`]), every time the
//!   list draws, so a leave or a stillness that happened under an open menu takes effect on the first frame after it
//!   closes. Pointer movement starts the 5 seconds over, also while a menu is open.
//! - **A menu closing is judged by where the pointer is.** A row's menu opens below its button and covers part of the
//!   rows, so while the pointer is on it the rows read as left. The frame on which the menu closes, while a hold is on
//!   and the rows read as left, therefore asks the pointer's position against the rows' bounds: over them, it counts
//!   as activity, which starts the 5 seconds over and keeps the hold, rather than releasing it under a pointer that
//!   never moved; off them, the leave stands.
//! - **Display only.** The snapshot is never sent anywhere and goes when the hold ends; the helm's pin order and its
//!   served order stay authoritative. While held, rows keep updating in place (they draw from the store); a session
//!   that disappears goes at once, since a deleted session must not stay clickable; a new session joins at the bottom,
//!   below every held row, until the hold ends ([`display`]).
//! - **A manual move takes effect at once.** The pins that were pinned when the hold began and still are follow the
//!   shared manual order within the slots those pins already hold, so a Move made in this window (or anywhere) shows
//!   without waiting. Any other pin change stays in place until the hold ends, and a pin made during the hold is never
//!   admitted as an original one, however many replies come, so it offers no Move items until then ([`super::pins`]).
//! - **A pin drag is activity.** While a pinned row is dragged by its grip, every move the drag captures counts as
//!   the pointer moving over the rows, wherever it is, as the old list kept hearing the moves its grip captured; the
//!   drop is a manual move like any other ([`super::pin_drag`]).
//! - **A different list is no hold's business.** Changing the host, the status filter or the order asks for a
//!   different list, so a hold taken for one list ends when the list on screen answers another ([`ListId`]), including
//!   a hold taken while the new list's reply was still on its way.
//!
//! The hold belongs to the UI session (`super::Current`): a reset forgets it with the rest of the list's state.
//!
//! The stillness deadline is the ticker's ([`ticker::after`]), replaced on every activity: dropping the previous one
//! cancels it, so the one left is always 5 seconds from the last activity, with no clock read here.

use std::collections::HashSet;
use std::time::Duration;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{Availability, Collection, SessionRow};
use gpui::{
    App, Context, Div, HoverListenerMode, InteractiveElement as _, Stateful,
    StatefulInteractiveElement as _, WeakEntity, Window,
};

use super::SessionList;
use super::filters::Query;
use super::order::Order;
use super::pins;
use super::selection::Listed;
use crate::{session, ticker};

/// How long the pointer must be completely still over the rows before the hold lets go (SPEC.md's 5 seconds).
pub(super) const STILLNESS: Duration = Duration::from_secs(5);

/// Which list the rows on screen answer: the filters the helm last accepted and the stored order. A hold is for one
/// list; any change here ends it. The accepted query rather than the chosen one, as the old app compared the committed
/// listing's: the rows stay held until the new list arrives, and a hold taken while it was on its way ends when it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ListId {
    pub(super) answered: Query,
    pub(super) order: Order,
}

/// The order on screen when the hold began, kept up to date with the slots of rows that arrived since.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Held {
    /// The displayed keys, top to bottom.
    ids: Vec<String>,
    /// The keys among `ids` that were pinned when the hold began and are still shown: the original pins, the only ones
    /// that follow the manual order during the hold and offer Move items.
    original_pins: HashSet<String>,
    /// The list the snapshot was taken of.
    list: ListId,
}

impl Held {
    /// A snapshot of `shown`, the rows as displayed, for the list `list`. Even an empty one is a hold: rows that arrive
    /// later are new to it.
    pub(super) fn capture(shown: &[Listed], list: ListId) -> Held {
        Held {
            ids: shown.iter().map(|session| session.key.to_owned()).collect(),
            original_pins: shown
                .iter()
                .filter(|session| session.row.pinned)
                .map(|session| session.key.to_owned())
                .collect(),
            list,
        }
    }
}

/// What the hold keeps for one UI session: the snapshot while it is on, and the pointer facts its release rule reads.
#[derive(Default)]
pub(super) struct Hold {
    held: Option<Held>,
    /// Whether the pointer is over the rows now.
    pointer_over: bool,
    /// Whether the stillness deadline the last activity armed has fired: the pointer has been still for [`STILLNESS`].
    still: bool,
    /// The stillness deadline the last activity armed. Replacing it cancels the one before.
    deadline: Option<ticker::Deadline>,
    /// Whether a row's menu or its question was open at the last frame's check, so the frame on which it closes is
    /// known ([`Hold::menu_closed`]).
    menu_was_open: bool,
}

impl Hold {
    /// The pointer was active over the rows: it is over them, it is not still, `deadline` (5 seconds from now) replaces
    /// the previous stillness deadline, and, if no hold is on, the hold begins with what `capture` snapshots of the
    /// rows on screen (`None` while there is no listing to hold, which takes nothing). `capture` runs only when it is
    /// needed, since activity arrives on every pointer move.
    pub(super) fn activity(
        &mut self,
        deadline: ticker::Deadline,
        capture: impl FnOnce() -> Option<Held>,
    ) {
        self.pointer_over = true;
        self.still = false;
        self.deadline = Some(deadline);
        if self.held.is_none() {
            self.held = capture();
        }
    }

    /// Whether a hold is on while the rows read as left: the one state in which a menu's close is judged afresh
    /// ([`SessionList::after_menus`]), since the menu covering the rows reads as the pointer leaving them.
    pub(super) fn reads_left_while_held(&self) -> bool {
        self.held.is_some() && !self.pointer_over
    }

    /// The pointer left the rows (or the rows went away under it).
    pub(super) fn left(&mut self) {
        self.pointer_over = false;
    }

    /// The stillness deadline fired: the pointer has not moved over the rows for [`STILLNESS`].
    pub(super) fn stilled(&mut self) {
        self.still = true;
        self.deadline = None;
    }

    /// Note whether a row's menu or its question is open now (`menu_open`), and say whether one closed since the last
    /// check: the frame whose pointer facts [`SessionList`] then judges afresh (see the module docs).
    pub(super) fn menu_closed(&mut self, menu_open: bool) -> bool {
        let closed = self.menu_was_open && !menu_open;
        self.menu_was_open = menu_open;
        closed
    }

    /// End the hold now if the release rule says so: never while a row's menu or its question is open (`menu_open`),
    /// and otherwise once the pointer is not over the rows or has been still over them for [`STILLNESS`].
    pub(super) fn release_if_due(&mut self, menu_open: bool) {
        if !menu_open && (!self.pointer_over || self.still) {
            self.held = None;
        }
    }

    /// The original pins while the hold is on, which the Move items read ([`pins::neighbors`]); `None` with no hold.
    pub(super) fn original_pins(&self) -> Option<&HashSet<String>> {
        self.held.as_ref().map(|held| &held.original_pins)
    }

    /// The rows to draw for `served`, the rows in the order the list would draw them unheld ([`pins::pinned_first`]
    /// of the helm's order), answering the list `list`: as the hold keeps them ([`display`]), and the snapshot brought
    /// up to date with what is drawn, or `served` itself with no hold. A hold taken for another list ends here.
    pub(super) fn apply<'a>(&mut self, served: Vec<Listed<'a>>, list: ListId) -> Vec<Listed<'a>> {
        let Some(held) = &mut self.held else {
            return served;
        };
        if held.list != list {
            self.held = None;
            return served;
        }
        let shown = display(held, &served);
        held.ids = shown.iter().map(|session| session.key.to_owned()).collect();
        let ids: HashSet<&str> = held.ids.iter().map(String::as_str).collect();
        held.original_pins.retain(|key| ids.contains(key.as_str()));
        shown
    }
}

// ---- The list's side -------------------------------------------------------------------------------------------------

/// `rows`, the element the list draws its rows in, as the hold's area: the pointer's activity over it and its leaving
/// it reach `list`. Hover is followed whatever the last input was, so a key press with the pointer over the rows (Enter
/// on a menu item) is not taken for the pointer leaving them, as GPUI's default would take it until the next move.
/// A press is seen on its way down to the rows, so a control that keeps its press (a row's status dot, its bell) still
/// counts as activity.
pub(super) fn area(rows: Stateful<Div>, list: &WeakEntity<SessionList>) -> Stateful<Div> {
    let (hovered, moved, pressed, scrolled) =
        (list.clone(), list.clone(), list.clone(), list.clone());
    rows.hover_listener_mode(HoverListenerMode::InputModalityIndependent)
        .on_hover(move |over, _, cx| {
            let over = *over;
            let _ = hovered.update(cx, |list, cx| {
                if over {
                    list.pointer_activity(cx);
                } else {
                    list.pointer_left(cx);
                }
            });
        })
        .on_mouse_move(move |_, _, cx| {
            let _ = moved.update(cx, |list, cx| list.pointer_activity(cx));
        })
        .capture_any_mouse_down(move |_, _, cx| {
            let _ = pressed.update(cx, |list, cx| list.pointer_activity(cx));
        })
        .on_scroll(move |_, _, cx| {
            let _ = scrolled.update(cx, |list, cx| list.pointer_activity(cx));
        })
}

impl SessionList {
    /// Which list the rows answer now ([`ListId`]): the query the helm accepted and the order the preferences in
    /// `collections` store.
    pub(super) fn list_id(&mut self, collections: &Collections, cx: &App) -> ListId {
        ListId {
            answered: self.current.current(cx).filters.answered(),
            order: Order::stored_in(collections),
        }
    }

    /// The pointer was active over the rows: note it, restart the 5 seconds of stillness, and take the hold with the
    /// rows on screen when none is on and the listing is available (a failed or missing read is nothing to hold).
    /// Taking the hold changes nothing on screen, so nothing is redrawn. A pin drag calls it for every move it captures
    /// ([`super::pin_drag`]), since it stops those moves before the rows' own listeners see them.
    pub(super) fn pointer_activity(&mut self, cx: &mut Context<Self>) {
        let list = cx.weak_entity();
        let deadline = ticker::after(STILLNESS, cx, move |cx| {
            let _ = list.update(cx, |list, cx| {
                list.current.current(cx).hold.stilled();
                // The frame's release check ends the hold unless a row's menu keeps it.
                cx.notify();
            });
        });
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let collections = store.read(cx).collections();
        let available = collections
            .meta(Collection::Sessions)
            .is_some_and(|meta| meta.availability == Availability::Available);
        let list = self.list_id(collections, cx);
        let entries = &self.entries;
        self.current.current(cx).hold.activity(deadline, || {
            // The rows as drawn: `entries` is the order the last frame drew.
            available.then(|| {
                let shown: Vec<Listed> = entries
                    .iter()
                    .filter_map(|(key, _)| {
                        collections
                            .get::<SessionRow>(key)
                            .map(|row| Listed { key, row })
                    })
                    .collect();
                Held::capture(&shown, list)
            })
        });
    }

    /// The pointer left the rows: redraw, so the frame's release check ([`Hold::release_if_due`]) ends the hold unless
    /// a row's menu keeps it.
    fn pointer_left(&mut self, cx: &mut Context<Self>) {
        self.current.current(cx).hold.left();
        cx.notify();
    }

    /// Read whether the pointer is inside the window with `read` from now on, for a test: GPUI's test window never says it
    /// is on Linux, where the production reading (`Window::is_window_hovered`) would make every menu close a leave.
    #[cfg(test)]
    pub(crate) fn pointer_in_window_reads(&mut self, read: fn(&Window) -> bool) {
        self.pointer_in_window = read;
    }

    /// Called at the top of every frame, before the release check, with whether a row's menu or its question is open
    /// (`menu_open`): on the frame one closes while a hold is on and the rows read as left, judge the pointer by its
    /// position against the rows' bounds as last laid out, since the menu may have covered them (the module docs).
    /// Over them, and with the pointer in the window, the close counts as activity; otherwise the facts stand, so a
    /// close with no hold on, or with the pointer resting on a visible part of the rows, changes nothing.
    ///
    /// NOTE: GPUI keeps the last position the pointer had in the window after it leaves, so the window's hover is asked
    /// too (`pointer_in_window`, `Window::is_window_hovered` outside tests). On macOS GPUI answers that with whether the window is active, so a pointer that left the window from the rows
    /// before the menu closed can still keep the hold there, for at most the 5 seconds of stillness the close arms.
    pub(super) fn after_menus(&mut self, menu_open: bool, window: &Window, cx: &mut Context<Self>) {
        let hold = &mut self.current.current(cx).hold;
        if !hold.menu_closed(menu_open) || !hold.reads_left_while_held() {
            return;
        }
        if (self.pointer_in_window)(window)
            && self
                .rows
                .viewport_bounds()
                .contains(&window.mouse_position())
        {
            self.pointer_activity(cx);
        }
    }
}

/// The rows `held` shows of `served`: every held row still served, in its held slot and with its current data; then
/// the rows new to the hold, in served order; and, when `served` carries the manual pin order, the original pins that
/// are still pinned sorted into the slots those pins occupy, so a move among them shows at once. Any other pin change
/// stays where it is.
pub(super) fn display<'a>(held: &Held, served: &[Listed<'a>]) -> Vec<Listed<'a>> {
    let held_ids: HashSet<&str> = held.ids.iter().map(String::as_str).collect();
    let mut shown: Vec<Listed<'a>> = held
        .ids
        .iter()
        .filter_map(|key| served.iter().find(|session| session.key == key).copied())
        .chain(
            served
                .iter()
                .filter(|session| !held_ids.contains(session.key))
                .copied(),
        )
        .collect();
    if pins::manual_order(served) {
        let original =
            |session: &Listed| session.row.pinned && held.original_pins.contains(session.key);
        let mut moved =
            pins::pinned_first(shown.iter().copied().filter(original).collect()).into_iter();
        for slot in shown.iter_mut() {
            if original(slot)
                && let Some(next) = moved.next()
            {
                *slot = next;
            }
        }
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_list::tests::session;
    use farhelm_proto::ui_channel::SessionRow;

    /// SPEC.md: "Changing the list's host, filter, or order is asking for a different list, so the hold does not apply
    /// to it", and the old app ended a hold taken while a changed query's reply was on its way once that reply landed.
    /// No window test reaches this, since the window's filters change through the helm's replies. Specified: a hold for
    /// one list keeps its slots when the helm serves the rows the other way round; the same rows answering another
    /// accepted query, or another stored order, come back in served order and the hold is off.
    #[test]
    fn a_different_list_ends_the_hold() {
        let rows: Vec<(&str, SessionRow)> = ["a", "b"]
            .into_iter()
            .map(|key| (key, session(key).row()))
            .collect();
        let shown = |order: [usize; 2]| -> Vec<Listed> {
            order
                .iter()
                .map(|ix| Listed {
                    key: rows[*ix].0,
                    row: &rows[*ix].1,
                })
                .collect()
        };
        let keys = |shown: Vec<Listed>| {
            shown
                .iter()
                .map(|session| session.key.to_owned())
                .collect::<Vec<String>>()
        };
        let list = ListId {
            answered: Query::default(),
            order: Order::Activity,
        };
        let others = [
            ListId {
                answered: Query {
                    host: Some(2),
                    ..Query::default()
                },
                ..list
            },
            ListId {
                order: Order::Title,
                ..list
            },
        ];
        for other in others {
            let mut hold = Hold {
                held: Some(Held::capture(&shown([0, 1]), list)),
                ..Hold::default()
            };
            assert_eq!(keys(hold.apply(shown([1, 0]), list)), ["a", "b"]);
            assert_eq!(keys(hold.apply(shown([1, 0]), other)), ["b", "a"]);
            assert!(hold.held.is_none(), "{other:?}");
        }
    }
}
