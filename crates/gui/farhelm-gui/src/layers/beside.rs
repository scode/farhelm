//! Hover help stays outside a menu or sidebar whenever the window leaves room. The parent layer manager passes the
//! open popovers' surfaces and the edges drawn in the page. Prepaint decides placement after both surfaces and
//! controls record their bounds, so a tooltip follows a moving menu or row in the same frame.
//!
//! The dimensions come from `placeSessionCard` and `placeTooltip` in `crates/farhelm-ui/assets/tooltip.js` at reference
//! commit `24bc0580`, along with the notch and ring rules in `app.css`.
//!
//! - **Beside.** The card sits 6 pixels beyond the surface, vertically centered on the control and clamped 4 pixels
//!   inside the content area. A menu can flip left and give way to an open submenu. The sidebar uses the right edge.
//! - **Fallback.** A card that can't fit outside the surface at its measured width uses ordinary placement. Ordinary
//!   help sits 6 pixels above the control, then tries 28 pixels below, then uses the roomier side and clamps to fit.
//! - **Notch.** The visible half of the old 10 by 10 square turned 45 degrees (`.farhelm-tooltip-pointer`, app.css
//!   lines 1884-1898): a triangle in the card's fill with the border color on its two outer edges, under the card, the
//!   square's center on the near edge level with the control's center, but never nearer the card's top or
//!   bottom than 12 pixels (half the card's height, for a short one), so it stays on the straight edge (lines 176-177).
//! - **Ring.** A 1-pixel line just inside the control's bounds in the accent (on-fill in a selected row), with the
//!   control's own corners (`.tooltip-target-ring`, app.css lines 1899-1917), drawn with the tooltip but clipped where
//!   the control is clipped: app.css line 1900 drew it inset so that the sidebar's clipping kept it inside the list,
//!   so a row half scrolled out of the list has its ring only on the half in view.
//!
//! Ordinary placement draws the card alone. Help without tracked control bounds goes at the pointer
//! ([`at_pointer`]), which the test probes use. Help shown by keyboard focus can use the focus coordinator's
//! recorded stop bounds for ordinary placement.
//!
//! The old app's window was its page, so "the window" in these rules is the window's content area
//! ([`crate::desktop::linux_title_bar::content_area`]).
//!
//! A control's bounds come from the tracker its hover help adds while the help waits or shows
//! ([`super::TooltipOwner::track`]), which covers the box inside the control's border and records the clip in force
//! there. The ring sits inside the border, and the clip keeps it inside the visible part of a scrolling control.
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
    Drawn, DrawnRing, HelpPlacement, HelpTarget, POINTER_OFFSET, ShownTooltip, TooltipSide,
};
use super::{Anchor, AnchorBounds, stack};
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

/// Track the right edge of an in-tree container. Its positioned wrapper must cover the outer border, because
/// the 6-pixel gap starts outside it. Supply a stable key, distinct from the wrapper's own element id.
pub fn beside_edge(
    key: impl Into<ElementId>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let edge = Anchor::for_element(key, window, cx);
    if let Some(stack) = stack(window, cx) {
        // The overlay reads the bounds during prepaint, so registration needs no extra frame.
        stack.update(cx, |stack, _| {
            stack
                .help_edges
                .retain(|other| other.state.upgrade().is_some());
            if !stack
                .help_edges
                .iter()
                .any(|other| other.state == edge.state)
            {
                stack.help_edges.push(edge.clone());
            }
        });
    }
    edge.track()
}

/// Help edges for one frame. Popover surfaces are already tracked for outside presses. Their order is the order
/// they are drawn in, so the visible top panel owns help where surfaces overlap.
pub(super) struct HelpEdges {
    pub(super) inline: Vec<Anchor>,
    pub(super) panels: Vec<AnchorBounds>,
}

impl HelpEdges {
    /// Select a surface using the visible center. A sidebar row belongs to its in-tree column even when
    /// a popover covers part of that row. Menu controls look only among popover surfaces.
    fn place(
        &self,
        placement: HelpPlacement,
        target: Drawn,
        size: Size<Pixels>,
        area: Bounds<Pixels>,
    ) -> Option<Placed> {
        let ordinary = || around_target(target.bounds, size, area, false);
        let visible = target.bounds.intersect(&target.clip);
        if visible.size.width <= px(0.) || visible.size.height <= px(0.) {
            return Some(ordinary());
        }
        let center = visible.center();
        match placement {
            HelpPlacement::Menu => {
                let panel = self
                    .panels
                    .iter()
                    .enumerate()
                    .rev()
                    .find_map(|(index, panel)| {
                        panel
                            .get()
                            .filter(|bounds| bounds.contains(&center))
                            .map(|bounds| (index, bounds))
                    });
                let Some((index, panel)) = panel else {
                    return Some(ordinary());
                };
                let lower = self.panels[..index]
                    .iter()
                    .filter_map(|surface| surface.get());
                let higher = self.panels[index + 1..]
                    .iter()
                    .filter_map(|surface| surface.get());
                place_menu(panel, target.bounds, size, area, lower, higher)
            }
            HelpPlacement::Beside(_) | HelpPlacement::Rail(_) => {
                let edge = self
                    .inline
                    .iter()
                    .rev()
                    .filter_map(Anchor::drawn_bounds)
                    .filter(|bounds| bounds.contains(&center))
                    .min_by(|a, b| {
                        let area = |bounds: &Bounds<Pixels>| {
                            f32::from(bounds.size.width) * f32::from(bounds.size.height)
                        };
                        area(a).total_cmp(&area(b))
                    });
                let placed = edge.map_or_else(ordinary, |edge| {
                    place(edge.right(), target.bounds, size, area)
                });
                // A rail button keeps accepting pointer hover while its menu is open beside it. Give the open
                // panel the space it already occupies, including when the tooltip falls back above the button.
                let card = Bounds::new(placed.origin, size);
                let clear = !matches!(placement, HelpPlacement::Rail(_))
                    || !self
                        .panels
                        .iter()
                        .filter_map(|surface| surface.get())
                        .any(|panel| panel.intersects(&card));
                clear.then_some(placed)
            }
            HelpPlacement::Ordinary | HelpPlacement::Below => Some(ordinary()),
        }
    }
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
    let left = (edge_right + GAP).max(area.left() + EDGE);
    beside_at(TooltipSide::Beside, left, target.center().y, size, area)
        .unwrap_or_else(|| around_target(target, size, area, false))
}

/// Place a card at its exact horizontal gap from the surface. A narrow window rejects the candidate instead of
/// shifting it across the surface's controls.
fn beside_at(
    side: TooltipSide,
    left: Pixels,
    center: Pixels,
    size: Size<Pixels>,
    area: Bounds<Pixels>,
) -> Option<Placed> {
    if left < area.left() + EDGE || left + size.width > area.right() - EDGE {
        return None;
    }
    let top = (center - size.height / 2.)
        .min(area.bottom() - EDGE - size.height)
        .max(area.top() + EDGE);
    let inset = NOTCH_INSET.min(size.height / 2.);
    let notch_y = center.min(top + size.height - inset).max(top + inset);
    Some(Placed {
        origin: point(left, top),
        side,
        notch_y: Some(notch_y),
    })
}

/// Menu help prefers the right side, then the left. A submenu already open on either side takes precedence over
/// the trigger's help. If even the ordinary fallback would cover that panel, this frame leaves the help hidden.
pub(super) fn place_menu(
    edge: Bounds<Pixels>,
    target: Bounds<Pixels>,
    size: Size<Pixels>,
    area: Bounds<Pixels>,
    lower_panels: impl Iterator<Item = Bounds<Pixels>> + Clone,
    higher_panels: impl Iterator<Item = Bounds<Pixels>> + Clone,
) -> Option<Placed> {
    let covers = |placed: &Placed, panel: Bounds<Pixels>| {
        panel.intersects(&Bounds::new(placed.origin, size))
    };
    let clear = |placed: &Placed| {
        !lower_panels
            .clone()
            .chain(higher_panels.clone())
            .any(|panel| covers(placed, panel))
    };
    let center = target.center().y;
    beside_at(TooltipSide::Beside, edge.right() + GAP, center, size, area)
        .filter(clear)
        .or_else(|| {
            beside_at(
                TooltipSide::BesideLeft,
                edge.left() - GAP - size.width,
                center,
                size,
                area,
            )
            .filter(clear)
        })
        .or_else(|| {
            let ordinary = around_target(target, size, area, false);
            (!higher_panels.clone().any(|panel| covers(&ordinary, panel))).then_some(ordinary)
        })
}

/// Vertical placement for both input methods. The top band and dialog answers prefer below when both sides
/// fit, using the same clearance and narrow-window fallback as ordinary above-first help.
pub(super) fn around_target(
    target: Bounds<Pixels>,
    size: Size<Pixels>,
    area: Bounds<Pixels>,
    below_first: bool,
) -> Placed {
    let left = (target.center().x - size.width / 2.)
        .min(area.right() - EDGE - size.width)
        .max(area.left() + EDGE);
    let above = target.top() - GAP - size.height;
    let below = target.bottom() + GAP_BELOW;
    let fits_below = below + size.height <= area.bottom() - EDGE;
    let (side, top) = if above >= area.top() + EDGE && (!below_first || !fits_below) {
        (TooltipSide::Above, above)
    } else if fits_below {
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

/// The tooltip card and its optional cues share the top overlay's prepaint. At that point it can use the current
/// frame's panel and control bounds. The measured placement and painted ring are available on [`ShownTooltip`].
/// The card leaves pointer presses to whatever the user can reach underneath.
pub(super) struct PlacedBeside {
    against: Against,
    below_first: bool,
    /// Menus that could occupy the space below a top-band control. Only below-first help pays for this lookup.
    panels: Vec<(AnchorBounds, AnchorBounds)>,
    side: Rc<Cell<TooltipSide>>,
    /// The shown tooltip's record of the painted ring, cleared at each prepaint so a frame that paints none (the
    /// fallback, once a resting hover's window narrows) does not keep reporting the last one.
    painted: Rc<Cell<Option<DrawnRing>>>,
    card: AnyElement,
}

/// What a [`PlacedBeside`] places its card against.
enum Against {
    /// The containing edge, found after the control and panels have recorded their bounds for this frame.
    Beside {
        target: HelpTarget,
        edges: HelpEdges,
        focus: Option<FocusHandle>,
        placement: HelpPlacement,
        notches: [AnyElement; 2],
        ring: Option<AnyElement>,
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
    /// Place help outside its surface when it fits, with ordinary help as the fallback for either input method.
    pub(super) fn new(
        shown: &ShownTooltip,
        placement: HelpPlacement,
        edges: HelpEdges,
        focus: Option<FocusHandle>,
        card: gpui::Div,
        palette: &theme::Palette,
    ) -> Self {
        let fill = theme::color(palette.bg_2);
        let border = theme::color(palette.border);
        let ring = if let HelpPlacement::Beside(cues) | HelpPlacement::Rail(cues) = placement {
            let color = theme::color(if cues.on_fill {
                palette.on_fill
            } else {
                palette.accent
            });
            Some(ring(color, cues.radius, shown.ring.clone()))
        } else {
            None
        };
        PlacedBeside {
            against: Against::Beside {
                target: shown.target.clone(),
                at: shown.at,
                edges,
                focus,
                placement,
                notches: [notch(fill, border, false), notch(fill, border, true)],
                ring,
            },
            below_first: false,
            panels: Vec::new(),
            side: shown.side.clone(),
            painted: shown.ring.clone(),
            card: card.into_any_element(),
        }
    }

    /// Place vertical help against the keyboard focus stop. It follows the stop through scrolling or reflow.
    pub(super) fn around(
        shown: &ShownTooltip,
        focus: FocusHandle,
        card: gpui::Div,
        panels: Vec<(AnchorBounds, AnchorBounds)>,
    ) -> Self {
        PlacedBeside {
            against: Against::Focused(focus),
            below_first: shown.placement == HelpPlacement::Below,
            panels,
            side: shown.side.clone(),
            painted: shown.ring.clone(),
            card: card.into_any_element(),
        }
    }

    /// Place pointer help above or below its drawn control. With no recorded bounds, place it at the pointer
    /// where the help began ([`ShownTooltip::at`], [`at_pointer`]).
    pub(super) fn ordinary(
        shown: &ShownTooltip,
        card: gpui::Div,
        panels: Vec<(AnchorBounds, AnchorBounds)>,
    ) -> Self {
        PlacedBeside {
            against: Against::Ordinary {
                control: shown.target.clone(),
                at: shown.at,
            },
            below_first: shown.placement == HelpPlacement::Below,
            panels,
            side: shown.side.clone(),
            painted: shown.ring.clone(),
            card: card.into_any_element(),
        }
    }

    /// A popover that opened under a toggle takes the lower space, even though the toggle can still show help.
    /// An answer within its own question considers the room in that question usable.
    fn vertical(
        &self,
        target: Bounds<Pixels>,
        size: Size<Pixels>,
        area: Bounds<Pixels>,
    ) -> Option<Placed> {
        let preferred = around_target(target, size, area, self.below_first);
        let clear = |placed: Placed| {
            let card = Bounds::new(placed.origin, size);
            self.panels.iter().all(|(anchor, surface)| {
                let center = target.center();
                surface.get().is_none_or(|panel| {
                    let inside = panel.contains(&center);
                    // A clamped popover can cross its own trigger without making the trigger part of the panel.
                    let trigger = anchor.get().is_some_and(|bounds| bounds.contains(&center));
                    (inside && !trigger) || !panel.intersects(&card)
                })
            })
        };
        if clear(preferred) {
            return Some(preferred);
        }
        let above = around_target(target, size, area, false);
        (above.side == TooltipSide::Above && clear(above)).then_some(above)
    }
}

/// What a frame of [`PlacedBeside`] paints, as its prepaint decided.
pub(super) enum Painting {
    /// A focused stop has no bounds, or the card would obscure a higher menu panel.
    Nothing,
    /// The card alone: the fallback placement, ordinary help, or help keyboard focus owns.
    Card,
    /// The card with its notch and, for the ringed variant, the control's own clip.
    Cues { clip: Bounds<Pixels>, left: bool },
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
                target,
                edges,
                focus,
                placement,
                at,
                ..
            } => {
                if let Some(target) = target.drawn() {
                    let Some(placed) = edges.place(*placement, target, size, area) else {
                        return Painting::Nothing;
                    };
                    (placed, Some(target))
                } else if let Some(focus) = focus {
                    let Some(bounds) = crate::focus::control_bounds(focus, window, cx) else {
                        return Painting::Nothing;
                    };
                    (around_target(bounds, size, area, false), None)
                } else {
                    (at_pointer(*at, size, area), None)
                }
            }
            Against::Focused(focus) => {
                let Some(target) = crate::focus::control_bounds(focus, window, cx) else {
                    return Painting::Nothing;
                };
                let Some(placed) = self.vertical(target, size, area) else {
                    return Painting::Nothing;
                };
                (placed, None)
            }
            Against::Ordinary { control, at } => match control.drawn() {
                Some(control) => {
                    let Some(placed) = self.vertical(control.bounds, size, area) else {
                        return Painting::Nothing;
                    };
                    (placed, None)
                }
                None => (at_pointer(*at, size, area), None),
            },
        };
        self.side.set(placed.side);
        let offset = placed.origin - bounds.origin;
        let offset = point(offset.x.round(), offset.y.round());
        window.with_element_offset(offset, |window| self.card.prepaint(window, cx));
        let (Some(notch_y), Some(target), Against::Beside { notches, ring, .. }) =
            (placed.notch_y, tracked, &mut self.against)
        else {
            return Painting::Card;
        };
        // Center the notch on the card edge facing the target. The card covers the half inside its box.
        let left = placed.side == TooltipSide::BesideLeft;
        let notch = &mut notches[usize::from(left)];
        let across = px(NOTCH_SIDE * SQRT_2);
        let mut edge = bounds.origin.x + offset.x;
        if left {
            edge += size.width;
        }
        let square = gpui::size(across, across).map(AvailableSpace::Definite);
        notch.layout_as_root(square, window, cx);
        let corner = point(edge - across / 2., notch_y.round() - across / 2.);
        notch.prepaint_at(corner, window, cx);
        if let Some(ring) = ring {
            let size = target.bounds.size.map(AvailableSpace::Definite);
            ring.layout_as_root(size, window, cx);
            let clip = Some(ContentMask {
                bounds: target.clip,
            });
            window.with_content_mask(clip, |window| {
                ring.prepaint_at(target.bounds.origin, window, cx)
            });
        }
        Painting::Cues {
            clip: target.clip,
            left,
        }
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
            Painting::Cues { clip, left } => {
                let Against::Beside { notches, ring, .. } = &mut self.against else {
                    return;
                };
                notches[usize::from(left)].paint(window, cx);
                self.card.paint(window, cx);
                if let Some(ring) = ring {
                    let clip = Some(ContentMask { bounds: clip });
                    window.with_content_mask(clip, |window| ring.paint(window, cx));
                }
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
fn notch(fill: Hsla, line: Hsla, points_right: bool) -> AnyElement {
    let paint = move |bounds: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
        let center = bounds.center();
        let reach = bounds.size.width / 2.;
        // A 1-pixel band along each 45-degree edge meets the base √2 pixels further in.
        for (color, reach) in [(line, reach), (fill, reach - px(SQRT_2))] {
            let mut path = PathBuilder::fill();
            let tip = if points_right {
                center.x + reach
            } else {
                center.x - reach
            };
            path.move_to(point(tip, center.y));
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
