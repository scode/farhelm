//! The store: the GUI's one copy of what the helm pushed, fed only by the UI channel.
//!
//! SPEC_impl.md, "The store, and how views redraw". Views reach the store through [`crate::session::current_store`]
//! when they draw, read pushed state with [`HelmStore::collections`], and never keep a copy of an entry. GPUI redraws a
//! window when an entity its last frame read notifies, but it cannot see a copied value, so a copy goes stale the
//! moment the helm changes the entry, silently. Changes reach the store only through the UI-session owner
//! ([`crate::session`]), which applies the channel's events in order, and every change notifies, so a view that read
//! the store while drawing redraws.
//!
//! NOTE: A cached view ([`crate::cache`]) is not covered by that. The cache wrapper holds its source entity for life
//! and replays without reading it, so a cached view built over one session's store would go on showing that store
//! after a reset retired it. A cached view over pushed state must be built per UI session and dropped with it; the
//! first cached list settles how.
//!
//! The data itself is the helm client's [`Collections`]: one generic apply for every collection, decoded once by the
//! channel. A store belongs to one UI session. A reset drops it with the session, and the next snapshot builds a new
//! one, so nothing pushed by an earlier connection survives a reset.

use farhelm_helm_client::{Collections, PushedChanges, PushedState};
use gpui::Context;

/// One UI session's copy of the pushed collections.
pub struct HelmStore {
    collections: Collections,
}

impl HelmStore {
    /// A store holding `snapshot`, the whole pushed state one connection opened with.
    pub(crate) fn from_snapshot(snapshot: &[PushedState]) -> HelmStore {
        let mut collections = Collections::default();
        collections.install(snapshot);
        HelmStore { collections }
    }

    /// The pushed collections, for a view to read while it draws.
    pub fn collections(&self) -> &Collections {
        &self.collections
    }

    /// Apply one collection's changes, in channel order, and notify the views that read the store.
    pub(crate) fn apply(&mut self, changes: &PushedChanges, cx: &mut Context<HelmStore>) {
        self.collections.apply(changes);
        cx.notify();
    }
}
