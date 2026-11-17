//! Hover help beside the sidebar: where a session row's help and the host progress card sit, and the two cues that
//! tie the help to its control while it sits there (SPEC.md, "Session list": "just right of the sidebar, vertically
//! centred on its target and shifted only to fit the window", with a left-edge notch and a thin accent ring, and
//! "ordinary above/below placement without a notch or ring" when there is no room to the right).
//!
//! A control asks for it through its hover help ([`crate::components::HoverHelp::beside`]), which makes the control
//! draw a tracker of its own bounds; the sidebar draws the edge the help sits against ([`beside_edge`]). Every number
//! here is the old app's, from `placeSessionCard` and `placeTooltip` in `crates/farhelm-ui/assets/tooltip.js` and the
//! notch and ring rules in `app.css`, at reference commit `24bc0580` (cited by line below):
//!
//! - **Beside.** The help's left edge is 6 pixels past the edge's right side (`max(4, right + 6)`, tooltip.js line
//!   172), whatever control asked; its top centers it on the control and keeps it 4 pixels inside the window's top and
//!   bottom (lines 174-175). It never shifts sideways.
//! - **Fallback.** Only when the help, measured at its own maximum width, would come within 4 pixels of the window's
//!   right edge (line 173). It then sits as the old ordinary tooltip did against the control (lines 143-161): centered
//!   across it and kept 4 pixels inside the window (pinned left when wider than it), above with a 6-pixel gap where
//!   that fits 4 pixels from the top, else below with a 28-pixel gap where that fits, else on the side with more
//!   room, clamped inside. No notch and no ring.
//! - **Notch.** The visible half of the old 10 by 10 square turned 45 degrees (`.farhelm-tooltip-pointer`, app.css
//!   lines 1884-1898): a triangle in the card's fill with the border color on its two outer edges, under the card, the
//!   square's center on the card's left edge level with the control's center, but never nearer the card's top or
//!   bottom than 12 pixels (half the card's height, for a short one), so it stays on the straight edge (lines 176-177).
//! - **Ring.** A 1-pixel line just inside the control's bounds in the accent (on-fill in a selected row), with the
//!   control's own corners (`.tooltip-target-ring`, app.css lines 1899-1917), drawn with the tooltip but clipped where
//!   the control is clipped: app.css line 1900 drew it inset so that the sidebar's clipping kept it inside the list,
//!   so a row half scrolled out of the list has its ring only on the half in view.
//!
//! The fallback's rule is also the old app's ordinary placement, and every other help is placed by it alone, with no
//! notch or ring: ordinary help against its control ([`PlacedBeside::ordinary`]), beside help while no edge is drawn,
//! and help keyboard focus owns against the focused control ([`PlacedBeside::around`]); [`super::overlay`] decides
//! which. Help whose control has no recorded bounds, ordinary or beside, shows at the pointer instead ([`at_pointer`]):
//! a probe a test or golden shows, or a control in a cached view (`cache.rs`, NOTE). The place is decided while each
//! frame is laid out, after every tracker has recorded this frame's bounds, so help follows a control that
//! moves (a re-sorted row) in the same frame, cues included.
//!
//! The old app's window was its page, so "the window" in these rules is the window's content area
//! ([`crate::desktop::linux_title_bar::content_area`]).
//!
//! A control's bounds come from the tracker its hover help adds while the help waits or shows
//! ([`super::TooltipOwner::track`]), which covers the box inside the control's border and records the clip in force
//! there. The ring is drawn in that box, as the old inset-shadow
//! ring sat inside the border. The old placement measured the border box instead, so for a control with a border the
//! ordinary rule's gaps above and below are measured from inside it (a pixel further for a 1-pixel border); centering
//! is the same either way.
//!
//! The old app placed help beside the sidebar only for a control inside the sidebar (`closest(".app-sidebar")`,
//! tooltip.js lines 419-421 and 468-472); anything else got the ordinary placement. Here any control whose help asks
//! for it is placed against the window's one edge, wherever it is drawn, so only controls inside the sidebar ask.
//!
//! NOTE: The old hover closed on any scroll of a container holding its control. Here it stays and follows the control
//! while the pointer stays on it; a scroll that brings another control under the pointer hands the help to that one, as
//! any hover change does. Closing on the control's own scroll container would need the layer manager to know which
//! scrolling element holds a control, which nothing records yet. And a showing help keeps the cues it began with: a row
//! that becomes selected some other way (an automatic pick, say) while its help shows keeps an accent ring until the
//! next hover, where the old stylesheet recolored it at once.

use std::cell::Cell;
use std::f32::consts::SQRT_2;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AvailableSpace, BorderStyle, Bounds, ContentMask, Corners, Element, ElementId,
    FocusHandle, GlobalElementId, Hsla, InspectorElementId, InteractiveElement as _, IntoElement,
    LayoutId, ParentElement as _, PathBuilder, Pixels, Point, Size, Style, Styled as _, Window,
    canvas, div, point, px,
};

use super::tooltip::{
    BesideCues, DrawnRing, HelpTarget, POINTER_OFFSET, ShownTooltip, TooltipSide,
};
use super::{Anchor, stack};
use crate::theme;

/// The nearest help comes to a window edge (`EDGE`, tooltip.js line 125).
const EDGE: Pixels = px(4.);

/// The gap between the sidebar's edge and help beside it, and between a control and help above it (`GAP_ABOVE`,
/// tooltip.js line 121: the old code used one constant for both).
const GAP: Pixels = px(6.);

/// The gap between a control and help that falls back below it, which clears the pointer's body (`GAP_BELOW`,
/// tooltip.js line 123).
const GAP_BELOW: Pixels = px(28.);

/// The nearest the notch comes to the card's top or bottom, past its rounded corner (tooltip.js line 176).
const NOTCH_INSET: Pixels = px(12.);

/// The side of the old notch's square, before it was turned 45 degrees.
const NOTCH_SIDE: f32 = 10.;

/// Draw the edge that help placed beside the sidebar sits against: a zero-cost child of the sidebar's outer column (its
/// whole width, the host rail included), which records where the column is drawn in every frame, as [`Anchor::track`]
/// does. The column must be positioned (`relative()`) so the tracker covers it. Draw it once per window, from the
/// sidebar's render.
///
/// A tracker covers the box inside its element's border, while the old app measured the 6 pixels from outside the
/// sidebar's 1-pixel `border-right`. So draw it in an element without a border of its own whose right edge is the
/// border's outer edge: a wrapper around the bordered column, or the column itself when a child draws the border.
///
/// ```ignore
/// div().relative().flex() // the sidebar's outer box: no border of its own
///     .child(column)      // the rail and the list, with the 1-pixel border on the right
///     .child(layers::beside_edge(window, cx))
/// ```
///
/// The help uses the column's right edge, so every control whose help asks for beside placement must be drawn inside
/// this column. While no edge is drawn, such help is placed as ordinary help is. Like an anchor, the edge is
/// still known in the one frame that stops drawing it. A second edge drawn in the same window replaces the first.
pub fn beside_edge(window: &mut Window, cx: &mut App) -> impl IntoElement {
    let edge = Anchor::for_element("farhelm-beside-edge", window, cx);
    if let Some(stack) = stack(window, cx) {
        // No notify: the overlay reads the edge as each frame is drawn, and writing it changes nothing on screen.
        stack.update(cx, |stack, _| stack.beside_edge = Some(edge.clone()));
    }
    edge.track()
}

/// Where a tooltip goes: its card's origin, which side that is, and for beside placement the height of the notch's
/// center. All in window coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Placed {
    pub(super) origin: Point<Pixels>,
    pub(super) side: TooltipSide,
    pub(super) notch_y: Option<Pixels>,
}

/// Place a card of `size` beside the edge whose right side is at `edge_right`, centered on `target`, or against
/// `target` when it does not fit there, inside `area`, the window's content area: every "window edge" in this module's
/// numbers is an edge of `area`.
pub(super) fn place(
    edge_right: Pixels,
    target: Bounds<Pixels>,
    size: Size<Pixels>,
    area: Bounds<Pixels>,
) -> Placed {
    beside_sidebar(edge_right, target, size, area)
        .unwrap_or_else(|| around_target(target, size, area))
}

/// `placeSessionCard`: beside the edge whose right side is at `edge_right`, or `None` when the card would come within
/// [`EDGE`] of `area`'s right edge there.
fn beside_sidebar(
    edge_right: Pixels,
    target: Bounds<Pixels>,
    size: Size<Pixels>,
    area: Bounds<Pixels>,
) -> Option<Placed> {
    let left = (edge_right + GAP).max(area.left() + EDGE);
    if left + size.width > area.right() - EDGE {
        return None;
    }
    let center = target.center().y;
    let top = (center - size.height / 2.)
        .min(area.bottom() - EDGE - size.height)
        .max(area.top() + EDGE);
    let inset = NOTCH_INSET.min(size.height / 2.);
    let notch_y = center.min(top + size.height - inset).max(top + inset);
    Some(Placed {
        origin: point(left, top),
        side: TooltipSide::Beside,
        notch_y: Some(notch_y),
    })
}

/// `placeTooltip`: the old ordinary tooltip's place against `target` inside `area`: ordinary help's, focus help's, and
/// the one beside placement falls back to.
fn around_target(target: Bounds<Pixels>, size: Size<Pixels>, area: Bounds<Pixels>) -> Placed {
    let left = (target.center().x - size.width / 2.)
        .min(area.right() - EDGE - size.width)
        .max(area.left() + EDGE);
    let above = target.top() - GAP - size.height;
    let below = target.bottom() + GAP_BELOW;
    let (side, top) = if above >= area.top() + EDGE {
        (TooltipSide::Above, above)
    } else if below + size.height <= area.bottom() - EDGE {
        (TooltipSide::Below, below)
    } else {
        // Fits on neither side: the roomier one, clamped inside the area even if that covers the control.
        let (side, preferred) = if target.top() - area.top() >= area.bottom() - target.bottom() {
            (TooltipSide::Above, above)
        } else {
            (TooltipSide::Below, below)
        };
        let top = preferred
            .min(area.bottom() - EDGE - size.height)
            .max(area.top() + EDGE);
        (side, top)
    };
    Placed {
        origin: point(left, top),
        side,
        notch_y: None,
    }
}

/// The place of help with no drawn control to sit against: offset from the pointer at `at` like a native tooltip, so
/// the pointer never covers the text, and shifted back inside `area` where it would run past it, flush with the edge
/// it overflows and pinned to the left or top when it is wider or taller than the area, as GPUI's `snap_to_window`
/// kept it inside the whole window before.
pub(super) fn at_pointer(at: Point<Pixels>, size: Size<Pixels>, area: Bounds<Pixels>) -> Placed {
    let origin = at + POINTER_OFFSET;
    Placed {
        origin: point(
            origin.x.min(area.right() - size.width).max(area.left()),
            origin.y.min(area.bottom() - size.height).max(area.top()),
        ),
        side: TooltipSide::Pointer,
        notch_y: None,
    }
}

/// The tooltip's card, placed as the frame is laid out (for the reason `PlacedPopover` is: trackers record this
/// frame's bounds before overlays are prepared): for help beside the sidebar by [`place`], with the notch under it and
/// the ring around its control while it sits beside the sidebar; for help keyboard focus owns away from the sidebar,
/// against the focused control by the old ordinary rule ([`PlacedBeside::around`]); and for every other help, against
/// its control by the same rule ([`PlacedBeside::ordinary`]); each inside the window's content area. Help whose control
/// has no recorded bounds goes at the pointer instead, ordinary or beside: a probe a test or golden shows, or a control
/// in a cached view ([`at_pointer`]). It writes the side it chose, and the ring it paints, to the shown tooltip
/// ([`ShownTooltip::side`], [`ShownTooltip::ring`]). Nothing in it asks for the pointer, so a press goes through it as
/// through any tooltip.
pub(super) struct PlacedBeside {
    against: Against,
    side: Rc<Cell<TooltipSide>>,
    /// The shown tooltip's record of the painted ring, cleared at each prepaint so a frame that paints none (the
    /// fallback, once a resting hover's window narrows) does not keep reporting the last one.
    painted: Rc<Cell<Option<DrawnRing>>>,
    card: AnyElement,
}

/// What a [`PlacedBeside`] places its card against.
enum Against {
    /// The sidebar's edge, centered on the control its tracker records (with the clip around it), with the notch and
    /// the ring while it fits there.
    Beside {
        target: HelpTarget,
        edge: Anchor,
        notch: AnyElement,
        ring: AnyElement,
        /// Where the pointer was when the help began, for a control with no recorded bounds ([`at_pointer`]).
        at: Point<Pixels>,
    },
    /// The control holding focus, as the focus coordinator records its stop, for help keyboard focus owns.
    Focused(FocusHandle),
    /// Ordinary help's control, as its tracker records it, or, while the tracker has never been drawn, the pointer
    /// where the help began, in window coordinates ([`at_pointer`]).
    Ordinary {
        control: HelpTarget,
        at: Point<Pixels>,
    },
}

impl PlacedBeside {
    /// The card of `shown`, whose control asked for beside help with `cues`, to be placed against `edge`.
    pub(super) fn new(
        shown: &ShownTooltip,
        cues: BesideCues,
        edge: Anchor,
        card: gpui::Div,
        palette: &theme::Palette,
    ) -> Self {
        let notch = notch(theme::color(palette.bg_2), theme::color(palette.border));
        let color = theme::color(if cues.on_fill {
            palette.on_fill
        } else {
            palette.accent
        });
        let ring = ring(color, cues.radius, shown.ring.clone());
        PlacedBeside {
            against: Against::Beside {
                target: shown.target.clone(),
                at: shown.at,
                edge,
                notch,
                ring,
            },
            side: shown.side.clone(),
            painted: shown.ring.clone(),
            card: card.into_any_element(),
        }
    }

    /// The card of `shown`, help keyboard focus owns, placed against the control whose focus stop is `focus` by the
    /// old ordinary rule ([`around_target`]: centered above it, else below), as the old app placed help shown on
    /// focus. Placed as each frame is laid out, it follows the control through a scroll or a reflow.
    pub(super) fn around(shown: &ShownTooltip, focus: FocusHandle, card: gpui::Div) -> Self {
        PlacedBeside {
            against: Against::Focused(focus),
            side: shown.side.clone(),
            painted: shown.ring.clone(),
            card: card.into_any_element(),
        }
    }

    /// The card of `shown`, ordinary help, placed against its control by the old ordinary rule ([`around_target`]), as
    /// the old app placed it; or, for help whose control has no recorded bounds (a test's or golden's probe, or a
    /// control in a cached view), at the pointer where it began ([`ShownTooltip::at`], [`at_pointer`]). Help that asks
    /// for beside placement comes here too while no sidebar edge is drawn.
    pub(super) fn ordinary(shown: &ShownTooltip, card: gpui::Div) -> Self {
        PlacedBeside {
            against: Against::Ordinary {
                control: shown.target.clone(),
                at: shown.at,
            },
            side: shown.side.clone(),
            painted: shown.ring.clone(),
            card: card.into_any_element(),
        }
    }
}

/// What a frame of [`PlacedBeside`] paints, as its prepaint decided.
pub(super) enum Painting {
    /// Nothing: the sidebar's edge is no longer drawn, or the focus coordinator has no bounds for the focused stop,
    /// so there is nowhere to put the help.
    Nothing,
    /// The card alone: the fallback placement, ordinary help, or help keyboard focus owns.
    Card,
    /// The card with its notch and its ring, the ring inside `clip`, the clip in force where the control is drawn.
    Cues { clip: Bounds<Pixels> },
}

impl IntoElement for PlacedBeside {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for PlacedBeside {
    /// The card's layout, laid out with the window so its width is its own up to its cap, as the old box was measured.
    type RequestLayoutState = LayoutId;
    type PrepaintState = Painting;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let style = Style {
            position: gpui::Position::Absolute,
            ..Style::default()
        };
        let card = self.card.request_layout(window, cx);
        (window.request_layout(style, [card], cx), card)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        card: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) -> Painting {
        self.painted.set(None);
        let size = window.layout_bounds(*card).size;
        let area = crate::desktop::linux_title_bar::content_area(window);
        // The place, and for beside help the control as its tracker drew it, which the cues need.
        let (placed, tracked) = match &self.against {
            Against::Beside {
                target, edge, at, ..
            } => {
                let Some(edge) = edge.drawn_bounds() else {
                    return Painting::Nothing;
                };
                // A control that did not draw its tracker when its help showed (in a cached view, `cache.rs`) has
                // no bounds: its card goes at the pointer, misplaced and without cues rather than invisible.
                match target.drawn() {
                    Some(target) => (place(edge.right(), target.bounds, size, area), Some(target)),
                    None => (at_pointer(*at, size, area), None),
                }
            }
            Against::Focused(focus) => {
                let Some(target) = crate::focus::control_bounds(focus, window, cx) else {
                    return Painting::Nothing;
                };
                (around_target(target, size, area), None)
            }
            Against::Ordinary { control, at } => match control.drawn() {
                Some(control) => (around_target(control.bounds, size, area), None),
                None => (at_pointer(*at, size, area), None),
            },
        };
        self.side.set(placed.side);
        let offset = placed.origin - bounds.origin;
        let offset = point(offset.x.round(), offset.y.round());
        window.with_element_offset(offset, |window| self.card.prepaint(window, cx));
        let (Some(notch_y), Some(target), Against::Beside { notch, ring, .. }) =
            (placed.notch_y, tracked, &mut self.against)
        else {
            return Painting::Card;
        };
        // The turned square's bounding box, centered on the card's left edge at the notch's height.
        let across = px(NOTCH_SIDE * SQRT_2);
        let left = bounds.origin.x + offset.x;
        let square = gpui::size(across, across).map(AvailableSpace::Definite);
        notch.layout_as_root(square, window, cx);
        let corner = point(left - across / 2., notch_y.round() - across / 2.);
        notch.prepaint_at(corner, window, cx);
        let size = target.bounds.size.map(AvailableSpace::Definite);
        ring.layout_as_root(size, window, cx);
        let clip = Some(ContentMask {
            bounds: target.clip,
        });
        window.with_content_mask(clip, |window| {
            ring.prepaint_at(target.bounds.origin, window, cx)
        });
        Painting::Cues { clip: target.clip }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        painting: &mut Painting,
        window: &mut Window,
        cx: &mut App,
    ) {
        match *painting {
            Painting::Nothing => {}
            Painting::Card => self.card.paint(window, cx),
            Painting::Cues { clip } => {
                let Against::Beside { notch, ring, .. } = &mut self.against else {
                    return;
                };
                // The notch first, so the card covers its right half as the old box (z-index 50) covered the notch
                // (49).
                notch.paint(window, cx);
                self.card.paint(window, cx);
                let clip = Some(ContentMask { bounds: clip });
                window.with_content_mask(clip, |window| ring.paint(window, cx));
            }
        }
    }
}

/// The ring: a 1-pixel `color` line just inside the box it is laid out in, with corners of `radius`, painted as one
/// quad whose color, corners and clip it records in `painted` as it paints, so what [`ShownTooltip::ring`] reports is
/// what the user sees.
fn ring(color: Hsla, radius: Pixels, painted: Rc<Cell<Option<DrawnRing>>>) -> AnyElement {
    let paint = move |bounds: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
        let quad =
            gpui::outline(bounds, color, BorderStyle::Solid).corner_radii(Corners::all(radius));
        painted.set(Some(DrawnRing {
            bounds: quad.bounds,
            clip: window.content_mask().bounds,
            color: quad.border_color,
            radius: quad.corner_radii.top_left,
        }));
        window.paint_quad(quad);
    };
    div()
        .debug_selector(|| "tooltip-ring".into())
        .size_full()
        .child(canvas(|_, _, _| {}, paint).size_full())
        .into_any_element()
}

/// The notch: GPUI cannot turn a box, so the visible half of the turned square is drawn as two triangles in a box the
/// size of the turned square, the outline's color and then the fill 1 pixel inside the two outer edges, where the old
/// square's `border-left` and `border-bottom` ended up after the turn. The half under the card is never drawn.
fn notch(fill: Hsla, line: Hsla) -> AnyElement {
    let paint = move |bounds: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
        let center = bounds.center();
        let reach = bounds.size.width / 2.;
        // A 1-pixel band along each 45-degree edge meets the base √2 pixels further in.
        for (color, reach) in [(line, reach), (fill, reach - px(SQRT_2))] {
            let mut path = PathBuilder::fill();
            path.move_to(point(center.x - reach, center.y));
            path.line_to(point(center.x, center.y - reach));
            path.line_to(point(center.x, center.y + reach));
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        }
    };
    div()
        .debug_selector(|| "tooltip-notch".into())
        .size_full()
        .child(canvas(|_, _, _| {}, paint).size_full())
        .into_any_element()
}
