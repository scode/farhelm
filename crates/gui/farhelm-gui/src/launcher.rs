//! The session launcher: New, Clone, Replace with and Restart with, one launcher with one set of controls (SPEC.md,
//! "Creation" and "Lifecycle operations": "Clone, replace with, and New are one launcher ... differing only in what is
//! pre-filled when they open and in what launching does").
//!
//! Built in pieces. So far:
//!
//! - [`draft`]: what the launcher holds, and every edit to it, with no GPUI types.
//! - [`entry`]: the entry points other surfaces call to open New and Clone, and what they need to draw their triggers;
//!   and a quick launch's: New's prefill as a draft (`new_prefill`), New opened with a banner or a refusal
//!   (`open_with`), and the open launcher a shortcut applies to (`open_launcher`).
//! - [`launch`]: a submitted New or Clone, from the press of Launch to what its reply opens or brings back, and a quick
//!   launch held while it is prepared out of sight (`view::prepare_unseen`), before it is started or shown.
//! - `pill`: the progress pill and the "Started" report a hidden launcher leaves at the bottom of the window.
//! - `model`: the model field's rules (which catalog rows it lists, what Enter in it means), with no GPUI types.
//!   The Templates editor's model suggestions are its rows for one agent type.
//! - [`recents`]: which recent setups the chosen host's launch history offers, in what order, and a row's words, with no
//!   GPUI types.
//! - [`request`]: whether a draft can launch, the one reason it cannot, and the request body it sends.
//! - [`search`]: the composer search's rules (what a query means, the rows it offers, the row preselected, what an
//!   empty result says), with no view.
//! - [`view`]: the launcher's dialog, drawing a draft and turning the user's input into its edits.
//! - `words`: the launcher's own words for agent choices (its chip order, docs link and agent notes); the permission
//!   and effort names other surfaces show too are `crate::permission::words`.
//!
//! The launcher's seams, which the templates-and-quick-launch work plugs into rather than adding a mechanism of its
//! own:
//!
//! - **Edits.** Every hand edit is one method on [`draft::LaunchDraft`] that carries all of the edit's effects
//!   (reconciliation, the remembered wrapper prefix, the escalation banner). Applying a launch template makes "the
//!   edits it contains, in the same way and with the same effects as making them by hand" (SPEC.md, "Launch
//!   templates"), so it goes through these same methods, as the composer search's results do (a recent setup instead
//!   restores a whole stored launch, as Clone's seeding does). A template's escalation is judged once, after all its
//!   edits ([`draft::LaunchDraft::escalation_since`] from the draft before it), not edit by edit (SPEC_impl.md, "Launch
//!   templates").
//! - **What a mode holds.** [`draft::LaunchDraft::holds`] is the one presence check for the fields Restart with and
//!   Replace with hold fixed. The equality rules a template needs beside it (a held value repeated as a no-op) are the
//!   templates work's to add next to it.
//! - **One submit path.** [`request`] turns a complete draft into its one request body, and [`launch`] sends it and
//!   deals with the reply; a launch started without showing the launcher goes through the same check, body and reply
//!   handling.

pub mod draft;
pub mod entry;
pub mod launch;
pub(crate) mod model;
mod pill;
pub mod recents;
pub mod request;
pub mod search;
pub mod view;
mod words;
mod yolo;

#[cfg(test)]
mod checkout_tests;
#[cfg(test)]
mod draft_tests;
#[cfg(test)]
mod entry_tests;
#[cfg(test)]
mod escalation_tests;
#[cfg(test)]
mod folder_check_tests;
#[cfg(test)]
mod folder_default_tests;
#[cfg(test)]
mod folder_list_tests;
#[cfg(test)]
pub(crate) mod launch_tests;
#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod recents_tests;
#[cfg(test)]
mod recents_view_tests;
#[cfg(test)]
mod replace_with_tests;
#[cfg(test)]
mod request_tests;
#[cfg(test)]
mod restart_with_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod search_view_tests;
#[cfg(test)]
pub(crate) mod view_tests;
#[cfg(test)]
pub(crate) mod yolo_tests;
