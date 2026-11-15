//! The stepped spinner: the old stylesheet's `.delete-spinner`, a ring that jumps 45 degrees per animation tick, eight
//! steps per 0.8-second turn (`farhelm-delete-spin 0.8s steps(8) infinite`).
//!
//! Four areas draw it: the session row's pending Delete, Stop and long Replace or Restart (`session_list::delete`,
//! `session_list::stop`, and the wait line in `session_list::outcome`), the session pane's Delete scrim and its
//! header's same waits (`session_pane`), the version readout's check glyph and its ring (`update_readout::view`), and
//! the launcher's launch pill (`launcher::pill`). The old app had one rule for all of them, so the turn and the drawing live here once.
//!
//! The turn ([`Spinner`]) and the drawing ([`spinner_icon`]) are separate on purpose. A view holds one [`Spinner`] for
//! everything it draws spinning, since every spinner in the view shows the same step, and passes the step to
//! [`spinner_icon`] for each glyph. A view that draws a glyph standing still (the version readout at rest, or any view
//! whose window is inactive) passes step 0 and holds no turn.
//!
//! Reduced motion needs nothing here: under it the ticker delivers no animation ticks at all, so the glyph stands
//! still, as the old stylesheet's `animation: none` left it ([`Spinner::step`] says at which step). The words beside a
//! spinner carry what is happening either way.
//!
//! Drawing only: the spinner takes no input or focus and names nothing (the element around it carries the status and
//! its accessible name), so the component rule of a label, hover help and a role does not reach it.

use std::f32::consts::TAU;

use gpui::{
    App, AppContext as _, Context, Entity, Hsla, Pixels, Styled as _, Subscription, Svg,
    Transformation, Window, radians, svg,
};

use crate::assets::Icon;
use crate::ticker;

/// How many steps the spinner's turn has: `steps(8)` in the old animation.
const STEPS: u8 = 8;

/// The spinner's turn, held by a view while it draws something spinning and its window is active, and dropped
/// otherwise: a live animation handle keeps the ticker's timer firing ten times a second, which an inactive window has
/// no use for. One handle serves every glyph the view draws, since they all show the same step.
///
/// The step is counted from the ticker's notifications of the turn's own entity: the ticker notifies it once per tick
/// and nothing else does, so the count is the number of ticks it has seen. The holding view observes that entity and
/// redraws on every step. Dropping the [`Spinner`] drops the turn, which ends both the ticks and the redraws.
pub struct Spinner {
    turn: Entity<Turn>,
    _redraw: Subscription,
}

/// The entity the ticker notifies on each tick: the step it has reached, and the animation handle that keeps the ticks
/// coming. See [`Spinner`].
struct Turn {
    step: u8,
    _animating: ticker::Animating,
    _ticks: Subscription,
}

impl Spinner {
    /// A spinner starting at its first step, redrawing the view `cx` belongs to on every step.
    pub fn start<V: 'static>(window: &Window, cx: &mut Context<V>) -> Spinner {
        let turn = cx.new(|cx| Turn {
            step: 0,
            _animating: ticker::animate(window, cx),
            _ticks: cx.observe_self(|turn: &mut Turn, _| turn.step = (turn.step + 1) % STEPS),
        });
        Spinner {
            _redraw: cx.observe(&turn, |_, _, cx| cx.notify()),
            turn,
        }
    }

    /// The step to draw, `0..8`. Under reduced motion no tick arrives, so the step stands still where it is: at 0 in
    /// the shipped app, where the host sets reduced motion at startup before any turn exists. A turn already past its
    /// first step when reduced motion is set stays at that step.
    pub fn step(&self, cx: &App) -> u8 {
        self.turn.read(cx).step
    }
}

/// The spinner's glyph at `step` (`0..8`, each an eighth of a turn), `size` square, in `ink`. Callers draw it at the
/// old `.delete-spinner`'s 12 pixels, except the session pane's Delete scrim, which draws it at its card's 16
/// (`.terminal-delete-overlay-card .delete-spinner`).
pub fn spinner_icon(step: u8, size: Pixels, ink: Hsla) -> Svg {
    svg()
        .path(Icon::Spinner.path())
        .size(size)
        .flex_none()
        .text_color(ink)
        .with_transformation(Transformation::rotate(radians(
            f32::from(step) * TAU / f32::from(STEPS),
        )))
}
