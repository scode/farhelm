//! An app-level count of how often each view rendered, compiled into test builds only.
//!
//! GPUI redraws a window when an entity it read while drawing notifies, but it cannot see a value a view copied out of
//! shared state, nor a change made without a notification. Either goes stale silently, and an uncached whole-window
//! repaint hides it (SPEC_impl.md, "The store, and how views redraw"). Counting renders per view is how tests see what
//! actually redrew: a change must redraw the views that read it, and must not redraw the ones that do not. No GPUI patch
//! is involved; views call [`record`] at the top of `render`.
//!
//! In a shipped build [`record`] compiles to nothing.

use gpui::App;

/// Install the counter, empty. The bootstrap calls this with the rest of the app-wide state, so no view's first render
/// is the one that creates a global; in a shipped build it does nothing.
pub fn install(cx: &mut App) {
    #[cfg(test)]
    cx.set_global(counting::RenderCounts::default());
    #[cfg(not(test))]
    let _ = cx;
}

/// Count one render of the view named `view`. Views call this first thing in `render`.
#[inline]
pub fn record(view: &'static str, cx: &mut App) {
    #[cfg(test)]
    counting::record(view, cx);
    #[cfg(not(test))]
    let _ = (view, cx);
}

/// How many times `view` rendered since the last [`reset`].
#[cfg(test)]
pub fn count(view: &str, cx: &App) -> usize {
    cx.try_global::<counting::RenderCounts>()
        .and_then(|counts| counts.0.get(view).copied())
        .unwrap_or(0)
}

/// Forget every count, so a test measures only what happens after this point.
#[cfg(test)]
pub fn reset(cx: &mut App) {
    cx.default_global::<counting::RenderCounts>().0.clear();
}

/// The counts themselves, compiled into test builds only. Kept last: clippy reads a `cfg(test)` module followed by
/// ordinary items as a misplaced test module.
#[cfg(test)]
mod counting {
    use std::collections::BTreeMap;

    use gpui::{App, Global};

    /// Renders per view name since the counter was last reset.
    #[derive(Default)]
    pub(super) struct RenderCounts(pub(super) BTreeMap<&'static str, usize>);
    impl Global for RenderCounts {}

    /// Count one render of `view`.
    pub(super) fn record(view: &'static str, cx: &mut App) {
        *cx.default_global::<RenderCounts>()
            .0
            .entry(view)
            .or_default() += 1;
    }
}
