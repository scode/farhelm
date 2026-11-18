//! The status sounds' owner: it watches what the helm pushes and decides, once per change, whether a sound plays.
//!
//! SPEC.md, "Status" (the sounds paragraph); SPEC_impl.md, "Notices: the notice center" (the three sounds are this
//! owner's, with no notice-center step, as the migration's orchestrator decided on 2026-11-14). Ported from the old
//! app's `SessionSounds` component (`crates/farhelm-ui/src/sounds.rs` at reference commit `24bc0580`) and the
//! `observe` it called in `assets/sounds.js`. Here's the TLDR of what it does:
//!
//! - **One observation per change.** It follows the current UI session's store ([`session::observe_current`]) and the
//!   sidebar's session list, whose query's answers arrive as the list's notifications. Each time, it
//!   builds one [`Observation`] from the store as it is then: the sessions the list shows (the pushed sessions
//!   collection's order, which is exactly what the sidebar's current host selector and filters select; the open
//!   session the helm carries outside that order is not among them), every pending approval, the switches from the
//!   shared preference row, and the quiet session. The rules ([`SoundRules`]) compare it with the previous one and
//!   name at most one sound, which goes to the installed player ([`super::play`]).
//! - **Silent baselines.** The first observation of a UI session (the snapshot a window start or a reset brings) plays
//!   nothing: the rules live in a [`PerSession`], so a reset starts them over, seen approval ids included.
//! - **An unreadable listing is an unknown view.** While the helm reports the sessions collection unavailable, and
//!   once more when it is readable again, observations record statuses without judging them
//!   ([`StatusSounds::listing`]).
//! - **A changed view is recorded, not judged.** Choosing another host or status filter changes which sessions the
//!   listing holds, and the helm pushes the new listing before it answers the query, with nothing in the listing to say
//!   which query it answers. So while the list says a filter change is unanswered ([`SessionList::listing_view`] is
//!   `None`), and once more when the answer arrives, the owner tells the rules the view changed: those observations
//!   record the statuses they see without a sound, while new approval requests, which no filter hides, still ring. A
//!   status change pushed during that round trip is recorded silently too; the old sidebar likewise accepted no
//!   listing for a superseded filter.
//! - **The quiet session** is the selected session while this window is active ([`ticker::is_active`], so the test
//!   bridge's override applies), read at each observation: its waiting, its turn finishing and its own approval
//!   requests stay quiet but are consumed, so they do not ring later when the window loses focus.
//!
//! It keeps no copy of pushed state: everything is read from the store at each observation, and what it holds between
//! observations is the rules' own history of the previous one.

#[cfg(test)]
mod tests;

use farhelm_proto::approvals::PendingApproval;
use farhelm_proto::ui_channel::{
    Availability, Collection, PREFERENCES_KEY, Preferences, SessionRow,
};
use gpui::{AnyWindowHandle, Context, Entity, Subscription, WeakEntity, Window};

use super::rules::{Approval, Observation, SoundRules, Status, Switches};
use crate::session::{self, ObservingCurrent, PerSession};
use crate::session_list::SessionList;
use crate::{local_state, ticker};

/// The owner of the status sounds for one window, kept by the main view for the window's life.
pub struct StatusSounds {
    /// The sidebar's list, asked which view its listing answers; weak, since the main view owns it.
    list: WeakEntity<SessionList>,
    /// The window whose activity decides whether the selected session is quiet.
    window: AnyWindowHandle,
    /// What the current UI session's decisions remember; started over by a reset.
    heard: PerSession<Heard>,
    /// Observes on every change of the current UI session's store, and on the connection's changes (a reset).
    _store: ObservingCurrent,
    /// Observes when the list's query is answered (see [`StatusSounds::new`]).
    _list: Subscription,
}

/// One UI session's memory: the rules' history, and how the owner numbers the views it hands them.
#[derive(Default)]
struct Heard {
    rules: SoundRules,
    /// The list's [`SessionList::listing_view`] at the previous observation; `None` before the first.
    listing: Option<Option<u64>>,
    /// The view number the rules last saw. The owner moves it whenever the list's answer differs from the last one or
    /// is unknown, so the rules record those observations' statuses without a sound.
    view: u64,
}

impl StatusSounds {
    /// The owner for the window being built, over the sidebar's `list`.
    pub fn new(list: &Entity<SessionList>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        StatusSounds {
            list: list.downgrade(),
            window: window.window_handle(),
            heard: PerSession::default(),
            _store: session::observe_current(cx, |owner: &mut Self, cx| owner.observe(cx)),
            _list: cx.observe(list, |owner, _, cx| {
                // The list notifies for everything it draws, its pulse and clock included, up to ten times a second;
                // only an answer to its query, which moves the view it reports, is news here.
                let listing = owner.listing(cx);
                if owner.heard.current(cx).listing != Some(listing) {
                    owner.observe(cx);
                }
            }),
        }
    }

    /// Which view the sidebar's listing answers ([`SessionList::listing_view`]), or `None`, an unknown view, while the
    /// current store's sessions collection is unavailable (or missing, with the window inert) and once the list is gone.
    ///
    /// An unavailable collection holds no rows the owner can trust: a snapshot that arrives unavailable holds none at
    /// all, so taking it as the view's baseline would make the read that recovers ring for every session already
    /// waiting. Counting it as an unknown view records the recovered listing silently instead, as the old app kept
    /// its last statuses while a listing read failed.
    fn listing(&self, cx: &mut Context<Self>) -> Option<u64> {
        let available = session::current_store(cx).is_some_and(|store| {
            store
                .read(cx)
                .collections()
                .meta(Collection::Sessions)
                .is_some_and(|meta| meta.availability == Availability::Available)
        });
        if !available {
            return None;
        }
        self.list
            .upgrade()
            .and_then(|list| list.update(cx, |list, cx| list.listing_view(cx)))
    }

    /// Observe the store as it is now and play what the rules choose, if anything. Nothing while the window is inert.
    ///
    /// Runs from GPUI's observers, never nested in the list's or the window's update, so it may read both.
    fn observe(&mut self, cx: &mut Context<Self>) {
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let listing = self.listing(cx);
        // The update fails only for a window that has closed or is being updated already, neither of which should
        // hold while GPUI runs its observers. Inactive is the safe answer: it can only make the open session ring where it
        // would have stayed quiet, never silence a sound the user should hear.
        let active = self
            .window
            .update(cx, |_, window, cx| ticker::is_active(window, cx))
            .unwrap_or(false);
        let quiet = active.then(|| local_state::selected(cx)).flatten();
        let collections = store.read(cx).collections();
        let mut statuses = std::collections::BTreeMap::new();
        let mut muted = std::collections::HashSet::new();
        for row in collections.ordered::<SessionRow>() {
            statuses.insert(row.info.id.clone(), Status::from(&row.info.status));
            if crate::session_list::status::muted(row) {
                muted.insert(row.info.id.clone());
            }
        }
        let approvals = collections
            .ordered::<PendingApproval>()
            .into_iter()
            .map(|approval| Approval {
                id: approval.id.clone(),
                session: approval.session.id.clone(),
            })
            .collect();
        let switches = Switches::from_preferences(collections.get::<Preferences>(PREFERENCES_KEY));

        let heard = self.heard.current(cx);
        if listing.is_none() || heard.listing != Some(listing) {
            heard.view += 1;
        }
        heard.listing = Some(listing);
        let sound = heard.rules.observe(Observation {
            view: heard.view,
            statuses,
            approvals,
            quiet,
            muted,
            switches,
        });
        if let Some(sound) = sound {
            super::play(sound, cx);
        }
    }
}
