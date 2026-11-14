//! The selected session's survivor: the one piece of window state that outlives a reset of the connection to the helm.
//!
//! SPEC.md, "Losing the connection to the helm": a reset throws away everything the window got from the helm, but the
//! selected session stays selected, by id, if the new snapshot still has it. The id lives here, outside every UI
//! session, so retiring a session cannot reach it (SPEC_impl.md, "One failure path: the reset"); the connection clears
//! it when a complete snapshot no longer lists that session.
//!
//! NOTE: This is not a device-local preferences store and must not grow into one. The settings the old app kept per
//! device (appearance, sidebar width, host rail, status sounds, terminal text size) are helm preferences in the new app,
//! so they come back with every snapshot like the rest of the helm's state (maintainer decision, 2026-11-13). Nothing
//! here is persisted: a selection never needs to outlive a launch.

use gpui::{App, Global};

/// The selected session's id, kept across resets and dropped only by the connection's survivor check.
#[derive(Default)]
pub struct LocalState {
    /// The selected session, by the key the helm files it under. Survives a reset only if the new snapshot has it.
    pub selected_session: Option<String>,
}

impl Global for LocalState {}

/// Install the empty survivor. Part of the bootstrap.
pub fn install(cx: &mut App) {
    cx.set_global(LocalState::default());
}
