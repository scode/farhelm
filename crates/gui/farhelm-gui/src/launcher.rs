//! The session launcher: New, Clone, Replace with and Restart with, one launcher with one set of controls (SPEC.md,
//! "Creation" and "Lifecycle operations": "Clone, replace with, and New are one launcher ... differing only in what is
//! pre-filled when they open and in what launching does").
//!
//! Built in pieces. So far:
//!
//! - [`draft`]: what the launcher holds, and every edit to it, with no GPUI types.
//! - [`request`]: whether a draft can launch, the one reason it cannot, and the request body it sends.
//! - [`view`]: the launcher's dialog, drawing a draft and turning the user's input into its edits.
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
//! - **One submit path.** [`request`] turns a complete draft into its one request body; a launch started without
//!   showing the launcher goes through the same check and body.

pub mod draft;
pub mod request;
pub mod view;

#[cfg(test)]
mod draft_tests;
#[cfg(test)]
mod request_tests;
#[cfg(test)]
mod view_tests;
