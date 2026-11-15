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
//! - [`session_list`]: the sidebar's session list and the selection rules; [`session_pane`]: the pane beside it.
//! - [`approvals`]: agent approval cards, the requests an agent's `farhelm` command waits on until the user answers.
//! - [`environment_ui`]: the GUI's words for each disposable-environment provider, one exhaustive answer per provider.
//! - [`settings`]: the Settings dialog the host rail's gear opens, with its sections.
//! - [`hosts`]: host management, the Hosts panel the sidebar swaps in for its session list, and the per-host rules
//!   (which actions a host offers).
//! - [`sidebar`]: the sidebar around the session list: the app header, and its width as a helm preference with the
//!   divider that changes it.
//! - [`operations`]: the operation store, which holds work that finishes later (on a session, a host, a template, the
//!   preferences, a launch), above the views that start it; [`drafts`]: editor fields that follow pushed changes
//!   until the user touches them, and their saves.
//! - [`ticker`]: the one source of time, for animation (at most 10 ticks a second), for one-shot deadlines, and for
//!   the wall clock that ages are drawn from.
//! - [`notices`]: the notice center, the one place that decides what notice shows.
//! - [`cache`]: the one way to cache a view.
//! - [`render_count`]: per-view render counts in test builds.
//! - [`root`]: a window's root view wiring, which attaches the layer manager and the focus coordinator together.
//! - [`assets`]: the bundled fonts and icons.
//! - [`terminal`]: a session terminal's renderer, an uncached view that owns its grid model.
//! - [`marks`]: the agent harnesses' marks as plain geometry, kept from the old Dioxus UI.
//! - [`permission`]: what a launch says about permission checks (the permission shield), the session list's
//!   classification, kept apart so the agent approval cards can show the same shield.
//! - [`harness`]: the GUI's per-harness vocabulary: which mark an agent type draws, its picture and identity color,
//!   the agent's product name, and its status-line words for a mode without checks.
//! - [`launcher`]: the session launcher (New, Clone, Replace with, Restart with).
//! - [`templates`]: launch templates and what launches from them (applying templates to the launcher's draft, so far).
//! - [`desktop`]: the desktop integration: the menu bar, quitting when the main window closes, the window's saved
//!   frame, its title band and decorations, and the open-source notices.
//! - [`update_readout`]: the desktop updater's state as the words and marks the version readout shows, pure so its
//!   rules are tested without a window.
//! - [`announcements`]: the Announcements dialog, updates and information from Farhelm the helm queues.
//! - [`sounds`]: the status sounds as plain Rust: the rules for which one an observation plays, and the three sounds
//!   synthesized into WAV bytes; observing the store and playing them come later.
//! - [`help_menu`]: the host rail foot's `?` button and the Help menu it opens.
//! - [`shortcuts`]: the Keyboard shortcuts overview, every app-wide shortcut in this client's form, from its chord
//!   or the Help menu.
//! - [`feedback`]: the Send feedback dialog, a private message to the maintainer sent through the helm.
//! - [`trash`]: the session list header's Trash button, the archived-checkout reads behind its count, and the Trash
//!   dialog it opens.
//! - [`notices_bell`]: a session row's notification bell and the list of the session's notifications it opens.
//! - [`rail_notices`]: the notices bell at the top of the host rail's foot and its panel of notices about hosts.
//! - [`welcome`]: the Welcome card, and the owner of the dialogs a window opens by itself when it starts or resets
//!   (the card, then the first batch of announcements).
//!
//! Still to come, each under its own module: the rest of actions and keys.
//!
//! The `agent` feature adds the test-automation bridge (`gpui-agent`), which `scripts/gui-agent.py` drives; see
//! `docs/gui-agent.md`. It also makes the app read `agent_startup`'s startup overrides (window size, appearance, start
//! dialogs off) from its environment, which a screenshot batch uses to capture each theme and size and every bridge run
//! uses to keep the Welcome card from opening over the app; the rest of that module is compiled into every build, so
//! ordinary test runs cover it.

pub mod actions;
pub mod announcements;
// Every build, so the bootstrap reads the overrides one way whatever the features: they are installed only by an agent
// build, which alone reads the process environment (`agent_startup::from_env`), and every other build finds none and
// starts with the defaults. Ordinary test runs, which never turn on the `agent` feature, cover the rest. The module
// also holds the guard that refuses the `agent` feature in optimized builds, so it must stay unconditional.
pub mod agent_startup;
pub mod approvals;
pub mod assets;
pub mod bootstrap;
pub mod cache;
pub mod components;
pub mod desktop;
pub mod drafts;
pub mod environment_ui;
pub mod feedback;
pub mod focus;
pub mod harness;
pub mod help_menu;
pub mod host_text;
pub mod hosts;
pub mod launcher;
pub mod layers;
pub mod local_state;
pub mod main_view;
pub mod marks;
pub mod notices;
pub mod notices_bell;
pub mod operations;
pub mod permission;
pub mod prompt;
pub mod rail_notices;
pub mod render_count;
pub mod root;
pub mod session;
pub mod session_list;
pub mod session_pane;
pub mod settings;
pub mod shortcuts;
pub mod sidebar;
pub mod sounds;
pub mod store;
pub mod templates;
pub mod terminal;
pub mod theme;
pub mod ticker;
pub mod trash;
pub mod update_readout;
pub mod welcome;

/// The license notices for the code this crate links from `farhelm-grid-model` and `farhelm-input-mapping`, verbatim.
///
/// Both crates carry ports of xterm.js and VS Code code (MIT), whose copyright and permission notices must travel with
/// every binary that contains them, however small the part (SPEC_impl.md, "Dependencies and licenses"). This crate is
/// where the desktop app first links them, so the notices are embedded here and the desktop binary prints them
/// (`farhelm-desktop --licenses`). The maintainer decided on 2026-11-13 that the notices live in an "Open-Source
/// Notices" window opened from the app menu, which shows the same text.
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
mod focus_tests;
#[cfg(test)]
mod goldens;
#[cfg(test)]
mod layers_tests;
#[cfg(test)]
mod operations_tests;
#[cfg(test)]
mod prompt_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod store_tests;
#[cfg(test)]
pub mod test_support;
#[cfg(test)]
mod ticker_tests;
