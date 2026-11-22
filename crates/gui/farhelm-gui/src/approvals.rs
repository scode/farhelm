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
//! card answers through that dialog; [`tunnel`], a link card's Tunnel and open in the Mac app; and [`region`], which
//! places the expanded card in the window, over open dialogs, with the other waiting requests listed below it.

use farhelm_proto::approvals::PendingApproval;
use gpui::App;

use crate::operations::OpKey;
use crate::session;
use crate::store::HelmStore;

pub mod answers;
pub mod card;
pub mod host_add;
pub mod model;
pub mod region;
pub mod tunnel;

#[cfg(test)]
mod answers_tests;
#[cfg(test)]
mod card_tests;
#[cfg(test)]
mod host_add_tests;
#[cfg(test)]
mod model_tests;
/// Its window fixtures are used by another module's tests (`templates::save_tests`), so it is `pub(crate)`.
#[cfg(test)]
pub(crate) mod region_tests;
#[cfg(test)]
mod tunnel_tests;

/// Check the store that owns a request. A consent warning watches its bound
/// store for removal of that request.
fn waiting_in(store: &HelmStore, id: &str) -> bool {
    store.collections().get::<PendingApproval>(id).is_some()
}

/// Read the current waiting list when a dialog or card is about to answer.
fn waiting(id: &str, cx: &App) -> bool {
    session::current_store(cx).is_some_and(|store| waiting_in(store.read(cx), id))
}

/// Every surface answering this request shares one operation key, so a
/// pending answer on a card also makes the same request busy in a dialog.
fn answering(id: &str, cx: &App) -> bool {
    session::current_operations(cx).is_some_and(|operations| {
        operations
            .read(cx)
            .in_flight(&OpKey::approval(id))
            .is_some()
    })
}
