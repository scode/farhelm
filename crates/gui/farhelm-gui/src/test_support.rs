//! The in-process test harness: what every GUI test sets up, and the checks every test step runs.
//!
//! Two kinds of context, because GPUI's test context fakes text. [`TestAppContext`] (with `#[gpui::test]`) gives a
//! deterministic scheduler and simulated input, and is right for behavior: what redraws, where focus goes, which
//! command is sent. But its text system gives every character the same width, so anything about layout (wrapping,
//! truncation, what fits at the minimum window size, the terminal's cell grid) proves nothing there; for those,
//! [`headless`] gives a real text system with the bundled fonts and a real (software, on Linux) renderer, the same
//! setup the screenshot goldens use.
//!
//! [`check_invariants`] is the hook every test step calls: the focus coordinator's rules (with a dialog open, focus is
//! inside the topmost one; with a home set, something holds focus) and the layer manager's (every open popover's anchor
//! was drawn). GPUI cannot hold two focused elements in one window, so "exactly one" needs only the "at least one" half,
//! which the rules give wherever SPEC.md requires it.
//!
//! For behavior tests, [`open`] is the harness: a window built the way the app builds its own, and step helpers
//! ([`keys`], [`click`], [`redraw`], [`advance`]) that each end with [`check`], so a test written with them runs the
//! invariant checks after every step without having to remember to. A test that drives the window some other way (a
//! mouse move, a scroll, a resize) calls [`check`] itself after its step. A test that needs a root view this harness
//! cannot give (one with state of its own) should build it through [`root::attach`] and [`root::wrap`], put it inside
//! GPUI Kit's `Root`, activate the window and call [`check`], so that its window and its checks are the same as
//! everyone else's. The self-tests at the end of this file make every step helper prove that it checks.

use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, HeadlessAppContext, IntoElement, Modifiers,
    ParentElement as _, Pixels, Point, Render, Styled as _, TestAppContext, VisualTestContext,
    Window, div, px,
};
use gpui_component::Root;

use crate::{assets, bootstrap, focus, layers, render_count, root};

/// Install the app-wide state a test needs, as the real bootstrap does, on a fake-platform test context.
pub fn install(cx: &mut TestAppContext) {
    cx.update(|cx| {
        if let Err(error) = bootstrap::install(cx) {
            panic!("bootstrap failed in a test: {error:#}");
        }
    });
}

/// A headless app with a real text system that knows only the bundled fonts (so a font installed on the machine can
/// never change a result) and the platform's headless renderer, bootstrapped like the real app.
///
/// A machine without a headless renderer fails at capture time; the screenshot goldens treat that as a failure, not a
/// skip.
pub fn headless() -> anyhow::Result<HeadlessAppContext> {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        Arc::new(assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(bootstrap::install)?;
    Ok(cx)
}

/// The checks every test step runs against `window`, called from inside an update of that window. Panics with what
/// was wrong, so the failing step is the one that broke the invariant, not a later assertion that tripped over its
/// consequences.
pub fn check_invariants(window: &mut Window, cx: &mut App) {
    if let Some(problem) =
        focus::invariant_violation(window, cx).or_else(|| layers::invariant_violation(window, cx))
    {
        panic!("invariant broken: {problem}");
    }
}

// ---- The behavior harness --------------------------------------------------------------------------------------------

/// What a test draws in its window, called on every render of the scene.
type Draw = dyn Fn(&mut Window, &mut App) -> AnyElement;

/// The root view [`open`] draws a test's content in, wired through [`root`] exactly as the main view is: Farhelm's key
/// context and focus actions, the layer stack, the focus coordinator and the overlay on top. The content is laid out as
/// a column from the top-left corner, inside 20 pixels of padding, so a point near the window's corner is empty space a
/// test can move or click to. It records its renders as `"scene"` (see [`render_count`]).
struct Scene {
    draw: Box<Draw>,
    root: root::Attachment,
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("scene", cx);
        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .items_start()
            .p(px(20.))
            .gap(px(8.))
            .child((self.draw)(window, cx));
        root::wrap(content, &self.root, window, cx)
    }
}

/// Open a bootstrapped, active window drawing `draw` in a [`Scene`] inside GPUI Kit's `Root`, as the app does, and
/// check it once its first frame has settled.
///
/// The window is activated because simulated windows start inactive, and an inactive window has no focus path for keys
/// to follow: without it a key test would pass or fail for reasons that have nothing to do with the app. `draw` is
/// called again on every render, so it may read state the test changes later; after a change that notifies nothing (a
/// plain cell), call [`redraw`].
pub fn open(
    cx: &mut TestAppContext,
    draw: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
) -> &mut VisualTestContext {
    install(cx);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let scene = cx.new(|cx| Scene {
            draw: Box::new(draw),
            root: root::attach(window, cx),
        });
        Root::new(scene, window, cx)
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    check(cx);
    cx
}

/// Run the per-step checks ([`check_invariants`]) against the test's window.
pub fn check(cx: &mut VisualTestContext) {
    cx.update(check_invariants);
}

/// Redraw after a change the scene's draw function reads from a plain cell, which notifies nothing, and check.
pub fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    check(cx);
}

/// Press keys, as the platform would deliver them (GPUI's keystroke syntax, space-separated: `"tab shift-tab"`), and
/// check.
pub fn keys(cx: &mut VisualTestContext, keystrokes: &str) {
    cx.simulate_keystrokes(keystrokes);
    check(cx);
}

/// Let `duration` pass on the executor's clock (a pointer resting for the hover delay, a deadline expiring), run
/// whatever that wakes until the app is idle, and check.
pub fn advance(cx: &mut VisualTestContext, duration: Duration) {
    cx.executor().advance_clock(duration);
    cx.run_until_parked();
    check(cx);
}

/// Click the center of the element drawn under `selector`, and check. Panics if nothing was drawn under it.
pub fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = center(cx, selector);
    cx.simulate_click(at, Modifiers::none());
    check(cx);
}

/// Where the element drawn under `selector` (its `debug_selector`, or a component's id) was last drawn. Panics if
/// nothing was, since every caller's next step would otherwise act on a point that is not the element.
pub fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    match cx.debug_bounds(selector) {
        Some(bounds) => bounds,
        None => panic!("{selector} was not drawn"),
    }
}

/// The center of the element drawn under `selector`; see [`bounds`].
pub fn center(cx: &mut VisualTestContext, selector: &'static str) -> Point<Pixels> {
    bounds(cx, selector).center()
}

// ---- Self-tests: every step helper checks ----------------------------------------------------------------------------

/// Proof that each step helper runs the invariant checks. Nothing else would notice a helper losing its [`check`]: the
/// checks fail only when the code under test breaks an invariant, and in a passing suite nothing does. So each test
/// breaks one on purpose, with a popover whose anchor never draws its tracker (which [`layers::invariant_violation`]
/// reports), at a moment when only the step under test can see it: the window is clean when [`open`] checks it, and the
/// popover is opened without a check, or by the step itself. Each test therefore panics in that step's check or not at
/// all.
mod self_tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    use gpui::{
        App, AppContext as _, Context, IntoElement, Render, TestAppContext, VisualTestContext,
        Window, div,
    };

    use super::{advance, click, keys, open, redraw};
    use crate::components::{Button, HoverHelp, Role};
    use crate::focus::Mark;
    use crate::layers::{self, Anchor, Popover};

    /// The anchor the scene drew last, shared with the test and the scene's button.
    type Shared = Rc<RefCell<Option<Anchor>>>;

    /// A popover's content: nothing, since only its anchor matters here.
    struct Empty;

    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    /// Open the harness on a scene holding an anchor whose tracker it never draws, and a button, `opener`, that opens a
    /// popover on that anchor when clicked. With `at_first_draw`, the first frame also opens one (deferred, as no view
    /// may open a layer while it draws), so the window is already broken when [`open`] checks it.
    fn scene(cx: &mut TestAppContext, at_first_draw: bool) -> (&mut VisualTestContext, Shared) {
        let anchor: Shared = Rc::new(RefCell::new(None));
        let (drawn, first) = (anchor.clone(), Cell::new(at_first_draw));
        let cx = open(cx, move |window, cx| {
            drawn.replace(Some(Anchor::for_element("untracked-anchor", window, cx)));
            if first.replace(false) {
                let deferred = drawn.clone();
                window.defer(cx, move |window, cx| open_popover_on(&deferred, window, cx));
            }
            let clicked = drawn.clone();
            Button::new(
                "opener",
                "Open",
                HoverHelp::tooltip("Opens the test popover"),
                Role::Button,
            )
            .on_click(move |_, window, cx| open_popover_on(&clicked, window, cx))
            .into_any_element()
        });
        (cx, anchor)
    }

    /// Open a popover on the scene's untracked anchor. Its premises panic with their own messages, which never match a
    /// test's expected one, so a broken fixture fails its test instead of passing it.
    fn open_popover_on(anchor: &Shared, window: &mut Window, cx: &mut App) {
        let Some(anchor) = anchor.borrow().clone() else {
            panic!("the premise: the scene drew its anchor");
        };
        let view = cx.new(|_| Empty);
        let mark = Mark::take(cx);
        let popover = Popover {
            view: view.into(),
            container: cx.focus_handle(),
            start: cx.focus_handle(),
            anchor,
        };
        if layers::open_popover(popover, &mark, window, cx).is_none() {
            panic!("the premise: the popover opened");
        }
    }

    /// Break the invariant without checking: what a step the harness does not wrap would do.
    fn break_unchecked(cx: &mut VisualTestContext, anchor: &Shared) {
        cx.update(|window, cx| open_popover_on(anchor, window, cx));
    }

    /// [`open`] checks the window once its first frame has settled, so a scene broken from the start fails there
    /// rather than at some later step that tripped over it.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn open_checks_the_first_frame(cx: &mut TestAppContext) {
        scene(cx, true);
    }

    /// [`redraw`] checks after the frame it asked for.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn redraw_checks(cx: &mut TestAppContext) {
        let (cx, anchor) = scene(cx, false);
        break_unchecked(cx, &anchor);
        redraw(cx);
    }

    /// [`keys`] checks after the keys are handled.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn keys_checks(cx: &mut TestAppContext) {
        let (cx, anchor) = scene(cx, false);
        break_unchecked(cx, &anchor);
        keys(cx, "a");
    }

    /// [`click`] checks after the click is handled: here the click itself breaks the invariant.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn click_checks(cx: &mut TestAppContext) {
        let (cx, _) = scene(cx, false);
        click(cx, "opener");
    }

    /// [`advance`] checks after what the clock woke has run.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn advance_checks(cx: &mut TestAppContext) {
        let (cx, anchor) = scene(cx, false);
        break_unchecked(cx, &anchor);
        advance(cx, Duration::from_millis(1));
    }
}
