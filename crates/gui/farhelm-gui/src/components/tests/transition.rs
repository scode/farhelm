//! Tests of the one-shot entrance (`components/transition.rs`): it follows CSS's `ease-out` to the element's own
//! opacity, asks for no frame once it ends, starts opaque under reduced motion, and in an inactive window ends on time
//! without playing again when the window is activated.
//!
//! These drive GPUI's animation the way the platform would: the executor's clock moves only when a test advances it,
//! and a display frame arrives only when a test delivers one ([`next_frame`]), so a test sees each frame the entrance
//! asked for and can prove that, once it ended, it asked for none.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    IntoElement, ParentElement as _, Styled, TestAppContext, VisualTestContext, Window, div,
};

use crate::components::transition::css_ease_out;
use crate::components::{ENTRANCE, entrance};
use crate::test_support::{advance, open, redraw, settle};
use crate::ticker;

/// A box that records the opacity each frame drew it with, so a test reads what an entrance did to the element it was
/// given rather than the entrance's own arithmetic. It lays out with the default style and paints nothing; an opacity
/// left unset is recorded as 1, which is how GPUI draws it.
struct OpacityProbe {
    style: gpui::StyleRefinement,
    seen: Rc<RefCell<Vec<f32>>>,
}

impl OpacityProbe {
    fn new(seen: &Rc<RefCell<Vec<f32>>>) -> Self {
        OpacityProbe {
            style: gpui::StyleRefinement::default(),
            seen: seen.clone(),
        }
    }
}

impl Styled for OpacityProbe {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.style
    }
}

impl IntoElement for OpacityProbe {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for OpacityProbe {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, ()) {
        self.seen
            .borrow_mut()
            .push(self.style.opacity.unwrap_or(1.));
        (window.request_layout(gpui::Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: gpui::Bounds<gpui::Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut gpui::App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: gpui::Bounds<gpui::Pixels>,
        _: &mut (),
        _: &mut (),
        _: &mut Window,
        _: &mut gpui::App,
    ) {
    }
}

/// The opacity a probe was last drawn with; NaN, which fails every comparison, if it was never drawn.
fn last_opacity(seen: &Rc<RefCell<Vec<f32>>>) -> f32 {
    seen.borrow().last().copied().unwrap_or(f32::NAN)
}

/// Deliver the next display frame, as the platform's frame source would, let the redraw it causes settle, and return
/// how many frame requests were waiting for it: what an animation asks for to draw again. The test platform has no
/// frame source, so without this an animation never gets past the frame it was first drawn in.
fn next_frame(cx: &mut VisualTestContext) -> usize {
    let waiting = cx.update(|window, cx| window.simulate_next_frame(cx));
    settle(cx);
    waiting
}

/// The share of a change CSS's `ease-out` makes at a point in time, from its definition, `cubic-bezier(0, 0, 0.58, 1)`.
/// The old pill's entrance named that curve, and a port that used GPUI's linear default or one of its own easings would
/// fade at a visibly different pace. Specified: the ends are exactly 0 and 1, and the quarter points match values
/// computed apart from this code (solving the Bézier to double precision): 0.3781, 0.6846 and 0.9065.
#[test]
fn the_entrance_curve_is_css_ease_out() {
    assert_eq!(css_ease_out(0.), 0.);
    assert_eq!(css_ease_out(1.), 1.);
    for (time, share) in [(0.25, 0.3781), (0.5, 0.6846), (0.75, 0.9065)] {
        let got = css_ease_out(time);
        assert!(
            (got - share).abs() < 1e-3,
            "at {time} of the time, {share} of the change, not {got}"
        );
    }
}

/// SPEC.md's quick-launch pill appears with a short fade, the old `launch-sequence-enter 200ms ease-out`. The fade must
/// end where the element would be without it, or the pill stays see-through. Specified: an element drawn through
/// `entrance` starts transparent, is about two thirds of the way in at half the time (the curve's 0.68, give or take
/// the test clock's jitter, against 0.5 for a linear fade), and ends at exactly its own opacity: fully opaque for an
/// element that sets none, half for one that sets half.
#[gpui::test]
fn an_entrance_fades_in_on_the_old_curve_and_ends_at_the_elements_own_opacity(
    cx: &mut TestAppContext,
) {
    let plain: Rc<RefCell<Vec<f32>>> = Rc::default();
    let dimmed: Rc<RefCell<Vec<f32>>> = Rc::default();
    let (plain_seen, dimmed_seen) = (plain.clone(), dimmed.clone());
    let cx = open(cx, move |_, cx| {
        div()
            .child(entrance("plain", OpacityProbe::new(&plain_seen), cx))
            .child(entrance(
                "dimmed",
                OpacityProbe::new(&dimmed_seen).opacity(0.5),
                cx,
            ))
            .into_any_element()
    });
    assert!(
        last_opacity(&plain) < 0.05 && last_opacity(&dimmed) < 0.05,
        "both start transparent: {} and {}",
        last_opacity(&plain),
        last_opacity(&dimmed)
    );

    advance(cx, ENTRANCE / 2);
    assert!(
        next_frame(cx) > 0,
        "halfway, the entrance still asks for frames"
    );
    assert!(
        (last_opacity(&plain) - 0.685).abs() < 0.05,
        "halfway, on the ease-out curve: {}",
        last_opacity(&plain)
    );
    assert!(
        (last_opacity(&dimmed) - 0.5 * 0.685).abs() < 0.05,
        "the dimmed element fades towards its own opacity: {}",
        last_opacity(&dimmed)
    );

    advance(cx, ENTRANCE);
    next_frame(cx);
    assert_eq!(last_opacity(&plain), 1., "the entrance ends fully opaque");
    assert_eq!(
        last_opacity(&dimmed),
        0.5,
        "and a dimmed element at its own opacity"
    );
}

/// SPEC.md: the pill's entrance never loops, and SPEC_impl.md allows GPUI's animation only for one-shot transitions,
/// because an animation that keeps asking for frames redraws the window at the display's rate for as long as it is
/// drawn. Specified: once the entrance's time has passed, the frame it asked for draws it opaque and it asks for no
/// further frame; a later redraw, long after, draws it opaque again without starting over and still asks for none.
#[gpui::test]
fn an_entrance_asks_for_no_frame_once_it_has_ended(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<f32>>> = Rc::default();
    let probe = seen.clone();
    let cx = open(cx, move |_, cx| {
        entrance("pill", OpacityProbe::new(&probe), cx).into_any_element()
    });
    assert!(
        last_opacity(&seen) < 0.05,
        "the premise: the entrance has started"
    );

    advance(cx, ENTRANCE * 2);
    assert!(
        next_frame(cx) > 0,
        "the premise: the entrance asked for a frame while it ran"
    );
    assert_eq!(last_opacity(&seen), 1., "drawn opaque once its time is up");
    assert_eq!(next_frame(cx), 0, "and it asks for no frame after that");

    advance(cx, Duration::from_secs(5));
    redraw(cx);
    assert_eq!(
        last_opacity(&seen),
        1.,
        "a later redraw does not start it over"
    );
    assert_eq!(next_frame(cx), 0, "nor ask for frames again");
}

/// SPEC.md: the pill's entrance is off under reduced motion, as the old stylesheet's `animation: none` under
/// `prefers-reduced-motion` made it. The setting is the one the host hands the ticker. GPUI's own flag, which its
/// animations read, is unset in tests (GPUI Kit writes it only outside the test scheduler), so an entrance that left
/// the decision to GPUI would still fade here, which is what makes this test see whether the ticker is asked.
/// Specified: with the ticker's reduced motion on, an element drawn through `entrance` is opaque in its very first
/// frame and asks for no frame, and stays so as time passes.
#[gpui::test]
fn under_reduced_motion_an_entrance_starts_opaque_and_asks_for_no_frame(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<f32>>> = Rc::default();
    let shown = Rc::new(Cell::new(false));
    let (probe, showing) = (seen.clone(), shown.clone());
    let cx = open(cx, move |_, cx| {
        div()
            .when(showing.get(), |this| {
                this.child(entrance("pill", OpacityProbe::new(&probe), cx))
            })
            .into_any_element()
    });
    cx.update(|_, cx| ticker::set_reduced_motion(true, cx));
    assert!(
        cx.update(|_, cx| ticker::reduced_motion(cx)),
        "the premise: the ticker reports reduced motion"
    );
    assert!(
        cx.update(|_, cx| !cx.reduce_motion()),
        "the premise: GPUI's own flag is off"
    );

    shown.set(true);
    redraw(cx);
    assert_eq!(
        seen.borrow().as_slice(),
        [1.],
        "opaque in the first frame it is drawn in"
    );
    assert_eq!(next_frame(cx), 0, "with no frame asked for");

    advance(cx, ENTRANCE / 2);
    redraw(cx);
    assert_eq!(
        last_opacity(&seen),
        1.,
        "and still opaque partway through its time"
    );
}

/// An entrance that starts in an inactive window must still end on time, and must not play again when the window is
/// activated. The old app paused every animation in an inactive window, and its stylesheet notes the trap that came
/// with it: an entrance that starts there sits on its first, invisible frame until the user comes back. Showing the
/// final state at once while inactive and animating once active would have the opposite fault, an element that goes
/// from shown to transparent as the window is activated. Specified: in a window the ticker counts as inactive, an
/// element drawn through `entrance` starts transparent; after the entrance's time, with no frame drawn in between, the
/// next redraw draws it opaque and it asks for no further frame; activating the window and redrawing leaves it opaque,
/// with no frame asked for.
#[gpui::test]
fn in_an_inactive_window_an_entrance_ends_on_time_and_does_not_replay_on_activation(
    cx: &mut TestAppContext,
) {
    let seen: Rc<RefCell<Vec<f32>>> = Rc::default();
    let shown = Rc::new(Cell::new(false));
    let (probe, showing) = (seen.clone(), shown.clone());
    let cx = open(cx, move |_, cx| {
        div()
            .when(showing.get(), |this| {
                this.child(entrance("pill", OpacityProbe::new(&probe), cx))
            })
            .into_any_element()
    });
    cx.deactivate_window();
    assert!(
        !cx.update(|window, cx| ticker::is_active(window, cx)),
        "the premise: the window counts as inactive"
    );

    shown.set(true);
    redraw(cx);
    assert!(
        last_opacity(&seen) < 0.05,
        "the entrance starts transparent there too: {}",
        last_opacity(&seen)
    );

    advance(cx, ENTRANCE + ENTRANCE / 4);
    redraw(cx);
    assert_eq!(
        last_opacity(&seen),
        1.,
        "once its time is up the next draw is opaque, though no frame came in between"
    );
    next_frame(cx);
    assert_eq!(next_frame(cx), 0, "and it asks for no frame after that");

    cx.update(|window, _| window.activate_window());
    redraw(cx);
    assert!(
        cx.update(|window, cx| ticker::is_active(window, cx)),
        "the premise: the window is active again"
    );
    assert_eq!(
        last_opacity(&seen),
        1.,
        "activating the window does not play the entrance again"
    );
    assert_eq!(next_frame(cx), 0, "nor make it ask for frames");
}
