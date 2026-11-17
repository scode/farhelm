//! The top row's fit: whether the actions take their ordinary or their condensed look, and how narrow the tab strip may
//! get beside a tunnel pill, decided from measured widths in the frame being drawn.
//!
//! SPEC.md, "Terminal experience": the row never wraps, and everything in it stays visible and keyboard reachable down
//! to a 650-pixel main pane. A tunnel pill keeps its floor (the width at which both of its ports stay readable), so at
//! that width it is the actions that give way. Ported from the old app's `assets/header-layout.js` and its
//! `.actions-compact` rules (app.css) at the reference commit: the row condenses its actions only while a pill shows and
//! the row cannot hold, side by side, the actions at their ordinary width, the pill at the width its content asks for
//! and the tab strip at its floor, with the row's gaps between them. A wide row keeps the ordinary look even with a
//! pill, and the row goes back to it as soon as the pressure leaves.
//!
//! - **The strip's floor beside a pill.** The old app gave the strip `min-width: 112px` while a pill showed
//!   ([`STRIP_FLOOR_BESIDE_PILL`]). Here that floor gives way when the row lacks room: once the actions' look is
//!   chosen, the strip gets the smaller of 112 pixels and what the left region has left after the width the pill needs
//!   to stay whole and the gap between them, never less than nothing ([`floor_beside_pill`]). The pill needs its own
//!   floor when it has one (154 pixels for one tunnel's mapping, which stays readable as the host's name is cut) and
//!   its content's width when it has none (a count of tunnels, whose words any narrowing cuts) ([`pill_needs`]); it
//!   gets that width as a floor of its own too, so a count pill cannot give up width alongside the strip. SPEC.md
//!   keeps the pill and the actions whole down to a 650-pixel pane, and the strip scrolls, so the strip is what gives.
//!   A fixed 112 pixels pushes the pill over the first action there: by far for an environment session, whose Suspend
//!   or Resume control the old row did not have, and with today's condensed actions a little for a remote session
//!   too. Decided with the session view's owner on 2026-11-16; the other notes of this rule point here. The decision
//!   whether to condense still counts the full 112, as the old script measured it. Without a pill the strip has no
//!   floor.
//! - **Measured, not remembered.** Every input is measured while the frame is laid out, and none depends on the
//!   choice: the actions at their ordinary size (as built for drawing, and drawn if they fit), the pill from a copy of
//!   its own laid out at the width its content asks for and again with no room at all (its floor), and the room from
//!   this element's own bounds, which the pane's width decides. So the choice cannot undo its own premise and flip back
//!   on the next frame, as the old script's comment warned ("Measure the normal presentation before choosing the
//!   compact one, so compacting cannot make its own premise false"), and it takes no second frame: the condensed
//!   actions are laid out in place of the ordinary ones before anything of the row is drawn. The strip is no exception:
//!   the decision counts it at the full 112 pixels, a constant, and never at the floor that gives way, which is
//!   decided from the choice and so could feed it back. Its other inputs, the ordinary actions and the pill's widths,
//!   are measured from elements built the same way whichever look the last frame drew.
//! - **The strip's floor is set, not measured.** The strip's own floor is nothing (`min_w_0`, so it scrolls), so with
//!   the floor above added it is exactly that floor; the decision takes it as [`STRIP_FLOOR_BESIDE_PILL`] rather than
//!   measuring the left region laid out with no room. Such a measurement cannot coexist with a floor that gives way:
//!   GPUI fixes a node's style when the node asks for its layout, once per frame (a second layout of the same element
//!   only recomputes it, reusing what taffy cached), so the floor that the final layout uses has to be known before
//!   the left region is laid out at all. The left region is therefore laid out once, at the width the actions leave,
//!   with the strip's and the pill's floors already decided ([`Floored`]).
//! - **What is only measured stays inert.** The pill's copy, and the ordinary actions when they condense, are laid out
//!   but never prepainted or painted, so they register no hitbox, focus, tooltip, driver node or anchor. The two looks
//!   of the actions are laid out at the same place in the element tree, so they share their ids and with them their
//!   element state, which laying out only reads and puts back. The pill's copy is laid out elsewhere (directly under the
//!   row, not in the left region), so it has element state of its own, which no size depends on.
//! - **What it lays out.** The row's own div keeps its padding, background and the title band's press rule; this element
//!   is its only child and fills its content box. Inside it, the actions sit at the right edge at their width and
//!   vertically centred, and the left region takes the rest less the gap, as the row's flex layout placed them before
//!   (a pane too narrow even for the condensed actions still clips them at the right). This element asks for an empty
//!   box, so the row's height is its minimum height and no longer grows with what it holds.
//!
//! NOTE: Only this element applies the strip's and the pill's floors. The row draws its left region without it when
//! there are no actions to fit (no store, no session row, or a Delete in flight), so neither has a floor there.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AlignSelf, AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, ParentElement as _, Pixels, Size, Style,
    Styled as _, Window, div, point, px, relative, size,
};

use crate::terminal::set::STRIP_FLOOR_BESIDE_PILL;

/// A floor for the frame being drawn, the strip's or the pill's, which [`Fit`] decides and [`Floored`] applies: shared,
/// since the two are built apart in the row's render. It is unset (`None`) until a fit sets it, and an element whose
/// floor stays unset is laid out exactly as it would be on its own: on a row with no actions to fit (no store, no
/// session row, a Delete running) neither the strip nor the pill is wrapped in a box that could shrink it.
pub(super) type Floor = Rc<Cell<Option<Pixels>>>;

/// The top row's content with the actions in whichever look fits (see the module docs).
pub(super) struct Fit {
    /// The left region: the terminal area's tab strip and pill, each drawn through [`Floored`], with `strip` and with
    /// `pill`'s floor.
    pub(super) left: AnyElement,
    /// The floor the strip in `left` is laid out with; `None` when the region has no strip, which the decision whether
    /// to condense then leaves out.
    pub(super) strip: Option<Floor>,
    /// A copy of the pill, laid out at the width its content asks for and with no room, and never drawn, and the floor
    /// the pill in `left` is laid out with; `None` without a pill, when the actions always take their ordinary look
    /// (the old rules applied only `:has(> .session-tunnel-pill)`) and the strip has no floor.
    pub(super) pill: Option<(AnyElement, Floor)>,
    /// The actions in their ordinary look: always laid out, to measure them, and drawn when they fit.
    pub(super) ordinary: AnyElement,
    /// The actions in their condensed look, built only while a pill shows: laid out and drawn only when the ordinary
    /// ones do not fit.
    pub(super) condensed: Option<AnyElement>,
    /// The row's gap, between the strip and the pill and between the left region and the actions.
    pub(super) gap: Pixels,
}

/// Whether the actions must condense: a pill shows (`pill`, its content's width), and the row's `room` cannot hold the
/// actions at their `ordinary` width beside it and the strip at its `floor` (`None` without a strip), with a `gap`
/// between each, as the old script's `required > available`.
pub(super) fn condenses(
    room: Pixels,
    ordinary: Pixels,
    pill: Option<Pixels>,
    floor: Option<Pixels>,
    gap: Pixels,
) -> bool {
    let Some(pill) = pill else {
        return false;
    };
    let strip = floor.map_or(px(0.), |floor| floor + gap);
    strip + pill + gap + ordinary > room
}

/// The strip's floor beside a pill in a left region `left` pixels wide: [`STRIP_FLOOR_BESIDE_PILL`], or less when the
/// region cannot hold that beside the width the pill needs (`pill`, [`pill_needs`]) and the `gap` between them, and
/// never below nothing.
pub(super) fn floor_beside_pill(left: Pixels, pill: Pixels, gap: Pixels) -> Pixels {
    px(STRIP_FLOOR_BESIDE_PILL).min((left - pill - gap).max(px(0.)))
}

/// The width a pill needs to be drawn whole, from its `content` width and its own `floor` (its width with no room):
/// the floor when it has one, since such a pill is built to shrink to it (a mapping cuts its host's name and keeps
/// both ports), and its content's width when it has none, since such a pill (a count of tunnels) has nothing to give
/// up without cutting its words. Counting a pill with a floor at that floor, not its content, is what lets the strip
/// keep the full 112 pixels where the pill can give up width instead.
pub(super) fn pill_needs(content: Pixels, floor: Pixels) -> Pixels {
    if floor > px(0.) { floor } else { content }
}

/// Lay `element` out by itself at the width its content asks for, returning its size.
fn natural(element: &mut AnyElement, window: &mut Window, cx: &mut App) -> Size<Pixels> {
    let space = size(AvailableSpace::MaxContent, AvailableSpace::MaxContent);
    element.layout_as_root(space, window, cx)
}

/// The widths of the pill's copy `pill`: at what its content asks for, and with no room at all, which is its own floor
/// (154 pixels while it shows a mapping, nothing for a count).
///
/// The copy is measured as an item of a flex row, the way the left region lays the pill out, so its minimum width
/// applies as it does there: laid out by itself with no room, the pill came out 0 pixels wide, its minimum ignored. The
/// row fills the width it is laid out at (`w_full`, which resolves to the content's width when that width is not
/// definite), and [`Recorded`] gives back the pill's own node in it.
fn pill_widths(pill: AnyElement, window: &mut Window, cx: &mut App) -> (Pixels, Pixels) {
    let slot = LayoutSlot::default();
    let mut row = div()
        .w_full()
        .flex()
        .flex_row()
        .child(Recorded {
            child: pill,
            layout: slot.clone(),
        })
        .into_any_element();
    let mut width = |row: &mut AnyElement, space: AvailableSpace| {
        row.layout_as_root(size(space, AvailableSpace::MaxContent), window, cx);
        slot.get()
            .map_or(px(0.), |layout| window.layout_bounds(layout).size.width)
    };
    let content = width(&mut row, AvailableSpace::MaxContent);
    (content, width(&mut row, AvailableSpace::Definite(px(0.))))
}

impl IntoElement for Fit {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Fit {
    type RequestLayoutState = ();
    /// What prepaint laid out, for paint: the left region in its frame, and the chosen actions.
    type PrepaintState = (AnyElement, AnyElement);

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    /// An empty box that fills the row's content box, as the left region did with `flex_1` and `min_w_0`: its children
    /// are laid out in prepaint, once its width is known.
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            flex_grow: 1.,
            flex_shrink: 1.,
            flex_basis: relative(0.).into(),
            min_size: size(px(0.).into(), gpui::Length::Auto),
            align_self: Some(AlignSelf::Stretch),
            ..Style::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> (AnyElement, AnyElement) {
        let room = bounds.size.width;
        let mut actions = std::mem::replace(&mut self.ordinary, gpui::Empty.into_any_element());
        let ordinary = natural(&mut actions, window, cx).width;
        // The pill at its content's width, which the decision counts, and at its own floor, which with the content
        // decides the width the strip's floor leaves it.
        let pill = self.pill.take().map(|(copy, floor)| {
            let (content, own) = pill_widths(copy, window, cx);
            (content, pill_needs(content, own), floor)
        });
        // The strip at its full floor, as the old script measured it: what the decision counts is the old rule's.
        let floor = pill
            .as_ref()
            .and(self.strip.as_ref())
            .map(|_| px(STRIP_FLOOR_BESIDE_PILL));
        if condenses(
            room,
            ordinary,
            pill.as_ref().map(|(content, _, _)| *content),
            floor,
            self.gap,
        ) && let Some(condensed) = self.condensed.take()
        {
            actions = condensed;
        }
        // A no-op for the ordinary actions, already laid out this way.
        let extent = natural(&mut actions, window, cx);
        let left_width = (room - extent.width - self.gap).max(px(0.));
        // Decided before the left region asks for its layout, which fixes the strip's and the pill's styles for the
        // frame (the module docs' "The strip's floor is set, not measured"). The pill's floor is capped at the left
        // region's width, so its box stays inside the region; a pill wider than that (only below the 650-pixel floor,
        // where SPEC.md lets the row clip) still overflows its box.
        if let Some(strip) = &self.strip {
            strip.set(Some(pill.as_ref().map_or(px(0.), |(_, needs, _)| {
                floor_beside_pill(left_width, *needs, self.gap)
            })));
        }
        if let Some((_, needs, floor)) = &pill {
            floor.set(Some((*needs).min(left_width)));
        }
        // The left region in a frame that fills the width it is laid out at and centres it vertically, as the row's
        // flex layout did.
        let left = std::mem::replace(&mut self.left, gpui::Empty.into_any_element());
        let mut left = div()
            .w_full()
            .h_full()
            .flex()
            .flex_row()
            .items_center()
            .child(left)
            .into_any_element();
        left.layout_as_root(
            size(
                AvailableSpace::Definite(left_width),
                AvailableSpace::Definite(bounds.size.height),
            ),
            window,
            cx,
        );
        left.prepaint_at(bounds.origin, window, cx);
        let at = point(
            bounds.origin.x + left_width + self.gap,
            bounds.origin.y + (bounds.size.height - extent.height) / 2.,
        );
        actions.prepaint_at(at, window, cx);
        (left, actions)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        (left, actions): &mut (AnyElement, AnyElement),
        window: &mut Window,
        cx: &mut App,
    ) {
        // The left region first, then the actions, as the row's children were: Tab's order follows the drawing order.
        left.paint(window, cx);
        actions.paint(window, cx);
    }
}

/// The tab strip or the pill as the left region lays it out: inside a box whose minimum width is the [`Floor`] the fit
/// decided, read as this element asks for its layout, so the fit must set it before laying the region out. With the
/// floor unset, there is no box: the child's own layout node is this element's.
///
/// The box shrinks as its child did on its own (`flex_shrink: 1`, its basis its content's width), and never below the
/// floor, though it is wider than its content when the floor is (the strip at 1400 pixels with a pill, say). The box
/// is a block (GPUI's default display), so its child takes the box's whole width whatever its own content asks for:
/// the strip keeps scrolling sideways when the box is narrower than its tabs, and needs nothing from its own style for
/// that.
pub(super) struct Floored {
    child: AnyElement,
    floor: Floor,
}

impl Floored {
    /// `child`, laid out with whatever minimum width `floor` holds when it asks for its layout.
    pub(super) fn new(child: AnyElement, floor: Floor) -> Floored {
        Floored { child, floor }
    }
}

impl IntoElement for Floored {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Floored {
    type RequestLayoutState = ();
    type PrepaintState = ();

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
    ) -> (LayoutId, ()) {
        let child = self.child.request_layout(window, cx);
        // No fit decided a floor: the child is the flex item itself, as it was before floors existed.
        let Some(floor) = self.floor.get() else {
            return (child, ());
        };
        let style = Style {
            flex_shrink: 1.,
            flex_basis: gpui::Length::Auto,
            min_size: size(floor.into(), gpui::Length::Auto),
            ..Style::default()
        };
        (window.request_layout(style, [child], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

/// Where [`Recorded`] puts its child's layout node.
type LayoutSlot = Rc<Cell<Option<LayoutId>>>;

/// An element drawn as its child, recording the child's layout node so its owner can read where the child was laid out
/// ([`Window::layout_bounds`]) after laying it out as part of something larger. Its layout node is the child's own, so
/// it adds nothing to the layout. Only the pill's copy goes through it, which is never drawn.
struct Recorded {
    child: AnyElement,
    layout: LayoutSlot,
}

impl IntoElement for Recorded {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Recorded {
    type RequestLayoutState = ();
    type PrepaintState = ();

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
    ) -> (LayoutId, ()) {
        let layout = self.child.request_layout(window, cx);
        self.layout.set(Some(layout));
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use gpui::px;

    use super::{condenses, floor_beside_pill, pill_needs};

    /// The rule is the old script's `required > available`, which the layout tests reach only at a few widths: it counts
    /// the pill at its content's width, the strip at its floor with a gap after it only when there is a strip, and the
    /// actions at their ordinary width, and condenses nothing without a pill. Specified: with a 100-pixel pill, a 20-pixel
    /// floor, 300-pixel actions and 6-pixel gaps, 432 pixels hold the row and 431 do not; with no strip, 406 pixels hold it
    /// and 405 do not; and with no pill, a row of no width does not condense.
    #[test]
    fn the_actions_condense_only_when_a_pill_and_the_rest_do_not_fit() {
        let (pill, floor, actions, gap) = (Some(px(100.)), Some(px(20.)), px(300.), px(6.));
        assert!(!condenses(px(432.), actions, pill, floor, gap));
        assert!(condenses(px(431.), actions, pill, floor, gap));
        assert!(!condenses(px(406.), actions, pill, None, gap));
        assert!(condenses(px(405.), actions, pill, None, gap));
        assert!(!condenses(px(0.), actions, None, floor, gap));
    }

    /// The strip's floor gives way only as far as the pill needs, which the layout tests reach at a few widths: a floor
    /// that never gave way would push the pill over the actions at 650 pixels, and one that gave way too readily would
    /// let a pill squeeze the strip below the old 112 pixels where both fit. Specified: with a pill that needs 154
    /// pixels and 6-pixel gaps, a 272-pixel region gives the full 112 and so does a wider one, a 271-pixel region gives
    /// 111, a 195-pixel region gives 35, and a region narrower than the pill's need gives nothing, not a negative
    /// width.
    #[test]
    fn the_strips_floor_gives_way_only_to_the_pills_floor() {
        let (pill, gap) = (px(154.), px(6.));
        assert_eq!(floor_beside_pill(px(272.), pill, gap), px(112.));
        assert_eq!(floor_beside_pill(px(900.), pill, gap), px(112.));
        assert_eq!(floor_beside_pill(px(271.), pill, gap), px(111.));
        assert_eq!(floor_beside_pill(px(195.), pill, gap), px(35.));
        assert_eq!(floor_beside_pill(px(100.), pill, gap), px(0.));
    }

    /// A count pill has no floor of its own, so counting it at its floor (nothing) let the strip's floor squeeze it
    /// until its words were cut, against SPEC.md's whole pill at 650 pixels. Specified: a pill with a 154-pixel floor
    /// needs 154 pixels whatever its content's width, and one with no floor needs its content's 99.
    #[test]
    fn a_pill_needs_its_floor_or_without_one_its_content() {
        assert_eq!(pill_needs(px(280.), px(154.)), px(154.));
        assert_eq!(pill_needs(px(99.), px(0.)), px(99.));
    }
}
