//! Farhelm's GPUI desktop GUI: its views and the app-wide foundations they build on.
//!
//! SPEC_impl.md, "GUI architecture", is the rulebook for everything here: each app-wide concern has one owner module,
//! one rule, and a mechanism that makes going around it fail (a lint in `crates/gui/clippy.toml`, a type, or a test).
//! This crate never depends on the helm crate; everything it knows about the helm will arrive over the helm's API.
//!
//! The foundations land in phases. This first phase is the ground the rest stands on:
//!
//! - [`bootstrap`]: installs all app-wide state before any window opens, then opens the main window.
//! - [`theme`]: the old stylesheet's tokens and the appearance choice; the only place colors are built.
//! - [`components`]: the controls, each requiring a label, hover help and an accessibility role.
//! - [`layers`]: the layer manager, the one owner of overlays (only its tooltip slot so far).
//! - [`cache`]: the one way to cache a view.
//! - [`render_count`]: per-view render counts in test builds.
//! - [`assets`]: the bundled fonts and icons.
//! - [`marks`]: the agent harnesses' marks as plain geometry, kept from the old Dioxus UI.
//!
//! Still to come, each under its own module: the store fed by the UI channel, the focus coordinator, the rest of the
//! layer manager, the prompt, the operation stores, drafts, host-supplied text, actions and keys, the ticker and the
//! notice center.

pub mod assets;
pub mod bootstrap;
pub mod cache;
pub mod components;
pub mod layers;
pub mod main_view;
pub mod marks;
pub mod render_count;
pub mod theme;

#[cfg(test)]
mod goldens;
#[cfg(test)]
pub mod test_support;
