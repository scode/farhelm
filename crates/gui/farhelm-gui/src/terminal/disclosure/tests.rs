//! Tests of the destination display's window-fit rule ([`super::fits`]); visibility in a terminal is
//! tested with its owner (`attached/tests.rs`, `files/tests.rs`).

use gpui::{point, size};

use super::*;

/// Why: SPEC.md lets a click open a link only while its display is wholly inside the page, and on Linux, when the
/// window draws its own frame, the page (the content area) is inset from the window by GPUI Kit's frame and the
/// title bar. A display that runs into them lies inside the window but not the page; checking against the whole
/// window, as this did before, let a click on it through.
/// Spec: with a content area inset at its top left, a display inside it, touching any of its edges included, is
/// fully within the area. Crossing any edge by a pixel or having no width or height fails the fit check.
#[test]
fn a_display_reaching_past_the_content_area_is_not_fully_visible() {
    let area = Bounds::new(point(px(20.), px(52.)), size(px(600.), px(400.)));
    let display = size(px(200.), px(80.));

    // The area's corners are 20 and 52 at the top left, 620 and 452 at the bottom right.
    for (label, origin) in [
        ("inside", point(px(100.), px(100.))),
        ("at the top left corner", point(px(20.), px(52.))),
        ("at the bottom right corner", point(px(420.), px(372.))),
    ] {
        assert!(
            fits(Bounds::new(origin, display), area),
            "a display {label} fits"
        );
    }
    for (label, origin) in [
        ("left", point(px(19.), px(100.))),
        ("top", point(px(100.), px(51.))),
        ("right", point(px(421.), px(100.))),
        ("bottom", point(px(100.), px(373.))),
    ] {
        assert!(
            !fits(Bounds::new(origin, display), area),
            "a display one pixel past the area's {label} edge is not fully visible"
        );
    }

    assert!(
        !fits(
            Bounds::new(point(px(100.), px(100.)), size(px(0.), px(80.))),
            area
        ),
        "a display with no width is not visible"
    );
    assert!(
        !fits(
            Bounds::new(point(px(100.), px(100.)), size(px(200.), px(0.))),
            area
        ),
        "a display with no height is not visible"
    );
}
