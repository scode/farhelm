//! The order hold keeps a session or group from moving under the pointer
//! while the user reaches for it.
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
//! - **A manual move takes effect at once.** Original groups and pins still eligible in their bands use the shared
//!   manual order within the slots they already hold. An accepted cross-area drop moves a held slot in the window
//!   that sent it. Other membership and pin changes keep their slots. A pin made during the hold offers Move after
//!   the hold ends ([`super::pins`]). Collapse is applied after reconciliation.
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

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::SessionGroup;
use gpui::{
    App, Context, Div, HoverListenerMode, InteractiveElement as _, Stateful,
    StatefulInteractiveElement as _, WeakEntity, Window,
};

use super::SessionList;
use super::filters::Query;
use super::groups::{Band, Layout};
use super::order::Order;
use super::pins;
use super::selection::Listed;
use crate::ticker;

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
    /// Last displayed band of each held row, even if the row's membership
    /// changes while its menu or the pointer keeps the hold in place.
    bands: HashMap<String, Band>,
    /// Preserve the header while held rows still describe a group the helm
    /// just removed. Live attributes take precedence whenever it exists.
    groups: Vec<SessionGroup>,
    /// The list the snapshot was taken of.
    list: ListId,
}

impl Held {
    /// A snapshot of `shown`, the rows as displayed, for the list `list`. Even an empty one is a hold: rows that arrive
    /// later are new to it.
    pub(super) fn capture(shown: &Layout<'_>, list: ListId) -> Held {
        Held {
            ids: shown
                .members()
                .map(|(_, session)| session.key.to_owned())
                .collect(),
            original_pins: shown
                .members()
                .filter(|(band, session)| *band != Band::Tail && session.row.pinned)
                .map(|(_, session)| session.key.to_owned())
                .collect(),
            bands: shown
                .members()
                .map(|(band, session)| (session.key.to_owned(), band))
                .collect(),
            groups: shown
                .groups
                .iter()
                .map(|block| block.group.clone())
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

    /// Refresh the last frame's snapshot only while there is no hold. The
    /// next pointer activity takes it before any new server order can draw.
    pub(super) fn snapshot(&self, layout: &Layout<'_>, list: ListId) -> Option<Held> {
        self.held.is_none().then(|| Held::capture(layout, list))
    }

    /// Move the held slot only after this window's cross-area drop was
    /// accepted. Membership pushes alone cannot move it during a hold.
    pub(super) fn crossed(
        &mut self,
        key: &str,
        band: Band,
        neighbor: Option<(&str, pins::Placement)>,
        pinned: impl Fn(&str) -> bool,
    ) {
        let Some(held) = &mut self.held else { return };
        let Some(old) = held.ids.iter().position(|id| id == key) else {
            return;
        };
        let moved = held.ids.remove(old);
        let insert = neighbor
            .and_then(|(key, placement)| {
                held.ids
                    .iter()
                    .position(|id| id == key)
                    .map(|index| index + usize::from(placement == pins::Placement::After))
            })
            .or_else(|| {
                // A pin append goes below every held pin slot, even across a gap left by unpinning. Inserting it
                // earlier would let manual order rotate another pin into the new slot. An unpinned row goes after
                // the leading pins so the divider and movable run keep their meaning.
                let mut positions = held
                    .ids
                    .iter()
                    .enumerate()
                    .filter(|(_, id)| held.bands.get(*id) == Some(&band));
                let (first, id) = positions.next()?;
                if pinned(&moved) {
                    return Some(
                        positions
                            .rfind(|(_, id)| pinned(id))
                            .map_or(first + usize::from(pinned(id)), |(index, _)| index + 1),
                    );
                }
                if !pinned(id) {
                    return Some(first);
                }
                let last = positions
                    .take_while(|(_, id)| pinned(id))
                    .last()
                    .map_or(first, |(i, _)| i);
                Some(last + 1)
            })
            .unwrap_or(held.ids.len());
        held.bands.insert(moved.clone(), band);
        held.ids.insert(insert, moved);
    }

    /// Reconcile the uncollapsed layout against its held bands and slots.
    /// A hold taken for another list ends here.
    pub(super) fn apply<'a>(
        &mut self,
        served: Layout<'a>,
        roster: &[SessionGroup],
        list: ListId,
    ) -> Layout<'a> {
        let Some(held) = &mut self.held else {
            return served;
        };
        if held.list != list {
            self.held = None;
            return served;
        }
        let shown = display(held, served, roster);
        let mut next = Held::capture(&shown, list);
        next.original_pins = std::mem::take(&mut held.original_pins);
        next.original_pins
            .retain(|key| next.bands.contains_key(key));
        *held = next;
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
        let current = self.current.current(cx);
        let snapshot = &mut current.last_uncollapsed;
        current.hold.activity(deadline, || snapshot.take());
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

/// Reconcile the held slots with current row data and group attributes.
/// The layout keeps vanished headers only while their held members remain.
/// Arrivals join the tail so a group assignment cannot move other rows.
fn display<'a>(held: &Held, served: Layout<'a>, roster: &[SessionGroup]) -> Layout<'a> {
    let served_rows: HashMap<_, _> = served.members().map(|(_, row)| (row.key, row)).collect();
    let rows = held
        .ids
        .iter()
        .filter_map(|key| served_rows.get(key.as_str()).copied())
        .chain(
            served
                .members()
                .map(|(_, row)| row)
                .filter(|row| !held.bands.contains_key(row.key)),
        );
    let live_groups: HashMap<_, _> = roster.iter().map(|group| (group.id, group)).collect();
    let mut groups: Vec<_> = held
        .groups
        .iter()
        .map(|group| live_groups.get(&group.id).copied().unwrap_or(group))
        .collect();
    reorder_slots(
        &mut groups,
        |group| live_groups.contains_key(&group.id),
        |a, b| (a.position, a.id).cmp(&(b.position, b.id)),
    );
    let mut shown = Layout::partition(rows, groups, |row| {
        held.bands.get(row.key).copied().unwrap_or(Band::Tail)
    });
    let reorder_pins = |rows: &mut [Listed<'a>], band| {
        if pins::manual_order(rows) {
            reorder_slots(
                rows,
                |row| {
                    row.row.pinned
                        && held.original_pins.contains(row.key)
                        && Band::served(row) == band
                },
                pins::by_position,
            );
        }
    };
    reorder_pins(&mut shown.global, Band::GlobalPins);
    for block in &mut shown.groups {
        reorder_pins(&mut block.rows, Band::Group(block.group.id));
    }
    shown
}

/// Let manual order through in the slots already held by surviving,
/// manually ordered items. Every other item stays in its displayed slot.
fn reorder_slots<T: Copy>(
    shown: &mut [T],
    movable: impl Fn(&T) -> bool,
    order: impl Fn(&T, &T) -> Ordering,
) {
    let mut ordered: Vec<_> = shown.iter().filter(|item| movable(item)).copied().collect();
    ordered.sort_by(order);
    for (slot, item) in shown.iter_mut().filter(|item| movable(item)).zip(ordered) {
        *slot = item;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_list::tests::session;
    use farhelm_proto::ui_channel::SessionRow;

    /// A remote unpin leaves a gap until the hold ends. Inserting an accepted pin in that gap would let
    /// manual order move another pin up into the gap. The arriving pin needs its slot below that survivor.
    #[test]
    fn a_pin_appends_below_the_last_held_pin_even_after_a_gap() {
        let list = ListId {
            answered: Query::default(),
            order: Order::Activity,
        };
        let group = Band::Group(10);
        let ids = ["source", "first", "gap", "last", "loose"].map(str::to_owned);
        let bands = ids
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, id)| (id, if index == 0 { Band::GlobalPins } else { group }))
            .collect();
        let mut hold = Hold {
            held: Some(Held {
                list,
                ids: ids.into(),
                bands,
                original_pins: ["source", "first", "gap", "last"].map(str::to_owned).into(),
                groups: Vec::new(),
            }),
            ..Hold::default()
        };
        hold.crossed("source", group, None, |key| {
            matches!(key, "source" | "first" | "last")
        });
        assert_eq!(
            hold.held.map(|held| held.ids),
            Some(
                ["first", "gap", "last", "source", "loose"]
                    .map(str::to_owned)
                    .into()
            )
        );
    }

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
        let shown = |order: [usize; 2]| -> Layout<'_> {
            let rows = order
                .iter()
                .map(|ix| Listed {
                    key: rows[*ix].0,
                    row: &rows[*ix].1,
                })
                .collect();
            Layout::served(rows, &[])
        };
        let keys = |shown: Layout<'_>| {
            shown
                .members()
                .map(|(_, session)| session.key.to_owned())
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
            assert_eq!(keys(hold.apply(shown([1, 0]), &[], list)), ["a", "b"]);
            assert_eq!(keys(hold.apply(shown([1, 0]), &[], other)), ["b", "a"]);
            assert!(hold.held.is_none(), "{other:?}");
        }
    }
}
