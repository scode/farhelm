//! Tests of the stepped spinner's turn (`components/spinner.rs`): one step per animation tick, round in eight; standing
//! still under reduced motion; and silent once the view drops it, as the ticker's rules ask of every animation.
//!
//! The glyph's look at each step is pinned by the goldens of the views that draw it, not here. Each test opens a view
//! that holds a turn ([`Holder`]) through the shared harness, and moves time only by advancing GPUI's simulated
//! clock, so the ticks a test sees are exactly the ticks it let pass.

use gpui::{Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext, Window, div};

use crate::components::Spinner;
use crate::test_support::{advance, install, open_view};
use crate::{render_count, ticker};

/// A view that holds a spinner's turn, as the session list or the version readout does while something spins, and
/// counts its own renders so a test can tell a redraw the turn caused from none.
struct Holder {
    spinner: Option<Spinner>,
}

impl Render for Holder {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("spinner-holder", cx);
        div()
    }
}

/// Open a window whose root view holds a turn started as the window opens, with the reduced-motion setting given, and
/// return the view with the window's context. The harness activates the window, so only the setting can stop ticks.
fn open_holder(
    reduced_motion: bool,
    cx: &mut TestAppContext,
) -> (Entity<Holder>, &mut VisualTestContext) {
    install(cx);
    cx.update(|cx| ticker::set_reduced_motion(reduced_motion, cx));
    let (holder, cx) = open_view(cx, |window, cx| Holder {
        spinner: Some(Spinner::start(window, cx)),
    });
    assert!(
        cx.update(|window, _| window.is_window_active()),
        "the premise: the window is active"
    );
    (holder, cx)
}

/// The step the holder's spinner shows now. Panics if the holder dropped its spinner.
fn step(holder: &Entity<Holder>, cx: &mut VisualTestContext) -> u8 {
    cx.update(|_, cx| match &holder.read(cx).spinner {
        Some(spinner) => spinner.step(cx),
        None => panic!("the holder holds no spinner"),
    })
}

/// How many times the holder has rendered since the last reset.
fn renders(cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| render_count::count("spinner-holder", cx))
}

/// The old `.delete-spinner` turned in eight 45-degree jumps per 0.8-second turn (`steps(8)`), which at the ticker's
/// ten ticks a second is one jump per tick; a turn that skipped or repeated steps would read as a stutter, and one that
/// counted past eight would draw past a full turn. Specified: a fresh turn shows step 0; each tick moves it one step
/// and redraws the view that holds it once; after the eighth step it is back at 0 and keeps going.
#[gpui::test]
fn a_spinner_moves_one_step_per_tick_and_comes_round_after_eight(cx: &mut TestAppContext) {
    let (holder, cx) = open_holder(false, cx);
    assert_eq!(
        step(&holder, cx),
        0,
        "a fresh turn starts at its first step"
    );
    for tick in 1..=10u8 {
        cx.update(|_, cx| render_count::reset(cx));
        advance(cx, ticker::TICK);
        assert_eq!(step(&holder, cx), tick % 8, "after tick {tick}");
        assert_eq!(renders(cx), 1, "tick {tick} redraws the holder once");
    }
}

/// SPEC.md asks for no motion under the operating system's reduced-motion setting, and the old stylesheet's
/// `prefers-reduced-motion` rule set the spinner's animation to `none`: the glyph stays where it starts and the words
/// beside it say what is happening. Specified: under reduced motion a held turn stays at step 0 over a second in an
/// active window, and never redraws its holder.
#[gpui::test]
fn a_spinner_stands_still_under_reduced_motion(cx: &mut TestAppContext) {
    let (holder, cx) = open_holder(true, cx);
    cx.update(|_, cx| render_count::reset(cx));
    advance(cx, ticker::TICK * 10);
    assert_eq!(step(&holder, cx), 0);
    assert_eq!(renders(cx), 0, "nothing redraws the holder");
}

/// A turn keeps the ticker's timer firing ten times a second, so views drop theirs once nothing spins or their window
/// goes inactive (SPEC_impl.md, "Animation time: the ticker"). A turn that outlived its handle would not redraw the
/// view, since the subscription that redraws it goes with the handle, but it would keep the timer firing ten times a
/// second for as long as the app runs, which only the tick count shows. Specified: while held, the turn redraws its
/// holder; once the holder drops it and one tick has let the loop find nobody left, a second passes with no redraw and
/// no tick at all. The holder is the only animating view here, so a tick count that stands still proves the turn's
/// animation handle is gone.
#[gpui::test]
fn a_dropped_spinner_stops_redrawing_its_view_and_ends_the_ticks(cx: &mut TestAppContext) {
    let (holder, cx) = open_holder(false, cx);
    cx.update(|_, cx| render_count::reset(cx));
    let before = cx.update(|_, cx| ticker::ticks(cx));
    advance(cx, ticker::TICK * 2);
    assert!(
        renders(cx) > 0,
        "the premise: a held turn redraws its holder"
    );
    assert!(
        cx.update(|_, cx| ticker::ticks(cx)) > before,
        "the premise: a held turn keeps the ticks coming"
    );

    holder.update(cx, |holder, _| holder.spinner = None);
    advance(cx, ticker::TICK);
    cx.update(|_, cx| render_count::reset(cx));
    let before = cx.update(|_, cx| ticker::ticks(cx));
    advance(cx, ticker::TICK * 10);
    assert_eq!(renders(cx), 0, "no redraw once the turn is dropped");
    assert_eq!(
        cx.update(|_, cx| ticker::ticks(cx)),
        before,
        "no tick once the turn is dropped"
    );
}
