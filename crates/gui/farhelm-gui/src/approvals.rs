//! Agent approval cards: the requests an agent's `farhelm` command waits on until the user answers them here (SPEC.md,
//! "Agent-spawned sessions").
//!
//! The helm decides everything about a request: whether to ask, the per-host permissions, the nine-minute expiry, the
//! agent YOLO rule and which answers it accepts (`crates/farhelm-helm/src/approvals.rs`). It pushes the requests
//! waiting for this window as the UI channel's approvals collection, oldest first, and the GUI only shows them and
//! sends the user's answer back, so nothing a card does can let a request through that the helm did not hold for it.
//!
//! Built so far: [`model`], what a card shows and offers for every request kind, decided without a window; and
//! [`answers`], sending an answer, the answer bar's 700 ms pause and a late answer's notice. The cards' drawing and the
//! region that places them come in their own changes.

pub mod answers;
pub mod model;

#[cfg(test)]
mod answers_tests;
#[cfg(test)]
mod model_tests;
