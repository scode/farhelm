//! Tests of the destination display's own rules ([`super::fully_visible`]); the display's behavior in a terminal is
//! tested with its owner (`attached/tests.rs`, `files/tests.rs`).

use gpui::{point, size};

use super::*;

/// Why: SPEC.md lets a click open a link only while its display is wholly inside the page, and on Linux, when the
/// window draws its own frame, the page (the content area) is inset from the window by GPUI Kit's frame and the
/// title bar. A display that runs into them lies inside the window but not the page; checking against the whole
/// window, as this did before, let a click on it through.
/// Spec: with a content area inset at its top left, a display inside it, touching any of its edges included, is
/// fully visible; one that crosses any edge by a pixel is not; an empty display (no width or no height), or one
/// never drawn, is not.
#[test]
fn a_display_reaching_past_the_content_area_is_not_fully_visible() {
    let area = Bounds::new(point(px(20.), px(52.)), size(px(600.), px(400.)));
    let display = size(px(200.), px(80.));
    let drawn: Drawn = Rc::new(Cell::new(None));
    assert!(
        !fully_visible(&drawn),
        "a display never drawn is not visible"
    );

    // The area's corners are 20 and 52 at the top left, 620 and 452 at the bottom right.
    for (label, origin) in [
        ("inside", point(px(100.), px(100.))),
        ("at the top left corner", point(px(20.), px(52.))),
        ("at the bottom right corner", point(px(420.), px(372.))),
    ] {
        drawn.set(Some((Bounds::new(origin, display), area)));
        assert!(fully_visible(&drawn), "a display {label} is fully visible");
    }
    for (label, origin) in [
        ("left", point(px(19.), px(100.))),
        ("top", point(px(100.), px(51.))),
        ("right", point(px(421.), px(100.))),
        ("bottom", point(px(100.), px(373.))),
    ] {
        drawn.set(Some((Bounds::new(origin, display), area)));
        assert!(
            !fully_visible(&drawn),
            "a display one pixel past the area's {label} edge is not fully visible"
        );
    }

    drawn.set(Some((
        Bounds::new(point(px(100.), px(100.)), size(px(0.), px(80.))),
        area,
    )));
    assert!(
        !fully_visible(&drawn),
        "a display with no width is not visible"
    );
    drawn.set(Some((
        Bounds::new(point(px(100.), px(100.)), size(px(200.), px(0.))),
        area,
    )));
    assert!(
        !fully_visible(&drawn),
        "a display with no height is not visible"
    );
}
