//! Agent approval cards: the requests an agent's `farhelm` command waits on until the user answers them here (SPEC.md,
//! "Agent-spawned sessions").
//!
//! The helm decides everything about a request: whether to ask, the per-host permissions, the nine-minute expiry, the
//! agent YOLO rule and which answers it accepts (`crates/farhelm-helm/src/approvals.rs`). It pushes the requests
//! waiting for this window as the UI channel's approvals collection, oldest first, and the GUI only shows them and
//! sends the user's answer back, so nothing a card does can let a request through that the helm did not hold for it.
//!
//! Built so far: [`model`], what a card shows and offers for every request kind, decided without a window;
//! [`answers`], sending an answer, the answer bar's 700 ms pause and a late answer's notice; [`card`], the expanded
//! card drawn from the model; [`host_add`], what the ordinary Add host dialog holds for an agent's host add, which a
//! card answers through that dialog; and [`tunnel`], a link card's Tunnel and open in the Mac app. The region that
//! places the cards and lists the other waiting requests comes in its own change.

pub mod answers;
pub mod card;
pub mod host_add;
pub mod model;
pub mod tunnel;

#[cfg(test)]
mod answers_tests;
#[cfg(test)]
mod card_tests;
#[cfg(test)]
mod host_add_tests;
#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod tunnel_tests;
