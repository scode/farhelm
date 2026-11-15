//! The running pulse's opacity: how far a pulsing mark dims at each step of the ticker's shared pulse.
//!
//! Two marks pulse with it: the session row's running dot (`session_list::status`) and the Hosts panel's setup progress
//! (`hosts::progress`: the inline status's dot and the running step's ring). Both copy the old stylesheet's one
//! `farhelm-status-pulse` animation, so the curve lives here once rather than in either area. The step itself comes from
//! the ticker ([`crate::ticker::Pulsing::step`]), which also keeps it at 0, full opacity, under reduced motion.

use crate::ticker;

/// The running mark's opacity at pulse step `step` (`0..ticker::PULSE_STEPS`), dipping to `min_percent` at its
/// dimmest. The old `farhelm-status-pulse` keyframes fall linearly from 1 to the minimum and climb back, and `steps(4)`
/// holds each quarter of each half at the value where it starts, so the steps read full, three quarters of the way,
/// half way, a quarter of the way, the minimum, and back up: with a 60% minimum, 1.0, 0.9, 0.8, 0.7, 0.6, 0.7, 0.8,
/// 0.9.
pub fn pulse_opacity(step: u64, min_percent: u8) -> f32 {
    let half = ticker::PULSE_STEPS / 2;
    let step = step % ticker::PULSE_STEPS;
    let depth = if step <= half {
        step
    } else {
        ticker::PULSE_STEPS - step
    };
    let dip = 1. - f32::from(min_percent) / 100.;
    1. - dip * depth as f32 / half as f32
}
