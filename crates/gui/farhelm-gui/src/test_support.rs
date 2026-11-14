//! The in-process test harness: what every GUI test sets up, and the checks every test step runs.
//!
//! Two kinds of context, because GPUI's test context fakes text. [`TestAppContext`] (with `#[gpui::test]`) gives a
//! deterministic scheduler and simulated input, and is right for behavior: what redraws, where focus goes, which
//! command is sent. But its text system gives every character the same width, so anything about layout (wrapping,
//! truncation, what fits at the minimum window size, the terminal's cell grid) proves nothing there; for those,
//! [`headless`] gives a real text system with the bundled fonts and a real (software, on Linux) renderer, the same
//! setup the screenshot goldens use.
//!
//! [`check_invariants`] is the hook every test step calls. In this phase it checks nothing: GPUI cannot hold two focus
//! handles in one window, so "at most one focused element" needs no check, and the rules that do need one (focus inside
//! the topmost modal, or in an approval card) arrive with the focus coordinator. The tests call it already so those
//! checks run after every existing step the moment they exist.

use std::sync::Arc;

use gpui::{App, HeadlessAppContext, TestAppContext, Window};

use crate::{assets, bootstrap};

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
///
/// NOTE: Empty until the focus coordinator exists; see the module doc. It asserts nothing yet.
pub fn check_invariants(window: &mut Window, cx: &mut App) {
    let _ = (window, cx);
}
