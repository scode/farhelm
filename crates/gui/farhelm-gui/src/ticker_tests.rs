//! Tests of the ticker: an animating view redraws at most ten times a second and only in an active window, the
//! override and reduced motion, the cursor blink and the running pulse that wake their views only when they change,
//! one-shot deadlines, the wall clock that follows the executor's clock, and the 30-second clock that redraws its views
//! whatever the window's activity and starts again after its last handle went.
//!
//! Each test installs the app-wide state as the real bootstrap does ([`crate::test_support::install`]), and each but
//! the deadline and wall-clock tests opens a window of its own. Time moves only when the test advances GPUI's simulated clock.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{Context, IntoElement, Render, TestAppContext, VisualTestContext, Window, div};

use crate::{render_count, test_support, ticker};

// ---- The ticker ----------------------------------------------------------------------------------------------------

/// A view that animates for as long as it holds the ticker's handle.
struct Spinner {
    animating: Option<ticker::Animating>,
}

impl Render for Spinner {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("spinner", cx);
        div()
    }
}

/// SPEC_impl.md, "Animation time: the ticker": nothing redraws on its own more than 10 times a second, and nothing
/// animates in a window that is not active. Without the shared ticker one animating row redraws its list at the
/// display's rate, and a background window keeps a CPU core busy. Activity comes from GPUI's own window state here (no
/// override), the path the shipped app takes.
#[gpui::test]
fn an_animating_view_redraws_at_most_ten_times_a_second_and_not_while_inactive(
    cx: &mut TestAppContext,
) {
    test_support::install(cx);
    let (spinner, cx) = cx.add_window_view(|window, cx| Spinner {
        animating: Some(ticker::animate(window, cx)),
    });
    // Simulated windows open inactive; activate it the way the platform reports a focused window.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(
        cx.update(|window, _| window.is_window_active()),
        "premise: the window is active"
    );
    cx.update(|_, cx| render_count::reset(cx));

    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    let active = cx.update(|_, cx| render_count::count("spinner", cx));
    assert!(
        (9..=10).contains(&active),
        "about ten redraws in a second while active, never more: {active}"
    );

    cx.deactivate_window();
    assert!(
        !cx.update(|window, _| window.is_window_active()),
        "premise: the window is now inactive"
    );
    cx.update(|_, cx| render_count::reset(cx));
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| render_count::count("spinner", cx)),
        0,
        "no redraws while the window is inactive"
    );

    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(
        cx.update(|window, _| window.is_window_active()),
        "premise: the window is active again"
    );
    cx.update(|_, cx| render_count::reset(cx));
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(
        cx.update(|_, cx| render_count::count("spinner", cx)) > 0,
        "ticks resume once the window is active again"
    );

    spinner.update(cx, |spinner, _| spinner.animating = None);
    cx.update(|_, cx| render_count::reset(cx));
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| render_count::count("spinner", cx)),
        0,
        "dropping the handle stops the ticks"
    );
}

/// The headless Mutter runs of the test-automation bridge report every window inactive forever, so agent builds pin
/// activity on; the override must make an inactive window tick, or nothing would animate in those runs.
#[gpui::test]
fn the_active_override_makes_an_inactive_window_tick(cx: &mut TestAppContext) {
    test_support::install(cx);
    let (_spinner, cx) = cx.add_window_view(|window, cx| Spinner {
        animating: Some(ticker::animate(window, cx)),
    });
    cx.run_until_parked();
    cx.deactivate_window();
    cx.update(|_, cx| {
        ticker::set_active_override(Some(true), cx);
        render_count::reset(cx);
    });
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(
        cx.update(|_, cx| render_count::count("spinner", cx)) > 0,
        "the override counts the inactive window as active"
    );
}

/// Under reduced motion views keep their state but nothing moves on its own: no animation ticks at all, even in an
/// active window, where they would otherwise run.
#[gpui::test]
fn reduced_motion_delivers_no_animation_ticks(cx: &mut TestAppContext) {
    test_support::install(cx);
    cx.update(|cx| ticker::set_reduced_motion(true, cx));
    let (_spinner, cx) = cx.add_window_view(|window, cx| Spinner {
        animating: Some(ticker::animate(window, cx)),
    });
    // Active, so that only reduced motion can be what stops the ticks.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(
        cx.update(|window, _| window.is_window_active()),
        "premise: the window is active"
    );
    cx.update(|_, cx| render_count::reset(cx));
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| render_count::count("spinner", cx)), 0);
}

/// A view whose cursor blinks for as long as it holds the ticker's blink handle.
struct Blinker {
    blinking: Option<ticker::Blinking>,
}

impl Render for Blinker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("blinker", cx);
        div()
    }
}

/// A cursor blink changes twice a second, so its view is woken twice a second, not at every tick: a focused terminal
/// otherwise redraws its window ten times a second for a cursor that changes twice. The handle starts on its shown
/// half and says which half it is in, half a second each (the old cursor's one-second blink); under reduced motion it
/// stays shown and nothing wakes the view.
#[gpui::test]
fn a_blink_wakes_its_view_only_when_it_changes_half(cx: &mut TestAppContext) {
    test_support::install(cx);
    let (blinker, cx) = cx.add_window_view(|window, cx| Blinker {
        blinking: Some(ticker::blink(window, cx)),
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let on = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| blinker.read(cx).blinking.as_ref().unwrap().is_on(cx))
    };
    assert!(on(cx), "a new blink starts shown");
    cx.update(|_, cx| render_count::reset(cx));
    let mut halves = Vec::new();
    for _ in 0..2 * ticker::BLINK_HALF_TICKS {
        cx.executor().advance_clock(ticker::TICK);
        cx.run_until_parked();
        halves.push(on(cx));
    }
    let half = ticker::BLINK_HALF_TICKS as usize;
    assert_eq!(&halves[..half - 1], vec![true; half - 1]);
    assert_eq!(&halves[half - 1..2 * half - 1], vec![false; half]);
    assert!(halves[2 * half - 1], "shown again after a whole blink");
    assert_eq!(
        cx.update(|_, cx| render_count::count("blinker", cx)),
        2,
        "woken once per change of half"
    );

    cx.update(|_, cx| ticker::set_reduced_motion(true, cx));
    assert!(
        on(cx),
        "shown under reduced motion, whichever half it was in"
    );
}

/// A view that draws the running pulse for as long as it holds the ticker's pulse handle.
struct Pulser {
    pulsing: ticker::Pulsing,
}

impl Render for Pulser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("pulser", cx);
        div()
    }
}

/// The running pulse is the old app's two-second, eight-step `farhelm-status-pulse`, as one phase for the whole app,
/// and a view is woken only when that phase steps: four times a second rather than at every tick. Specified: from a
/// fresh ticker, the step read after each of a cycle's 20 ticks is 0, 0, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6, 6,
/// 7, 7, 0 (steps of 300 and 200 ms in turn, since 250 ms steps do not land on 100 ms ticks), and the view is woken
/// eight times; under reduced motion the step reads 0.
#[gpui::test]
fn a_pulse_steps_at_one_shared_phase_and_wakes_its_view_once_per_step(cx: &mut TestAppContext) {
    test_support::install(cx);
    let (pulser, cx) = cx.add_window_view(|window, cx| Pulser {
        pulsing: ticker::pulse(window, cx),
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let step = |cx: &mut VisualTestContext| cx.update(|_, cx| pulser.read(cx).pulsing.step(cx));
    assert_eq!(
        step(cx),
        0,
        "premise: a fresh ticker starts at full brightness"
    );
    cx.update(|_, cx| render_count::reset(cx));
    let mut cycle = Vec::new();
    for _ in 0..ticker::PULSE_CYCLE_TICKS {
        cx.executor().advance_clock(ticker::TICK);
        cx.run_until_parked();
        cycle.push(step(cx));
    }
    assert_eq!(
        cycle,
        [0, 0, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6, 6, 7, 7, 0]
    );
    assert_eq!(
        cx.update(|_, cx| render_count::count("pulser", cx)),
        ticker::PULSE_STEPS as usize,
        "woken once per step"
    );

    cx.executor().advance_clock(ticker::TICK * 3);
    cx.run_until_parked();
    assert_eq!(step(cx), 1, "premise: the pulse is off full brightness");
    cx.update(|_, cx| ticker::set_reduced_motion(true, cx));
    assert_eq!(step(cx), 0, "still under reduced motion");
}

/// One-shot deadlines are about the user's time, not drawing: they fire on time even with every window inactive, and
/// dropping one cancels it (how a hover that ends cancels its tooltip).
#[gpui::test]
fn a_deadline_fires_on_time_whatever_the_activity_and_dropping_it_cancels_it(
    cx: &mut TestAppContext,
) {
    test_support::install(cx);
    cx.update(|cx| ticker::set_active_override(Some(false), cx));
    let fired = Rc::new(Cell::new(false));
    let cancelled = Rc::new(Cell::new(false));
    let (on_fire, on_cancelled) = (fired.clone(), cancelled.clone());
    let _kept =
        cx.update(|cx| ticker::after(Duration::from_millis(500), cx, move |_| on_fire.set(true)));
    let dropped = cx.update(|cx| {
        ticker::after(Duration::from_millis(500), cx, move |_| {
            on_cancelled.set(true)
        })
    });
    drop(dropped);

    cx.executor().advance_clock(Duration::from_millis(499));
    cx.run_until_parked();
    assert!(!fired.get(), "not before its time");
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert!(fired.get(), "on time, with no window active");
    assert!(!cancelled.get(), "a dropped deadline never fires");
}

// ---- Wall-clock time -------------------------------------------------------------------------------------------------

/// Ages are drawn from `ticker::wall_now`, so a test can only check an age if that clock is one it controls: a reading
/// of the real system clock would make every age a test asserts depend on how long the test took. Specified: across a
/// stretch of the executor's simulated clock, the wall clock moves by exactly that stretch, and it stands still while
/// the executor's clock does.
#[gpui::test]
fn the_wall_clock_follows_the_executors_clock(cx: &mut TestAppContext) {
    test_support::install(cx);
    let start = cx.update(|cx| ticker::wall_now(cx));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| ticker::wall_now(cx)),
        start,
        "no time passes while the test does not advance the clock"
    );
    cx.executor().advance_clock(Duration::from_secs(90));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| ticker::wall_now(cx)),
        start + Duration::from_secs(90)
    );
}

/// A view that shows an age for as long as it holds the ticker's clock handle.
struct Ager {
    _clock: ticker::Clock,
}

impl Render for Ager {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("ager", cx);
        div()
    }
}

/// Whether GPUI's simulated scheduler holds any task or timer, after the test has run everything runnable: the
/// evidence that a periodic loop ended rather than waiting on its next wake-up.
/// Takes the executor rather than a context, so a test can ask with or without a window of its own.
fn scheduler_holds_work(executor: &gpui::BackgroundExecutor) -> bool {
    executor
        .scheduler_executor()
        .scheduler()
        .as_test()
        .expect("GPUI tests run on the simulated scheduler")
        .has_pending_tasks()
}

/// A session row's age must keep moving in a window the user is not typing into (it is often still on screen) and
/// under reduced motion (an age is information, not motion), and the old app re-rendered ages every 30 seconds; but a
/// view that is gone must leave no timer behind, or every closed window would keep the app waking twice a minute.
/// Specified: with the window inactive and reduced motion on, a view holding a `Clock` is not redrawn before a whole
/// `CLOCK_PERIOD` has passed and is redrawn once per period after that; once the view is dropped with its window, the
/// clock loop ends at its next wake-up and leaves no task or timer in the scheduler.
#[gpui::test]
fn a_clock_redraws_its_view_every_period_in_an_inactive_window_until_the_view_goes(
    cx: &mut TestAppContext,
) {
    test_support::install(cx);
    cx.update(|cx| ticker::set_reduced_motion(true, cx));
    let (ager, cx) = cx.add_window_view(|_, cx| Ager {
        _clock: ticker::clock(cx),
    });
    cx.run_until_parked();
    cx.deactivate_window();
    assert!(
        !cx.update(|window, cx| ticker::is_active(window, cx)),
        "premise: the window counts as inactive"
    );
    cx.update(|_, cx| render_count::reset(cx));

    let count = |cx: &mut VisualTestContext| cx.update(|_, cx| render_count::count("ager", cx));
    cx.executor()
        .advance_clock(ticker::CLOCK_PERIOD - Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(count(cx), 0, "not before a whole period");
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(count(cx), 1, "once the period ends");
    for redraws in 2..=3 {
        cx.executor().advance_clock(ticker::CLOCK_PERIOD);
        cx.run_until_parked();
        assert_eq!(count(cx), redraws, "once more per period");
    }

    assert!(
        scheduler_holds_work(&cx.executor()),
        "premise: the clock loop waits on its next period while the view lives"
    );
    let gone = ager.downgrade();
    drop(ager);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
    assert!(
        gone.upgrade().is_none(),
        "premise: the view was dropped with its window"
    );
    cx.executor().advance_clock(ticker::CLOCK_PERIOD);
    cx.run_until_parked();
    assert!(
        !scheduler_holds_work(&cx.executor()),
        "the clock loop ended at its first wake-up after the view went, leaving no timer"
    );
}

/// The clock loop ends once its last handle is gone, so the next handle must start a new one: closing the only window
/// that shows ages and opening another (a macOS app keeps running with no window) would otherwise freeze every age in
/// the new window for good. Specified: after the only `Clock` went with its window and the loop ended (the scheduler
/// holds no work), a view in a new window that takes a `Clock` is redrawn once when the next `CLOCK_PERIOD` ends.
#[gpui::test]
fn a_clock_starts_again_after_its_last_handle_went(cx: &mut TestAppContext) {
    test_support::install(cx);
    let first = cx.add_window(|_, cx| Ager {
        _clock: ticker::clock(cx),
    });
    cx.run_until_parked();
    first
        .update(cx, |_, window, _| window.remove_window())
        .expect("premise: the first window is open");
    cx.run_until_parked();
    cx.executor().advance_clock(ticker::CLOCK_PERIOD);
    cx.run_until_parked();
    assert!(
        !scheduler_holds_work(&cx.executor()),
        "premise: the clock loop ended with its last handle"
    );

    let _second = cx.add_window(|_, cx| Ager {
        _clock: ticker::clock(cx),
    });
    cx.run_until_parked();
    cx.update(render_count::reset);
    cx.executor().advance_clock(ticker::CLOCK_PERIOD);
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| render_count::count("ager", cx)),
        1,
        "a new loop redrew the new view when its period ended"
    );
}
