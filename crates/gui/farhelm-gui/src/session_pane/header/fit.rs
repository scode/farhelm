//! The top row's fit: whether the actions take their ordinary or their condensed look, decided from measured widths in
//! the frame being drawn.
//!
//! SPEC.md, "Terminal experience": the row never wraps, and everything in it stays visible and keyboard reachable down
//! to a 650-pixel main pane. A tunnel pill keeps its floor (the width at which both of its ports stay readable), so at
//! that width it is the actions that give way. Ported from the old app's `assets/header-layout.js` and its
//! `.actions-compact` rules (app.css) at the reference commit: the row condenses its actions only while a pill shows and
//! the row cannot hold, side by side, the actions at their ordinary width, the pill at the width its content asks for
//! and the tab strip at its floor, with the row's gaps between them. A wide row keeps the ordinary look even with a
//! pill, and the row goes back to it as soon as the pressure leaves.
//!
//! - **Measured, not remembered.** Every input is measured while the frame is laid out, and none depends on the
//!   choice: the actions at their ordinary size (as built for drawing, and drawn if they fit), the pill from a copy of
//!   its own laid out at the width its content asks for, the strip's floor from the left region laid out with no room at
//!   all, and the room from this element's own bounds, which the pane's width decides. So the choice cannot undo its own
//!   premise and flip back on the next frame, as the old script's comment warned ("Measure the normal presentation
//!   before choosing the compact one, so compacting cannot make its own premise false"), and it takes no second frame:
//!   the condensed actions are laid out in place of the ordinary ones before anything of the row is drawn.
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

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AlignSelf, AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, ParentElement as _, Pixels, Size, Style,
    Styled as _, Window, div, point, px, relative, size,
};

/// Where [`Measured`] records its child's layout node: the left region's tab strip's, for the floor measurement.
pub(super) type LayoutSlot = Rc<Cell<Option<LayoutId>>>;

/// The top row's content with the actions in whichever look fits (see the module docs).
pub(super) struct Fit {
    /// The left region: the terminal area's tab strip (drawn through [`Measured`] with `strip`) and pill.
    pub(super) left: AnyElement,
    /// The strip's layout node, set once the left region is laid out; never set when the region has no strip.
    pub(super) strip: LayoutSlot,
    /// A copy of the pill, laid out at the width its content asks for and never drawn; `None` without a pill, when the
    /// actions always take their ordinary look (the old rules applied only `:has(> .session-tunnel-pill)`).
    pub(super) pill: Option<AnyElement>,
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
        let mut actions = std::mem::replace(&mut self.ordinary, gpui::Empty.into_any_element());
        let ordinary = natural(&mut actions, window, cx).width;
        let pill = self
            .pill
            .as_mut()
            .map(|pill| natural(pill, window, cx).width);
        // The left region in a frame that fills whatever width it is laid out at, so it can be laid out twice: first with
        // no room, where the strip shrinks to its floor, then with the room the actions leave.
        let left = std::mem::replace(&mut self.left, gpui::Empty.into_any_element());
        let mut left = div()
            .w_full()
            .h_full()
            .flex()
            .flex_row()
            .items_center()
            .child(left)
            .into_any_element();
        let floor = if pill.is_some() {
            left.layout_as_root(
                size(
                    AvailableSpace::Definite(px(0.)),
                    AvailableSpace::Definite(bounds.size.height),
                ),
                window,
                cx,
            );
            self.strip
                .get()
                .map(|strip| window.layout_bounds(strip).size.width)
        } else {
            None
        };
        if condenses(room, ordinary, pill, floor, self.gap)
            && let Some(condensed) = self.condensed.take()
        {
            actions = condensed;
        }
        // A no-op for the ordinary actions, already laid out this way.
        let extent = natural(&mut actions, window, cx);
        let left_width = (room - extent.width - self.gap).max(px(0.));
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

/// An element drawn as its child, recording the child's layout node so its owner can read where the child was laid out
/// ([`Window::layout_bounds`]) after laying it out as part of something larger. Its layout node is the child's own, so
/// it adds nothing to the layout.
pub(super) struct Measured {
    child: AnyElement,
    layout: LayoutSlot,
}

impl Measured {
    /// `child`, with its layout node recorded in `layout` each time it is laid out.
    pub(super) fn new(child: AnyElement, layout: LayoutSlot) -> Measured {
        Measured { child, layout }
    }
}

impl IntoElement for Measured {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Measured {
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

    use super::condenses;

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
}
