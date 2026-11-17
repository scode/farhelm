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
//!   to stay whole and the gap between them, never less than nothing ([`floor_beside_pill`]). The pill needs the floor
//!   it is built to shrink to when it has one (154 pixels for one tunnel's mapping, which stays readable as the host's
//!   name is cut) and its content's width when it has none (a count of tunnels, whose words any narrowing cuts)
//!   ([`TunnelPill::shrink_floor`]); it gets that width as a floor of its own too, so a count pill cannot give up width
//!   alongside the strip. SPEC.md keeps the pill and the actions whole down to a 650-pixel pane, and the strip scrolls,
//!   so the strip is what gives. A fixed 112 pixels pushes the pill over the first action there: beside the condensed
//!   actions a remote session's left region is about 251 pixels wide at 650, short of the 272 that the strip's 112,
//!   the 6-pixel gap and the pill's 154 add up to, and an environment session's, whose actions carry its Suspend or
//!   Resume control as well, about 176. Decided with the session view's owner on 2026-11-16; the other notes
//!   of this rule point here. The decision whether to condense still counts the full 112, as the old script measured
//!   it. Without a pill the strip has no floor.
//! - **Measured, not remembered.** Every input is measured while the frame is laid out, and none depends on the
//!   choice: the actions at their ordinary size (as built for drawing, and drawn if they fit), the pill from a copy of
//!   its own laid out at the width its content asks for, and the room from this element's own bounds, which the pane's
//!   width decides. So the choice cannot undo its own premise and flip back on the next frame, as the old script's
//!   comment warned ("Measure the normal presentation before choosing the compact one, so compacting cannot make its
//!   own premise false"), and it takes no second frame: the condensed actions are laid out in place of the ordinary
//!   ones before anything of the row is drawn. The strip is no exception: the decision counts it at the full 112
//!   pixels, a constant, and never at the floor that gives way, which is decided from the choice and so could feed it
//!   back. Its other inputs, the ordinary actions and the pill's content, are measured from elements built the same
//!   way whichever look the last frame drew.
//! - **The floors are decided before the left region exists.** GPUI fixes a node's style when the node asks for its
//!   layout, once per frame (a second layout of the same element only recomputes it, reusing what taffy cached), so a
//!   floor has to be in the left region's style before the region is laid out at all. The row therefore hands this
//!   element the strip and the pill unassembled ([`Left`]), and it builds the region only once the floors are decided,
//!   as plain minimum widths, and lays it out once, at the width the actions leave. The strip's own floor is nothing
//!   (`min_w_0`, so it scrolls), so with the floor above added it is exactly that floor, and the decision takes it as
//!   [`STRIP_FLOOR_BESIDE_PILL`] rather than measuring it.
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
//! NOTE: Only this element applies the strip's and the pill's floors. The row builds its left region without it when
//! there are no actions to fit (no store, no session row, or a Delete in flight), and then neither has a floor there.

use gpui::{
    AlignSelf, AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, InteractiveElement as _, IntoElement, LayoutId, ParentElement as _, Pixels,
    Size, Style, Styled as _, Window, div, point, px, relative, size,
};

use super::TABS_SLOT;
use crate::terminal::set::STRIP_FLOOR_BESIDE_PILL;
use crate::terminal::tunnels::TunnelPill;

/// The top row's left region before it is built: the terminal area's tab strip and tunnel pill as the terminal area
/// built them, and the row's gap. Whoever draws the region builds it ([`Left::build`]): the fit, once it
/// has decided the floors, or the row itself, with none, when it has no actions to fit.
#[derive(Default)]
pub(super) struct Left {
    pub(super) strip: Option<AnyElement>,
    pub(super) pill: Option<AnyElement>,
    /// The row's gap: between the strip and the pill, and, in the fit, between this region and the actions.
    pub(super) gap: Pixels,
}

impl Left {
    /// The left region ([`TABS_SLOT`]), with `floors` (the strip's, then the pill's) when the fit decided them. It takes
    /// the width the actions leave and gives it up first (`min_w_0`), so the actions never shrink at the 650-pixel floor.
    ///
    /// With floors, the strip and the pill each sit in a box that shrinks as its child would on its own (GPUI's default
    /// `flex_shrink: 1` and automatic basis) but never below its floor, though it is wider than its content when the
    /// floor is (the strip at 1400 pixels with a pill, say). The box is a block, so its child takes the box's whole width
    /// whatever its own content asks for: the strip keeps scrolling sideways when the box is narrower than its tabs.
    /// Without floors there is no box at all, which is not the same as a zero floor: a box with no floor shrank
    /// alongside the strip and let the pill spill out of it over a Delete's progress.
    pub(super) fn build(self, floors: Option<(Pixels, Pixels)>) -> AnyElement {
        let floored =
            |child: AnyElement, floor: Pixels| div().min_w(floor).child(child).into_any_element();
        let (strip, pill) = match floors {
            Some((strip, pill)) => (
                self.strip.map(|child| floored(child, strip)),
                self.pill.map(|child| floored(child, pill)),
            ),
            None => (self.strip, self.pill),
        };
        div()
            .id(TABS_SLOT)
            .debug_selector(|| TABS_SLOT.into())
            .flex()
            .flex_row()
            .items_center()
            .flex_1()
            .min_w_0()
            .gap(self.gap)
            .children(strip)
            .children(pill)
            .into_any_element()
    }
}

/// The top row's content with the actions in whichever look fits (see the module docs).
pub(super) struct Fit {
    /// The strip and the pill as the left region draws them, built into it once the floors are decided.
    pub(super) left: Left,
    /// A copy of the pill, laid out at the width its content asks for and never drawn, with the floor it is built to
    /// shrink to; `None` without a pill, when the actions always take their ordinary look (the old rules applied only
    /// `:has(> .session-tunnel-pill)`) and the strip has no floor.
    pub(super) pill: Option<TunnelPill>,
    /// The actions in their ordinary look: always laid out, to measure them, and drawn when they fit.
    pub(super) ordinary: AnyElement,
    /// The actions in their condensed look, built only while a pill shows: laid out and drawn only when the ordinary
    /// ones do not fit.
    pub(super) condensed: Option<AnyElement>,
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
/// region cannot hold that beside the width the pill needs to stay whole (`pill`) and the `gap` between them, and
/// never below nothing.
pub(super) fn floor_beside_pill(left: Pixels, pill: Pixels, gap: Pixels) -> Pixels {
    px(STRIP_FLOOR_BESIDE_PILL).min((left - pill - gap).max(px(0.)))
}

/// Lay `element` out by itself at the width its content asks for, returning its size.
fn natural(element: &mut AnyElement, window: &mut Window, cx: &mut App) -> Size<Pixels> {
    let space = size(AvailableSpace::MaxContent, AvailableSpace::MaxContent);
    element.layout_as_root(space, window, cx)
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
        let left = std::mem::take(&mut self.left);
        let gap = left.gap;
        let mut actions = std::mem::replace(&mut self.ordinary, gpui::Empty.into_any_element());
        let ordinary = natural(&mut actions, window, cx).width;
        // The pill at its content's width, which the decision counts, and the width it needs to stay whole, which
        // decides the width the strip's floor leaves it.
        let pill = self.pill.take().map(|mut copy| {
            let content = natural(&mut copy.element, window, cx).width;
            (content, copy.shrink_floor.unwrap_or(content))
        });
        // The strip at its full floor, as the old script measured it: what the decision counts is the old rule's.
        let floor = pill
            .and(left.strip.as_ref())
            .map(|_| px(STRIP_FLOOR_BESIDE_PILL));
        if condenses(room, ordinary, pill.map(|(content, _)| content), floor, gap)
            && let Some(condensed) = self.condensed.take()
        {
            actions = condensed;
        }
        // A no-op for the ordinary actions, already laid out this way.
        let extent = natural(&mut actions, window, cx);
        let left_width = (room - extent.width - gap).max(px(0.));
        // The pill's floor is capped at the left region's width, so its box stays inside the region; a pill wider than
        // that (only below the 650-pixel floor, where SPEC.md lets the row clip) still overflows its box.
        let floors = pill.map_or((px(0.), px(0.)), |(_, needs)| {
            (
                floor_beside_pill(left_width, needs, gap),
                needs.min(left_width),
            )
        });
        // The left region in a frame that fills the width it is laid out at and centres it vertically, as the row's
        // flex layout did.
        let mut left = div()
            .w_full()
            .h_full()
            .flex()
            .flex_row()
            .items_center()
            .child(left.build(Some(floors)))
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
            bounds.origin.x + left_width + gap,
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

#[cfg(test)]
mod tests {
    use gpui::px;

    use super::{condenses, floor_beside_pill};

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
}
