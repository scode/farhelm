//! Tests of the golden harness's render budget ([`MAX_SCENE_RENDERS`]), run through the same batch loop the goldens use
//! ([`render_batch`]) but without a renderer, so they need no software Vulkan and run in an ordinary test run.
//!
//! NOTE: These scenes are not in `SCENES` and must never be. A scene that fails on purpose there would fail every
//! golden run.

use super::*;
use std::sync::Arc;

/// [`test_support::headless`] without the renderer: the same bundled fonts, bootstrap and click rule, but nothing
/// behind its windows. Windows still open and draw; only a screenshot would fail, and [`fake_capture`] stands in for
/// that.
fn headless_without_renderer() -> anyhow::Result<gpui::HeadlessAppContext> {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        crate::assets::UI_FONT,
    ));
    let mut cx =
        gpui::HeadlessAppContext::with_asset_source(text_system, Arc::new(crate::assets::Assets));
    cx.update(crate::bootstrap::install)?;
    cx.update(|cx| crate::focus::set_click_focuses_controls(true, cx));
    Ok(cx)
}

/// The same picture every time, so `settle` sees two identical captures and calls a scene settled as soon as its
/// staging is done. What these tests check (whether a scene finishes at all) does not depend on pixels.
fn fake_capture(
    _: &mut gpui::HeadlessAppContext,
    _: gpui::AnyWindowHandle,
) -> anyhow::Result<image::RgbaImage> {
    Ok(image::RgbaImage::new(1, 1))
}

const WITHOUT_RENDERER: Renderer = Renderer {
    headless: headless_without_renderer,
    capture: fake_capture,
};

/// How many frames a re-arming fixture re-arms before it gives up and lets its scene settle: ten times the budget.
///
/// A real re-arming fixture never stops, but this one has to, or a test of a missing budget would hang the test run
/// instead of failing it (and the GUI crates spawn no threads, so a watchdog thread is not an option). Without the
/// budget the scene settles after this many frames and the test fails because it rendered.
const STOP_REARMING_AFTER: u32 = 10 * MAX_SCENE_RENDERS;

/// How many times a re-arming fixture has re-armed in one appearance, kept among the scene's kept values.
struct Rearmed(Cell<u32>);

/// The appearance's [`Rearmed`] counter, if a draw or the scene's setup has made one.
fn rearmed(state: &SceneState) -> Option<Rc<Rearmed>> {
    state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<Rc<Rearmed>>())
        .cloned()
}

/// Make the appearance's [`Rearmed`] counter, which starts the re-arming of [`rearming_once_armed`].
fn arm(state: &SceneState) -> Rc<Rearmed> {
    let rearmed = Rc::new(Rearmed(Cell::new(0)));
    state.keep.borrow_mut().push(Box::new(rearmed.clone()));
    rearmed
}

/// The shape of both fixtures that hung real golden runs on 2026-11-25: the draw leaves work behind that dirties the
/// window again (there, a scroll event or a second press of Launch; here, a plain refresh). GPUI's test mode redraws a
/// dirty window inside its effect flush, so this loops inside a single draw call. It stops after
/// [`STOP_REARMING_AFTER`] frames, which the budget never lets it reach.
fn rearm(rearmed: &Rearmed, window: &mut Window, cx: &mut App) {
    if rearmed.0.get() < STOP_REARMING_AFTER {
        rearmed.0.set(rearmed.0.get() + 1);
        window.defer(cx, |window, _| window.refresh());
    }
}

/// Re-arms from the very first draw, which GPUI makes inside `open_window`, so the budget trips before the scene's
/// setup or `settle` ever run.
fn rearming_from_the_start(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    let rearmed = rearmed(state).unwrap_or_else(|| arm(state));
    rearm(&rearmed, window, cx);
    div().into_any_element()
}

/// Re-arms only once something has armed it; [`arm_in_settle`] does that.
fn rearming_once_armed(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    if let Some(rearmed) = rearmed(state) {
        rearm(&rearmed, window, cx);
    }
    div().into_any_element()
}

/// Arm [`rearming_once_armed`] from a task, so the loop starts in `settle`, where both real hangs spun, and nowhere
/// else: the headless app runs tasks only in `run_until_parked`, and `settle` is the first to call it. That is the
/// case where the budget's panic unwinds out of GPUI while it holds the scene's view, so the harness must still report
/// the budget's message rather than whatever touching that view afterwards panics with.
///
/// It also leaves a scene cleanup ([`panic_after_capture`]) and an app teardown ([`panic_at_quit`]) that panic, as a
/// broken app after a failed render can, so the test shows neither replaces the render's own failure.
fn arm_in_settle(state: &SceneState, window: &mut Window, cx: &mut App) {
    let armed = state.clone();
    let handle = window.window_handle();
    let task = cx.spawn(async move |cx| {
        arm(&armed);
        cx.update_window(handle, |_, window, _| window.refresh())
            .ok();
    });
    state.keep.borrow_mut().push(Box::new(task));
    state
        .after_capture
        .set(Some(panic_after_capture as AfterCapture));
    panic_in_teardown(cx);
}

/// A scene cleanup that fails the way a fixture's own code can.
fn panic_after_capture(_: &SceneState) {
    panic!("the scene's cleanup panicked");
}

/// Make the headless app's teardown panic: shutdown runs its quit observers. Detached so the observer outlives the
/// scene's kept values, which are gone before teardown starts.
fn panic_in_teardown(cx: &mut App) {
    cx.on_app_quit(panic_at_quit).detach();
}

fn panic_at_quit(_: &mut App) -> std::future::Ready<()> {
    panic!("the app's teardown panicked");
}

static FROM_THE_START: Scene = Scene {
    name: "rearming-from-the-start",
    size: (120., 80.),
    draw: rearming_from_the_start,
    setup: no_setup,
};

static IN_SETTLE: Scene = Scene {
    name: "rearming-in-settle",
    size: (120., 80.),
    draw: rearming_once_armed,
    setup: arm_in_settle,
};

/// A scene that renders fine but whose app's teardown panics: that failure is still the appearance's to report.
static BROKEN_TEARDOWN: Scene = Scene {
    name: "broken-teardown",
    size: (120., 80.),
    draw: empty,
    setup: |_, _, cx| panic_in_teardown(cx),
};

/// An ordinary scene that settles, drawn after the re-arming ones in the same batch.
static QUIET: Scene = Scene {
    name: "quiet-fixture",
    size: (120., 80.),
    draw: empty,
    setup: no_setup,
};

/// A fixture that re-arms itself on every frame fails its own appearances with a message naming the scene and the
/// likely cause, and the scene after it in the same batch still renders.
///
/// This is the guard against the hang itself: before the budget, a fixture like this held a renderer at full CPU for
/// hours with no output naming the scene, and every visual PR adds fixtures that could do it again.
///
/// Spec: a batch of [`FROM_THE_START`], [`IN_SETTLE`], [`BROKEN_TEARDOWN`] and [`QUIET`] reports eight appearances in
/// catalog order. Each appearance of the two re-arming scenes fails with the budget's message for its own scene at
/// render `MAX_SCENE_RENDERS + 1`: one trips while its window opens, the other inside `settle`, and the latter's
/// panicking cleanup and teardown do not replace that message. `BROKEN_TEARDOWN`'s appearances fail as teardown
/// failures, and both of `QUIET`'s succeed. Without the budget each re-arming fixture runs until
/// [`STOP_REARMING_AFTER`] and then settles, so the test fails on that appearance rendering instead of hanging.
#[test]
fn a_fixture_that_rearms_every_frame_fails_its_scene_and_the_batch_goes_on() {
    let mut results = Vec::new();
    render_batch(
        &[&FROM_THE_START, &IN_SETTLE, &BROKEN_TEARDOWN, &QUIET],
        &WITHOUT_RENDERER,
        |name, result| results.push((name, result.map(|_| ()))),
    );

    let names: Vec<&str> = results.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "rearming-from-the-start--light.png",
            "rearming-from-the-start--dark.png",
            "rearming-in-settle--light.png",
            "rearming-in-settle--dark.png",
            "broken-teardown--light.png",
            "broken-teardown--dark.png",
            "quiet-fixture--light.png",
            "quiet-fixture--dark.png",
        ]
    );
    let failures = [
        format!(
            "scene {} redrew {} times without settling; a fixture step is probably re-arming itself every \
             frame (see Pending)",
            FROM_THE_START.name,
            MAX_SCENE_RENDERS + 1
        ),
        format!(
            "scene {} redrew {} times without settling; a fixture step is probably re-arming itself every \
             frame (see Pending)",
            IN_SETTLE.name,
            MAX_SCENE_RENDERS + 1
        ),
        "tearing down the scene's headless app: scene panicked: the app's teardown panicked"
            .to_owned(),
    ];
    for (expected, appearances) in failures.iter().zip(results.chunks(2)) {
        for (name, result) in appearances {
            match result {
                Err(error) => assert!(
                    format!("{error:#}").contains(expected.as_str()),
                    "{name} failed with {error:#}, not with {expected:?}"
                ),
                Ok(()) => panic!("{name} rendered, but should have failed with {expected:?}"),
            }
        }
    }
    for (name, result) in &results[6..] {
        assert!(
            result.is_ok(),
            "{name} after the failed scenes did not render: {result:?}"
        );
    }
}
