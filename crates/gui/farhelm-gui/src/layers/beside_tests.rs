//! Tests of hover help placed beside the sidebar ([`super::beside`]): the placement's numbers as pure functions, each
//! pinned to the old app's unit cases (`crates/farhelm-ui/js-tests/tooltip.test.js` at reference commit `24bc0580`),
//! then the same rules, and ordinary help at the pointer, fitted into a Linux window's content area inside GPUI Kit's
//! frame and below the window's own title bar (as pure functions too, since no test window reports client
//! decorations), and in a window, the cues, the fallback and the card form as a control's hover help draws them,
//! including the card whose view is made only while it shows.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyView, AppContext as _, Bounds, Context, InteractiveElement as _, IntoElement, Modifiers,
    ParentElement as _, Pixels, Render, Styled as _, TestAppContext, VisualTestContext, WeakEntity,
    Window, div, point, px, size,
};

use super::beside::{Placed, at_pointer, place};
use crate::components::{Button, HoverHelp, Role};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, TooltipSide};
use crate::test_support::{advance, bounds, center, linux_content_area, open, rect, redraw};
use crate::theme;

// ---- The placement's numbers ----------------------------------------------------------------------------------------

/// The old tests' 1000 by 800 window.
fn viewport() -> gpui::Size<Pixels> {
    size(px(1000.), px(800.))
}

/// A window of `viewport` with no frame and no bar of its own, as the old app's page was and as macOS's and a
/// compositor-decorated Linux window's content area is: the content area is the whole window.
fn whole(viewport: gpui::Size<Pixels>) -> Bounds<Pixels> {
    Bounds::new(point(px(0.), px(0.)), viewport)
}

/// [`place`] for a control at `target` and a card of `width` by `height` in a whole window of `viewport`, beside the
/// sidebar the old cases put the help against, whose right edge is at 396.
fn placed(target: Bounds<Pixels>, width: f32, height: f32, viewport: gpui::Size<Pixels>) -> Placed {
    place(
        px(396.),
        target,
        size(px(width), px(height)),
        whole(viewport),
    )
}

/// A beside placement at `(left, top)` with the notch's center at `notch`.
fn beside(left: f32, top: f32, notch: f32) -> Placed {
    Placed {
        origin: point(px(left), px(top)),
        side: TooltipSide::Beside,
        notch_y: Some(px(notch)),
    }
}

/// A fallback placement at `(left, top)` on `side`.
fn fallback(left: f32, top: f32, side: TooltipSide) -> Placed {
    Placed {
        origin: point(px(left), px(top)),
        side,
        notch_y: None,
    }
}

/// Why this matters: SPEC.md puts session-row help "just right of the sidebar" so it covers no neighbouring row, and
/// the old unit test pinned that it clears the whole sidebar, not just the narrower open button.
///
/// Specification: with the sidebar's right edge at 396, a 310 by 180 card for a row at y 200 to 250 sits at left 402
/// (6 pixels past the edge), top 135 (centered on the row), its notch at 225 (the row's center); a row near the bottom
/// (y 700 to 750) moves up only to keep 4 pixels inside the 800-pixel window, to top 616, its notch still at 725; and
/// an edge left of the window's start keeps the card 4 pixels inside the window's left edge.
#[test]
fn beside_help_clears_the_sidebar_and_centers_on_its_control_inside_the_window() {
    let row = rect(60., 200., 300., 50.);
    assert_eq!(
        placed(row, 310., 180., viewport()),
        beside(402., 135., 225.)
    );
    let low = rect(60., 700., 300., 50.);
    assert_eq!(
        placed(low, 310., 180., viewport()),
        beside(402., 616., 725.)
    );
    let leftmost = place(px(-10.), row, size(px(310.), px(180.)), whole(viewport()));
    assert_eq!(leftmost.origin.x, px(4.));
}

/// Why this matters: the old app chose beside placement only while the card fits 4 pixels inside the window, and kept
/// the ordinary rule exactly otherwise, rather than shifting the card left over the rows it is about. One pixel
/// decides.
///
/// Specification: for the 310 by 180 card beside the 396 edge, a 716-pixel window still places it beside, and a
/// 715-pixel one gives the ordinary placement against the row instead (centered across the row, kept 4 pixels inside:
/// left 55; above it with a 6-pixel gap: top 14), with no notch.
#[test]
fn beside_help_falls_back_one_pixel_short_of_fitting() {
    let row = rect(60., 200., 300., 50.);
    assert_eq!(
        placed(row, 310., 180., size(px(716.), px(800.))),
        beside(402., 135., 225.)
    );
    assert_eq!(
        placed(row, 310., 180., size(px(715.), px(800.))),
        fallback(55., 14., TooltipSide::Above)
    );
}

/// Why this matters: the fallback is the old ordinary tooltip, whose asymmetric gaps keep the pointer off the text
/// (above with a small gap, below only with a gap that clears the pointer's body), and which never leaves the window.
///
/// Specification, in a 600-pixel window where nothing fits beside the 396 edge, for a 310 by 180 card: a row at y 10
/// gets it below with a 28-pixel gap (top 88) and one at y 400 above with a 6-pixel gap (top 214). In the old tests'
/// 1000 by 800 window for a 20-pixel control (the edge moved past the window so that nothing fits beside): a 100 by 30
/// card above is kept while it fits exactly at the 4-pixel margin; a control at the right edge shifts the card left to
/// stay 4 pixels inside; a card wider than the window is pinned 4 pixels from its left. A 300-pixel card that fits on
/// neither side of a control in a 400-pixel window takes the roomier side, clamped 4 pixels inside the window.
#[test]
fn the_fallback_is_the_old_ordinary_placement_against_the_control() {
    let narrow = size(px(600.), px(800.));
    assert_eq!(
        placed(rect(60., 10., 300., 50.), 310., 180., narrow),
        fallback(55., 88., TooltipSide::Below)
    );
    assert_eq!(
        placed(rect(60., 400., 300., 50.), 310., 180., narrow),
        fallback(55., 214., TooltipSide::Above)
    );

    let around = |target: Bounds<Pixels>, width: f32, height: f32, viewport: gpui::Size<Pixels>| {
        place(
            px(2000.),
            target,
            size(px(width), px(height)),
            whole(viewport),
        )
    };
    assert_eq!(
        around(rect(500., 40., 20., 20.), 100., 30., viewport()),
        fallback(460., 4., TooltipSide::Above)
    );
    assert_eq!(
        around(rect(990., 400., 10., 16.), 200., 30., viewport())
            .origin
            .x,
        px(796.)
    );
    assert_eq!(
        around(rect(400., 400., 20., 20.), 1200., 30., viewport())
            .origin
            .x,
        px(4.)
    );
    let short = size(px(1000.), px(400.));
    assert_eq!(
        around(rect(500., 250., 20., 20.), 100., 300., short),
        fallback(460., 4., TooltipSide::Above)
    );
    assert_eq!(
        around(rect(500., 100., 20., 20.), 100., 300., short),
        fallback(460., 96., TooltipSide::Below)
    );
}

/// Why this matters: the notch must point at its control even after the card was clamped, except where the card's
/// rounded corner leaves no straight edge, as the old unit test pinned for text and cards alike.
///
/// Specification: for a 16-pixel control and a 100 by 180 card, the control at y 20 gives top 4 and the notch at the
/// control's center, 28; at y 0 the notch stops 12 pixels below the card's top, at 16; at y 780 it stops 12 pixels
/// above the card's bottom, at 784; and on a 24-pixel card the inset is half its height, so at y 200 the card's top is
/// 196 and the notch 208.
#[test]
fn the_notch_stays_level_with_its_control_on_the_cards_straight_edge() {
    assert_eq!(
        placed(rect(60., 20., 16., 16.), 100., 180., viewport()),
        beside(402., 4., 28.)
    );
    assert_eq!(
        placed(rect(60., 0., 16., 16.), 100., 180., viewport()).notch_y,
        Some(px(16.))
    );
    assert_eq!(
        placed(rect(60., 780., 16., 16.), 100., 180., viewport()).notch_y,
        Some(px(784.))
    );
    assert_eq!(
        placed(rect(60., 200., 16., 16.), 100., 24., viewport()),
        beside(402., 196., 208.)
    );
}

// ---- Inside the Linux window's frame --------------------------------------------------------------------------------

/// Why this matters: on Linux the window holds GPUI Kit's frame and the window's own title bar, and a card placed
/// against the whole window covered the bar and reached into the frame's shadow (the host progress card, seen by
/// settings-and-hosts on 2026-11-15, its top at about y 5). The old rules' window edges are the content area's.
///
/// Specification, in [`linux_content_area`] beside a sidebar whose right edge is at 401: a 310 by 992 card (the tooltip's
/// height cap, the area less 8) for a row centered at y 100 sits at left 407, top 59 (4 pixels below the bar) and
/// bottom 1051 (4 pixels above the frame), its notch still level with the row; a row near the bottom gets the same
/// top; and the same card placed against the whole window would have started at y 4, over the bar.
#[test]
fn a_tall_card_beside_the_sidebar_stays_inside_the_linux_content_area() {
    let area = linux_content_area();
    let card = size(px(310.), area.size.height - px(8.));
    let high = place(px(401.), rect(21., 80., 380., 40.), card, area);
    assert_eq!(high, beside(407., 59., 100.));
    assert!(
        high.origin.y >= area.top() + px(4.)
            && high.origin.y + card.height <= area.bottom() - px(4.),
        "the card is inside the content area: {high:?} in {area:?}"
    );
    let low = place(px(401.), rect(21., 1000., 380., 40.), card, area);
    assert_eq!(low.origin, point(px(407.), px(59.)));
    let whole_window = place(
        px(401.),
        rect(21., 80., 380., 40.),
        card,
        whole(size(px(1642.), px(1076.))),
    );
    assert_eq!(
        whole_window.origin.y,
        px(4.),
        "the premise: the whole window puts it over the bar"
    );
}

/// Why this matters: the fallback is the old ordinary tooltip, which never left the page; above a control near the top
/// of the content area it must not go up over the title bar, but below the control instead, as it would below the top
/// of the old page.
///
/// Specification, in [`linux_content_area`] with nothing fitting beside the sidebar (a 340-pixel card past an edge at 1300):
/// a 340 by 100 card for a control at 40, 120, 320 by 20 goes below it with the 28-pixel gap (left 30, top 168),
/// where the whole window would have put it above, at top 14, over the bar.
#[test]
fn the_fallback_stays_below_the_title_bar() {
    let control = rect(40., 120., 320., 20.);
    let card = size(px(340.), px(100.));
    assert_eq!(
        place(px(1300.), control, card, linux_content_area()),
        fallback(30., 168., TooltipSide::Below)
    );
    assert_eq!(
        place(px(1300.), control, card, whole(size(px(1642.), px(1076.)))),
        fallback(30., 14., TooltipSide::Above),
        "the premise: the whole window puts it over the bar"
    );
}

/// Why this matters: ordinary help sits at the pointer, and the pointer can be on the title bar's own buttons or in the
/// window's bottom-right corner; the help must still land in the content area, and in a window with no frame it must
/// land exactly where GPUI's window snap put it before.
///
/// Specification: help begins 8 pixels right of and 16 below the pointer. In [`linux_content_area`], for a 200 by 60 card, a
/// pointer on the bar at 300, 20 puts it at 308, 55 (the area's top); a pointer at 1630, 1070 puts it flush with the
/// area's right and bottom edges, at 1421, 995; a pointer at 500, 500 leaves it at 508, 516. In a whole 1642 by 1076
/// window the corner pointer puts it flush with the window's edges, at 1442, 1016.
#[test]
fn help_at_the_pointer_stays_inside_the_content_area() {
    let card = size(px(200.), px(60.));
    let at = |x: f32, y: f32, area: Bounds<Pixels>| at_pointer(point(px(x), px(y)), card, area);
    let pointer = |left: f32, top: f32| Placed {
        origin: point(px(left), px(top)),
        side: TooltipSide::Pointer,
        notch_y: None,
    };
    assert_eq!(at(300., 20., linux_content_area()), pointer(308., 55.));
    assert_eq!(at(1630., 1070., linux_content_area()), pointer(1421., 995.));
    assert_eq!(at(500., 500., linux_content_area()), pointer(508., 516.));
    assert_eq!(
        at(1630., 1070., whole(size(px(1642.), px(1076.)))),
        pointer(1442., 1016.)
    );
}

// ---- In a window ----------------------------------------------------------------------------------------------------

/// A view the card form draws: a block of known height that fills the card's width.
struct Summary;

impl Render for Summary {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "summary-view".into())
            .w_full()
            .h(px(120.))
    }
}

/// A sidebar column 380 pixels wide at the scene's 20-pixel inset (its right edge at 400), drawing the beside edge when
/// `edge` is set, holding one button under the selector `target` with `help`, `down` pixels below the column's top.
fn sidebar_scene(
    edge: bool,
    down: Rc<Cell<f32>>,
    help: HoverHelp,
) -> impl Fn(&mut Window, &mut gpui::App) -> gpui::AnyElement {
    move |window, cx| {
        let column = div()
            .debug_selector(|| "sidebar".into())
            .relative()
            .w(px(380.))
            .h(px(600.))
            .pt(px(down.get()))
            .child(Button::new("target", "Row", help.clone(), Role::Button));
        let column = if edge {
            column.child(layers::beside_edge(window, cx))
        } else {
            column
        };
        column.into_any_element()
    }
}

/// Open `scene` in a window `width` wide and 800 high, rest the pointer on the target until its help shows, and draw a
/// frame. The window is sized before the hover: resizing a simulated window under a showing tooltip took it down.
fn hovered(
    cx: &mut TestAppContext,
    width: f32,
    scene: impl Fn(&mut Window, &mut gpui::App) -> gpui::AnyElement + 'static,
) -> &mut VisualTestContext {
    let cx = opened(cx, width, scene);
    let at = center(cx, "target");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    advance(cx, layers::HOVER_DELAY);
    redraw(cx);
    cx
}

/// The showing tooltip; fails when none shows.
fn shown(cx: &mut VisualTestContext) -> layers::ShownTooltip {
    match cx.update(|window, cx| layers::shown_tooltip(window, cx)) {
        Some(shown) => shown,
        None => panic!("no tooltip shows"),
    }
}

/// Whether the element under `selector` was drawn in the last frame.
fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// Why this matters: a control may ask for beside help in a window that draws no sidebar (a dialog's own copy of a
/// row, a test scene); its help must then show where ordinary help does, not vanish or float against nothing.
///
/// Specification: with no edge drawn, the button's beside help shows 8 right and 16 down from where the pointer came to
/// rest, as ordinary help does, with neither notch nor ring, and the tooltip reports [`TooltipSide::Pointer`].
#[gpui::test]
fn beside_help_with_no_edge_drawn_sits_where_ordinary_help_does(cx: &mut TestAppContext) {
    let down = Rc::new(Cell::new(200.));
    let cx = hovered(
        cx,
        1000.,
        sidebar_scene(false, down, HoverHelp::tooltip("Row help").beside()),
    );
    let tooltip = shown(cx);
    assert_eq!(tooltip.side(), TooltipSide::Pointer);
    let at = tooltip.at + point(px(8.), px(16.));
    assert_eq!(
        bounds(cx, "tooltip-card").origin,
        point(at.x.round(), at.y.round())
    );
    assert!(
        !drawn(cx, "tooltip-notch") && !drawn(cx, "tooltip-ring") && tooltip.ring().is_none(),
        "no cues at the pointer"
    );
}

/// `bounds` less the button's 1-pixel border on every side: a tracker covers the box inside its element's border, so
/// that is where the ring goes, as the old ring (an inset shadow) sat inside the target's border.
fn inside_border(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        bounds.origin + point(px(1.), px(1.)),
        size(bounds.size.width - px(2.), bounds.size.height - px(2.)),
    )
}

/// The words of the summary-like card: a session's title, a host value, with a right-to-left override in it that
/// assistive technology must hear escaped, as it would see it on screen.
fn card_words() -> HostSentence {
    HostSentence::new("summary-words")
        .host(&HostText::from_host("Fixture\u{202e}1"))
        .text(", idle")
}

/// The 310-pixel summary-like card help, beside the sidebar.
fn card_help(cx: &mut TestAppContext) -> HoverHelp {
    let summary = cx.new(|_| Summary);
    HoverHelp::card(px(310.), summary, card_words()).beside()
}

/// Why this matters: the session row's summary card is rich content, not words, at the old card's fixed width, and it
/// must sit beside the list in any window wide enough to hold it there, down to the last pixel.
///
/// Specification: a 310-pixel card help beside the sidebar's 400 edge shows beside in a 720-pixel window (406 + 310 is
/// 4 pixels inside), exactly 310 pixels wide, with the caller's view drawn inside it at its own height, and reports the
/// caller's words as its text, with the host value in them escaped as the sentence's accessible name escapes it.
#[gpui::test]
fn card_help_draws_its_view_at_its_width_beside_the_sidebar(cx: &mut TestAppContext) {
    let help = card_help(cx);
    let cx = hovered(
        cx,
        720.,
        sidebar_scene(true, Rc::new(Cell::new(200.)), help),
    );
    let text = shown(cx).text;
    assert_eq!(text, card_words().accessible_name());
    assert!(
        !text.contains('\u{202e}'),
        "the override is escaped: {text:?}"
    );
    assert_eq!(shown(cx).side(), TooltipSide::Beside, "it fits at 720");
    let card = bounds(cx, "tooltip-card");
    assert_eq!(card.size.width, px(310.), "the card's fixed width");
    let view = bounds(cx, "summary-view");
    assert!(
        card.contains(&view.origin) && view.size.height == px(120.),
        "its view inside: {view:?} {card:?}"
    );
}

/// Why this matters: in a window too narrow for the card beside the list, the old app kept the ordinary placement
/// against the control and dropped both cues, which would otherwise point at help that is not beside anything.
///
/// Specification: the same card help in a 719-pixel window, one pixel short, shows above the button with a 6-pixel
/// gap and centered across it (the button spans the column, so the card stays clear of the window's edge), with
/// neither notch nor ring, reporting [`TooltipSide::Above`].
#[gpui::test]
fn card_help_falls_back_without_cues_in_a_window_too_narrow(cx: &mut TestAppContext) {
    let help = card_help(cx);
    let cx = hovered(
        cx,
        719.,
        sidebar_scene(true, Rc::new(Cell::new(200.)), help),
    );
    assert_eq!(shown(cx).side(), TooltipSide::Above);
    let card = bounds(cx, "tooltip-card");
    let target = inside_border(bounds(cx, "target"));
    assert_eq!(
        card.origin,
        point(
            (target.center().x - card.size.width / 2.).round(),
            (target.top() - px(6.) - card.size.height).round()
        )
    );
    assert!(
        !drawn(cx, "tooltip-notch") && !drawn(cx, "tooltip-ring") && shown(cx).ring().is_none(),
        "no cues on the fallback"
    );
}

/// Why this matters: a selected row already wears the accent fill, so an accent ring on its marks would not show; the
/// old app drew it in the on-fill color there, with the control's own corners (an outline on the round status dot).
///
/// Specification: a control whose help asks for the on-fill ring and a 5-pixel radius has its ring painted in the
/// palette's on-fill color with 5-pixel corners, as the painted quad reports it ([`layers::ShownTooltip::ring`]).
#[gpui::test]
fn a_selected_rows_ring_takes_the_on_fill_color_and_its_controls_corners(cx: &mut TestAppContext) {
    let down = Rc::new(Cell::new(200.));
    let help = HoverHelp::tooltip("Row help")
        .ring_on_fill()
        .ring_radius(px(5.));
    let cx = hovered(cx, 1000., sidebar_scene(true, down, help));
    let Some(ring) = shown(cx).ring() else {
        panic!("the premise: a ring was painted");
    };
    let on_fill = cx.update(|_, cx| theme::color(theme::palette(cx).on_fill));
    assert_eq!((ring.color, ring.radius), (on_fill, px(5.)));
}

/// Why this matters: this is SPEC.md's session-row hover as a user sees it: the help beside the list rather than over
/// the rows, a notch pointing back at what it is about and a ring on that control, and the bridge and tests learn where
/// it sits from the tooltip itself. A row can also move under a resting pointer (a re-sort on a status change), and
/// help placed from the previous frame's bounds would sit a frame behind, pointing at where the row was.
///
/// Specification: with the sidebar's edge drawn, a button's beside help shows with its card 6 pixels past the
/// sidebar's right edge, centered on the button; the notch's center is on the card's left edge level with the button's
/// center; the ring is painted just inside the button's border in the accent with square corners; and the tooltip
/// reports [`TooltipSide::Beside`]. When the button then moves 8 pixels down while the pointer stays on it, the very
/// next frame draws the card 8 pixels lower and the notch and ring at the button's new place.
#[gpui::test]
fn beside_help_sits_past_the_sidebar_with_its_cues_and_follows_its_control(
    cx: &mut TestAppContext,
) {
    let down = Rc::new(Cell::new(200.));
    let help = HoverHelp::tooltip("Row help").beside();
    let cx = hovered(cx, 1000., sidebar_scene(true, down.clone(), help));
    assert_eq!(
        shown(cx).text,
        "Row help",
        "the premise: the button's help shows"
    );
    let accent = cx.update(|_, cx| theme::color(theme::palette(cx).accent));
    let check = |cx: &mut VisualTestContext| {
        assert_eq!(shown(cx).side(), TooltipSide::Beside);
        let (sidebar, target) = (bounds(cx, "sidebar"), bounds(cx, "target"));
        let card = bounds(cx, "tooltip-card");
        assert_eq!(
            card.left(),
            sidebar.right() + px(6.),
            "6 pixels past the sidebar"
        );
        assert!(
            (card.center().y - target.center().y).abs() <= px(0.5),
            "centered: {card:?} {target:?}"
        );
        let notch = bounds(cx, "tooltip-notch").center();
        assert!(
            (notch.x - card.left()).abs() < px(0.01),
            "on the card's edge: {notch:?} {card:?}"
        );
        assert!(
            (notch.y - target.center().y).abs() <= px(0.5),
            "level with the button: {notch:?}"
        );
        let ring = shown(cx).ring();
        let ring = ring.map(|ring| (ring.bounds, ring.color, ring.radius));
        assert_eq!(
            ring,
            Some((inside_border(target), accent, px(0.))),
            "the ring inside the border"
        );
        card
    };
    let before = check(cx);
    down.set(208.);
    redraw(cx);
    assert_eq!(
        check(cx).top(),
        before.top() + px(8.),
        "the card moved with the button"
    );
}

/// Why this matters: a row half scrolled out of the list must not have its ring drawn over whatever covers the hidden
/// half (the list's header or footer): the old ring was an inset shadow on the row, clipped with it by the list
/// (app.css line 1900), and here the ring is drawn by the overlay, above everything.
///
/// Specification: a button whose lower part is cut off by a 40-pixel clipping container, hovered on its visible part,
/// has its ring laid out over its whole box but painted inside the container's clip only.
#[gpui::test]
fn the_ring_of_a_control_cut_off_by_its_container_is_painted_only_inside_it(
    cx: &mut TestAppContext,
) {
    let scene = |window: &mut Window, cx: &mut gpui::App| {
        let list = div()
            .debug_selector(|| "list".into())
            .w(px(380.))
            .h(px(40.))
            .pt(px(25.))
            .overflow_hidden()
            .child(Button::new(
                "target",
                "Row",
                HoverHelp::tooltip("Row help").beside(),
                Role::Button,
            ));
        div()
            .relative()
            .w(px(380.))
            .child(list)
            .child(layers::beside_edge(window, cx))
            .into_any_element()
    };
    let cx = open(cx, scene);
    cx.simulate_resize(size(px(1000.), px(800.)));
    redraw(cx);
    let list = bounds(cx, "list");
    let target = bounds(cx, "target");
    assert!(
        target.bottom() > list.bottom(),
        "the premise: the button runs past the list: {target:?} {list:?}"
    );
    cx.simulate_mouse_move(
        point(target.center().x, list.bottom() - px(4.)),
        None,
        Modifiers::none(),
    );
    advance(cx, layers::HOVER_DELAY);
    redraw(cx);
    let Some(ring) = shown(cx).ring() else {
        panic!("the premise: a ring was painted");
    };
    assert_eq!(
        ring.bounds,
        inside_border(target),
        "laid out over the whole button"
    );
    assert!(
        list.contains(&ring.clip.origin) && ring.clip.bottom() <= list.bottom(),
        "painted inside the list's clip: {ring:?} {list:?}"
    );
}

/// Every view a lazy card's builder made, oldest first, each with the host its builder captured, as weak handles, so a
/// test counts builds without keeping a view alive.
type Built = Rc<RefCell<Vec<(u32, WeakEntity<Summary>)>>>;

/// Lazy card help ([`HoverHelp::card_with`]) whose builder captures `host`, as a host progress card's would, and
/// records each view it makes in `built` with that host. Every caller makes its help here, at this one call site.
fn lazy_help(built: &Built, host: u32) -> HoverHelp {
    let record = built.clone();
    let build = move |_: &mut Window, cx: &mut gpui::App| {
        let view = cx.new(|_| Summary);
        record.borrow_mut().push((host, view.downgrade()));
        AnyView::from(view)
    };
    HoverHelp::card_with(px(310.), build, "Host progress")
}

/// A scene like [`sidebar_scene`] whose button's help is a lazy card made anew in every render, as a control's help
/// is, beside the sidebar when `edge` is set.
fn lazy_scene(
    edge: bool,
    built: Built,
) -> impl Fn(&mut Window, &mut gpui::App) -> gpui::AnyElement {
    move |window, cx| {
        let help = lazy_help(&built, 0);
        let help = if edge { help.beside() } else { help };
        let scene = sidebar_scene(edge, Rc::new(Cell::new(200.)), help);
        scene(window, cx)
    }
}

/// Open `scene` in a window `width` wide and 800 high, drawn, with the pointer not yet on its target.
fn opened(
    cx: &mut TestAppContext,
    width: f32,
    scene: impl Fn(&mut Window, &mut gpui::App) -> gpui::AnyElement + 'static,
) -> &mut VisualTestContext {
    let cx = open(cx, scene);
    cx.simulate_resize(size(px(width), px(800.)));
    redraw(cx);
    cx
}

/// Move the pointer onto the element drawn under `selector`, which begins its hover, without waiting for its help.
fn point_at(cx: &mut VisualTestContext, selector: &'static str) {
    let at = center(cx, selector);
    cx.simulate_mouse_move(at, None, Modifiers::none());
}

/// Whether the `index`th view the builder made is still alive.
fn alive(built: &Built, index: usize) -> bool {
    built.borrow()[index].1.upgrade().is_some()
}

/// Why this matters: the host progress card's view holds the ticker's clock and a running step's pulse for as long as
/// it lives, so it must not exist for a row nobody looks at, nor be made again in every frame it shows (which would
/// restart its pulse and leak a view per frame).
///
/// Specification: beside the sidebar, a lazy card's view is not made while the control is drawn, nor while the pointer
/// rests on it until the hover delay ends; it is made once when the tooltip shows, which then sits beside the sidebar
/// with that view drawn on its card, and further frames, each re-rendering the control and refreshing its help, draw
/// the same view and make no other.
#[gpui::test]
fn a_lazy_card_is_made_once_when_its_tooltip_shows_and_kept_while_it_shows(
    cx: &mut TestAppContext,
) {
    let built = Built::default();
    let cx = opened(cx, 1000., lazy_scene(true, built.clone()));
    assert!(built.borrow().is_empty(), "nothing made before the hover");
    point_at(cx, "target");
    advance(cx, layers::HOVER_DELAY - Duration::from_millis(1));
    assert!(
        built.borrow().is_empty(),
        "nothing made while the hover waits"
    );
    advance(cx, Duration::from_millis(1));
    assert_eq!(built.borrow().len(), 1, "made when the tooltip shows");
    for _ in 0..3 {
        redraw(cx);
    }
    assert_eq!(shown(cx).side(), TooltipSide::Beside);
    assert!(drawn(cx, "summary-view"), "its view is drawn on the card");
    assert!(
        built.borrow().len() == 1 && alive(&built, 0),
        "the same view kept across frames"
    );
}

/// Why this matters: a view kept after its tooltip went would keep its clock and pulse running with nothing drawing
/// them, the cost the lazy card exists to avoid; and a view kept from an earlier hover would show stale progress.
///
/// Specification: at the pointer (no sidebar edge), a lazy card's tooltip shows with the view it made; once the pointer
/// leaves and the tooltip hides, that view is gone (its weak handle no longer upgrades); and a new hover on the same
/// control, which shows at once in the grace period a hide leaves, makes a new view.
#[gpui::test]
fn a_lazy_cards_view_goes_with_its_tooltip_and_a_new_hover_makes_another(cx: &mut TestAppContext) {
    let built = Built::default();
    let cx = opened(cx, 1000., lazy_scene(false, built.clone()));
    point_at(cx, "target");
    advance(cx, layers::HOVER_DELAY);
    redraw(cx);
    assert_eq!(shown(cx).side(), TooltipSide::Pointer);
    assert!(
        alive(&built, 0),
        "the premise: the view was made and is drawn"
    );

    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    redraw(cx);
    let hidden = cx
        .update(|window, cx| layers::shown_tooltip(window, cx))
        .is_none();
    assert!(hidden, "the premise: the tooltip hid");
    assert!(!alive(&built, 0), "its view went with it");

    point_at(cx, "target");
    redraw(cx);
    assert!(
        built.borrow().len() == 2 && alive(&built, 1),
        "a new hover made a new view"
    );
}

/// Why this matters: one control can keep its help across a change of what it is about (a row re-keyed in place to
/// another host), and the pointer can rest on a control inside it meanwhile; when the pointer returns, the row's help
/// shows at once from the copy kept while it was hovered. That copy must be the latest render's, or the card is built
/// for the previous host.
///
/// Specification: a row whose lazy card's builder captures host 1, holding a button with help of its own; with the
/// pointer resting on the button (its help showing, no card made), the row is drawn for host 2; moving back onto the
/// row shows its card, built once, for host 2.
#[gpui::test]
fn a_lazy_card_shown_after_its_control_changed_is_built_for_the_latest_render(
    cx: &mut TestAppContext,
) {
    let (built, host) = (Built::default(), Rc::new(Cell::new(1)));
    let (record, current) = (built.clone(), host.clone());
    let scene = move |window: &mut Window, cx: &mut gpui::App| {
        let id: gpui::SharedString = "row".into();
        let row = div()
            .id("row")
            .debug_selector(|| "row".into())
            .w(px(380.))
            .p(px(20.))
            .child(Button::new(
                "inner",
                "Run",
                HoverHelp::tooltip("Inner help"),
                Role::Button,
            ));
        let help = lazy_help(&record, current.get());
        crate::components::with_hover_help(row, &id, &help, false, None, window, cx)
            .into_any_element()
    };
    let cx = opened(cx, 1000., scene);
    point_at(cx, "inner");
    advance(cx, layers::HOVER_DELAY);
    assert_eq!(
        shown(cx).text,
        "Inner help",
        "the premise: the inner help shows"
    );
    host.set(2);
    redraw(cx);
    let row = bounds(cx, "row");
    cx.simulate_mouse_move(row.origin + point(px(4.), px(4.)), None, Modifiers::none());
    redraw(cx);
    assert_eq!(
        shown(cx).text,
        "Host progress",
        "the premise: the row's card shows"
    );
    let hosts: Vec<u32> = built.borrow().iter().map(|(host, _)| *host).collect();
    assert_eq!(hosts, [2], "built once, for the latest host");
}

/// Why this matters: moving from one row to the next replaces the tooltip at once, and the first row's card must not
/// live on with its clock and pulse after another took its place.
///
/// Specification: with two buttons' lazy cards, moving from the first (its card showing) onto the second shows the
/// second's card, built for it, and the first's view is gone.
#[gpui::test]
fn a_lazy_cards_view_goes_when_another_tooltip_replaces_it(cx: &mut TestAppContext) {
    let built = Built::default();
    let record = built.clone();
    let scene = move |_: &mut Window, _: &mut gpui::App| {
        div()
            .flex()
            .flex_col()
            .child(Button::new(
                "target",
                "First",
                lazy_help(&record, 1),
                Role::Button,
            ))
            .child(Button::new(
                "next",
                "Second",
                lazy_help(&record, 2),
                Role::Button,
            ))
            .into_any_element()
    };
    let cx = opened(cx, 1000., scene);
    point_at(cx, "target");
    advance(cx, layers::HOVER_DELAY);
    assert!(alive(&built, 0), "the premise: the first card shows");
    point_at(cx, "next");
    redraw(cx);
    assert!(
        built.borrow().len() == 2 && built.borrow()[1].0 == 2 && alive(&built, 1),
        "the second card replaced it"
    );
    assert!(!alive(&built, 0), "the first card's view is gone");
}

/// Why this matters: a keyboard user reaching a session row or a host's progress must get the same help a pointer
/// user does, beside the list with its cues, and a lazy card must be made for it as for the pointer.
///
/// Specification: Tab onto a control whose lazy card help asks for beside placement shows, after the hover delay, the
/// card beside the sidebar with its ring, made once.
#[gpui::test]
fn keyboard_focus_shows_beside_help_and_makes_its_lazy_card(cx: &mut TestAppContext) {
    let built = Built::default();
    let cx = opened(cx, 1000., lazy_scene(true, built.clone()));
    crate::test_support::keys(cx, "tab");
    redraw(cx);
    advance(cx, layers::HOVER_DELAY);
    redraw(cx);
    assert_eq!(shown(cx).side(), TooltipSide::Beside);
    assert!(shown(cx).ring().is_some(), "the ring shows");
    assert!(
        built.borrow().len() == 1 && alive(&built, 0),
        "the card made once"
    );
}
