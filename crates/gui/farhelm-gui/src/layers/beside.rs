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
//! With no edge drawn, the help shows at the pointer through the ordinary tooltip's own placement ([`super::overlay`]
//! decides). The place is decided while each frame is laid out, after every tracker has recorded this frame's bounds,
//! so help follows a control that moves (a re-sorted row) in the same frame, cues included.
//!
//! A control's bounds come from the tracker its hover help adds ([`super::BesideTarget`]), which covers the box inside
//! the control's border and records the clip in force there. The ring is drawn in that box, as the old inset-shadow
//! ring sat inside the border. The old placement measured the border box instead, so for a control with a border the
//! fallback's gaps above and below are measured from inside it (a pixel further for a 1-pixel border); centering is the
//! same either way.
//!
//! The old app placed help beside the sidebar only for a control inside the sidebar (`closest(".app-sidebar")`,
//! tooltip.js lines 419-421 and 468-472); anything else got the ordinary placement. Here any control whose help asks
//! for it is placed against the window's one edge, wherever it is drawn, so only controls inside the sidebar ask.
//!
//! NOTE: The old hover closed on any scroll of a container holding its control. Here it stays and follows the control
//! while the pointer stays on it; a scroll that brings another control under the pointer hands the help to that one, as
//! any hover change does. Closing on the control's own scroll container would need the layer manager to know which
//! scrolling element holds a control, which nothing records yet. And a showing help keeps the cues it began with: a row
//! selected elsewhere (another client) while its help shows keeps an accent ring until the next hover, where the old
//! stylesheet recolored it at once.

use std::cell::Cell;
use std::f32::consts::SQRT_2;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AvailableSpace, BorderStyle, Bounds, ContentMask, Corners, Element, ElementId,
    GlobalElementId, Hsla, InspectorElementId, InteractiveElement as _, IntoElement, LayoutId,
    ParentElement as _, PathBuilder, Pixels, Point, Size, Style, Styled as _, Window, canvas, div,
    point, px,
};

use super::tooltip::{Beside, Drawn, DrawnRing, ShownTooltip, TooltipSide};
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
/// this column. While no edge is drawn, such help shows at the pointer, as ordinary help. Like an anchor, the edge is
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

/// Place a card of `size` in a window of `viewport` beside the edge whose right side is at `edge_right`, centered on
/// `target`, or against `target` when it does not fit there.
pub(super) fn place(
    edge_right: Pixels,
    target: Bounds<Pixels>,
    size: Size<Pixels>,
    viewport: Size<Pixels>,
) -> Placed {
    beside_sidebar(edge_right, target, size, viewport)
        .unwrap_or_else(|| around_target(target, size, viewport))
}

/// `placeSessionCard`: beside the edge whose right side is at `edge_right`, or `None` when the card would come within
/// [`EDGE`] of the window's right edge there.
fn beside_sidebar(
    edge_right: Pixels,
    target: Bounds<Pixels>,
    size: Size<Pixels>,
    viewport: Size<Pixels>,
) -> Option<Placed> {
    let left = (edge_right + GAP).max(EDGE);
    if left + size.width > viewport.width - EDGE {
        return None;
    }
    let center = target.center().y;
    let top = (center - size.height / 2.)
        .min(viewport.height - EDGE - size.height)
        .max(EDGE);
    let inset = NOTCH_INSET.min(size.height / 2.);
    let notch_y = center.min(top + size.height - inset).max(top + inset);
    Some(Placed {
        origin: point(left, top),
        side: TooltipSide::Beside,
        notch_y: Some(notch_y),
    })
}

/// `placeTooltip`: the old ordinary tooltip's place against `target`, which beside placement falls back to.
fn around_target(target: Bounds<Pixels>, size: Size<Pixels>, viewport: Size<Pixels>) -> Placed {
    let left = (target.center().x - size.width / 2.)
        .min(viewport.width - EDGE - size.width)
        .max(EDGE);
    let above = target.top() - GAP - size.height;
    let below = target.bottom() + GAP_BELOW;
    let (side, top) = if above >= EDGE {
        (TooltipSide::Above, above)
    } else if below + size.height <= viewport.height - EDGE {
        (TooltipSide::Below, below)
    } else {
        // Fits on neither side: the roomier one, clamped inside the window even if that covers the control.
        let (side, preferred) = if target.top() >= viewport.height - target.bottom() {
            (TooltipSide::Above, above)
        } else {
            (TooltipSide::Below, below)
        };
        let top = preferred
            .min(viewport.height - EDGE - size.height)
            .max(EDGE);
        (side, top)
    };
    Placed {
        origin: point(left, top),
        side,
        notch_y: None,
    }
}

/// The tooltip's card for help beside the sidebar, placed by [`place`] as the frame is laid out (for the reason
/// `PlacedPopover` is: trackers record this frame's bounds before overlays are prepared), with the notch under it and
/// the ring around its control while it sits beside the sidebar. It writes the side it chose, and the ring it paints,
/// to the shown tooltip ([`ShownTooltip::side`], [`ShownTooltip::ring`]). Nothing in it asks for the pointer, so a
/// press goes through it as through any tooltip. The overlay makes one only while the sidebar's edge is drawn.
pub(super) struct PlacedBeside {
    target: Rc<Cell<Option<Drawn>>>,
    edge: Anchor,
    side: Rc<Cell<TooltipSide>>,
    /// The shown tooltip's record of the painted ring, cleared at each prepaint so a frame that paints none (the
    /// fallback, once a resting hover's window narrows) does not keep reporting the last one.
    painted: Rc<Cell<Option<DrawnRing>>>,
    card: AnyElement,
    notch: AnyElement,
    ring: AnyElement,
}

impl PlacedBeside {
    /// The card of `shown`, whose control asked for `beside`, to be placed against `edge`.
    pub(super) fn new(
        shown: &ShownTooltip,
        beside: &Beside,
        edge: Anchor,
        card: gpui::Div,
        palette: &theme::Palette,
    ) -> Self {
        let notch = notch(theme::color(palette.bg_2), theme::color(palette.border));
        let color = theme::color(if beside.cues.on_fill {
            palette.on_fill
        } else {
            palette.accent
        });
        let ring = ring(color, beside.cues.radius, shown.ring.clone());
        PlacedBeside {
            target: beside.target.clone(),
            edge,
            side: shown.side.clone(),
            painted: shown.ring.clone(),
            card: card.into_any_element(),
            notch,
            ring,
        }
    }
}

/// What a frame of [`PlacedBeside`] paints, as its prepaint decided.
pub(super) enum Painting {
    /// Nothing: the control has no bounds yet (help shown at once before its control's first frame, which only a
    /// golden's `show_tooltip_at` does), so there is nowhere to put the help. The overlay builds this element only
    /// once the edge has bounds.
    Nothing,
    /// The card alone: the fallback placement.
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
        let (Some(edge), Some(target)) = (self.edge.drawn_bounds(), self.target.get()) else {
            return Painting::Nothing;
        };
        let size = window.layout_bounds(*card).size;
        let placed = place(edge.right(), target.bounds, size, window.viewport_size());
        self.side.set(placed.side);
        let offset = placed.origin - bounds.origin;
        let offset = point(offset.x.round(), offset.y.round());
        window.with_element_offset(offset, |window| self.card.prepaint(window, cx));
        let Some(notch_y) = placed.notch_y else {
            return Painting::Card;
        };
        // The turned square's bounding box, centered on the card's left edge at the notch's height.
        let across = px(NOTCH_SIDE * SQRT_2);
        let left = bounds.origin.x + offset.x;
        let square = gpui::size(across, across).map(AvailableSpace::Definite);
        self.notch.layout_as_root(square, window, cx);
        let corner = point(left - across / 2., notch_y.round() - across / 2.);
        self.notch.prepaint_at(corner, window, cx);
        let size = target.bounds.size.map(AvailableSpace::Definite);
        self.ring.layout_as_root(size, window, cx);
        let clip = Some(ContentMask {
            bounds: target.clip,
        });
        window.with_content_mask(clip, |window| {
            self.ring.prepaint_at(target.bounds.origin, window, cx)
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
                // The notch first, so the card covers its right half as the old box (z-index 50) covered the notch
                // (49).
                self.notch.paint(window, cx);
                self.card.paint(window, cx);
                let clip = Some(ContentMask { bounds: clip });
                window.with_content_mask(clip, |window| self.ring.paint(window, cx));
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
