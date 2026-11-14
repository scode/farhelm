//! Farhelm's GPUI desktop GUI: its views and the app-wide foundations they build on.
//!
//! SPEC_impl.md, "GUI architecture", is the rulebook for everything here: each app-wide concern has one owner module,
//! one rule, and a mechanism that makes going around it fail (a lint in `crates/gui/clippy.toml`, a type, or a test).
//! This crate never depends on the helm crate; everything it knows about the helm will arrive over the helm's API.
//!
//! The foundations land in phases. Built so far:
//!
//! - [`bootstrap`]: installs all app-wide state before any window opens, then opens the main window.
//! - [`theme`]: the old stylesheet's tokens and the appearance choice; the only place colors are built.
//! - [`components`]: the controls, each requiring a label, hover help and an accessibility role.
//! - [`focus`]: the focus coordinator, the only code that moves focus (SPEC.md, "Keyboard focus").
//! - [`layers`]: the layer manager, the one owner of overlays: dialogs, popovers and the tooltip.
//! - [`prompt`]: the one way to ask a question, frozen at the moment it opens.
//! - [`actions`]: the app's actions and their keys, declared in one place.
//! - [`cache`]: the one way to cache a view.
//! - [`render_count`]: per-view render counts in test builds.
//! - [`assets`]: the bundled fonts and icons.
//! - [`marks`]: the agent harnesses' marks as plain geometry, kept from the old Dioxus UI.
//!
//! Still to come, each under its own module: the store fed by the UI channel, the operation stores, drafts,
//! host-supplied text, the rest of actions and keys, the ticker and the notice center.
//!
//! The `agent` feature adds the test-automation bridge (`gpui-agent`), which `scripts/gui-agent.py` drives; see
//! `docs/gui-agent.md`.

// The bridge's socket is unauthenticated, and on a Mac the local agent sessions run as the same user, so a build that
// contains it lets any of them click Allow on its own approval card (SPEC_impl.md, "How the rules are enforced"). Shipped
// builds and devbuilds are optimized builds (`--release`, or the `dist` profile that inherits from it), so refusing the
// feature wherever debug assertions are off keeps it out of both without having to name each profile. A debug build
// that turns debug assertions off by hand is refused too, which costs nothing: the bridge is only for development runs.
#[cfg(all(feature = "agent", not(debug_assertions)))]
compile_error!(
    "the `agent` feature (the test-automation bridge) is for debug builds only; it must never reach a release or \
     devbuild, whose optimized profiles turn debug assertions off"
);

pub mod actions;
pub mod assets;
pub mod bootstrap;
pub mod cache;
pub mod components;
pub mod focus;
pub mod layers;
pub mod main_view;
pub mod marks;
pub mod prompt;
pub mod render_count;
pub mod theme;

#[cfg(test)]
mod foundation_tests;
#[cfg(test)]
mod goldens;
#[cfg(test)]
pub mod test_support;
