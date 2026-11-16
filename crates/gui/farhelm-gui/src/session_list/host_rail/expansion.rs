//! The host rail's expansion: whether the rail is its collapsed 56 pixels or its expanded 220, how that choice is
//! saved, and how much of the window the expanded rail takes from the main pane.
//!
//! SPEC.md, "Session list": the Expand control "expands the whole rail, its foot included, to 220px beside the session
//! list", "Expansion is a helm preference that every client shares, so a window opens with the rail as it was last
//! left; a helm that has none starts collapsed", and "Expanding the rail adds 164px to the sidebar's shown width and
//! takes that space from the main pane". Ported from the old app's `assets/sidebar-width.js` (`storedRailOpen`,
//! `shownWidth`, `mountRailToggle`) and the stylesheet's `--host-rail-width` rules at reference commit `24bc0580`; the
//! old app kept the choice per device in `localStorage`, and the maintainer decided on 2026-11-13 that it becomes the
//! helm's `host_rail_open` preference, starting unset, with nothing carried over.
//!
//! - **The preference row decides.** The rail is expanded exactly while the helm stores `host_rail_open` true; unset,
//!   false, or no preference row at all is collapsed. The list keeps no copy of it: [`Expansion::shown`] reads the store
//!   as the rail draws, so a choice made in another window or by an agent expands or collapses this one as it arrives.
//! - **The control writes it the way Compact is written** (`super::super::order`, "Compact"): a sparse
//!   `PUT /api/preferences` naming only `host_rail_open`, saved through a [`Draft`] under the preference row's key, shown
//!   as the choice being saved while the write is in flight and as the stored value at every other moment. A press
//!   while another preference write is in flight (an order, Compact, a Settings switch) is dropped and logged, as those
//!   drop theirs. A refused write is logged and shows nothing, and the rail goes back to what the helm stores at once.
//!   That is Compact's behavior, copied as the precedent rather than taken from SPEC.md: "Errors and diagnostics" lists
//!   the best-effort preferences whose failed save is silent, and the rail's expansion is not among them. The parity
//!   file records it as a deviation (`docs/gpui/parity/sidebar-and-session-view.md`, "Deviations from the old app").
//! - **The stored width keeps its meaning.** Expanding adds [`EXTRA`] to the width the sidebar shows, never to the
//!   stored one; `crate::sidebar` has the rule.
//!
//! The draft belongs to one UI session, like everything the list keeps per session, but the sidebar must read the
//! expansion with only `&App` (to place its divider's grip and decide whether its header floats), which a
//! [`crate::session::PerSession`] cannot give. So [`Expansion`] records the store it saved under and ignores its draft
//! under any other: after a reset the rail shows what the new snapshot stores, and the next press starts a fresh draft.

#[cfg(test)]
mod tests;

use farhelm_helm_client::{ApiPath, Collections, Method, PathError};
use gpui::{App, Context, EntityId, Pixels, px};

use crate::drafts::{self, Draft};
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::session;
use crate::session_list::selection;

/// `--host-rail-width` while the rail is collapsed: its whole width, its right edge included.
const COLLAPSED_WIDTH: f32 = 56.;

/// `html.host-rail-open { --host-rail-width: 220px }`: the expanded rail's whole width.
const EXPANDED_WIDTH: f32 = 220.;

/// What expanding adds to the sidebar's shown width: the difference between the two rail widths (the old script's
/// `RAIL_EXTRA`, which it kept "in sync with app.css" by hand; here it is worked out from them).
pub(crate) const EXTRA: u32 = (EXPANDED_WIDTH - COLLAPSED_WIDTH) as u32;

/// The rail's whole width, its right edge included, expanded or not.
pub(crate) fn width(expanded: bool) -> Pixels {
    px(if expanded {
        EXPANDED_WIDTH
    } else {
        COLLAPSED_WIDTH
    })
}

/// Whether the helm stores the rail expanded in `collections`: only an explicit `true` is, as only the old script's
/// own `"true"` opted a device into the wider rail.
pub(crate) fn stored(collections: &Collections) -> bool {
    selection::preferences(collections).and_then(|preferences| preferences.host_rail_open)
        == Some(true)
}

/// `PUT /api/preferences` naming only `host_rail_open`: sparse, so it never puts back a stale copy of a field another
/// client changed.
fn command(expanded: bool) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Save,
        Method::Put,
        ApiPath::api(&["preferences"])?,
        Some(serde_json::json!({ "host_rail_open": expanded })),
        "PUT /api/preferences",
    ))
}

/// The rail's expansion as this window shows it: the stored preference, or the choice being saved while its write is
/// in flight. The session list holds one for the window's life and hands it to the sidebar through
/// [`crate::session_list::SessionList::rail_expanded`].
pub(crate) struct Expansion {
    /// The choice being saved while a write is in flight; its value means nothing at any other moment.
    draft: Draft<bool>,
    /// The store of the UI session the draft was saved under. The draft counts only while that store is the current
    /// one (see the module docs).
    store: Option<EntityId>,
}

impl Expansion {
    /// No choice in flight: the rail shows what the helm stores.
    pub(crate) fn new() -> Self {
        Expansion {
            draft: Draft::load(false),
            store: None,
        }
    }

    /// Whether the rail is expanded now: the choice in flight under the current UI session, else the stored
    /// preference. Collapsed while the window is inert.
    pub(crate) fn shown(&self, cx: &App) -> bool {
        let Some(store) = session::current_store(cx) else {
            return false;
        };
        if self.draft.is_saving() && self.store == Some(store.entity_id()) {
            return *self.draft.value();
        }
        stored(store.read(cx).collections())
    }

    /// The Expand or Collapse control was pressed: save the opposite of what the rail shows. Nothing happens while the
    /// window is inert. A press the draft or the operation store refuses (a write in flight) is logged and changes
    /// nothing, and a refused save is logged and shows nothing (the module docs).
    pub(crate) fn toggle(&mut self, cx: &mut Context<Self>) {
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let expanded = !self.shown(cx);
        // A draft saved under an earlier UI session may still read as saving: a reset drops its reply. It means nothing
        // now, so the new session starts with a fresh one.
        if self.store != Some(store.entity_id()) {
            self.store = Some(store.entity_id());
            self.draft = Draft::load(!expanded);
        }
        if !self.draft.edit(expanded) {
            tracing::debug!(
                expanded,
                "the rail's expansion was not saved: its last save is in flight"
            );
            return;
        }
        let started = drafts::save(
            self,
            |expansion: &mut Expansion| &mut expansion.draft,
            &OpKey::preferences(),
            |expanded| command(*expanded),
            cx,
            move |_, settled, _| {
                if let Outcome::Refused(refusal) = &settled.outcome {
                    let words = &refusal.words;
                    tracing::warn!(expanded, ?words, "the helm refused the rail's expansion");
                }
            },
        );
        if let Err(not_started) = started {
            tracing::debug!(expanded, ?not_started, "the rail's expansion was not saved");
        }
    }
}
