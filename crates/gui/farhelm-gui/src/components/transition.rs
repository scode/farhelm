//! One-shot transitions: the one place in the GUI crates' own code that draws with GPUI's animation (SPEC_impl.md,
//! "Animation time: the ticker"). GPUI Kit's internals animate too, inside the dependency, where the GUI crates' lint
//! bans do not reach.
//!
//! NOTE: This is not an animation framework. It holds the one transition an area needs today, the fade the quick-launch
//! pill makes as it appears ([`entrance`]), and a new one is added here only when SPEC.md asks for it.
//!
//! Everything else that moves goes through the ticker, at most ten redraws a second and none in an inactive window. A
//! one-shot entrance cannot: the old pill's was a 200 ms CSS animation at the display's rate, and at the ticker's
//! 100 ms ticks it would be two or three drawn frames (its first, one step, its last), which reads as a jump rather
//! than a fade, where a 60 Hz display draws 12 and a 120 Hz one 24. A ticker handle also carries no elapsed time to
//! draw from, and an inactive window gets no ticks, so an entrance started there would stay on its first, invisible
//! frame until something else redrew the window: the trap the old stylesheet's note on paused animations warns about.
//! GPUI's animation has neither problem. It works out its value from the executor's clock each time it draws, so it
//! shows the state for the time that has passed whenever the window draws, it ends on its final state on time whether
//! or not the window was active or drew in between, and it asks for no frame once it ends. The cost is a burst of
//! frames at the display's rate for 200 ms, bounded and never repeating, which is why the ten-a-second rule's exception
//! covers one-shot transitions and nothing that loops.
//!
//! The rules this module keeps, and why they live in one place (`crates/gui/clippy.toml` bans GPUI's animation calls
//! everywhere else in the GUI crates):
//!
//! - It never loops on its own. Callers pass no GPUI `Animation`, only an element, so none is told to repeat. An id
//!   that changes every frame defeats this, though: each frame is then a new entrance (see [`entrance`]).
//! - Under reduced motion the element is drawn in its final state at once and no frame is asked for. The setting comes
//!   from the ticker ([`crate::ticker::reduced_motion`]), not from GPUI's own flag, so that all of the motion the app's
//!   own code draws follows one reading: the host's, read once at startup and handed to the ticker, which tests can
//!   drive. GPUI's flag holds another: GPUI Kit's init writes its own reading of the platform into it (on Linux from
//!   the desktop portal, followed live), which can differ from the host's, and under the test scheduler nothing writes
//!   it at all. Kit's own motion (its text input's overscroll bounce, say) follows that flag, not the ticker.
//! - An inactive window gets the same transition as an active one. It is not paused, because a paused entrance is an
//!   invisible one, and it is not skipped, because skipping it there and drawing it on activation would replay it: the
//!   element would go from shown to transparent when the user came back.
//!
//! An entrance starts the first time its element is drawn under its id, and it plays again only after a frame that did
//! not draw it, as a CSS animation plays again when its element is put back into the page.
//!
//! NOTE: Only the fade is drawn. The old entrance also scaled the pill up from 96%, and GPUI cannot scale an element:
//! its scene transforms only sprites (an SVG, a glyph), never quads or shadows, so a real scale needs a GPUI patch.
//! Animating the text size in its place would be wrong twice over, since it re-lays out the text every frame and
//! rasterizes each in-between size into a glyph atlas that never evicts (SPEC_impl.md, "GPUI cookbook: traps").

use std::time::Duration;

use gpui::{Animation, AnimationExt as _, AnyElement, App, ElementId, IntoElement, Styled};

use crate::ticker;

/// How long an entrance lasts: the old pill's `launch-sequence-enter 200ms ease-out` (app.css at the reference commit).
pub const ENTRANCE: Duration = Duration::from_millis(200);

/// Draw `element` fading in when it appears: from transparent to its own opacity over [`ENTRANCE`], on CSS's `ease-out`
/// curve, as the old quick-launch pill did. Under reduced motion it is drawn as given, with no transition.
///
/// `id` names the entrance among its siblings and must stay the same from frame to frame, because the transition's
/// start is kept in GPUI's per-element state under it; a new id is a new entrance. A caller whose id changes every
/// frame (one built from a counter or a timestamp) therefore never finishes the entrance: the element stays transparent
/// and asks for a frame on every display frame for as long as it is drawn. Key it on something fixed for the element's
/// lifetime. The element's own opacity, if it sets one, is where the fade ends, so a dimmed element fades in to its
/// dimmed look rather than to fully opaque.
pub fn entrance<E: IntoElement + Styled + 'static>(
    id: impl Into<ElementId>,
    element: E,
    cx: &App,
) -> AnyElement {
    if ticker::reduced_motion(cx) {
        return element.into_any_element();
    }
    one_shot(
        id.into(),
        element,
        ENTRANCE,
        css_ease_out,
        |mut element, progress| {
            let own = element.style().opacity.unwrap_or(1.);
            element.opacity(own * progress)
        },
    )
    .into_any_element()
}

/// The one call to GPUI's animation in the GUI crates' own code. It builds the animation itself, as `Animation::new`'s
/// one-shot default, and takes no `Animation` from its caller, so nothing it is handed is told to repeat. Callers in
/// this module decide reduced motion before calling it, from the ticker rather than GPUI's own flag (the module docs
/// say why).
#[allow(
    clippy::disallowed_methods,
    reason = "the component layer's one owner of one-shot transitions (SPEC_impl.md, \"Animation time: the ticker\")"
)]
fn one_shot<E: IntoElement + 'static>(
    id: ElementId,
    element: E,
    duration: Duration,
    easing: fn(f32) -> f32,
    animator: impl Fn(E, f32) -> E + 'static,
) -> impl IntoElement {
    element.with_animation(id, Animation::new(duration).with_easing(easing), animator)
}

/// CSS's `ease-out` timing function, `cubic-bezier(0, 0, 0.58, 1)`: the share of the change made at `progress` (0 to
/// 1) of the time. GPUI's own easings include no CSS curve, and the old entrance named this one, so it is solved here.
///
/// A CSS timing function is a Bézier curve from (0, 0) to (1, 1) with time on x and the share of the change on y, so
/// the share at a given time is y at the parameter whose x is that time. With these control points x rises steadily
/// in the parameter (its slope, 3.48t - 2.22t², stays positive), so bisection finds the parameter; 24 halvings put it
/// within f32's precision. The ends are exact, so an entrance's last frame is exactly the element's own opacity.
pub(super) fn css_ease_out(progress: f32) -> f32 {
    if progress <= 0. {
        return 0.;
    }
    if progress >= 1. {
        return 1.;
    }
    // x(t) = 3(1-t)t²·x2 + t³ with x1 = 0 and x2 = 0.58; y(t) = 3(1-t)t²·y2 + t³ = 3t² - 2t³ with y1 = 0 and y2 = 1.
    let x = |t: f32| 3. * (1. - t) * t * t * 0.58 + t * t * t;
    let (mut low, mut high) = (0f32, 1f32);
    for _ in 0..24 {
        let mid = (low + high) / 2.;
        if x(mid) < progress {
            low = mid;
        } else {
            high = mid;
        }
    }
    let t = (low + high) / 2.;
    t * t * (3. - 2. * t)
}
