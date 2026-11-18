//! Dragging a pinned row by its grip to another place in the pinned group: the pointer's way to make the move that Move
//! up and Move down make from the row's menu.
//!
//! SPEC.md, "Session list": "Hovering an eligible pinned row reveals a grip beside its name. Dragging that grip within
//! the pinned group shows the row lifted and a drop line; release moves it relative to the pin at that line. Escape,
//! interrupted pointer capture or releasing outside the pinned group cancels." Ported from the old app's `pin_drag.rs`
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
//! - **The drop.** A release on a drop line sends one move through the route the menu's items use, naming that pin and
//!   the side of it ([`pins::send_move`]); the order hold lets the helm's new order through at once, as it does for a
//!   Move from the menu. A release outside the group, on the row's own slot or before the lift sends nothing.
//! - **The list cancels too.** The drag ends with nothing sent when its row stops being movable (deleted, unpinned, a
//!   request of its session in flight), when its row is scrolled so far away that it is no longer drawn, when the
//!   listing becomes unavailable, or when the Hosts panel takes the rows' place: the old list root that a failed
//!   listing or a hidden list took away took its gesture with it. A grip drawn again afterwards drags as before. A drag
//!   that began in an active window also ends when the window is no longer active (above).
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

use gpui::{
    AnyElement, App, Bounds, Context, CursorStyle, DispatchPhase, Div, Hsla, InteractiveElement,
    IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point,
    SharedString, StatefulInteractiveElement as _, Styled, WeakEntity, Window, canvas, div,
    prelude::FluentBuilder as _, px,
};

use super::SessionList;
use super::menu::{self, SharedRows};
use super::pins::{self, Placement};
use super::row_marks::HelpPlace;
use crate::actions::Dismiss;
use crate::components::{HoverHelp, Role, with_hover_help};
use crate::focus::{self, Mark};
use crate::layers;
use crate::theme::{self, Palette, Token};

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

// ---- State -------------------------------------------------------------------------------------------------------------

/// Where a drop would land: the movable pin the drop line is drawn on and the side of it the dragged pin goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub(super) neighbor: String,
    pub(super) placement: Placement,
}

/// A drag in progress, from the press on a grip to its release or cancellation.
#[derive(Debug)]
struct Drag {
    /// The dragged row's session key.
    source: String,
    /// Where the press was, in window coordinates.
    origin: Point<Pixels>,
    /// Whether the pointer has moved [`LIFT_DISTANCE`] from `origin` since the press: the row shows lifted, and only
    /// then can a drop land.
    lifted: bool,
    /// Where a drop would land now, as the drop line shows it.
    target: Option<Target>,
    /// Whether the window was active at the press. Only such a drag ends when the window goes inactive: a headless
    /// compositor reports its window inactive throughout, and a drag there must still work.
    began_active: bool,
}

/// What the pin drag keeps for one UI session: the drag, if one is on, and what the last frame drew of the movable
/// rows, which is what a move or the release is judged against. Shared with the virtualized list's item callback, which
/// records each movable row's bounds as it draws it, and with the window-wide listeners [`watch`] registers.
#[derive(Default)]
pub(super) struct PinDrag {
    drag: Option<Drag>,
    /// The movable pins in the order the last frame drew them; empty while the listing is unavailable or the Hosts
    /// panel shows, so no grip is drawn then and a drag on cancels.
    movable: Vec<String>,
    /// Each movable row's bounds as the last frame drew it: its whole list item, the lines under it included and the
    /// separator after the pinned run not, as the old row's rectangle was. Only rows the list drew are here.
    bounds: HashMap<String, Bounds<Pixels>>,
}

/// [`PinDrag`] as the list, its item callback and its listeners share it.
pub(super) type Shared = Rc<RefCell<PinDrag>>;

impl PinDrag {
    /// Start a frame: forget the rows the last frame drew, and take `movable`, this frame's movable pins in order.
    fn start_frame(&mut self, movable: Vec<String>) {
        self.bounds.clear();
        self.movable = movable;
    }

    /// Whether `key`'s row is drawn lifted now.
    fn lifted(&self, key: &str) -> bool {
        self.drag
            .as_ref()
            .is_some_and(|drag| drag.lifted && drag.source == key)
    }

    /// The side of `key`'s row the drop line is drawn on now, if it is the drop target.
    fn drop_on(&self, key: &str) -> Option<Placement> {
        let target = self.drag.as_ref()?.target.as_ref()?;
        (target.neighbor == key).then_some(target.placement)
    }

    /// The movable rows the last frame drew, in order, with their bounds.
    fn drawn_rows(&self) -> Vec<(&str, Bounds<Pixels>)> {
        self.movable
            .iter()
            .filter_map(|key| self.bounds.get(key).map(|bounds| (key.as_str(), *bounds)))
            .collect()
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
/// no other pin to go by.
///
/// Rows the list did not draw (far from the view) are not in `rows`. That cannot change the answer for a point inside
/// the view: the list draws a margin of rows beyond each edge, so the rows on either side of any such point are drawn,
/// and "before this one" or "after that one" names the same place either way.
pub(super) fn drop_target(
    rows: &[(&str, Bounds<Pixels>)],
    source: &str,
    clip: Bounds<Pixels>,
    at: Point<Pixels>,
) -> Option<Target> {
    drop_in_band(rows, Some(source), clip, at)
}

/// Resolve the insertion line in one ordered band. The source's index is
/// only meaningful when the source occupies a slot in this band.
fn drop_in_band(
    rows: &[(&str, Bounds<Pixels>)],
    source: Option<&str>,
    clip: Bounds<Pixels>,
    at: Point<Pixels>,
) -> Option<Target> {
    let (first, last) = (rows.first()?, rows.last()?);
    let top = first.1.top().max(clip.top());
    let bottom = last.1.bottom().min(clip.bottom());
    if at.x < clip.left() || at.x >= clip.right() || at.y < top || at.y >= bottom {
        return None;
    }
    let source_ix = match source {
        Some(source) => Some(rows.iter().position(|(key, _)| *key == source)?),
        None => None,
    };
    let others = || {
        rows.iter()
            .enumerate()
            .filter(|(ix, _)| Some(*ix) != source_ix)
    };
    let (ix, placement) = match others().find(|(_, (_, bounds))| at.y < bounds.center().y) {
        Some((ix, _)) => (ix, Placement::Before),
        None => (others().next_back()?.0, Placement::After),
    };
    let stays = source_ix.is_some_and(|source| match placement {
        Placement::Before => ix == source + 1,
        Placement::After => ix + 1 == source,
    });
    (!stays).then(|| Target {
        neighbor: rows[ix].0.to_owned(),
        placement,
    })
}

// ---- The row's part ----------------------------------------------------------------------------------------------------

/// What a row draws for the pin drag ([`paint`]): its grip, whether it is lifted, the drop line when it is the target,
/// and what records its bounds for the drag to judge a drop by.
pub(super) struct RowPaint {
    /// The grip, over the row's leading gutter; `None` for a row that cannot be dragged now.
    pub(super) grip: Option<AnyElement>,
    /// Whether the row is the one being dragged, lifted.
    pub(super) lifted: bool,
    /// The drop line, inside the row's top or bottom edge, when a drop would land beside this row.
    pub(super) drop_line: Option<AnyElement>,
    /// Records the row's bounds as it is laid out, for a movable row.
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
    if !state.movable.iter().any(|movable| movable == key) {
        return RowPaint {
            grip: None,
            lifted: false,
            drop_line: None,
            recorder: None,
        };
    }
    let palette = *theme::palette(cx);
    let lifted = state.lifted(key);
    let drop_on = state.drop_on(key);
    drop(state);
    let can_drag = !menu::refused(key, cx)
        && !retired_row(key, cx)
        && !rows.borrow_mut().asks(key, window, cx);
    let grip = can_drag.then(|| {
        grip(
            key,
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
    let recorder = {
        let (shared, key) = (shared.clone(), key.to_owned());
        canvas(
            move |bounds, _, _| {
                shared.borrow_mut().bounds.insert(key, bounds);
            },
            |_, (), _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element()
    };
    RowPaint {
        grip,
        lifted,
        drop_line,
        recorder: Some(recorder),
    }
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
fn grip(
    key: &str,
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
    let help = HelpPlace::Row { selected }.place(HoverHelp::tooltip(GRIP_HELP));
    let selector = id.clone();
    let grip = div()
        .id(id.clone())
        .role(Role::Button)
        .aria_label(GRIP_LABEL)
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
    /// `movable` is this frame's movable pins in order, empty when no row may be dragged, and `window_active` whether
    /// the window is active now. A drag whose row is no longer movable, whose session has a request in flight, or whose
    /// window went inactive since its press ends here with nothing sent, so the frame draws it gone. GPUI redraws the
    /// window when its activation changes, so this frame comes.
    pub(super) fn pin_drag_frame(
        &mut self,
        movable: Vec<String>,
        window_active: bool,
        cx: &mut Context<Self>,
    ) -> Shared {
        let shared = self.pin_drag(cx);
        let gone = {
            let mut state = shared.borrow_mut();
            state.start_frame(movable);
            state.drag.as_ref().is_some_and(|drag| {
                !state.movable.contains(&drag.source)
                    || menu::refused(&drag.source, cx)
                    || (drag.began_active && !window_active)
            })
        };
        if gone {
            shared.borrow_mut().drag = None;
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
        {
            let state = shared.borrow();
            if state.drag.is_some()
                || !state.movable.iter().any(|movable| movable == key)
                || menu::refused(key, cx)
                || retired_row(key, cx)
            {
                return false;
            }
        }
        shared.borrow_mut().drag = Some(Drag {
            source: key.to_owned(),
            origin: at,
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
    }

    /// The primary button came up at `at` during a drag: the drag ends, and a drop on a drop line sends its one move.
    pub(super) fn pin_drag_released(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        // A resample that ends the drag (its row no longer drawn, a request in flight) leaves nothing to take below,
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
            pins::send_move(&drag.source, &target.neighbor, target.placement, cx);
        }
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
    /// paint and before the drop: the drag ends when its row is no longer movable or drawn, or its session has a
    /// request in flight; otherwise it lifts once the pointer is [`LIFT_DISTANCE`] from the press, and once lifted its
    /// target is where a drop at `at` lands ([`drop_target`]). Returns whether what the rows show changed.
    fn resample_pin_drag(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let clip = self.rows.viewport_bounds();
        let shared = self.pin_drag(cx);
        let mut state = shared.borrow_mut();
        let Some(drag) = state.drag.as_ref() else {
            return false;
        };
        let source = drag.source.clone();
        if !state.movable.contains(&source)
            || !state.bounds.contains_key(&source)
            || menu::refused(&source, cx)
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
            drop_target(&state.drawn_rows(), &source, clip, at)
        } else {
            None
        };
        let Some(drag) = state.drag.as_mut() else {
            return false;
        };
        let changed = (drag.lifted, &drag.target) != (lifted, &target);
        drag.lifted = lifted;
        drag.target = target;
        changed
    }
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
