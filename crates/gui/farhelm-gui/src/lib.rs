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
//! - [`actions`]: the app's actions and their keys, declared in one place, and which keys a focused terminal keeps
//!   from its program.
//! - [`host_text`]: host-supplied text, escaped and direction-isolated wherever the GUI shows it.
//! - [`session`]: the connection to the helm and the resettable UI-session owner (SPEC_impl.md, "One failure path:
//!   the reset"); [`store`]: the one copy of what the helm pushed, which views read when they draw;
//!   [`local_state`]: the selected session's id, the one piece of window state that survives a reset.
//! - [`ticker`]: the one source of time, for animation (at most 10 ticks a second) and for one-shot deadlines.
//! - [`notices`]: the notice center, the one place that decides what notice shows.
//! - [`cache`]: the one way to cache a view.
//! - [`render_count`]: per-view render counts in test builds.
//! - [`assets`]: the bundled fonts and icons.
//! - [`terminal`]: a session terminal's renderer, an uncached view that owns its grid model.
//! - [`marks`]: the agent harnesses' marks as plain geometry, kept from the old Dioxus UI.
//!
//! Still to come, each under its own module: the operation stores, drafts and the rest of actions and keys.
//!
//! The `agent` feature adds the test-automation bridge (`gpui-agent`), which `scripts/gui-agent.py` drives; see
//! `docs/gui-agent.md`. It also makes the app read `agent_startup`'s startup overrides (window size, appearance) from
//! its environment, which a screenshot batch uses to capture each theme and size; the overrides themselves are also
//! compiled into test builds, so ordinary test runs cover them.

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
// Test builds too: the parsing and the bootstrap's use of the overrides are tested without the `agent` feature, which
// no ordinary test run turns on. Only reading the process environment stays agent-only (`agent_startup::from_env`).
#[cfg(any(feature = "agent", test))]
pub mod agent_startup;
pub mod assets;
pub mod bootstrap;
pub mod cache;
pub mod components;
pub mod focus;
pub mod host_text;
pub mod layers;
pub mod local_state;
pub mod main_view;
pub mod marks;
pub mod notices;
pub mod prompt;
pub mod render_count;
pub mod session;
pub mod store;
pub mod terminal;
pub mod theme;
pub mod ticker;

/// The license notices for the code this crate links from `farhelm-grid-model` and `farhelm-input-mapping`, verbatim.
///
/// Both crates carry ports of xterm.js and VS Code code (MIT), whose copyright and permission notices must travel with
/// every binary that contains them, however small the part (SPEC_impl.md, "Dependencies and licenses"). This crate is
/// where the desktop app first links them, so the notices are embedded here and the desktop binary prints them
/// (`farhelm-desktop --licenses`). The maintainer decided on 2026-11-13 that the notices live in an "Open-Source
/// Notices" window opened from the app menu; that window will show the same text.
/// Each file's own preamble says only "this crate", so a `=== crate ===` header names the crate before its file.
pub const PORTED_CODE_NOTICES: &str = concat!(
    "=== farhelm-grid-model ===\n\n",
    include_str!("../../farhelm-grid-model/LICENSE-PORTED"),
    "\n=== farhelm-input-mapping ===\n\n",
    include_str!("../../farhelm-input-mapping/LICENSE-PORTED"),
);

#[cfg(test)]
mod actions_tests;
#[cfg(test)]
mod foundation_tests;
#[cfg(test)]
mod goldens;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
pub mod test_support;
