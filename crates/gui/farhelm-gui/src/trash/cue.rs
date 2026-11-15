//! The archive cue: the Trash button's can wiggles when this window's session Delete moved a checkout into the trash.
//!
//! SPEC.md, "Managed checkouts": "Only the client that issued a session Delete plays an archive cue, and only when the
//! host confirms that a checkout moved: the row's dashed outline flies into the trash, which then wiggles briefly.
//! Reduced motion skips both parts." Ported from the old app's `finish_row_cue` in `list/trash.rs` at reference commit
//! `24bc0580`. Here's the TLDR:
//!
//! - **Only this window's confirmed archive.** The session Delete's completion hands its reply to [`archived`], and only
//!   a reply that says a checkout was archived plays the cue. A bare Delete (no checkout), a refused or failed Delete,
//!   and a session that only disappeared from the listing (another client's Delete) play nothing: none of them reaches
//!   this module with that flag.
//! - **The wiggle.** The can turns by the old keyframes, 0, -16, 12, -8, 4 and back to 0 degrees, over 900 ms eased
//!   out, a step per ticker tick ([`ticker::animate`]), and then stands upright again ([`wiggle_angle`]). It plays only
//!   while the button is drawn and the window active, and never under reduced motion, as the old one skipped a hidden
//!   button and `prefers-reduced-motion`.
//! - **Over the bob.** A can already upside down and bobbing (a delete of this window's running, a host freeing space)
//!   wiggles too, the turn added to its upside-down one: the old wiggle rotated the whole button, around the bobbing
//!   can, whatever the can was doing.
//!
//! NOTE: Deviations from the old cue, each recorded in the parity file under "### Archive cue". The row's dashed outline
//! flying into the can waits for a foundation change (an overlay drawn above the sidebar, which only the layer manager
//! may draw), so the can wiggles at once, as the old cue did when it had no row to fly from. Only the can turns, where
//! the old button turned as a whole, count and edge included: GPUI turns an SVG, not a box of elements. And the turn is
//! drawn at the ticker's ten steps a second, so the sharpest swings fall between ticks.

#[cfg(test)]
pub(crate) mod tests;

use std::time::Duration;

use farhelm_proto::SessionDeleteOutcome;
use gpui::{App, BorrowAppContext as _, Context, Entity, Global, WeakEntity};
use serde_json::Value;

use super::TrashButton;
use crate::ticker;

/// How long the wiggle lasts: the old animation's 900 ms.
pub(super) const WIGGLE: Duration = Duration::from_millis(900);

/// The old wiggle's keyframes in degrees, evenly spaced over [`WIGGLE`]; a positive turn is clockwise, as in CSS.
const KEYFRAMES: [f32; 6] = [0., -16., 12., -8., 4., 0.];

/// Every Trash button the app has made, so a Delete's completion, which runs without a window, can reach the one it
/// cues. The app has one window and one session list (the same assumption as the session list's `DeletedHere`), so in
/// practice this is one button.
#[derive(Default)]
pub(super) struct Buttons(Vec<WeakEntity<TrashButton>>);

impl Global for Buttons {}

/// Remember `button` as one the cue reaches; buttons that went away are dropped as new ones come.
pub(super) fn register(button: WeakEntity<TrashButton>, cx: &mut App) {
    cx.update_default_global::<Buttons, _>(|buttons, _| {
        buttons.0.retain(|kept| kept.upgrade().is_some());
        buttons.0.push(button);
    });
}

/// A session Delete of this window's succeeded with `reply`, the route's own answer: play the cue if the reply says the
/// Delete archived a managed checkout. The helm sets that flag only when a checkout really moved, so a bare Delete's
/// reply, or none, plays nothing. The session list's Delete calls this from its completion, for a success only.
pub(crate) fn archived(reply: Option<&Value>, cx: &mut App) {
    let archived = reply
        .and_then(|reply| serde_json::from_value::<SessionDeleteOutcome>(reply.clone()).ok())
        .is_some_and(|outcome| outcome.archived);
    if !archived {
        return;
    }
    let buttons: Vec<Entity<TrashButton>> = cx
        .try_global::<Buttons>()
        .map(|buttons| buttons.0.iter().filter_map(WeakEntity::upgrade).collect())
        .unwrap_or_default();
    for button in buttons {
        button.update(cx, |button, cx| button.wiggle(cx));
    }
}

impl TrashButton {
    /// Ask for the wiggle, unless motion is reduced or the button was not drawn in the last frame (the old cue skipped
    /// a button that took no room). The wiggle starts when the button next draws, which this asks for, since starting
    /// one needs the window.
    fn wiggle(&mut self, cx: &mut Context<Self>) {
        if ticker::reduced_motion(cx) || self.shown.upgrade().is_none() {
            return;
        }
        self.wiggle_asked = Some(self.shown.entity_id());
        cx.notify();
    }

    /// Whether the button still holds the wiggle's ticker handle: for the tests that it lets go once the wiggle ends or
    /// the window goes inactive, since a held handle keeps the ticker's timer running.
    #[cfg(test)]
    pub(crate) fn wiggling(&self) -> bool {
        self.wiggle.is_some()
    }

    /// The can's wiggle turn now, in degrees, or `None` while the button holds no wiggle: for the tests of the cue.
    #[cfg(test)]
    pub(crate) fn wiggle_turn(&self, cx: &App) -> Option<f32> {
        self.wiggle
            .as_ref()
            .and_then(|ticks| wiggle_angle(ticks.count(cx)))
    }
}

/// The wiggle's turn in degrees `ticks` ticks after it started, or `None` once it has ended. The old animation's
/// `ease-out` applies over its whole length, so the keyframes are spaced in eased time, with a straight line between
/// two of them.
pub(super) fn wiggle_angle(ticks: u64) -> Option<f32> {
    let elapsed = ticker::TICK.as_secs_f32() * ticks as f32;
    let progress = elapsed / WIGGLE.as_secs_f32();
    if progress >= 1. {
        return None;
    }
    let spans = (KEYFRAMES.len() - 1) as f32;
    let at = ease_out(progress) * spans;
    let span = (at as usize).min(KEYFRAMES.len() - 2);
    let within = at - span as f32;
    Some(KEYFRAMES[span] + (KEYFRAMES[span + 1] - KEYFRAMES[span]) * within)
}

/// CSS's `ease-out`, `cubic-bezier(0, 0, 0.58, 1)`, at `progress` in 0..=1: the curve's parameter is found by bisection
/// on its x, and its y is the eased progress. The start is exact, so the wiggle starts with the can exactly upright
/// (bisection would leave it a hair off).
fn ease_out(progress: f32) -> f32 {
    if progress <= 0. {
        return 0.;
    }
    let bezier = |p1: f32, p2: f32, s: f32| {
        3. * (1. - s) * (1. - s) * s * p1 + 3. * (1. - s) * s * s * p2 + s * s * s
    };
    let (mut low, mut high) = (0f32, 1f32);
    for _ in 0..32 {
        let middle = (low + high) / 2.;
        if bezier(0., 0.58, middle) < progress {
            low = middle;
        } else {
            high = middle;
        }
    }
    bezier(0., 1., (low + high) / 2.)
}
