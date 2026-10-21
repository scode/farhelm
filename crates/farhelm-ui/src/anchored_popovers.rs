//! Close registered fixed popovers when a causal report invalidates their row.
//!
//! The open slots and their focus/measurement lifetimes stay with the existing
//! components. This owner reads those slots without subscribing to them, then
//! invokes their idempotent closers at the reporting caller's own boundary.
//! It keeps no movement history: an opening after a report measures the new
//! layout and must never receive a replay of that earlier invalidation.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use dioxus::prelude::*;

/// Independent row orders; a host permutation need not move any session row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnchorRegion {
    Hosts,
    Sessions,
}

/// Exact displayed rows whose old anchor coordinates may no longer be valid.
///
/// Producers derive this from their own before/after displayed order. Keeping
/// identities rather than only the first changed index preserves unchanged
/// anchors below a harmless permutation of equal-height rows above them.
pub(crate) struct MovedRows {
    region: AnchorRegion,
    rows: HashSet<String>,
}

impl MovedRows {
    /// Capture affected identities within one region; repeated keys have the
    /// same meaning as a single report, and no state survives this dispatch.
    pub(crate) fn new(region: AnchorRegion, rows: impl IntoIterator<Item = String>) -> Self {
        Self {
            region,
            rows: rows.into_iter().collect(),
        }
    }
}

/// The nonreactive registry shared by the application and its popover owners.
///
/// Registration order is stable and doubles as the existing sole-anchor
/// priority for local session deletion. Dispatch does not establish mutual
/// exclusion: independently owned popovers may coexist and each can receive
/// a movement report. Callers retain their existing effects and callbacks.
#[derive(Clone, Default)]
pub(crate) struct MovementOwner(Rc<RefCell<Registry>>);

/// Registry records have no DOM handles and do not own focus or geometry.
#[derive(Default)]
struct Registry {
    next_id: u64,
    entries: Vec<(u64, Rc<Entry>)>,
}

/// A live reader prevents a delayed effect from acting on a captured opening.
struct Entry {
    region: AnchorRegion,
    current_row: Box<dyn Fn() -> Option<String>>,
    close: Box<dyn Fn()>,
}

/// A mount's registration; clones share one lease and unregister together.
///
/// The weak reference avoids extending the application's lifetime. Explicit
/// scope cleanup removes callbacks before their component signals are retired;
/// dropping the last lease also removes plain, non-component registrations.
#[derive(Clone)]
pub(crate) struct Registration(Rc<Lease>);

/// Shared cleanup identity for render-local clones of a mount's token.
struct Lease {
    owner: Weak<RefCell<Registry>>,
    id: u64,
}

impl MovementOwner {
    /// Register an existing open slot and its idempotent closer once per mount.
    ///
    /// `current_row` must use untracked reads (`peek` for signals), returning
    /// None while closed. `close` must close rather than toggle, and must keep
    /// any existing focus teardown. Neither callback may cache an opening.
    pub(crate) fn register(
        &self,
        region: AnchorRegion,
        current_row: impl Fn() -> Option<String> + 'static,
        close: impl Fn() + 'static,
    ) -> Registration {
        let mut registry = self.0.borrow_mut();
        let id = registry.next_id;
        registry.next_id += 1;
        registry.entries.push((
            id,
            Rc::new(Entry {
                region,
                current_row: Box::new(current_row),
                close: Box::new(close),
            }),
        ));
        Registration(Rc::new(Lease {
            owner: Rc::downgrade(&self.0),
            id,
        }))
    }

    /// Close open registrations on selected rows, at the caller's boundary.
    pub(crate) fn close_affected(&self, moved: &MovedRows) {
        self.dispatch(|entry| {
            entry.region == moved.region
                && (entry.current_row)().is_some_and(|row| moved.rows.contains(&row))
        });
    }

    /// Invalidate whole row regions without writing already-closed slots.
    ///
    /// The broad sidebar layout effect historically checked each slot first;
    /// opening a slot must not itself become a dependency of that effect.
    pub(crate) fn close_open_in(&self, regions: &[AnchorRegion]) {
        self.dispatch(|entry| regions.contains(&entry.region) && (entry.current_row)().is_some());
    }

    /// Deliver an unconditional regional clear, including to closed slots.
    ///
    /// Compact changes and a local Delete that invalidates its sole anchor
    /// write every session-popover slot even when one is already None. Those
    /// writes can schedule effects, so they are distinct from close_affected.
    pub(crate) fn close_registered(&self, region: AnchorRegion) {
        self.dispatch(|entry| entry.region == region);
    }

    /// Read the first open anchor in a region using registration priority.
    ///
    /// Local Delete historically chose the session menu before the bell,
    /// then cleared both slots if that sole anchor moved or disappeared.
    /// Preserve that choice even if an inconsistent state contains both.
    pub(crate) fn first_open_row(&self, region: AnchorRegion) -> Option<String> {
        self.snapshot().into_iter().find_map(|(id, entry)| {
            if entry.region == region && self.contains(id) {
                (entry.current_row)()
            } else {
                None
            }
        })
    }

    /// Release the registry borrow before calling readers or closers.
    ///
    /// A closer can unmount another registered component. Rechecking its id
    /// before delivery prevents the snapshot from calling a retired callback.
    fn dispatch(&self, selected: impl Fn(&Entry) -> bool) {
        for (id, entry) in self.snapshot() {
            if self.contains(id) && selected(&entry) && self.contains(id) {
                (entry.close)();
            }
        }
    }

    /// Snapshot callback handles in registration order, without calling them.
    fn snapshot(&self) -> Vec<(u64, Rc<Entry>)> {
        self.0.borrow().entries.clone()
    }

    /// A callback removed during dispatch is no longer eligible for delivery.
    fn contains(&self, id: u64) -> bool {
        self.0
            .borrow()
            .entries
            .iter()
            .any(|(registered, _)| *registered == id)
    }
}

impl Registration {
    /// Retire this mount's callbacks before its signal storage disappears.
    fn unregister(&self) {
        self.0.unregister();
    }
}

impl Lease {
    /// Idempotent cleanup also handles explicit scope teardown before Drop.
    fn unregister(&self) {
        if let Some(owner) = self.owner.upgrade() {
            owner.borrow_mut().entries.retain(|(id, _)| *id != self.id);
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.unregister();
    }
}

/// Attach one registration to the current component without an effect.
///
/// The hook keeps the lease across renders; on scope teardown it unregisters
/// explicitly, before the signal handles captured by the callbacks expire.
pub(crate) fn use_registration(
    owner: MovementOwner,
    region: AnchorRegion,
    current_row: impl Fn() -> Option<String> + 'static,
    close: impl Fn() + 'static,
) -> Registration {
    let registration = use_hook(move || owner.register(region, current_row, close));
    let on_drop = registration.clone();
    use_drop(move || on_drop.unregister());
    registration
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Plain slots exercise the production callbacks without a fake renderer;
    /// a delivery counter also exposes unconditional writes to closed state.
    #[derive(Clone, Default)]
    struct Slot {
        row: Rc<RefCell<Option<String>>>,
        deliveries: Rc<Cell<usize>>,
    }

    impl Slot {
        /// Open a fresh anchor; the owner is deliberately not notified.
        fn open(&self, row: &str) {
            *self.row.borrow_mut() = Some(row.to_string());
        }

        /// Register the same reader/closer contract the real signal slots use.
        fn register(&self, owner: &MovementOwner, region: AnchorRegion) -> Registration {
            let current = self.clone();
            let closing = self.clone();
            owner.register(
                region,
                move || current.row.borrow().clone(),
                move || {
                    closing.deliveries.set(closing.deliveries.get() + 1);
                    *closing.row.borrow_mut() = None;
                },
            )
        }
    }

    /// A movement selects rows and regions, not popup kinds; unrelated open
    /// anchors survive and every independently registered matching kind closes.
    #[farhelm_testtrace::test]
    fn selected_rows_close_all_matching_consumers_only() {
        let owner = MovementOwner::default();
        let menu = Slot::default();
        let bell = Slot::default();
        let below = Slot::default();
        let host = Slot::default();
        let _menu = menu.register(&owner, AnchorRegion::Sessions);
        let _bell = bell.register(&owner, AnchorRegion::Sessions);
        let _below = below.register(&owner, AnchorRegion::Sessions);
        let _host = host.register(&owner, AnchorRegion::Hosts);
        menu.open("middle");
        bell.open("middle");
        below.open("first");
        host.open("middle");
        owner.close_affected(&MovedRows::new(
            AnchorRegion::Sessions,
            ["middle".to_string(), "last".to_string()],
        ));
        assert!(menu.row.borrow().is_none());
        assert!(bell.row.borrow().is_none());
        assert_eq!(below.row.borrow().as_deref(), Some("first"));
        assert_eq!(host.row.borrow().as_deref(), Some("middle"));
        assert_eq!((menu.deliveries.get(), bell.deliveries.get()), (1, 1));
    }

    /// Initial layout reports see closed slots, and completed reports cannot
    /// replay against a later opening or a later registration on the same row.
    #[farhelm_testtrace::test]
    fn initial_closed_state_and_later_openings_receive_no_replay() {
        let owner = MovementOwner::default();
        let slot = Slot::default();
        let registration = slot.register(&owner, AnchorRegion::Sessions);
        owner.close_open_in(&[AnchorRegion::Sessions]);
        assert_eq!(slot.deliveries.get(), 0);
        slot.open("row");
        assert_eq!(slot.deliveries.get(), 0);
        owner.close_affected(&MovedRows::new(AnchorRegion::Sessions, ["row".to_string()]));
        assert_eq!(slot.deliveries.get(), 1);
        slot.open("row");
        assert_eq!(slot.row.borrow().as_deref(), Some("row"));
        drop(registration);
        owner.close_open_in(&[AnchorRegion::Sessions]);
        assert_eq!(slot.deliveries.get(), 1);
        let _new_registration = slot.register(&owner, AnchorRegion::Sessions);
        assert_eq!(slot.row.borrow().as_deref(), Some("row"));
        owner.close_open_in(&[AnchorRegion::Sessions]);
        assert_eq!(slot.deliveries.get(), 2);
    }

    /// Local deletion chooses the first open anchor and then clears both
    /// session slots, even the closed one; a whole-session change excludes hosts.
    #[farhelm_testtrace::test]
    fn sole_anchor_priority_and_unconditional_regional_writes_are_distinct() {
        let owner = MovementOwner::default();
        let menu = Slot::default();
        let bell = Slot::default();
        let host = Slot::default();
        let _menu = menu.register(&owner, AnchorRegion::Sessions);
        let _host = host.register(&owner, AnchorRegion::Hosts);
        let _bell = bell.register(&owner, AnchorRegion::Sessions);
        menu.open("menu-row");
        bell.open("bell-row");
        assert_eq!(
            owner.first_open_row(AnchorRegion::Sessions).as_deref(),
            Some("menu-row")
        );
        *menu.row.borrow_mut() = None;
        assert_eq!(
            owner.first_open_row(AnchorRegion::Sessions).as_deref(),
            Some("bell-row")
        );
        owner.close_registered(AnchorRegion::Sessions);
        assert_eq!(
            (
                menu.deliveries.get(),
                bell.deliveries.get(),
                host.deliveries.get()
            ),
            (1, 1, 0)
        );
        assert!(owner.first_open_row(AnchorRegion::Sessions).is_none());
        owner.close_registered(AnchorRegion::Sessions);
        assert_eq!((menu.deliveries.get(), bell.deliveries.get()), (2, 2));
    }

    /// A closer can retire a later registration synchronously; dispatch must
    /// release the registry borrow and skip that now-invalid callback handle.
    #[farhelm_testtrace::test]
    fn unregister_during_dispatch_skips_the_retired_callback() {
        let owner = MovementOwner::default();
        let retiring = Rc::new(RefCell::new(None::<Registration>));
        let retire = retiring.clone();
        let _first = owner.register(
            AnchorRegion::Sessions,
            || Some("first".to_string()),
            move || {
                drop(retire.borrow_mut().take());
            },
        );
        let second = Slot::default();
        second.open("second");
        *retiring.borrow_mut() = Some(second.register(&owner, AnchorRegion::Sessions));
        owner.close_open_in(&[AnchorRegion::Sessions]);
        assert_eq!(second.deliveries.get(), 0);
        assert_eq!(second.row.borrow().as_deref(), Some("second"));
    }
}
