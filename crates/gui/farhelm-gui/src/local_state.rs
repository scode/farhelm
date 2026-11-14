//! The selected session's survivor: the one piece of window state that outlives a reset of the connection to the helm.
//!
//! SPEC.md, "Losing the connection to the helm": a reset throws away everything the window got from the helm, but the
//! selected session stays selected, by id, if the new snapshot still has it. The id lives here, outside every UI
//! session, so retiring a session cannot reach it (SPEC_impl.md, "One failure path: the reset").
//!
//! Two owners write it, each through [`select`] and [`clear`]. The session list ([`crate::session_list`]) owns the
//! selection rules: a click, the automatic choice when nothing is selected, and dropping a session a complete listing no
//! longer has. The connection keeps only its snapshot check: it clears the id when a complete snapshot no longer lists
//! that session ([`crate::session`]). Every write that changes the id notifies the global's observers (GPUI notifies
//! them on every mutable access, so the functions here skip the access when nothing would change), which is how every
//! view that draws the selection redraws; a view that draws it observes [`LocalState`] with `cx.observe_global`.
//!
//! NOTE: This is not a device-local preferences store and must not grow into one. The settings the old app kept per
//! device (appearance, sidebar width, host rail, status sounds, terminal text size) are helm preferences in the new app,
//! so they come back with every snapshot like the rest of the helm's state (maintainer decision, 2026-11-13). Nothing
//! here is persisted: a selection never needs to outlive a launch, and the helm's `last_selected` preference is what
//! carries a selection from one launch to the next.

use gpui::{App, Global};

use crate::bootstrap::{installed, installed_mut};

/// The selected session's id, kept across resets. Read it with [`selected`]; write it with [`select`] or [`clear`].
#[derive(Default)]
pub struct LocalState {
    /// The selected session, by the key the helm files it under. Survives a reset only if the new snapshot has it.
    selected_session: Option<String>,
}

impl Global for LocalState {}

/// Install the empty survivor. Part of the bootstrap.
pub fn install(cx: &mut App) {
    cx.set_global(LocalState::default());
}

/// The selected session's key, if a session is selected. `None` also before [`install`], which is a bootstrap bug
/// ([`installed`] reports it).
pub fn selected(cx: &App) -> Option<String> {
    installed::<LocalState>(cx).and_then(|local| local.selected_session.clone())
}

/// Select the session filed under `key`. Returns whether the selection changed; selecting the selected session again
/// changes nothing and notifies nobody. Does nothing before [`install`].
pub fn select(key: &str, cx: &mut App) -> bool {
    if selected(cx).as_deref() == Some(key) {
        return false;
    }
    let Some(local) = installed_mut::<LocalState>(cx) else {
        return false;
    };
    local.selected_session = Some(key.to_owned());
    true
}

/// Select nothing. Returns whether a session was selected. Does nothing before [`install`].
pub fn clear(cx: &mut App) -> bool {
    if selected(cx).is_none() {
        return false;
    }
    let Some(local) = installed_mut::<LocalState>(cx) else {
        return false;
    };
    local.selected_session = None;
    true
}
