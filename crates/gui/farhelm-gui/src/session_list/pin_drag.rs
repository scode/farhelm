//! One grip gesture moves sessions between groups and reorders pins and group headers.
//!
//! SPEC.md, "Session list". The grip, capture and drop line come from the old app's `pin_drag.rs`
//! and `assets/pin-order.js` (the gesture), `list/row.rs` (the grip, its name and help, and when it could not drag) and
//! the stylesheet's `.session-pin-grip`, `[data-pin-dragging]` and `[data-pin-drop]` rules at the reference commit.
//! Here's the TLDR:
//!
//! - **The grip.** Every movable pin has one: a row with Move neighbors ([`pins::neighbors`]), which is the leading run
//!   of pins, and while the order hold is on only the pins it began with, so a pin made during the hold has neither a
//!   grip nor Move items. It sits at the top of the row's 12-pixel leading gutter, over the open control, and shows
//!   while the pointer is over the row, focus is inside the row, or the row is lifted, as the old opacity rules had it.
//!   It is one control, a button named "Drag pinned session" and helped "Drag to reorder pinned sessions" beside the
//!   sidebar, and is no Tab stop and takes no keys of its own: the row's menu is the keyboard way to rearrange pins
//!   (the old grip was `tabindex="-1"` and had no key handler). While the row cannot move just now (a request of its
//!   session in flight, a question its commands or its card asked open, the listing unavailable) no grip is drawn. A
//!   retired containerized session's pin ([`super::retired`]), which supports Delete only, never has one, as the old
//!   app refused a retired row's drag; it stays a drop target, since the old app and the helm both take it as a
//!   neighbor, as Move up and Move down from another pin's menu do.
//!   An unpinned session's grip occupies the same gutter and offers a membership move. The row body stays available
//!   for clicking and marking.
//! - **Focus, as the old grip had it.** A primary press on the grip gives it focus, through the focus coordinator
//!   ([`focus::focus_control`], with a handle of the grip's own that is no Tab stop), as the old press called
//!   `grip.focus()`. So keys other than Escape typed during a drag go where they went in the old app: the app's
//!   shortcuts still act (Rename, Delete, the template keys), Tab moves focus on, and anything else reaches nothing.
//!   The press cannot always focus the grip: with a layer that holds focus open as it lands (a row's menu, whose own
//!   outside-press rule runs after the grip's), the coordinator refuses the move, and focus goes back to what the
//!   closing layer names (the menu's button). After a cancel focus stays on the grip, as it did on the old one. After a
//!   drop the move is in flight, the grip is not drawn meanwhile, and the coordinator sends the lost focus home, where
//!   the old disabled grip lost it to the page; that holds while the move is still in flight when the next frame is
//!   drawn, and should the helm's reply come first, focus stays on the grip as after a cancel. A drag whose row goes
//!   away also sends focus home. No focus ring is drawn on the grip, which only the pointer focuses, as the old
//!   stylesheet drew none.
//! - **Escape.** While a drag is on, Escape ends it and goes no further wherever focus is in the sidebar's body (the
//!   host rail, the list's column and the rail's foot: the grip, a row's menu button after the press closed its menu, a
//!   control Tab reached), through a capture-phase listener on that body ([`catch_escape`]); the old app's
//!   document-level listener did the same in capture. Escape typed elsewhere keeps the rules that hold for it
//!   everywhere: in an open dialog (Rename, opened by its shortcut mid-drag) it closes that dialog, the layer manager's
//!   rule for Escape with a layer open (SPEC_impl.md, "Overlays: the layer manager"), where the old listener ended the
//!   drag and left the dialog open; in a focused terminal it is the program's; and with focus on the sidebar's width
//!   divider or the session pane's controls, which only Tab can reach mid-drag, it does not reach the drag.
//! - **The press stays the grip's.** It never opens the row, and on its way down it still reaches the order hold's
//!   area, so it takes the hold like any other press (the old "do not stop pointer events").
//! - **Capture.** From the press to the release the drag follows the pointer across the whole window ([`watch`]), in the
//!   capture phase, and stops each move and the release there, as the old pointer capture did, so nothing under the
//!   pointer (a terminal whose program reports the mouse, say) sees a drag that is not its. Each captured move counts
//!   as activity over the rows for the hold, as the old list kept hearing the moves its grip captured. A move with the
//!   primary button no longer held is a release the window never saw (outside it, or lost to another app): the old
//!   `lostpointercapture`, which cancels. The window learns of such a release only at its next move inside it, so a
//!   drag started in a window with keyboard activation also ends when the window loses it (another app came to the
//!   front), where the old capture's loss arrived at once; a drag begun by the press that activated the window (macOS's
//!   first mouse) is outside that rule and ends by the next-move rule alone. On macOS GPUI repeats the last move every
//!   16 ms while the button is held, so a drag held still there keeps the hold on, where 5 still seconds end it
//!   elsewhere.
//! - **Lift and drop line.** Once the pointer has moved [`LIFT_DISTANCE`] from the press, the row is lifted (`--bg-2`
//!   and the menu shadow, `--accent-fill-hover` on the selected row) and the window's cursor is the closed hand. Where a
//!   drop would land is worked out afresh at every move and at the release ([`drop_target`]) from the movable rows as
//!   last drawn, and shown as a 2-pixel accent line inside the top or bottom edge of the pin it names. Nothing moves in
//!   advance: the rows stay put until the helm pushes the new order.
//!   For membership drops the whole destination is outlined and tinted. A pin gets the positional targets first,
//!   then the unpinned members of another group, which accept it at the end of their pin area.
//!   A grip press on a marked row carries its visible marked set. A group drop moves the still-shown and still-marked
//!   members through the marked menu's admission rules. The marks stay, and the lift shows how many others it carries.
//! - **The drop.** A release on a drop line sends a relative move or appends as the first visible pin in an empty area.
//!   The source area comes from the press. For a cross-area drop the initiating window moves the held slot on success.
//!   A release outside a valid band, on the original slot or before the lift sends nothing.
//! - **The list cancels too.** The drag ends with nothing sent when its row stops being movable (deleted, unpinned, a
//!   request of its session in flight), when the listing becomes unavailable, or when the Hosts panel takes the
//!   rows' place: the old list root that a failed
//!   listing or a hidden list took away took its gesture with it. A grip drawn again afterwards drags as before. A drag
//!   that began in an active window also ends when the window is no longer active (above).
//! - **Edge scrolling.** After the lift, bringing the pointer near a list edge can scroll while the button stays
//!   held and the pointer stays still. The virtualized source can scroll out of view as its captured grip stays
//!   active. Each tick judges its destination against the current bounds, and the drop samples them again.
//!
//! The drag belongs to the UI session ([`super::Current`]), so a reset of the connection forgets it with the rest of the
//! list's state.
//!
//! NOTE: Not ported: the old dot field's fourth row, which its 14-pixel box cut to a one-pixel sliver, and the old
//! grip's disabled state, which drew a hidden button that still took the press: a row that cannot move just now draws
//! no grip, so a press in its gutter opens the row as a press anywhere else on it does.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use farhelm_proto::session_requests::{PinMoveAreas, SessionGroupMembership, SessionGroupTarget};
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, Bounds, Context, CursorStyle, DispatchPhase, Div, Hsla, InteractiveElement,
    IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point,
    SharedString, StatefulInteractiveElement as _, Styled, WeakEntity, Window, canvas, div,
    prelude::FluentBuilder as _, px,
};

use super::menu::{self, SharedRows};
use super::pins::{self, Placement};
use super::row_marks::HelpPlace;
use super::{SessionList, groups};
use crate::actions::Dismiss;
use crate::components::{HoverHelp, Role, with_hover_help};
use crate::focus::{self, Mark};
use crate::layers;
use crate::operations::{OpKey, Outcome, Settled};
use crate::theme::{self, Palette, Token};
use crate::ticker;

/// The id prefix of a row's grip; the rest is the session's key. Not `session-row-`, which the
/// row's open control and its marks share and the list's tests read rows by.
pub(crate) const GRIP_ID_PREFIX: &str = "session-pin-grip-";

/// The grip's accessible name (the old `aria-label`).
pub(crate) const GRIP_LABEL: &str = "Drag pinned session";

/// The grip's hover help (the old `data-tooltip`).
pub(crate) const GRIP_HELP: &str = "Drag to reorder pinned sessions";

/// How far the pointer must move from the press before the row lifts and a drop can land, in either direction: the old
/// gesture's 4 pixels, so a press that wobbles is not a drag.
pub(super) const LIFT_DISTANCE: f32 = 4.;

/// The grip's box (`.session-pin-grip`: 12 by 28), the width of the open control's leading padding.
const GRIP_SIZE: (f32, f32) = (12., 28.);

/// The top-left corner of each 2-pixel dot inside the grip's box. The old dots were a radial gradient tiled at 4
/// pixels over an 8 by 14 field centered in the box: a dot of 1-pixel radius at the middle of each tile, two across and
/// three down (the fourth row's tile was cut to a sliver by the field's height and is not drawn).
const DOTS: [(f32, f32); 6] = [
    (3., 8.),
    (7., 8.),
    (3., 12.),
    (7., 12.),
    (3., 16.),
    (7., 16.),
];

/// The drop line's thickness (`box-shadow: inset 0 2px var(--accent)`).
const DROP_LINE: f32 = 2.;

/// The scroll strip stays inside the list. A stationary pointer advances a small step on each deadline.
const EDGE_SCROLL_ZONE: Pixels = px(32.);
const EDGE_SCROLL_STEP: Pixels = px(18.);
const EDGE_SCROLL_TICK: Duration = Duration::from_millis(32);

// ---- State -------------------------------------------------------------------------------------------------------------

/// Where a drop would land: the movable pin the drop line is drawn on and the side of it the dragged pin goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub(super) neighbor: String,
    pub(super) placement: Placement,
}

/// A grip's source determines whether it can land at a position, change membership, or do either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Area {
    Groups,
    Pins(SessionGroupTarget),
    Unpinned(SessionGroupTarget),
}

/// The movable entries of one band in display order. An empty pin band
/// can name the nearest header edge as its insertion line.
pub(super) struct Band {
    pub(super) area: Area,
    pub(super) items: Vec<String>,
    pub(super) empty: Option<Target>,
}

/// The visible pieces of a whole-membership target. A single pin accepts only the unpinned rows of a group here,
/// after its existing positional targets have been tried.
pub(super) struct MembershipBand {
    pub(super) target: SessionGroupTarget,
    pub(super) items: Vec<String>,
    pub(super) append_pin: Vec<String>,
}

/// Geometry and draggable sources projected from the same held list as the displayed rows.
#[derive(Default)]
pub(super) struct Targets {
    pub(super) bands: Vec<Band>,
    pub(super) membership: Vec<MembershipBand>,
    pub(super) unpinned: HashMap<String, SessionGroupTarget>,
}

/// An in-list target that can be drawn even while the ungrouped part is empty. No real row can use this key.
pub(super) const UNGROUP_STRIP: &str = "group/ungroup-drop";

#[derive(Clone, Debug, PartialEq, Eq)]
struct PositionalDrop {
    area: Area,
    line: Target,
    empty: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Drop {
    Position(PositionalDrop),
    Membership(SessionGroupTarget),
}

/// A drag in progress, from the press on a grip to its release or cancellation.
struct Drag {
    /// The dragged row's session key.
    source: String,
    /// The visible marks at the press, including the source. Later marks cannot join an active drag.
    marked: Option<Rc<[String]>>,
    /// Fix the source area when the drag begins. Another client's move
    /// removes this grip from that area and cancels the gesture.
    area: Area,
    /// Where the press was, in window coordinates.
    origin: Point<Pixels>,
    /// Last captured position in window coordinates. A stationary pointer can keep scrolling.
    at: Point<Pixels>,
    /// At most one pending tick, canceled along with this drag or when the pointer leaves the edge zone.
    edge_scroll: Option<ticker::Deadline>,
    /// Whether the pointer has moved [`LIFT_DISTANCE`] from `origin` since the press: the row shows lifted, and only
    /// then can a drop land.
    lifted: bool,
    /// Where a drop would land now, as the drop line shows it.
    target: Option<Drop>,
    /// Whether the window was active at the press. Only such a drag ends when the window goes inactive: a headless
    /// compositor reports its window inactive throughout, and a drag there must still work.
    began_active: bool,
}

impl Drag {
    /// Only a multi-session drag needs membership-only targets or an accompanying count.
    fn others(&self) -> usize {
        self.marked.as_ref().map_or(0, |keys| keys.len() - 1)
    }
}

/// What the pin drag keeps for one UI session: the drag, if one is on, and what the last frame drew of the movable
/// rows, which is what a move or the release is judged against. Shared with the virtualized list's item callback, which
/// records each movable row's bounds as it draws it, and with the window-wide listeners [`watch`] registers.
#[derive(Default)]
pub(super) struct PinDrag {
    drag: Option<Drag>,
    /// The movable pins in the order the last frame drew them; empty while the listing is unavailable or the Hosts
    /// panel shows, so no grip is drawn then and a drag on cancels.
    targets: Targets,
    /// Each movable row's bounds as the last frame drew it: its whole list item, the lines under it included and the
    /// separator after the pinned run not, as the old row's rectangle was. Only rows the list drew are here.
    bounds: HashMap<String, Bounds<Pixels>>,
}

/// [`PinDrag`] as the list, its item callback and its listeners share it.
pub(super) type Shared = Rc<RefCell<PinDrag>>;

impl PinDrag {
    /// Replace the targets with the current layout. Items in its viewport record fresh bounds as they draw.
    fn start_frame(&mut self, targets: Targets) {
        self.bounds.clear();
        self.targets = targets;
    }

    /// Only a movable item can start a drag. An empty edge has bounds but
    /// does not grant a grip to its anchor.
    fn area_of(&self, key: &str) -> Option<Area> {
        self.targets
            .bands
            .iter()
            .find(|band| band.items.iter().any(|item| item == key))
            .map(|band| band.area)
            .or_else(|| self.targets.unpinned.get(key).copied().map(Area::Unpinned))
    }

    /// Record the bounds of each movable item and each empty-area edge.
    fn records(&self, key: &str) -> bool {
        self.targets.bands.iter().any(|band| {
            band.items.iter().any(|item| item == key)
                || band.empty.as_ref().is_some_and(|edge| edge.neighbor == key)
        }) || self
            .targets
            .membership
            .iter()
            .any(|band| band.items.iter().any(|item| item == key))
    }

    /// Whether `key`'s row is drawn lifted now.
    fn lifted(&self, key: &str) -> bool {
        self.drag
            .as_ref()
            .is_some_and(|drag| drag.lifted && drag.source == key)
    }

    /// Put the count on the lifted row alone, while it is carrying other sessions.
    fn accompanying(&self, key: &str) -> usize {
        self.drag
            .as_ref()
            .filter(|drag| drag.lifted && drag.source == key)
            .map_or(0, Drag::others)
    }

    /// The side of `key`'s row the drop line is drawn on now, if it is the drop target.
    fn drop_on(&self, key: &str) -> Option<Placement> {
        let Drop::Position(target) = self.drag.as_ref()?.target.as_ref()? else {
            return None;
        };
        (target.line.neighbor == key).then_some(target.line.placement)
    }

    /// The gap below the groups can receive a whole marked set or a single unpinned member, once lifted.
    pub(super) fn wants_ungroup_strip(&self) -> bool {
        self.drag.as_ref().is_some_and(|drag| {
            drag.lifted
                && (drag.others() > 0
                    || matches!(drag.area, Area::Unpinned(SessionGroupTarget::Group(_))))
        })
    }

    /// Group headers and pins keep their positional priority. A membership drop has no insertion line because the
    /// existing order in the destination determines the row's position.
    fn target(&self, drag: &Drag, clip: Bounds<Pixels>, at: Point<Pixels>) -> Option<Drop> {
        let multiple = drag.others() > 0;
        if !multiple && let Some(position) = self.positional_target(drag, clip, at) {
            return Some(Drop::Position(position));
        }
        let source = match drag.area {
            Area::Groups => return None,
            Area::Pins(group) | Area::Unpinned(group) => group,
        };
        if !clip.contains(&at) {
            return None;
        }
        self.targets.membership.iter().find_map(|band| {
            if !multiple && band.target == source {
                return None;
            }
            let pin_append = !multiple && matches!(drag.area, Area::Pins(_));
            let hit = match (pin_append, band.target) {
                (true, _) => self.hit_rows(&band.append_pin, at),
                (false, SessionGroupTarget::Group(_)) => {
                    // The divider and every rendered member form one destination. Between measured items only
                    // gaps inside the same group are included, never the separator after its last item.
                    let mut bounds = band.items.iter().filter_map(|key| self.bounds.get(key));
                    let first = bounds.next()?;
                    let last = bounds.next_back().unwrap_or(first);
                    inside_band(clip, at, first.top(), last.bottom())
                }
                (false, SessionGroupTarget::Ungrouped) => self.hit_rows(&band.items, at),
            };
            hit.then_some(Drop::Membership(band.target))
        })
    }

    /// Hit the individual members when the destination deliberately excludes neighboring rows or gaps.
    fn hit_rows(&self, keys: &[String], at: Point<Pixels>) -> bool {
        keys.iter().any(|key| {
            self.bounds
                .get(key)
                .is_some_and(|bounds| bounds.contains(&at))
        })
    }

    /// A header lands among headers. Test each pin band's original geometry independently of neighboring groups.
    fn positional_target(
        &self,
        drag: &Drag,
        clip: Bounds<Pixels>,
        at: Point<Pixels>,
    ) -> Option<PositionalDrop> {
        self.targets
            .bands
            .iter()
            .filter(|band| match drag.area {
                Area::Groups => band.area == Area::Groups,
                Area::Pins(_) => matches!(band.area, Area::Pins(_)),
                Area::Unpinned(_) => false,
            })
            .find_map(|band| {
                if let Some(line) = &band.empty {
                    if drag.area == band.area {
                        return None;
                    }
                    let bounds = self.bounds.get(&line.neighbor)?;
                    let (top, bottom) = match line.placement {
                        Placement::Before => (bounds.top(), bounds.center().y),
                        Placement::After => (bounds.center().y, bounds.bottom()),
                    };
                    return inside_band(clip, at, top, bottom).then(|| PositionalDrop {
                        area: band.area,
                        line: line.clone(),
                        empty: true,
                    });
                }
                let rows: Vec<_> = band
                    .items
                    .iter()
                    .filter_map(|key| self.bounds.get(key).map(|bounds| (key.as_str(), *bounds)))
                    .collect();
                let source = (drag.area == band.area).then_some(drag.source.as_str());
                let order = band.items.iter().map(String::as_str);
                let line = drop_in_band(&rows, source, order, clip, at)?;
                Some(PositionalDrop {
                    area: band.area,
                    line,
                    empty: false,
                })
            })
    }

    /// Which edges close the outline laid over this row. The list can paint the highlight one item at a time.
    fn highlighted(&self, key: &str) -> Option<(bool, bool)> {
        let Drop::Membership(target) = self.drag.as_ref()?.target.as_ref()? else {
            return None;
        };
        let band = self
            .targets
            .membership
            .iter()
            .find(|band| band.target == *target)?;
        let index = band.items.iter().position(|item| item == key)?;
        Some((index == 0, index + 1 == band.items.len()))
    }

    /// The divider between a group's pins and loose members is internal to the group outline.
    pub(super) fn highlights_group(&self, id: i64) -> bool {
        self.drag.as_ref().is_some_and(|drag| {
            drag.target == Some(Drop::Membership(SessionGroupTarget::Group(id)))
        })
    }
}

// ---- Where a drop lands --------------------------------------------------------------------------------------------

/// Where a drop at `at` lands for the drag of `source`: `rows` are the movable pins the list drew, top to bottom, with
/// their bounds, and `clip` is the list's view, outside which nothing can be released on. Ported from the old
/// `dropTarget` and `groupBounds`.
///
/// The group is the band from the first drawn movable row's top to the last one's bottom, cut to `clip`, across the
/// whole of `clip`'s width: a release above or below it (on the separator, an unpinned row, a row made pinned during the
/// hold) or beside it (past the sidebar's edge) is outside, and so is any point of a row scrolled out of view. Inside
/// it, the target is the first other pin whose middle lies below `at`, with the dragged pin going before it, or failing
/// that the last other pin, with it going after. A target that would leave the pin where it is (before the pin right
/// below it, after the one right above it) is no drop. `None` for all of those, and when `source` was not drawn or has
/// no other pin to go by. This test helper uses one list for both the measured bounds and the band order.
#[cfg(test)]
pub(super) fn drop_target(
    rows: &[(&str, Bounds<Pixels>)],
    source: &str,
    clip: Bounds<Pixels>,
    at: Point<Pixels>,
) -> Option<Target> {
    if !rows.iter().any(|(key, _)| *key == source) {
        return None;
    }
    drop_in_band(
        rows,
        Some(source),
        rows.iter().map(|(key, _)| *key),
        clip,
        at,
    )
}

/// Resolve the insertion line using measured rows. Compare it to the source's slot in the full band order so a
/// source clipped just past the viewport can still be adjacent to a visible neighbor.
fn drop_in_band<'a>(
    rows: &[(&str, Bounds<Pixels>)],
    source: Option<&str>,
    order: impl Iterator<Item = &'a str>,
    clip: Bounds<Pixels>,
    at: Point<Pixels>,
) -> Option<Target> {
    let (first, last) = (rows.first()?, rows.last()?);
    if !inside_band(clip, at, first.1.top(), last.1.bottom()) {
        return None;
    }
    let others = || rows.iter().filter(|(key, _)| Some(*key) != source);
    let (neighbor, placement) = match others().find(|(_, bounds)| at.y < bounds.center().y) {
        Some((key, _)) => (*key, Placement::Before),
        None => (others().next_back()?.0, Placement::After),
    };
    let stays = source.is_some_and(|source| {
        let (first, second) = match placement {
            Placement::Before => (source, neighbor),
            Placement::After => (neighbor, source),
        };
        let mut neighbors = order.skip_while(|key| *key != first);
        neighbors.next();
        neighbors.next() == Some(second)
    });
    (!stays).then(|| Target {
        neighbor: neighbor.to_owned(),
        placement,
    })
}

/// All insertion bands are clipped to the visible list, including a
/// half-header target for an empty area.
fn inside_band(clip: Bounds<Pixels>, at: Point<Pixels>, top: Pixels, bottom: Pixels) -> bool {
    at.x >= clip.left()
        && at.x < clip.right()
        && at.y >= top.max(clip.top())
        && at.y < bottom.min(clip.bottom())
}

// ---- The row's part ----------------------------------------------------------------------------------------------------

/// What a row draws for the pin drag ([`paint`]): its grip, whether it is lifted, the drop line when it is the target,
/// and what records its bounds for the drag to judge a drop by.
pub(super) struct RowPaint {
    /// The grip, over the row's leading gutter; `None` for a row that cannot be dragged now.
    pub(super) grip: Option<AnyElement>,
    /// Whether the row is the one being dragged, lifted.
    pub(super) lifted: bool,
    /// How many other marked sessions the lifted row is carrying.
    pub(super) accompanying: usize,
    /// The drop line, inside the row's top or bottom edge, when a drop would land beside this row.
    pub(super) drop_line: Option<AnyElement>,
    /// A membership destination closes its outline at the ends of the band, rather than drawing a separate box per row.
    pub(super) membership_outline: Option<AnyElement>,
    /// Records the row's bounds as it is laid out, for a source or a destination.
    pub(super) recorder: Option<AnyElement>,
}

/// The fill of a lifted row (`.session-row[data-pin-dragging]`): `--bg-2`, or one step up the accent family,
/// `--accent-fill-hover`, on the selected row, so the lift does not hide which row is selected.
pub(super) fn lifted_fill(palette: &Palette, selected: bool) -> Hsla {
    theme::color(if selected {
        palette.accent_fill_hover
    } else {
        palette.bg_2
    })
}

/// The shadow under a lifted row: the old `--shadow-menu`.
pub(super) fn lifted_shadow(palette: &Palette) -> Vec<gpui::BoxShadow> {
    layers::menu_shadow(palette)
}

/// What `key`'s row draws for the pin drag this frame, from `shared`, the list's drag state, and `rows`, its row menus'
/// state (for whether a question of the row is open). `row_active` says whether the pointer is over the row or focus is
/// inside it, two of the grip's reveal reasons. Called from the virtualized list's item callback while the row's
/// elements are made.
#[allow(
    clippy::too_many_arguments,
    reason = "one row's part of the drag: its identity, its state and the window it is drawn in"
)]
pub(super) fn paint(
    shared: &Shared,
    rows: &SharedRows,
    list: &WeakEntity<SessionList>,
    key: &str,
    selected: bool,
    row_active: bool,
    window: &mut Window,
    cx: &mut App,
) -> RowPaint {
    let state = shared.borrow();
    if !state.records(key) {
        return RowPaint {
            grip: None,
            lifted: false,
            accompanying: 0,
            drop_line: None,
            membership_outline: None,
            recorder: None,
        };
    }
    let palette = *theme::palette(cx);
    let lifted = state.lifted(key);
    let accompanying = state.accompanying(key);
    let drop_on = state.drop_on(key);
    let area = state.area_of(key);
    let outline = state.highlighted(key);
    drop(state);
    let can_drag = area.is_some()
        && !refused(key, cx)
        && !retired_row(key, cx)
        && !rows.borrow_mut().asks(key, window, cx);
    let grip = can_drag.then(|| {
        grip(
            key,
            area,
            selected,
            row_active || lifted,
            list,
            &palette,
            window,
            cx,
        )
        .into_any_element()
    });
    let drop_line = drop_on.map(|placement| drop_line(key, placement, &palette).into_any_element());
    RowPaint {
        grip,
        lifted,
        accompanying,
        drop_line,
        membership_outline: outline
            .map(|(first, last)| membership_outline(key, first, last, &palette).into_any_element()),
        recorder: Some(record_bounds(shared, key).into_any_element()),
    }
}

/// The same bounds recorder serves a list item or the temporary strip. It adds no layout or hit area.
fn record_bounds(shared: &Shared, key: &str) -> impl IntoElement + use<> {
    let (shared, key) = (shared.clone(), key.to_owned());
    canvas(
        move |bounds, window, _| {
            // GPUI lays out an offscreen focused item below the visible rows to keep its keys alive. That
            // synthetic rectangle cannot describe the first or last edge of a target band.
            if bounds.intersects(&window.content_mask().bounds) {
                shared.borrow_mut().bounds.insert(key, bounds);
            }
        },
        |_, (), _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// No insertion position is implied by this highlight. Consecutive items close only the edges they own.
fn membership_outline(
    key: &str,
    first: bool,
    last: bool,
    palette: &Palette,
) -> impl IntoElement + use<> {
    let key = key.to_owned();
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .right_0()
        .border_l_1()
        .border_r_1()
        .when(first, |outline| outline.border_t_1())
        .when(last, |outline| outline.border_b_1())
        .border_color(theme::color(palette.accent))
        .debug_selector(|| format!("session-membership-drop-{key}"))
}

/// When no ungrouped rows are visible, put their destination in the list so it can be scrolled to.
pub(super) fn ungroup_strip(shared: &Shared, palette: &Palette) -> AnyElement {
    let active = shared.borrow().highlighted(UNGROUP_STRIP).is_some();
    div()
        .id("session-ungroup-drop")
        .debug_selector(|| "session-ungroup-drop".into())
        .relative()
        .px(px(12.))
        .py(px(6.))
        .border_1()
        .border_dashed()
        .border_color(theme::color(palette.accent))
        .when(active, |strip| strip.bg(theme::color(palette.marked_bg)))
        .child("Drop here to remove from group")
        .child(record_bounds(shared, UNGROUP_STRIP))
        .into_any_element()
}

/// `key`'s grip: one control, a button named and helped as the old grip was, its help beside the sidebar as every row
/// hover's, drawing the dot field in the row's secondary ink (`--fg-1`, or `--on-fill` on the selected row) at the top
/// of the row's leading gutter, with the open hand. A primary press starts a drag and moves focus to it. Transparent
/// unless `shown`, but there to press either way, as the old grip was. The debug selector says which, as the menu
/// button's does: `session-pin-grip-<key>:shown` or `:hidden`.
///
/// It tracks the grip's own focus handle, keyed element state (no Tab stop, as the old `tabindex="-1"`), so the
/// handle lives while the grip is drawn and goes with it. The press is taken in the capture phase, on its way down,
/// where it starts the drag and moves focus to the grip through the coordinator; in the bubble phase the grip then keeps
/// it, so it never reaches the row's open control (no open) and GPUI's own focus-on-press does not run.
#[allow(
    clippy::too_many_arguments,
    reason = "the existing grip takes the source area to name its action"
)]
fn grip(
    key: &str,
    area: Option<Area>,
    selected: bool,
    shown: bool,
    list: &WeakEntity<SessionList>,
    palette: &Palette,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let id = SharedString::from(format!("{GRIP_ID_PREFIX}{key}"));
    let focus = window
        .use_keyed_state(
            SharedString::from(format!("{id}-focus-handle")),
            cx,
            |_, cx| GripFocus(cx.focus_handle()),
        )
        .read(cx)
        .0
        .clone();
    let ink = if selected {
        palette.on_fill
    } else {
        palette.fg_1
    };
    let (label, words) = match area {
        Some(Area::Groups) => ("Drag group", "Drag to reorder groups"),
        Some(Area::Unpinned(_)) => (
            "Drag session",
            "Drag to move this session into or out of a group",
        ),
        _ => (GRIP_LABEL, GRIP_HELP),
    };
    let help = HelpPlace::Row { selected }.place(HoverHelp::tooltip(words));
    let selector = id.clone();
    let grip = div()
        .id(id.clone())
        .role(Role::Button)
        .aria_label(label)
        .absolute()
        .top_0()
        .left_0()
        .opacity(if shown { 1. } else { 0. })
        .cursor_grab()
        .debug_selector(move || format!("{selector}:{}", if shown { "shown" } else { "hidden" }))
        .child(dots(ink));
    let grip = with_hover_help(grip, &id, &help, true, Some(&focus), window, cx);
    let grip = focus::track(grip, &focus, focus::OnPress::Keep);
    let (pressed_list, pressed, pressed_focus) = (list.clone(), key.to_owned(), focus.clone());
    grip.capture_any_mouse_down(move |event, window, cx| {
        if event.button != MouseButton::Left {
            return;
        }
        let active = window.is_window_active();
        let began = pressed_list
            .update(cx, |list, cx| {
                list.begin_pin_drag(&pressed, event.position, active, cx)
            })
            .unwrap_or(false);
        if began {
            focus::focus_control(&pressed_focus, &Mark::take(cx), window, cx);
        }
    })
    // Registered after GPUI's own focus-on-press for the element, so in the bubble phase it runs first and keeps the
    // press from it and from everything around the grip, as the status dot keeps its press.
    .on_mouse_down(MouseButton::Left, |_, window, cx| {
        window.prevent_default();
        cx.stop_propagation();
    })
}

/// Whether `key`'s row is a retired containerized session's ([`super::retired`]): its pin stays a drop target, but its
/// grip is never drawn and no drag of it starts, as the old app disabled that grip alone.
fn retired_row(key: &str, cx: &App) -> bool {
    crate::session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<farhelm_proto::ui_channel::SessionRow>(key)
            .is_some_and(super::retired::is)
    })
}

/// Group operations have their own busy key. Rows also inherit their
/// lifecycle and confirmation rules from their ordinary menu.
fn refused(key: &str, cx: &App) -> bool {
    match groups::header_id(key) {
        Some(id) => crate::session::current_operations(cx).is_none_or(|operations| {
            operations
                .read(cx)
                .in_flight(&OpKey::session_group(id))
                .is_some()
        }),
        None => menu::refused(key, cx),
    }
}

/// The grip's focus handle, kept in its element state.
struct GripFocus(gpui::FocusHandle);

/// The grip's dot field in `ink`, in the grip's box ([`DOTS`]).
fn dots(ink: Token) -> impl IntoElement {
    let ink = theme::color(ink);
    div()
        .relative()
        .w(px(GRIP_SIZE.0))
        .h(px(GRIP_SIZE.1))
        .children(DOTS.iter().map(|(x, y)| {
            div()
                .absolute()
                .left(px(*x))
                .top(px(*y))
                .size(px(2.))
                .rounded_full()
                .bg(ink)
        }))
}

/// The drop line on `key`'s row: [`DROP_LINE`] pixels of `--accent` across the row inside its top edge (the dragged pin
/// goes before this one) or its bottom edge (after). Tests find it by its debug selector,
/// `session-pin-drop-<key>:before` or `:after`.
fn drop_line(key: &str, placement: Placement, palette: &Palette) -> impl IntoElement + use<> {
    let selector = format!("session-pin-drop-{key}:{}", placement.word());
    div()
        .absolute()
        .left_0()
        .right_0()
        .h(px(DROP_LINE))
        .map(|line| match placement {
            Placement::Before => line.top_0(),
            Placement::After => line.bottom_0(),
        })
        .bg(theme::color(palette.accent))
        .debug_selector(move || selector.clone())
}

// ---- Capture -------------------------------------------------------------------------------------------------------

/// The drag's window-wide listeners, drawn by the list on every frame that draws its rows: a drag's moves and its
/// release are watched wherever the pointer is, in the capture phase, and stopped there while a drag is on (see the
/// module docs). Registered on every frame, not only while a drag is on, because a press, its moves and its release can
/// all arrive before the next frame; with no drag on they return at once, having read only `shared`. While the row is
/// lifted the whole window shows the closed hand, as the old list forced `grabbing` during a drag.
pub(super) fn watch(shared: &Shared, list: &WeakEntity<SessionList>) -> impl IntoElement + use<> {
    let (shared, list) = (shared.clone(), list.clone());
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            if shared
                .borrow()
                .drag
                .as_ref()
                .is_some_and(|drag| drag.lifted)
            {
                window.set_window_cursor_style(CursorStyle::ClosedHand);
            }
            let (moving, moved) = (shared.clone(), list.clone());
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture || moving.borrow().drag.is_none() {
                    return;
                }
                cx.stop_propagation();
                let held = event.pressed_button == Some(MouseButton::Left);
                let _ = moved.update(cx, |list, cx| {
                    if held {
                        list.pin_drag_moved(event.position, cx);
                    } else {
                        // The release went where the window never saw it: the old `lostpointercapture`.
                        let _ = list.cancel_pin_drag(cx);
                    }
                });
            });
            let (releasing, released) = (shared, list);
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture
                    || event.button != MouseButton::Left
                    || releasing.borrow().drag.is_none()
                {
                    return;
                }
                cx.stop_propagation();
                let _ = released.update(cx, |list, cx| list.pin_drag_released(event.position, cx));
            });
        },
    )
    .absolute()
    .size_0()
}

/// `body`, the sidebar's body (the host rail, the list's column and the rail's foot), with Escape caught on its way
/// down while a drag is on: a capture-phase listener for the dismiss action that ends the drag and stops the key there,
/// so neither the list's own Escape (which clears the marks) nor the root's (which closes the topmost layer or a
/// tooltip) acts on it, wherever focus is inside the body (see the module docs' "Escape"). With no drag on it does
/// nothing and the key goes on.
pub(super) fn catch_escape(body: Div, cx: &mut Context<SessionList>) -> Div {
    body.capture_action(cx.listener(|list, _: &Dismiss, _, cx| {
        if list.cancel_pin_drag(cx) {
            cx.stop_propagation();
        }
    }))
}

// ---- The list's side ---------------------------------------------------------------------------------------------------

impl SessionList {
    /// The list's pin drag state for the current UI session.
    fn pin_drag(&mut self, cx: &App) -> Shared {
        self.current.current(cx).pin_drag.clone()
    }

    /// At the top of a frame that draws the rows (or would, but for the Hosts panel or an unavailable listing):
    /// `targets` is this frame's destinations and movable rows, empty when no row may be dragged. `window_active` says
    /// the window is active now. A drag whose row is no longer movable, whose session has a request in flight, or whose
    /// window went inactive since its press ends here with nothing sent, so the frame draws it gone. GPUI redraws the
    /// window when its activation changes, so this frame comes.
    pub(super) fn pin_drag_frame(
        &mut self,
        targets: Targets,
        window_active: bool,
        cx: &mut Context<Self>,
    ) -> Shared {
        let shared = self.pin_drag(cx);
        let gone = {
            let mut state = shared.borrow_mut();
            state.start_frame(targets);
            state.drag.as_ref().is_some_and(|drag| {
                state.area_of(&drag.source) != Some(drag.area)
                    || refused(&drag.source, cx)
                    || (drag.began_active && !window_active)
            })
        };
        if gone {
            shared.borrow_mut().drag = None;
            // The layout was already projected while the drag was live, so it can still include its strip.
            cx.notify();
        }
        shared
    }

    /// A primary press on `key`'s grip at `at`, in a window that is `active` or not: start a drag, unless one is on or
    /// the row cannot be dragged now (the frame that drew the grip may be older than the press). Returns whether it
    /// started one, so the grip takes focus only for a drag.
    pub(super) fn begin_pin_drag(
        &mut self,
        key: &str,
        at: Point<Pixels>,
        active: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let shared = self.pin_drag(cx);
        let area = {
            let state = shared.borrow();
            if state.drag.is_some() || refused(key, cx) || retired_row(key, cx) {
                return false;
            }
            let Some(area) = state.area_of(key) else {
                return false;
            };
            area
        };
        let shown = self.marking.shown();
        let marked = (area != Area::Groups && shown.iter().any(|row| row == key)).then_some(shown);
        shared.borrow_mut().drag = Some(Drag {
            source: key.to_owned(),
            marked,
            area,
            origin: at,
            at,
            edge_scroll: None,
            lifted: false,
            target: None,
            began_active: active,
        });
        true
    }

    /// The pointer moved to `at` with the primary button held during a drag: it is activity over the rows for the order
    /// hold, and the lift and the drop line are worked out again; a change redraws.
    pub(super) fn pin_drag_moved(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.pointer_activity(cx);
        if self.resample_pin_drag(at, cx) {
            cx.notify();
        }
        self.arm_edge_scroll(cx);
    }

    /// A lift can add the ungroup strip in the next frame, so identify the edge without using the old scroll limit.
    fn edge_scroll_delta(&self, at: Point<Pixels>) -> Pixels {
        let clip = self.rows.viewport_bounds();
        if !clip.contains(&at) {
            return px(0.);
        }
        let zone = EDGE_SCROLL_ZONE.min(clip.size.height / 2.);
        if at.y < clip.top() + zone {
            -EDGE_SCROLL_STEP
        } else if at.y > clip.bottom() - zone {
            EDGE_SCROLL_STEP
        } else {
            px(0.)
        }
    }

    /// Use the list's measured scroll bounds at tick time. Stop at the limit even while the pointer stays in the zone.
    fn edge_scroll_step(&self, at: Point<Pixels>) -> Pixels {
        let delta = self.edge_scroll_delta(at);
        let offset = -self.rows.scroll_px_offset_for_scrollbar().y;
        let end = self.rows.max_offset_for_scrollbar().y;
        delta.clamp(-offset.max(px(0.)), (end - offset).max(px(0.)))
    }

    /// A captured move or tick can arm one deadline. Dropping it on cancellation keeps stale callbacks from moving
    /// the viewport for a later gesture.
    fn arm_edge_scroll(&mut self, cx: &mut Context<Self>) {
        let shared = self.pin_drag(cx);
        let mut state = shared.borrow_mut();
        let Some(drag) = state.drag.as_mut() else {
            return;
        };
        if !drag.lifted || self.edge_scroll_delta(drag.at) == px(0.) {
            drag.edge_scroll = None;
        } else if drag.edge_scroll.is_none() {
            let list = cx.weak_entity();
            drag.edge_scroll = Some(ticker::after(EDGE_SCROLL_TICK, cx, move |cx| {
                let _ = list.update(cx, Self::edge_scroll_tick);
            }));
        }
    }

    /// Resample the previous step's frame, then move the list and request new measured bounds. A tick counts as
    /// pointer activity for the order hold, just as a wheel event does.
    fn edge_scroll_tick(&mut self, cx: &mut Context<Self>) {
        let shared = self.pin_drag(cx);
        let at = {
            let mut state = shared.borrow_mut();
            let Some(drag) = state.drag.as_mut() else {
                return;
            };
            drag.edge_scroll = None;
            drag.at
        };
        if self.resample_pin_drag(at, cx) {
            cx.notify();
        }
        if shared.borrow().drag.is_none() {
            return;
        }
        let step = self.edge_scroll_step(at);
        if step != px(0.) {
            self.pointer_activity(cx);
            // Arm before the scroll so its final step still gets a fresh target sample at the limit.
            self.arm_edge_scroll(cx);
            self.rows.scroll_by(step);
            cx.notify();
        }
    }

    /// The primary button came up at `at` during a drag: the drag ends, and a drop on a drop line sends its one move.
    pub(super) fn pin_drag_released(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        // A resample that ends the drag (its row no longer eligible, a request in flight) leaves nothing to take below,
        // and the row it lifted must still be redrawn.
        if self.resample_pin_drag(at, cx) {
            cx.notify();
        }
        let shared = self.pin_drag(cx);
        let Some(drag) = shared.borrow_mut().drag.take() else {
            return;
        };
        cx.notify();
        if let (true, Some(target)) = (drag.lifted, drag.target) {
            match (drag.marked, target) {
                (Some(marked), Drop::Membership(target)) => self.drop_marked(&marked, target, cx),
                // A lone marked pin whose mark was cleared mid-drag: the drag no longer stands for a marked
                // selection, and a positional move was never offered for one, so it is dropped on purpose, as
                // `drop_marked` skips rows that are no longer marked.
                (Some(_), Drop::Position(_))
                    if !self.current.current(cx).marks.contains(&drag.source) => {}
                (_, target) => self.send_drop(&drag.source, drag.area, target, cx),
            }
        }
    }

    /// Re-admit the marked set caught at the press. The drop keeps the marks and makes no focus handoff of its own.
    fn drop_marked(
        &mut self,
        marked: &[String],
        target: SessionGroupTarget,
        cx: &mut Context<Self>,
    ) {
        let shown = self.marking.shown();
        let marks = &self.current.current(cx).marks;
        let keys: Vec<_> = marked
            .iter()
            .filter(|key| marks.contains(key) && shown.contains(key))
            .map(String::as_str)
            .collect();
        groups::move_marked(
            keys,
            groups::Destination::Existing(target),
            Some(cx.weak_entity()),
            cx,
        );
    }

    /// The same relative move serves keyboard and grip. With no neighbor,
    /// the visible destination's first pin uses the membership append.
    fn send_drop(&mut self, key: &str, source: Area, drop: Drop, cx: &mut Context<Self>) {
        let drop = match drop {
            Drop::Position(drop) => drop,
            Drop::Membership(target) => {
                if groups::eligible_member(key, Some(target), cx) {
                    let pin_source = match source {
                        Area::Pins(group) => Some(group),
                        Area::Unpinned(_) => None,
                        Area::Groups => return,
                    };
                    Self::send_membership_drop(&cx.weak_entity(), key, pin_source, target, cx);
                }
                return;
            }
        };
        let Target {
            neighbor,
            placement,
        } = drop.line;
        match (source, drop.area) {
            (Area::Groups, Area::Groups) => pins::send_move(key, &neighbor, placement, None, cx),
            (Area::Pins(source), Area::Pins(destination)) => {
                let areas = PinMoveAreas {
                    source,
                    destination,
                };
                if source == destination {
                    pins::send_move(key, &neighbor, placement, Some(areas), cx);
                    return;
                }
                if drop.empty {
                    Self::send_membership_drop(
                        &cx.weak_entity(),
                        key,
                        Some(source),
                        destination,
                        cx,
                    );
                } else {
                    let command = pins::move_command(key, &neighbor, placement, Some(areas));
                    Self::send_crossing(
                        &cx.weak_entity(),
                        key,
                        destination,
                        true,
                        Some((neighbor, placement)),
                        command,
                        cx,
                    );
                }
            }
            _ => {}
        }
    }

    /// Whole-group drops and empty pin-band drops use the same membership request. Pins carry their expected area so
    /// an intervening move or unpin turns the write into a refusal.
    fn send_membership_drop(
        list: &WeakEntity<Self>,
        key: &str,
        pin_source: Option<SessionGroupTarget>,
        target: SessionGroupTarget,
        cx: &mut App,
    ) {
        let command = groups::membership(key, SessionGroupMembership { target, pin_source });
        Self::send_crossing(list, key, target, pin_source.is_some(), None, command, cx);
    }

    /// A single drop uses the row operation path and the same held-slot completion as a marked drop.
    fn send_crossing(
        list: &WeakEntity<Self>,
        key: &str,
        target: SessionGroupTarget,
        pinned: bool,
        neighbor: Option<(String, Placement)>,
        command: Result<crate::operations::Command, farhelm_helm_client::PathError>,
        cx: &mut App,
    ) {
        let key = key.to_owned();
        let list = list.clone();
        menu::send_with(&OpKey::session(&key), command, cx, move |settled, cx| {
            let neighbor = neighbor
                .as_ref()
                .map(|(key, placement)| (key.as_str(), *placement));
            crossed_after(&list, &key, target, pinned, neighbor, settled, cx);
        });
    }

    /// End the drag on, if any, with nothing sent: Escape, a lost release, or the list taking its row away. Returns
    /// whether a drag was on, so Escape goes on to whatever else takes it when none was.
    pub(super) fn cancel_pin_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let shared = self.pin_drag(cx);
        let ended = shared.borrow_mut().drag.take().is_some();
        if ended {
            cx.notify();
        }
        ended
    }

    /// Judge the drag on at `at` against the movable rows as last drawn, as the old gesture's `sample` did before every
    /// paint and before the drop: losing its grip before lift or ceasing to be movable ends the drag. Otherwise it
    /// lifts once the pointer is [`LIFT_DISTANCE`] from the press, and once lifted its
    /// target is where a drop at `at` lands ([`drop_target`]). Returns whether what the rows show changed.
    fn resample_pin_drag(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let clip = self.rows.viewport_bounds();
        let shared = self.pin_drag(cx);
        let mut state = shared.borrow_mut();
        let Some(drag) = state.drag.as_ref() else {
            return false;
        };
        let source = drag.source.clone();
        if state.area_of(&source) != Some(drag.area)
            || (!drag.lifted && !state.bounds.contains_key(&source))
            || refused(&source, cx)
            || !source_matches(&source, drag.area, cx)
        {
            state.drag = None;
            return true;
        }
        let lifted = drag.lifted || {
            let (dx, dy) = (
                f32::from(at.x - drag.origin.x),
                f32::from(at.y - drag.origin.y),
            );
            dx.hypot(dy) >= LIFT_DISTANCE
        };
        let target = if lifted {
            state.target(drag, clip, at)
        } else {
            None
        };
        let Some(drag) = state.drag.as_mut() else {
            return false;
        };
        let changed = (drag.lifted, &drag.target) != (lifted, &target);
        drag.lifted = lifted;
        drag.target = target;
        drag.at = at;
        changed
    }
}

/// Grip geometry comes from the last frame, which can precede a membership push. Recheck the dragged session's
/// pin state and source group, so accepting a drop never derives another expected source from a changed row.
fn source_matches(key: &str, area: Area, cx: &App) -> bool {
    let target = match area {
        Area::Groups => return true,
        Area::Pins(target) | Area::Unpinned(target) => target,
    };
    crate::session::current_store(cx)
        .and_then(|store| store.read(cx).collections().get::<SessionRow>(key))
        .is_some_and(|row| {
            row.pinned == matches!(area, Area::Pins(_))
                && row.group == target.group()
                && !super::retired::is(row)
        })
}

/// A successful drag advances the held slot in its own window. The helm's push precedes its reply, so the row must
/// already be in the requested destination with the pin state it carried. An outside move leaves the held slot alone.
pub(super) fn crossed_after(
    list: &WeakEntity<SessionList>,
    key: &str,
    target: SessionGroupTarget,
    pinned: bool,
    neighbor: Option<(&str, Placement)>,
    settled: &Settled,
    cx: &mut App,
) {
    if !matches!(settled.outcome, Outcome::Done(_)) {
        return;
    }
    let _ = list.update(cx, |list, cx| {
        let Some(store) = crate::session::current_store(cx) else {
            return;
        };
        let collections = store.read(cx).collections();
        let row_of = |key: &str| collections.get::<SessionRow>(key);
        let Some(row) =
            row_of(key).filter(|row| row.pinned == pinned && row.group == target.group())
        else {
            return;
        };
        let band = groups::Band::served(&super::selection::Listed { key, row });
        let is_pinned = |key: &str| row_of(key).is_some_and(|row| row.pinned);
        list.current
            .current(cx)
            .hold
            .crossed(key, band, neighbor, is_pinned);
        cx.notify();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    /// Three rows of 40 pixels from the top of a 400-pixel view, as the list would draw a, b and c.
    fn rows() -> Vec<(&'static str, Bounds<Pixels>)> {
        ["a", "b", "c"]
            .into_iter()
            .enumerate()
            .map(|(ix, key)| {
                let top = px(40. * ix as f32);
                (
                    key,
                    Bounds::new(point(px(0.), top), size(px(300.), px(40.))),
                )
            })
            .collect()
    }

    fn view() -> Bounds<Pixels> {
        Bounds::new(point(px(0.), px(0.)), size(px(300.), px(400.)))
    }

    fn target(neighbor: &str, placement: Placement) -> Option<Target> {
        Some(Target {
            neighbor: neighbor.to_owned(),
            placement,
        })
    }

    /// The old `dropTarget` decided which pin a release names and on which side, and this is that rule; a wrong side
    /// moves a pin one place off from where the drop line showed it would go, and a missed own slot sends a move that
    /// changes nothing. Specified, among a, b, c (40 pixels each, middles at 20, 60 and 100): dragging c, a point above
    /// a's middle is before a and one between a's and b's middles before b, while one below b's middle (after b) or on
    /// c itself is c's own slot and no drop; dragging a, a point in b's upper half (before b) is a's own slot, one
    /// between b's and c's middles is before c, and one below c's middle after c; dragging b, a point above a's middle
    /// is before a, and b's own middle (before c) is its own slot.
    #[test]
    fn a_drop_names_the_pin_below_the_pointers_line_or_the_last_one() {
        let rows = rows();
        let at = |y: f32| point(px(150.), px(y));
        let drop = |source: &str, y: f32| drop_target(&rows, source, view(), at(y));
        assert_eq!(drop("c", 3.), target("a", Placement::Before));
        assert_eq!(drop("c", 25.), target("b", Placement::Before));
        assert_eq!(drop("c", 65.), None, "after b is c's own slot");
        assert_eq!(drop("c", 115.), None, "on c itself");
        assert_eq!(drop("a", 45.), None, "before b is a's own slot");
        assert_eq!(drop("a", 65.), target("c", Placement::Before));
        assert_eq!(drop("a", 110.), target("c", Placement::After));
        assert_eq!(drop("b", 10.), target("a", Placement::Before));
        assert_eq!(drop("b", 60.), None, "before c is b's own slot");
    }

    /// SPEC.md: "releasing outside the pinned group cancels", and the old `groupBounds` cut the group to what the user
    /// could see and release on, so a pin scrolled out of sight was no target. Specified: a point above the first row,
    /// at or below the last row's bottom (the separator, an unpinned row), beside the view, or in a row's part outside
    /// the view lands nowhere; with a lone movable pin nothing does; a source that was not drawn lands nowhere.
    #[test]
    fn a_release_outside_the_drawn_group_lands_nowhere() {
        let rows = rows();
        assert_eq!(
            drop_target(&rows, "c", view(), point(px(150.), px(-1.))),
            None
        );
        assert_eq!(
            drop_target(&rows, "c", view(), point(px(150.), px(120.))),
            None
        );
        assert_eq!(
            drop_target(&rows, "c", view(), point(px(300.), px(3.))),
            None
        );
        let scrolled = Bounds::new(point(px(0.), px(20.)), size(px(300.), px(380.)));
        assert_eq!(
            drop_target(&rows, "c", scrolled, point(px(150.), px(10.))),
            None
        );
        assert_eq!(
            drop_target(&rows, "c", scrolled, point(px(150.), px(25.))),
            target("b", Placement::Before),
            "inside the view the same rule holds"
        );
        assert_eq!(
            drop_target(&rows[..1], "a", view(), point(px(150.), px(3.))),
            None
        );
        assert_eq!(
            drop_target(&rows[..2], "c", view(), point(px(150.), px(3.))),
            None
        );
    }
}
